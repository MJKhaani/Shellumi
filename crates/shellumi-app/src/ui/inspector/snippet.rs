#![allow(dead_code)]
//! Snippet inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{SavedSnippet, Shellumi, SnippetFormState, TextInput};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};
use shellumi_storage::HostStore;

impl Shellumi {
    /// Render the unified Snippet Inspector for viewing, running, or editing a snippet.
    pub(crate) fn render_snippet_inspector(
        &mut self,
        snippet_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_snippet_view_inspector(snippet_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_snippet_edit_inspector(snippet_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render snippet inspector in View mode.
    fn render_snippet_view_inspector(
        &mut self,
        snippet_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let snippet = snippet_id.and_then(|id| self.snippets.iter().find(|s| s.id == id));

        let Some(snippet) = snippet else {
            return InspectorShell::new(
                div().p_4().child("Snippet not found"),
                div()
                    .p_4()
                    .child("The selected snippet does not exist or was deleted."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let id_val = snippet.id;
        let s_name = snippet.name.clone();
        let s_cmd = snippet.command.clone();
        let s_desc = snippet.description.clone().unwrap_or_else(|| "—".into());
        let tags_str = if snippet.tags.is_empty() {
            "—".into()
        } else {
            snippet.tags.join(", ")
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x183322))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x3dd68c))
            .child("Snippet");

        let edit_btn = div()
            .id("inspector-edit-snippet-btn")
            .px_2p5()
            .py_1()
            .rounded_md()
            .bg(surface())
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(text_primary())
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.open_snippet_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-snippet-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            s_name.clone(),
            Some(format!("{} variables", snippet.variables.len())),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "snippet-view-run-btn",
                    "Run Snippet",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.insert_snippet(id_val, true, cx);
                })),
            )
            .child(
                AppButton::new("snippet-view-paste-btn", "Paste", ButtonVariant::Secondary)
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.insert_snippet(id_val, false, cx);
                    })),
            )
            .child(
                AppButton::new("snippet-view-delete-btn", "Delete", ButtonVariant::Danger)
                    .on_click({
                        let name_for_del = s_name.clone();
                        cx.listener(move |app, _, _, cx| {
                            app.delete_confirm =
                                Some(crate::DeleteTarget::Snippet(id_val, name_for_del.clone()));
                            cx.notify();
                        })
                    }),
            );
        body = body.child(actions_row);

        // Command script card
        let cmd_card = div()
            .p_3()
            .rounded_md()
            .bg(rgb(0x12141c))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .text_xs()
            .font_family(crate::MONOSPACE_FONT)
            .text_color(rgb(0x389bf2))
            .child(s_cmd);
        body = body.child(simple_section("Command / Script", cmd_card));

        // Metadata section
        let mut meta_items = div().flex().flex_col().gap_1();
        meta_items = meta_items
            .child(self.render_detail_row("Description", s_desc))
            .child(self.render_detail_row("Tags", tags_str));
        body = body.child(simple_section("Properties", meta_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render snippet inspector in Edit or Create mode.
    fn render_snippet_edit_inspector(
        &mut self,
        _snippet_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(form) = &self.snippet_form else {
            return InspectorShell::new(
                div().p_4().child("Form not initialized"),
                div().p_4().child("Snippet draft is missing."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let title = if is_create {
            "New Snippet"
        } else {
            "Edit Snippet"
        };

        let back_btn = InspectorShell::close_button(
            "close-snippet-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            title,
            Some("Configure reusable terminal script and parameters"),
            None,
            None,
            Some(back_btn),
        );

        let mut form_body = div().flex().flex_col().gap_3();

        form_body = form_body
            .child(simple_field("Snippet Title", form.name.clone()))
            .child(simple_field(
                "Description (optional)",
                form.description.clone(),
            ))
            .child(simple_field(
                "Command Script (e.g. docker ps)",
                form.command.clone(),
            ))
            .child(simple_field(
                "Tags (comma-separated, e.g. docker, ops)",
                form.tags.clone(),
            ));

        let mut footer = div()
            .p_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(surface())
            .flex()
            .flex_col()
            .gap_2();

        if let Some(ref err) = form.error {
            footer = footer.child(error_notice(err.clone()));
        }

        let is_saving = form.saving;
        let buttons_row = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new(
                    "snippet-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    if !app.snippet_form.as_ref().is_some_and(|f| f.saving) {
                        app.inspector_back(cx);
                    }
                })),
            )
            .child(if is_saving {
                AppButton::new(
                    "snippet-inspector-save-btn",
                    "Saving…",
                    ButtonVariant::Secondary,
                )
            } else {
                AppButton::new("snippet-inspector-save-btn", "Save", ButtonVariant::Primary)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.save_snippet_inspector(cx);
                    }))
            });

        footer = footer.child(buttons_row);

        InspectorShell::new(
            header,
            form_body,
            Some(footer),
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Open snippet inspector in a specific mode.
    pub(crate) fn open_snippet_inspector_mode(
        &mut self,
        snippet_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Snippet(snippet_id);
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
            && (mode == InspectorMode::Edit || mode == InspectorMode::Create)
        {
            let existing = snippet_id.and_then(|id| self.snippets.iter().find(|s| s.id == id));
            let name_val = existing.map(|s| s.name.as_str()).unwrap_or("");
            let desc_val = existing
                .and_then(|s| s.description.as_deref())
                .unwrap_or("");
            let cmd_val = existing.map(|s| s.command.as_str()).unwrap_or("");
            let tags_val = existing.map(|s| s.tags.join(", ")).unwrap_or_default();

            let name = cx.new(|cx| TextInput::new(cx, name_val, "Snippet title"));
            let description = cx.new(|cx| TextInput::new(cx, desc_val, "Description (optional)"));
            let command =
                cx.new(|cx| TextInput::new(cx, cmd_val, "Command script (e.g. docker ps)"));
            let tags = cx.new(|cx| TextInput::new(cx, &tags_val, "Tags (comma-separated)"));

            cx.observe(&name, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();
            cx.observe(&description, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();
            cx.observe(&command, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();
            cx.observe(&tags, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();

            self.snippet_form = Some(SnippetFormState {
                id: snippet_id,
                name,
                description,
                command,
                tags,
                error: None,
                saving: false,
            });
        }
        cx.notify();
    }

    /// Save snippet form asynchronously.
    pub(crate) fn save_snippet_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.snippet_form else {
            return;
        };
        if form.saving {
            return;
        }

        let name = form.name.read(cx).value().trim().to_string();
        let desc = form.description.read(cx).value().trim().to_string();
        let command = form.command.read(cx).value().trim().to_string();
        let tags: Vec<String> = form
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let id = form.id;

        if name.is_empty() || command.is_empty() {
            form.error = Some("Snippet title and command are required.".into());
            cx.notify();
            return;
        }

        let snippet_res = shellumi_core::Snippet::new(
            name.clone(),
            command,
            if desc.is_empty() { None } else { Some(desc) },
            &tags,
        );

        let snippet = match snippet_res {
            Ok(s) => s,
            Err(e) => {
                if let Some(f) = &mut self.snippet_form {
                    f.error = Some(e.to_string());
                    cx.notify();
                }
                return;
            }
        };

        form.saving = true;
        form.error = None;
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let vault_key = self.active_vault_key();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                    store.set_vault_id(vid);
                }
                let saved_id = match id {
                    Some(existing_id) => {
                        store
                            .update_snippet(existing_id, &snippet)
                            .await
                            .map_err(|e| e.to_string())?;
                        existing_id
                    }
                    None => store
                        .add_snippet(&snippet)
                        .await
                        .map_err(|e| e.to_string())?,
                };
                Ok::<i64, String>(saved_id)
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.snippet_form {
                    f.saving = false;
                }
                match res {
                    Ok(saved_id) => {
                        app.snippet_form = None;
                        app.load_hosts(cx);
                        app.inspector_coordinator.set_dirty(false);
                        app.open_snippet_inspector_mode(
                            Some(saved_id),
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                        app.desktop.message =
                            Some(format!("Snippet '{}' saved successfully", name));
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.snippet_form {
                            f.error = Some(err);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete snippet asynchronously.
    pub(crate) fn delete_snippet_inspector(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let vault_key = self.active_vault_key();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                    store.set_vault_id(vid);
                }
                store.delete_snippet(id).await.map_err(|e| e.to_string())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(_) => {
                        app.load_hosts(cx);
                        app.inspector_close(cx);
                        app.desktop.message = Some("Snippet deleted successfully".into());
                    }
                    Err(e) => {
                        app.desktop.message = Some(format!("Failed to delete snippet: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
