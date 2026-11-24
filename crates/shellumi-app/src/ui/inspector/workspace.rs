#![allow(dead_code)]
//! Workspace inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout::{self, InspectorPresentation};
use crate::{SavedWorkspace, Shellumi, TextInput, WorkspaceFormState};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};
use shellumi_storage::HostStore;

impl Shellumi {
    /// Render the unified Workspace Inspector for viewing or editing multi-session workspaces.
    pub(crate) fn render_workspace_inspector(
        &mut self,
        workspace_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_workspace_view_inspector(workspace_id, cx),
            InspectorMode::Edit | InspectorMode::Create => self.render_workspace_edit_inspector(
                workspace_id,
                mode == InspectorMode::Create,
                cx,
            ),
        }
    }

    /// Render workspace inspector in View mode.
    fn render_workspace_view_inspector(
        &mut self,
        workspace_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let ws = workspace_id.and_then(|id| self.workspaces.iter().find(|w| w.id == id));

        let Some(ws) = ws else {
            return InspectorShell::new(
                div().p_4().child("Workspace not found"),
                div()
                    .p_4()
                    .child("The selected workspace layout does not exist or was deleted."),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let id_val = ws.id;
        let ws_name = ws.name.clone();
        let session_count = ws.sessions.len();
        let layout_str = ws.layout.clone();
        let desc_str = ws.description.clone().unwrap_or_else(|| "—".into());

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x2d1f3d))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0xbb86fc))
            .child("Workspace");

        let edit_btn = div()
            .id("inspector-edit-workspace-btn")
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
                app.open_workspace_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-workspace-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            ws_name.clone(),
            Some(format!(
                "{} sessions (Layout: {})",
                session_count, layout_str
            )),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let delete_name = ws_name.clone();
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "workspace-view-launch-btn",
                    "Launch Workspace",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(move |_app, _, _, _cx| {
                    // Launch workspace
                })),
            )
            .child(
                AppButton::new(
                    "workspace-view-edit-btn",
                    "Edit Layout",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.open_workspace_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
                })),
            )
            .child(
                AppButton::new("workspace-view-delete-btn", "Delete", ButtonVariant::Danger)
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.delete_confirm =
                            Some(crate::DeleteTarget::Workspace(id_val, delete_name.clone()));
                        cx.notify();
                    })),
            );
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Workspace Name", ws_name))
            .child(self.render_detail_row("Layout Type", layout_str))
            .child(self.render_detail_row("Description", desc_str));
        body = body.child(simple_section("Workspace Properties", details_items));

        InspectorShell::new(header, body, None::<Div>, self.active_inspector_width())
    }

    /// Render workspace inspector in Edit or Create mode.
    fn render_workspace_edit_inspector(
        &mut self,
        _workspace_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create {
            "New Workspace"
        } else {
            "Edit Workspace"
        };

        let close_btn = InspectorShell::close_button(
            "close-workspace-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            title,
            Some("Configure multi-session grid and pane presets"),
            None,
            None,
            Some(close_btn),
        );

        let Some(form) = &self.workspace_form else {
            return InspectorShell::new(
                header,
                div().p_4().child("Form not initialized"),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let mut body = div().flex().flex_col().gap_4().p_1();

        if let Some(err) = &form.error {
            body = body.child(error_notice(err));
        }

        // Name input
        let name_input = form.name.clone();
        body = body.child(simple_field(
            "Workspace Name *",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(name_input),
        ));

        // Layout selector
        let current_layout = form.layout.clone();
        let mut layout_selector = div().flex().gap_1p5().py_1();
        for (idx, (l_id, label)) in [
            ("single", "Single"),
            ("split_vertical", "2-Split Vert"),
            ("split_horizontal", "2-Split Horiz"),
            ("dual_grid", "4-Grid"),
        ]
        .iter()
        .enumerate()
        {
            let is_sel = current_layout == *l_id;
            let l_val = l_id.to_string();
            layout_selector = layout_selector.child(
                div()
                    .id(("workspace-layout", idx))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if is_sel {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_sel {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        surface()
                    })
                    .text_color(if is_sel {
                        accent_blue()
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(ref mut f) = app.workspace_form {
                            f.layout = l_val.clone();
                            app.inspector_mark_dirty(cx);
                        }
                    }))
                    .child(*label),
            );
        }
        body = body.child(simple_field("Layout Preset", layout_selector));

        // Description input
        let desc_input = form.description.clone();
        body = body.child(simple_field(
            "Description (Optional)",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(desc_input),
        ));

        let is_saving = form.saving;
        let footer = div()
            .p_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(surface())
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new(
                    "workspace-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_back(cx);
                })),
            )
            .child(
                AppButton::new(
                    "workspace-inspector-save-btn",
                    if is_saving { "Saving…" } else { "Save" },
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(f) = &app.workspace_form {
                        if !f.saving {
                            app.save_workspace_inspector(cx);
                        }
                    }
                })),
            );

        InspectorShell::new(header, body, Some(footer), self.active_inspector_width())
    }

    /// Open workspace inspector in a specific mode.
    pub(crate) fn open_workspace_inspector_mode(
        &mut self,
        workspace_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Workspace(workspace_id);
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
        {
            if mode == InspectorMode::Edit || mode == InspectorMode::Create {
                let existing =
                    workspace_id.and_then(|id| self.workspaces.iter().find(|w| w.id == id));
                let name_val = existing.map(|w| w.name.as_str()).unwrap_or("");
                let desc_val = existing
                    .and_then(|w| w.description.as_deref())
                    .unwrap_or("");
                let layout_val = existing
                    .map(|w| w.layout.clone())
                    .unwrap_or_else(|| "single".into());

                let name = cx.new(|cx| TextInput::new(cx, name_val, "Workspace name"));
                let description =
                    cx.new(|cx| TextInput::new(cx, desc_val, "Description (optional)"));

                cx.observe(&name, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&description, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();

                self.workspace_form = Some(WorkspaceFormState {
                    id: workspace_id,
                    name,
                    layout: layout_val,
                    description,
                    error: None,
                    saving: false,
                });
            } else {
                self.workspace_form = None;
            }
        }
        cx.notify();
    }

    /// Save workspace inspector form data.
    pub(crate) fn save_workspace_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };

        let Some(form) = &mut self.workspace_form else {
            return;
        };

        let active_vid = self.active_vault_id.clone();
        let name = form.name.read(cx).value().trim().to_string();
        let desc = form.description.read(cx).value().trim().to_string();
        let layout_str = form.layout.clone();
        let id = form.id;

        if name.is_empty() {
            form.error = Some("Workspace name is required.".into());
            self.inspector_coordinator.fail_save(
                &active_vid,
                gen,
                "Workspace name is required.".into(),
            );
            cx.notify();
            return;
        }

        form.saving = true;
        form.error = None;
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();
        let name_for_msg = name.clone();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }
                let desc_opt = if desc.is_empty() {
                    None
                } else {
                    Some(desc.as_str())
                };
                let saved_id = match id {
                    Some(existing_id) => {
                        store
                            .update_workspace(existing_id, &name, desc_opt, &layout_str, &[], &[])
                            .await
                            .map_err(|e| e.to_string())?;
                        existing_id
                    }
                    None => store
                        .add_workspace(&name, desc_opt, &layout_str, &[], &[])
                        .await
                        .map_err(|e| e.to_string())?,
                };
                Ok::<i64, String>(saved_id)
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.workspace_form {
                    f.saving = false;
                }
                match res {
                    Ok(saved_id) => {
                        let target = InspectorTarget::Workspace(Some(saved_id));
                        let ok =
                            app.inspector_coordinator
                                .complete_save(&active_vault_id, gen, target);
                        if !ok {
                            tracing::warn!(
                                originating_vault = %active_vault_id,
                                current_vault = %app.active_vault_id,
                                gen = gen,
                                "Discarded stale save completion for workspace"
                            );
                            return;
                        }

                        app.workspace_form = None;
                        app.load_workspaces(cx);
                        app.desktop.message =
                            Some(format!("Workspace '{}' saved successfully", name_for_msg));

                        if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                            app.apply_navigation_request(req, None, cx);
                        } else {
                            app.open_workspace_inspector_mode(
                                Some(saved_id),
                                crate::ui::inspector::InspectorMode::View,
                                cx,
                            );
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.workspace_form {
                            f.error = Some(err.clone());
                        }
                        app.inspector_coordinator
                            .fail_save(&active_vault_id, gen, err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete a workspace.
    pub(crate) fn delete_workspace_inspector(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }
                let deleted = store
                    .delete_workspace(id)
                    .await
                    .map_err(|e| e.to_string())?;
                if !deleted {
                    return Err(format!("Workspace with ID {id} not found"));
                }
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(()) => {
                        if app.active_vault_id == active_vault_id {
                            app.load_workspaces(cx);
                            app.inspector_close(cx);
                            app.desktop.message = Some("Workspace deleted successfully".into());
                        }
                    }
                    Err(e) => {
                        app.desktop.message = Some(format!("Failed to delete workspace: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
