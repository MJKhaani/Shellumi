//! Native GPUI multiline administrative text/code editor for local and remote SFTP files.
//! Uses `shellumi_core::EditorBuffer` as the sole authoritative document model (no `TextInput`
//! single-line copy, no 40-line preview) and fails closed on remote/local verification errors.

use super::*;
use gpui::{
    ClipboardItem, EntityInputHandler, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    MouseDownEvent, UTF16Selection,
};
use shellumi_core::{
    detect_language, format_document, highlight_line, EditorBuffer, EditorId, EditorLanguage,
    EditorSessionMeta, EndpointKind, FormatOutcome, LineEnding, SftpConnectOptions, SftpError,
    SyntaxTokenKind, VaultScope, DEFAULT_EDITABLE_LIMIT_BYTES,
};
use std::ops::Range;

#[derive(Clone, Debug)]
pub(crate) struct EditorConflictState {
    pub message: String,
    pub remote_sha256: String,
}

/// Dedicated GPUI multiline code editor entity backed by a single authoritative `EditorBuffer`.
pub(crate) struct CodeEditorView {
    pub buffer: EditorBuffer,
    pub language: EditorLanguage,
    pub read_only: bool,
    pub word_wrap: bool,
    pub focus_handle: FocusHandle,
    pub scroll_line: usize,
    pub marked_range: Option<Range<usize>>,
}

impl CodeEditorView {
    pub fn new(
        buffer: EditorBuffer,
        language: EditorLanguage,
        read_only: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            buffer,
            language,
            read_only,
            word_wrap: false,
            focus_handle: cx.focus_handle(),
            scroll_line: 0,
            marked_range: None,
        }
    }

    fn insert_str(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let pos = self.buffer.cursor();
        self.buffer.apply_edit(pos..pos, text);
        cx.notify();
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let pos = self.buffer.cursor();
        if pos == 0 {
            return;
        }
        let text = self.buffer.text();
        let prev = text[..pos]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.buffer.apply_edit(prev..pos, "");
        cx.notify();
    }

    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let pos = self.buffer.cursor();
        let text = self.buffer.text();
        if pos >= text.len() {
            return;
        }
        let next = text[pos..]
            .chars()
            .next()
            .map(|c| pos + c.len_utf8())
            .unwrap_or(text.len());
        self.buffer.apply_edit(pos..next, "");
        cx.notify();
    }

    fn move_left(&mut self, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        if pos > 0 {
            let prev = self.buffer.text()[..pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.buffer.set_cursor(prev);
            cx.notify();
        }
    }

    fn move_right(&mut self, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        let text = self.buffer.text();
        if pos < text.len() {
            let next = text[pos..]
                .chars()
                .next()
                .map(|c| pos + c.len_utf8())
                .unwrap_or(text.len());
            self.buffer.set_cursor(next);
            cx.notify();
        }
    }

    fn move_vertical(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (cur_line, cur_col) = self.buffer.line_col();
        let lines: Vec<&str> = self.buffer.text().split('\n').collect();
        let target_line = (cur_line as isize - 1 + delta)
            .clamp(0, lines.len().saturating_sub(1) as isize) as usize;
        let mut offset = 0usize;
        for line in lines.iter().take(target_line) {
            offset += line.len() + 1;
        }
        let line_str = lines.get(target_line).copied().unwrap_or("");
        let target_char_idx = cur_col.saturating_sub(1);
        let byte_in_line = line_str
            .char_indices()
            .nth(target_char_idx)
            .map(|(i, _)| i)
            .unwrap_or(line_str.len());
        self.buffer.set_cursor(offset + byte_in_line);
        cx.notify();
    }

    fn move_home(&mut self, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        let start = self.buffer.text()[..pos]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.buffer.set_cursor(start);
        cx.notify();
    }

    fn move_end(&mut self, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        let text = self.buffer.text();
        let end = text[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(text.len());
        self.buffer.set_cursor(end);
        cx.notify();
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;
        let primary = ks.modifiers.platform || ks.modifiers.control;
        if primary {
            match ks.key.as_str() {
                "z" if ks.modifiers.shift => {
                    if !self.read_only && self.buffer.redo() {
                        cx.notify();
                    }
                    return;
                }
                "z" => {
                    if !self.read_only && self.buffer.undo() {
                        cx.notify();
                    }
                    return;
                }
                "y" => {
                    if !self.read_only && self.buffer.redo() {
                        cx.notify();
                    }
                    return;
                }
                "c" => {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.buffer.text().to_string(),
                    ));
                    return;
                }
                "v" => {
                    if !self.read_only {
                        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                            self.insert_str(&text, cx);
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        match ks.key.as_str() {
            "backspace" => self.backspace(cx),
            "delete" => self.delete_forward(cx),
            "enter" | "return" => {
                let nl = match self.buffer.line_ending() {
                    LineEnding::Lf => "\n",
                    LineEnding::Crlf => "\r\n",
                };
                self.insert_str(nl, cx);
            }
            "tab" => {
                self.insert_str("  ", cx);
            }
            "left" => self.move_left(cx),
            "right" => self.move_right(cx),
            "up" => self.move_vertical(-1, cx),
            "down" => self.move_vertical(1, cx),
            "home" => self.move_home(cx),
            "end" => self.move_end(cx),
            _ => {
                if let Some(ch) = &ks.key_char {
                    if !ch.is_empty() && !ks.modifiers.control && !ks.modifiers.platform {
                        self.insert_str(ch, cx);
                    }
                }
            }
        }
    }
}

impl Focusable for CodeEditorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for CodeEditorView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.buffer.text();
        let start = range_utf16.start.min(text.len());
        let end = range_utf16.end.min(text.len()).max(start);
        *actual_range = Some(start..end);
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let pos = self.buffer.cursor();
        Some(UTF16Selection {
            range: pos..pos,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range.clone()
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let pos = self.buffer.cursor();
        let range = range_utf16.unwrap_or(pos..pos);
        self.buffer.apply_edit(range, new_text);
        self.marked_range = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let pos = self.buffer.cursor();
        let range = range_utf16.unwrap_or(pos..pos);
        let start = range.start;
        self.buffer.apply_edit(range, new_text);
        self.marked_range = Some(start..start + new_text.len());
        if let Some(sel) = new_selected_range_utf16 {
            self.buffer.set_cursor(sel.end);
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

fn token_color(kind: SyntaxTokenKind) -> gpui::Rgba {
    match kind {
        SyntaxTokenKind::Comment => rgb(0x6b7280),
        SyntaxTokenKind::Keyword | SyntaxTokenKind::Directive => rgb(0x60a5fa),
        SyntaxTokenKind::String => rgb(0x34d399),
        SyntaxTokenKind::Key | SyntaxTokenKind::Section => rgb(0xfbbf24),
        SyntaxTokenKind::Variable => rgb(0xc084fc),
        SyntaxTokenKind::Number | SyntaxTokenKind::Boolean => rgb(0xf472b6),
        SyntaxTokenKind::Punctuation => rgb(0x94a3b8),
        SyntaxTokenKind::Plain => rgb(MacTheme::TEXT_PRIMARY),
    }
}

impl Render for CodeEditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (cur_line, cur_col) = self.buffer.line_col();
        let all_lines: Vec<String> = self
            .buffer
            .text()
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
            .collect();
        let total_lines = all_lines.len();
        let start = self.scroll_line.min(total_lines.saturating_sub(1));
        let visible_count = 120usize;
        let end = (start + visible_count).min(total_lines);

        div()
            .id("sftp-code-editor-surface")
            .track_focus(&self.focus_handle)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .bg(rgb(MacTheme::SURFACE))
            .p_2()
            .font_family(MONOSPACE_FONT)
            .text_xs()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle);
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(Self::handle_key_down))
            .children((start..end).map(|line_idx| {
                let line_num = line_idx + 1;
                let line_text = &all_lines[line_idx];
                let is_current = line_num == cur_line;
                let spans = highlight_line(self.language, line_text);

                // Build multi-span token segments covering every byte of the line
                let mut segments: Vec<(String, gpui::Rgba)> = Vec::new();
                let mut cursor = 0usize;
                for span in spans {
                    if span.range.start > cursor && line_text.is_char_boundary(span.range.start) {
                        segments.push((
                            line_text[cursor..span.range.start].to_string(),
                            rgb(MacTheme::TEXT_PRIMARY),
                        ));
                    }
                    if line_text.is_char_boundary(span.range.start)
                        && line_text.is_char_boundary(span.range.end)
                    {
                        segments.push((
                            line_text[span.range.start..span.range.end].to_string(),
                            token_color(span.kind),
                        ));
                        cursor = span.range.end;
                    }
                }
                if cursor < line_text.len() {
                    segments.push((line_text[cursor..].to_string(), rgb(MacTheme::TEXT_PRIMARY)));
                }
                if segments.is_empty() {
                    segments.push((" ".to_string(), rgb(MacTheme::TEXT_PRIMARY)));
                }

                let mut line_row = div()
                    .id(("ed-line", line_idx))
                    .flex()
                    .items_center()
                    .px_1()
                    .rounded_sm()
                    .when(is_current, |d| d.bg(rgb(MacTheme::SURFACE_ACTIVE)))
                    .child(
                        div()
                            .w(px(42.0))
                            .flex_shrink_0()
                            .text_color(if is_current {
                                rgb(MacTheme::TEXT_PRIMARY)
                            } else {
                                rgb(MacTheme::TEXT_TERTIARY)
                            })
                            .child(format!("{:>4}", line_num)),
                    );

                let mut text_flow = div().flex().flex_row().min_w_0();
                if self.word_wrap {
                    text_flow = text_flow.flex_wrap();
                }
                for (seg_text, seg_color) in segments {
                    text_flow = text_flow.child(div().text_color(seg_color).child(seg_text));
                }
                if is_current {
                    text_flow = text_flow.child(
                        div()
                            .ml_1()
                            .px_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .child(format!("│{cur_col}")),
                    );
                }
                line_row = line_row.child(text_flow);
                line_row
            }))
    }
}

pub(crate) struct SftpEditorTabState {
    pub meta: EditorSessionMeta,
    pub editor: Entity<CodeEditorView>,
    pub read_only_reason: Option<String>,
    pub find_open: bool,
    pub find_input: Entity<TextInput>,
    pub replace_input: Entity<TextInput>,
    pub is_saving: bool,
    pub dirty_close_confirm: bool,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub conflict: Option<EditorConflictState>,
    pub connect_options: Option<SftpConnectOptions>,
    pub database_path: Option<String>,
    pub host_id: Option<i64>,
}

impl SftpEditorTabState {
    pub fn title(&self, cx: &App) -> String {
        let base = std::path::Path::new(&self.meta.path_display)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.meta.path_display);
        if self.editor.read(cx).buffer.is_dirty() {
            format!("● {base}")
        } else {
            base.to_string()
        }
    }

    pub fn is_same_target(&self, scope: &VaultScope, endpoint: &EndpointKind, path: &str) -> bool {
        self.meta.vault_scope.is_authorized_under(scope)
            && self.meta.endpoint.same_underlying_target(endpoint)
            && self.meta.path_display == path
    }
}

impl Shellumi {
    /// Open a local or remote file in the integrated GPUI multiline editor, deduplicating by
    /// `(VaultScope, EndpointKind, path)` and registering a vault-scoped operation.
    pub(crate) fn sftp_open_in_editor(
        &mut self,
        is_left: bool,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };
        let Some(host) = pane.host.clone() else {
            return;
        };
        let scope = self.current_vault_scope();
        let endpoint = match &host {
            SftpPaneHost::Local => EndpointKind::local(scope.clone()),
            SftpPaneHost::Remote(h) => EndpointKind::Remote(shellumi_core::RemoteEndpoint {
                endpoint_id: pane.endpoint_id.unwrap_or_default(),
                vault_scope: scope.clone(),
                host_id: h.id,
                host_uuid: None,
                display_name: h.name.clone(),
                destination: h.destination.clone(),
                connection_generation: pane.listing_generation,
                database_path: self.database.clone(),
            }),
        };

        if let Some(idx) = self
            .sftp_editors
            .iter()
            .position(|ed| ed.is_same_target(&scope, &endpoint, &entry_path))
        {
            self.active_sftp_editor_idx = Some(idx);
            cx.notify();
            return;
        }

        let controller = self.sftp_controller.clone();
        let Ok((op_id, ctrl)) = controller.register_op(&scope) else {
            self.desktop.message = Some("Cannot open editor: vault is locked.".into());
            cx.notify();
            return;
        };
        let database = self.database.clone();
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_key = self.active_vault_key();
        let entry_path_clone = entry_path.clone();
        let ep_for_worker = endpoint.clone();

        let load_task = cx.background_executor().spawn(async move {
            if !controller.is_op_authorized(op_id, &scope) {
                return Err(SftpError::Cancelled);
            }
            let res = match &host {
                SftpPaneHost::Local => {
                    let p = std::path::Path::new(&entry_path_clone);
                    let meta = std::fs::symlink_metadata(p)?;
                    if !meta.file_type().is_file() {
                        return Err(SftpError::UnexpectedResponse(
                            "Only regular files can be opened in the editor.".into(),
                        ));
                    }
                    if meta.len() > shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES {
                        return Err(SftpError::UnexpectedResponse(format!(
                            "File is {} bytes (exceeds {} MiB preview limit)",
                            meta.len(),
                            shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES / (1024 * 1024)
                        )));
                    }
                    let bytes = std::fs::read(p)?;
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs());
                    #[cfg(unix)]
                    let mode = {
                        use std::os::unix::fs::MetadataExt;
                        Some(meta.mode())
                    };
                    #[cfg(not(unix))]
                    let mode = None;
                    Ok((bytes, meta.len(), mtime, mode, None, None))
                }
                SftpPaneHost::Remote(h) => {
                    let mut opts = SftpConnectOptions {
                        port: Some(h.destination.port().get()),
                        ..Default::default()
                    };
                    if let Some(jump) = &h.jump_host {
                        opts.extra_ssh_options
                            .push(("ProxyJump".to_string(), jump.clone()));
                    }
                    let mut client = controller
                        .open_worker_client(&scope, &ep_for_worker, Some(&opts), &ctrl)
                        .or_else(|_| {
                            connect_saved_sftp(
                                database.clone(),
                                Some(&active_vault_id),
                                active_vault_key.as_ref(),
                                h.id,
                                &h.destination,
                                &opts,
                            )
                        })?;
                    let attrs = client.lstat(&entry_path_clone)?;
                    if let Some(sz) = attrs.size {
                        if sz > shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES {
                            return Err(SftpError::UnexpectedResponse(format!(
                                "Remote file is {sz} bytes (exceeds {} MiB preview limit)",
                                shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES / (1024 * 1024)
                            )));
                        }
                    }
                    let tmp = tempfile::NamedTempFile::new()?;
                    let downloaded = client.download_file_with_control(
                        &entry_path_clone,
                        tmp.path(),
                        &ctrl,
                        None::<fn(u64, u64)>,
                    )?;
                    let bytes = std::fs::read(tmp.path())?;
                    Ok((
                        bytes,
                        downloaded,
                        attrs.mtime.map(|m| m as u64),
                        attrs.permissions,
                        Some(opts),
                        Some(h.id),
                    ))
                }
            };
            controller.complete_op(op_id);
            res
        });

        let controller_ui = self.sftp_controller.clone();
        let scope_ui = self.current_vault_scope();
        cx.spawn(async move |app, cx| {
            let res = load_task.await;
            let _ = app.update(cx, |app, cx| {
                // Reject delayed completion after vault lock/switch (even errors).
                if !controller_ui.auth_gate.is_authorized(&scope_ui) {
                    return;
                }
                match res {
                    Ok((bytes, size, mtime, mode, connect_options, host_id)) => {
                        let buffer = match EditorBuffer::from_bytes(&bytes) {
                            Ok(b) => b,
                            Err(reason) => {
                                app.desktop.message =
                                    Some(format!("Cannot open in editor: {reason}"));
                                cx.notify();
                                return;
                            }
                        };
                        let read_only_reason = if size > DEFAULT_EDITABLE_LIMIT_BYTES {
                            Some(format!(
                                "Read-only preview (file exceeds {} MiB editable limit)",
                                DEFAULT_EDITABLE_LIMIT_BYTES / (1024 * 1024)
                            ))
                        } else {
                            None
                        };
                        let language = detect_language(&entry_path, buffer.text());
                        let sha = buffer.sha256_hex();
                        let read_only = read_only_reason.is_some();
                        let editor =
                            cx.new(|cx| CodeEditorView::new(buffer, language, read_only, cx));
                        let find_input = cx.new(|cx| TextInput::new(cx, "", "Find…"));
                        let replace_input = cx.new(|cx| TextInput::new(cx, "", "Replace…"));

                        let state = SftpEditorTabState {
                            meta: EditorSessionMeta {
                                editor_id: EditorId::new(),
                                vault_scope: scope_ui,
                                endpoint,
                                path_display: entry_path.clone(),
                                language,
                                original_sha256: sha,
                                original_size: size,
                                original_mtime: mtime,
                                original_mode: mode,
                                read_only,
                            },
                            editor,
                            read_only_reason,
                            find_open: false,
                            find_input,
                            replace_input,
                            is_saving: false,
                            dirty_close_confirm: false,
                            status_message: None,
                            error_message: None,
                            conflict: None,
                            connect_options,
                            database_path: app.database.clone(),
                            host_id,
                        };
                        app.sftp_editors.push(state);
                        app.active_sftp_editor_idx = Some(app.sftp_editors.len() - 1);
                        cx.notify();
                    }
                    Err(err) => {
                        app.desktop.message = Some(format!("Failed to open file: {err}"));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn sftp_editor_format_active(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_sftp_editor_idx else {
            return;
        };
        let Some(ed) = self.sftp_editors.get_mut(idx) else {
            return;
        };
        if ed.meta.read_only {
            ed.error_message = Some("Cannot format a read-only preview.".into());
            cx.notify();
            return;
        }
        let current = ed.editor.read(cx).buffer.text().to_string();
        match format_document(ed.meta.language, &current, 2) {
            FormatOutcome::Formatted { text, changed } => {
                if changed {
                    ed.editor.update(cx, |view, cx| {
                        let len = view.buffer.text().len();
                        view.buffer.apply_edit(0..len, &text);
                        cx.notify();
                    });
                    ed.status_message = Some(format!(
                        "Formatted {} document.",
                        ed.meta.language.display_name()
                    ));
                } else {
                    ed.status_message = Some("Document already formatted.".into());
                }
                ed.error_message = None;
            }
            FormatOutcome::Unsupported(msg) => {
                ed.error_message = Some(msg);
            }
            FormatOutcome::SyntaxError {
                line,
                column,
                message,
            } => {
                ed.error_message = Some(format!(
                    "Format aborted at line {line}, col {column}: {message}"
                ));
            }
        }
        cx.notify();
    }

    /// Save the active editor document using staged atomic publication and **fail-closed**
    /// remote/local fingerprint verification.
    pub(crate) fn sftp_editor_save_active(
        &mut self,
        force_overwrite: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(idx) = self.active_sftp_editor_idx else {
            return;
        };
        let Some(ed) = self.sftp_editors.get_mut(idx) else {
            return;
        };
        if ed.meta.read_only || ed.is_saving {
            return;
        }

        let (snapshot_bytes, new_sha, save_rev) = {
            let view = ed.editor.read(cx);
            (
                view.buffer.to_bytes(),
                view.buffer.sha256_hex(),
                view.buffer.revision(),
            )
        };
        let editor_id = ed.meta.editor_id;
        let scope = ed.meta.vault_scope.clone();
        let orig_sha = ed.meta.original_sha256.clone();
        let endpoint = ed.meta.endpoint.clone();
        let path = ed.meta.path_display.clone();
        let opts = ed.connect_options.clone().unwrap_or_default();
        let database = ed.database_path.clone();
        let host_id = ed.host_id;

        let controller = self.sftp_controller.clone();
        let Ok((op_id, ctrl)) = controller.register_op(&scope) else {
            ed.error_message = Some("Cannot save: vault scope is locked or inactive.".into());
            cx.notify();
            return;
        };

        ed.is_saving = true;
        ed.error_message = None;
        ed.status_message = Some("Saving…".into());
        let active_vault_id = scope.vault_id.clone();
        let active_vault_key = self.active_vault_keys.get(&active_vault_id).cloned();
        cx.notify();

        let scope_worker = scope.clone();
        let save_task = cx.background_executor().spawn(async move {
            if !controller.is_op_authorized(op_id, &scope_worker) {
                return Err(SftpError::Cancelled);
            }
            let res = match &endpoint {
                EndpointKind::Local { .. } => {
                    let p = std::path::Path::new(&path);
                    if !force_overwrite {
                        match std::fs::symlink_metadata(p) {
                            Ok(_) => {
                                let disk_sha = shellumi_core::compute_local_sha256(p, &ctrl)?;
                                if disk_sha != orig_sha {
                                    return Ok(Err(EditorConflictState {
                                        message: "File changed on disk since checkout.".into(),
                                        remote_sha256: disk_sha,
                                    }));
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                                return Ok(Err(EditorConflictState {
                                    message: "Target file was deleted on disk since checkout."
                                        .into(),
                                    remote_sha256: "<missing>".into(),
                                }));
                            }
                            Err(e) => return Err(SftpError::Io(e)),
                        }
                    }
                    let tmp = tempfile::NamedTempFile::new_in(
                        p.parent().unwrap_or_else(|| std::path::Path::new(".")),
                    )?;
                    std::fs::write(tmp.path(), &snapshot_bytes)?;
                    shellumi_core::staged_local_copy_file(tmp.path(), p, true, &ctrl, None)?;
                    Ok::<Result<String, EditorConflictState>, SftpError>(Ok(new_sha))
                }
                EndpointKind::Remote(r) => {
                    let mut client = controller
                        .open_worker_client(&scope_worker, &endpoint, Some(&opts), &ctrl)
                        .or_else(|_| {
                            connect_saved_sftp(
                                database,
                                Some(&active_vault_id),
                                active_vault_key.as_ref(),
                                host_id.unwrap_or(r.host_id),
                                &r.destination,
                                &opts,
                            )
                        })?;
                    if !force_overwrite {
                        // Fail closed: any verification error (other than FileNotFound) aborts save!
                        match shellumi_core::compute_remote_sha256(&mut client, &path, &ctrl) {
                            Ok(remote_sha) if remote_sha != orig_sha => {
                                return Ok(Err(EditorConflictState {
                                    message: "Remote file changed since checkout.".into(),
                                    remote_sha256: remote_sha,
                                }));
                            }
                            Ok(_) => {}
                            Err(SftpError::FileNotFound(_)) => {
                                return Ok(Err(EditorConflictState {
                                    message: "Remote file was deleted since checkout.".into(),
                                    remote_sha256: "<missing>".into(),
                                }));
                            }
                            Err(err) => return Err(err),
                        }
                    }
                    let tmp = tempfile::NamedTempFile::new()?;
                    std::fs::write(tmp.path(), &snapshot_bytes)?;
                    client.upload_file_with_control(
                        tmp.path(),
                        &path,
                        &ctrl,
                        None::<fn(u64, u64)>,
                    )?;
                    Ok::<Result<String, EditorConflictState>, SftpError>(Ok(new_sha))
                }
            };
            controller.complete_op(op_id);
            res
        });

        let controller_ui = self.sftp_controller.clone();
        cx.spawn(async move |app, cx| {
            let res = save_task.await;
            let _ = app.update(cx, |app, cx| {
                if !controller_ui.auth_gate.is_authorized(&scope) {
                    return;
                }
                let Some(ed) = app
                    .sftp_editors
                    .iter_mut()
                    .find(|e| e.meta.editor_id == editor_id)
                else {
                    return;
                };
                ed.is_saving = false;
                match res {
                    Ok(Ok(new_sha)) => {
                        ed.meta.original_sha256 = new_sha;
                        let still_dirty = ed.editor.update(cx, |view, cx| {
                            view.buffer.mark_saved_revision(save_rev);
                            cx.notify();
                            view.buffer.is_dirty()
                        });
                        ed.conflict = None;
                        ed.status_message = Some(if still_dirty {
                            "Saved prior revision; newer edits remain unsaved.".into()
                        } else {
                            "Saved.".into()
                        });
                        if ed.dirty_close_confirm && !still_dirty {
                            ed.dirty_close_confirm = false;
                            app.sftp_close_editor_by_id(editor_id, true, cx);
                            return;
                        }
                    }
                    Ok(Err(conflict)) => {
                        ed.conflict = Some(conflict);
                        ed.status_message = None;
                    }
                    Err(err) => {
                        ed.error_message = Some(format!("Save failed: {err}"));
                        ed.status_message = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn sftp_close_editor_by_id(
        &mut self,
        editor_id: EditorId,
        force_discard: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(idx) = self
            .sftp_editors
            .iter()
            .position(|e| e.meta.editor_id == editor_id)
        else {
            return;
        };
        let is_dirty = self.sftp_editors[idx].editor.read(cx).buffer.is_dirty();
        if is_dirty && !force_discard {
            self.sftp_editors[idx].dirty_close_confirm = true;
            cx.notify();
            return;
        }
        self.sftp_editors.remove(idx);
        if self.sftp_editors.is_empty() {
            self.active_sftp_editor_idx = None;
        } else if let Some(active) = self.active_sftp_editor_idx {
            if active >= self.sftp_editors.len() {
                self.active_sftp_editor_idx = Some(self.sftp_editors.len() - 1);
            }
        }
        cx.notify();
    }

    pub(crate) fn render_sftp_editor_panel(
        &self,
        ed: &SftpEditorTabState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let editor_id = ed.meta.editor_id;
        let view = ed.editor.read(cx);
        let (line, col) = view.buffer.line_col();
        let is_dirty = view.buffer.is_dirty();
        let word_wrap = view.word_wrap;
        let line_ending_str = match view.buffer.line_ending() {
            LineEnding::Lf => "LF",
            LineEnding::Crlf => "CRLF",
        };

        let mut panel = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(MacTheme::CONTENT_BG));

        // Toolbar
        panel = panel.child(
            div()
                .h(px(40.0))
                .px_3()
                .border_b_1()
                .border_color(rgb(MacTheme::BORDER))
                .bg(rgb(MacTheme::SURFACE))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(ed.title(cx)),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE_ACTIVE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child(ed.meta.language.display_name()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("sftp-editor-find-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if let Some(ed) = app
                                        .active_sftp_editor_idx
                                        .and_then(|i| app.sftp_editors.get_mut(i))
                                    {
                                        ed.find_open = !ed.find_open;
                                        cx.notify();
                                    }
                                }))
                                .child("Find"),
                        )
                        .child(
                            div()
                                .id("sftp-editor-wrap-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if let Some(ed) = app
                                        .active_sftp_editor_idx
                                        .and_then(|i| app.sftp_editors.get_mut(i))
                                    {
                                        ed.editor.update(cx, |v, cx| {
                                            v.word_wrap = !v.word_wrap;
                                            cx.notify();
                                        });
                                        cx.notify();
                                    }
                                }))
                                .child(if word_wrap { "Wrap: On" } else { "Wrap: Off" }),
                        )
                        .child(
                            div()
                                .id("sftp-editor-format-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(if ed.meta.language.supports_safe_format() {
                                    rgb(MacTheme::TEXT_PRIMARY)
                                } else {
                                    rgb(MacTheme::TEXT_TERTIARY)
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_editor_format_active(cx);
                                }))
                                .child("Format"),
                        )
                        .child(
                            div()
                                .id("sftp-editor-save-btn")
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_editor_save_active(false, cx);
                                }))
                                .child(if ed.is_saving { "Saving…" } else { "Save" }),
                        )
                        .child(
                            div()
                                .id("sftp-editor-close-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.sftp_close_editor_by_id(editor_id, false, cx);
                                }))
                                .child("Close"),
                        ),
                ),
        );

        if ed.dirty_close_confirm {
            panel = panel.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x4a2a12))
                    .border_b_1()
                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Unsaved changes will be lost if you close this tab."),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("sftp-editor-close-save")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.sftp_editor_save_active(false, cx);
                                    }))
                                    .child("Save & Close"),
                            )
                            .child(
                                div()
                                    .id("sftp-editor-close-discard")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::DANGER))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.sftp_close_editor_by_id(editor_id, true, cx);
                                    }))
                                    .child("Discard"),
                            )
                            .child(
                                div()
                                    .id("sftp-editor-close-cancel")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_xs()
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if let Some(e) = app
                                            .sftp_editors
                                            .iter_mut()
                                            .find(|e| e.meta.editor_id == editor_id)
                                        {
                                            e.dirty_close_confirm = false;
                                        }
                                        cx.notify();
                                    }))
                                    .child("Cancel"),
                            ),
                    ),
            );
        }

        if let Some(reason) = &ed.read_only_reason {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(reason.clone()),
            );
        }

        if ed.find_open {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(180.0)).child(ed.find_input.clone()))
                    .child(div().w(px(180.0)).child(ed.replace_input.clone()))
                    .child(
                        div()
                            .id("sftp-editor-replace-all")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_xs()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(ed) = app
                                    .active_sftp_editor_idx
                                    .and_then(|i| app.sftp_editors.get_mut(i))
                                {
                                    let q = ed.find_input.read(cx).value().to_string();
                                    let r = ed.replace_input.read(cx).value().to_string();
                                    let count = ed.editor.update(cx, |v, cx| {
                                        let c = v.buffer.replace_all(&q, &r, true);
                                        cx.notify();
                                        c
                                    });
                                    ed.status_message =
                                        Some(format!("Replaced {count} match(es)."));
                                    cx.notify();
                                }
                            }))
                            .child("Replace All"),
                    ),
            );
        }

        if let Some(conflict) = &ed.conflict {
            panel = panel.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x4a1818))
                    .border_b_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::DANGER))
                            .child(format!(
                                "Conflict: {} (remote SHA-256: {}…)",
                                conflict.message,
                                &conflict.remote_sha256[..conflict.remote_sha256.len().min(12)]
                            )),
                    )
                    .child(
                        div()
                            .id("sftp-editor-force-overwrite")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::DANGER))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sftp_editor_save_active(true, cx);
                            }))
                            .child("Overwrite Anyway"),
                    ),
            );
        }

        if let Some(err) = &ed.error_message {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(0x4a1818))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(err.clone()),
            );
        } else if let Some(msg) = &ed.status_message {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(0x133820))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(msg.clone()),
            );
        }

        // Native multiline GPUI CodeEditorView surface
        panel = panel.child(ed.editor.clone());

        // Status bar
        panel.child(
            div()
                .h(px(26.0))
                .px_3()
                .border_t_1()
                .border_color(rgb(MacTheme::BORDER))
                .bg(rgb(MacTheme::SURFACE))
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(format!("Ln {line}, Col {col}"))
                .child(format!(
                    "{} · UTF-8 · {} · {}",
                    ed.meta.language.display_name(),
                    line_ending_str,
                    if ed.meta.read_only {
                        "Read-Only"
                    } else if is_dirty {
                        "Modified"
                    } else {
                        "Clean"
                    }
                )),
        )
    }
}
