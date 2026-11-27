//! Native GPUI integrated administrative text/code editor for local and remote SFTP files.

use super::*;
use shellumi_core::{
    detect_language, format_document, highlight_line, EditorBuffer, EditorId, EditorSessionMeta,
    EndpointKind, FormatOutcome, LineEnding, SftpConnectOptions, SftpError, SyntaxTokenKind,
    VaultScope, DEFAULT_EDITABLE_LIMIT_BYTES,
};

#[derive(Clone, Debug)]
pub(crate) struct EditorConflictState {
    pub message: String,
    pub remote_sha256: String,
}

pub(crate) struct SftpEditorTabState {
    pub meta: EditorSessionMeta,
    pub buffer: EditorBuffer,
    pub read_only_reason: Option<String>,
    pub find_open: bool,
    pub find_input: Entity<TextInput>,
    pub replace_input: Entity<TextInput>,
    pub word_wrap: bool,
    pub is_saving: bool,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub conflict: Option<EditorConflictState>,
    pub edit_input: Entity<TextInput>,
    pub connect_options: Option<SftpConnectOptions>,
    pub database_path: Option<String>,
    pub host_id: Option<i64>,
}

impl SftpEditorTabState {
    pub fn title(&self) -> String {
        let base = std::path::Path::new(&self.meta.path_display)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.meta.path_display);
        if self.buffer.is_dirty() {
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
    /// Open a local or remote file in the integrated GPUI editor, deduplicating by
    /// `(VaultScope, EndpointKind, path)`.
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

        // Deduplicate: focus existing editor if already open for the same authorized target.
        if let Some(idx) = self
            .sftp_editors
            .iter()
            .position(|ed| ed.is_same_target(&scope, &endpoint, &entry_path))
        {
            self.active_sftp_editor_idx = Some(idx);
            cx.notify();
            return;
        }

        let database = self.database.clone();
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_key = self.active_vault_key();
        let entry_path_clone = entry_path.clone();

        let load_task = cx.background_executor().spawn(async move {
            match &host {
                SftpPaneHost::Local => {
                    let p = std::path::Path::new(&entry_path_clone);
                    let meta = std::fs::symlink_metadata(p)?;
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
                    let mut client = connect_saved_sftp(
                        database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        h.id,
                        &h.destination,
                        &opts,
                    )?;
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
                    let downloaded = client.download_file(
                        &entry_path_clone,
                        tmp.path(),
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
            }
        });

        cx.spawn(async move |app, cx| {
            let res = load_task.await;
            let _ = app.update(cx, |app, cx| match res {
                Ok((bytes, size, mtime, mode, connect_options, host_id)) => {
                    let buffer = match EditorBuffer::from_bytes(&bytes) {
                        Ok(b) => b,
                        Err(reason) => {
                            app.desktop.message = Some(format!("Cannot open in editor: {reason}"));
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
                    let find_input = cx.new(|cx| TextInput::new(cx, "", "Find…"));
                    let replace_input = cx.new(|cx| TextInput::new(cx, "", "Replace…"));
                    let edit_input =
                        cx.new(|cx| TextInput::new(cx, buffer.text(), "Edit content…"));

                    let state = SftpEditorTabState {
                        meta: EditorSessionMeta {
                            editor_id: EditorId::new(),
                            vault_scope: scope,
                            endpoint,
                            path_display: entry_path.clone(),
                            language,
                            original_sha256: sha,
                            original_size: size,
                            original_mtime: mtime,
                            original_mode: mode,
                            read_only: read_only_reason.is_some(),
                        },
                        buffer,
                        read_only_reason,
                        find_open: false,
                        find_input,
                        replace_input,
                        word_wrap: false,
                        is_saving: false,
                        status_message: None,
                        error_message: None,
                        conflict: None,
                        edit_input,
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
            });
        })
        .detach();
    }

    /// Format the active editor document conservatively using `shellumi_core::format_document`.
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
        let current = ed.edit_input.read(cx).value().to_string();
        match format_document(ed.meta.language, &current, 2) {
            FormatOutcome::Formatted { text, changed } => {
                if changed {
                    let len = ed.buffer.text().len();
                    ed.buffer.apply_edit(0..len, &text);
                    ed.edit_input.update(cx, |input, cx| {
                        input.set_value(&text, cx);
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

    /// Save the active editor document using staged atomic publication and conflict detection.
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
        let current_text = ed.edit_input.read(cx).value().to_string();
        if current_text != ed.buffer.text() {
            let len = ed.buffer.text().len();
            ed.buffer.apply_edit(0..len, &current_text);
        }
        let snapshot_bytes = ed.buffer.to_bytes();
        let new_sha = ed.buffer.sha256_hex();
        let save_rev = ed.buffer.revision();
        let editor_id = ed.meta.editor_id;
        let orig_sha = ed.meta.original_sha256.clone();
        let endpoint = ed.meta.endpoint.clone();
        let path = ed.meta.path_display.clone();
        let opts = ed.connect_options.clone().unwrap_or_default();
        let database = ed.database_path.clone();
        let host_id = ed.host_id;
        ed.is_saving = true;
        ed.error_message = None;
        ed.status_message = Some("Saving…".into());
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_key = self.active_vault_key();
        cx.notify();

        let save_task = cx.background_executor().spawn(async move {
            match &endpoint {
                EndpointKind::Local { .. } => {
                    let p = std::path::Path::new(&path);
                    if !force_overwrite && p.exists() {
                        let ctrl = shellumi_core::TransferControl::default();
                        let disk_sha = shellumi_core::compute_local_sha256(p, &ctrl)?;
                        if disk_sha != orig_sha {
                            return Ok(Err(EditorConflictState {
                                message: "File changed on disk since checkout.".into(),
                                remote_sha256: disk_sha,
                            }));
                        }
                    }
                    let tmp = tempfile::NamedTempFile::new_in(
                        p.parent().unwrap_or_else(|| std::path::Path::new(".")),
                    )?;
                    std::fs::write(tmp.path(), &snapshot_bytes)?;
                    let ctrl = shellumi_core::TransferControl::default();
                    shellumi_core::staged_local_copy_file(tmp.path(), p, true, &ctrl, None)?;
                    Ok::<Result<String, EditorConflictState>, SftpError>(Ok(new_sha))
                }
                EndpointKind::Remote(r) => {
                    let mut client = connect_saved_sftp(
                        database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        host_id.unwrap_or(r.host_id),
                        &r.destination,
                        &opts,
                    )?;
                    if !force_overwrite {
                        let ctrl = shellumi_core::TransferControl::default();
                        if let Ok(remote_sha) =
                            shellumi_core::compute_remote_sha256(&mut client, &path, &ctrl)
                        {
                            if remote_sha != orig_sha {
                                return Ok(Err(EditorConflictState {
                                    message: "Remote file changed since checkout.".into(),
                                    remote_sha256: remote_sha,
                                }));
                            }
                        }
                    }
                    let tmp = tempfile::NamedTempFile::new()?;
                    std::fs::write(tmp.path(), &snapshot_bytes)?;
                    client.upload_file(tmp.path(), &path, None::<fn(u64, u64)>)?;
                    Ok::<Result<String, EditorConflictState>, SftpError>(Ok(new_sha))
                }
            }
        });

        cx.spawn(async move |app, cx| {
            let res = save_task.await;
            let _ = app.update(cx, |app, cx| {
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
                        ed.buffer.mark_saved_revision(save_rev);
                        ed.conflict = None;
                        ed.status_message = Some(if ed.buffer.is_dirty() {
                            "Saved prior revision; newer edits remain unsaved.".into()
                        } else {
                            "Saved.".into()
                        });
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

    pub(crate) fn render_sftp_editor_panel(
        &self,
        ed: &SftpEditorTabState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let (line, col) = ed.buffer.line_col();
        let line_ending_str = match ed.buffer.line_ending() {
            LineEnding::Lf => "LF",
            LineEnding::Crlf => "CRLF",
        };
        let preview_lines: Vec<String> = ed
            .edit_input
            .read(cx)
            .value()
            .lines()
            .take(40)
            .map(ToString::to_string)
            .collect();

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
                                .child(ed.title()),
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
                                        ed.word_wrap = !ed.word_wrap;
                                        cx.notify();
                                    }
                                }))
                                .child(if ed.word_wrap {
                                    "Wrap: On"
                                } else {
                                    "Wrap: Off"
                                }),
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
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if let Some(idx) = app.active_sftp_editor_idx.take() {
                                        if idx < app.sftp_editors.len() {
                                            app.sftp_editors.remove(idx);
                                        }
                                    }
                                    cx.notify();
                                }))
                                .child("Close"),
                        ),
                ),
        );

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
                                    let count = ed.buffer.replace_all(&q, &r, true);
                                    let updated = ed.buffer.text().to_string();
                                    ed.edit_input
                                        .update(cx, |inp, cx| inp.set_value(&updated, cx));
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

        // Editable input + Syntax-highlighted preview lines
        panel =
            panel.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .p_3()
                    .gap_2()
                    .child(ed.edit_input.clone())
                    .child(
                        div()
                            .id("sftp-editor-syntax-preview")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE))
                            .p_2()
                            .font_family(MONOSPACE_FONT)
                            .text_xs()
                            .children(preview_lines.into_iter().enumerate().map(
                                |(ln, line_text)| {
                                    let spans = highlight_line(ed.meta.language, &line_text);
                                    let color = spans
                                        .first()
                                        .map(|s| match s.kind {
                                            SyntaxTokenKind::Comment => rgb(0x6b7280),
                                            SyntaxTokenKind::Keyword
                                            | SyntaxTokenKind::Directive => rgb(0x60a5fa),
                                            SyntaxTokenKind::String => rgb(0x34d399),
                                            SyntaxTokenKind::Key | SyntaxTokenKind::Section => {
                                                rgb(0xfbbf24)
                                            }
                                            SyntaxTokenKind::Variable => rgb(0xc084fc),
                                            SyntaxTokenKind::Number | SyntaxTokenKind::Boolean => {
                                                rgb(0xf472b6)
                                            }
                                            _ => rgb(MacTheme::TEXT_PRIMARY),
                                        })
                                        .unwrap_or_else(|| rgb(MacTheme::TEXT_PRIMARY));
                                    div()
                                        .flex()
                                        .gap_3()
                                        .child(
                                            div()
                                                .w(px(36.0))
                                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                .child(format!("{:>3}", ln + 1)),
                                        )
                                        .child(div().text_color(color).child(line_text))
                                },
                            )),
                    ),
            );

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
                    } else if ed.buffer.is_dirty() {
                        "Modified"
                    } else {
                        "Clean"
                    }
                )),
        )
    }
}
