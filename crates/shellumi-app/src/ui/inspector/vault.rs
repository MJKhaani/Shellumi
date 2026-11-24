#![allow(dead_code)]
//! Vault inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout::{self, InspectorPresentation};
use crate::{Shellumi, TextInput, VaultFormState};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};
use shellumi_storage::HostStore;

impl Shellumi {
    /// Render the unified Vault Inspector for viewing or editing vaults.
    pub(crate) fn render_vault_inspector(
        &mut self,
        vault_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_vault_view_inspector(vault_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_vault_edit_inspector(vault_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render vault inspector in View mode.
    fn render_vault_view_inspector(
        &mut self,
        vault_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let id_str = vault_id.unwrap_or_else(|| self.active_vault_id.clone());
        let vault = self
            .available_vaults
            .iter()
            .find(|v| v.id == id_str)
            .cloned();

        let Some(vault) = vault else {
            return InspectorShell::new(
                div().p_4().child("Vault not found"),
                div()
                    .p_4()
                    .child("The requested vault does not exist or has been removed."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let v_id = vault.id.clone();
        let v_name = vault.name.clone();
        let is_active = v_id == self.active_vault_id;
        let mode_str = if vault.is_remote {
            "Cloud / Remote"
        } else {
            "Local Encrypted"
        };
        let lock_str = if vault.is_locked {
            "Locked"
        } else {
            "Unlocked / Active"
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x332800))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0xf5a623))
            .child(mode_str);

        let edit_btn = div()
            .id("inspector-edit-vault-btn")
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
            .on_click(cx.listener({
                let id_clone = v_id.clone();
                move |app, _, _, cx| {
                    app.open_vault_inspector_mode(Some(id_clone.clone()), InspectorMode::Edit, cx);
                }
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-vault-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            v_name.clone(),
            Some(format!("Vault ID: {}", v_id)),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let mut actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new("vault-view-sync-btn", "Sync Now", ButtonVariant::Primary).on_click(
                    cx.listener(|app, _, _, cx| {
                        app.sync_active_vault(cx);
                    }),
                ),
            )
            .child(
                AppButton::new(
                    "vault-view-lock-btn",
                    "Lock Vault",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.lock_active_vault(cx);
                })),
            );

        if !is_active {
            let delete_id = v_id.clone();
            let delete_name = v_name.clone();
            actions_row = actions_row.child(
                AppButton::new("vault-view-delete-btn", "Delete", ButtonVariant::Danger).on_click(
                    cx.listener(move |app, _, _, cx| {
                        app.delete_confirm = Some(crate::DeleteTarget::Vault(
                            delete_id.clone(),
                            delete_name.clone(),
                        ));
                        cx.notify();
                    }),
                ),
            );
        }

        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Vault Name", v_name))
            .child(self.render_detail_row("Storage Mode", mode_str.into()))
            .child(self.render_detail_row("Status", lock_str.into()));
        if let Some(ref url) = vault.server_url {
            details_items = details_items.child(self.render_detail_row("Server URL", url.clone()));
        }
        body = body.child(simple_section("Vault Security & Sync", details_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render vault inspector in Edit or Create mode.
    fn render_vault_edit_inspector(
        &mut self,
        _vault_id: Option<String>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create { "New Vault" } else { "Edit Vault" };

        let close_btn = InspectorShell::close_button(
            "close-vault-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            title,
            Some("Configure encrypted database and sync credentials"),
            None,
            None,
            Some(close_btn),
        );

        let Some(form) = &self.vault_form else {
            return InspectorShell::new(
                header,
                div().p_4().child("Form not initialized"),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let mut body = div().flex().flex_col().gap_4().p_1();

        if let Some(err) = &form.error {
            body = body.child(error_notice(err));
        }

        // Vault name
        let name_input = form.name.clone();
        body = body.child(simple_field(
            "Vault Name *",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(name_input),
        ));

        // Storage Mode selector (Local vs Remote)
        let is_remote = form.is_remote;
        let mode_selector = div()
            .flex()
            .gap_2()
            .child(
                div()
                    .id("vault-mode-local")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if !is_remote {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if !is_remote {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        surface()
                    })
                    .text_color(if !is_remote {
                        accent_blue()
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(ref mut f) = app.vault_form {
                            f.is_remote = false;
                            app.inspector_mark_dirty(cx);
                        }
                    }))
                    .child("Local Encrypted"),
            )
            .child(
                div()
                    .id("vault-mode-remote")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if is_remote {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_remote {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        surface()
                    })
                    .text_color(if is_remote {
                        accent_blue()
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(ref mut f) = app.vault_form {
                            f.is_remote = true;
                            app.inspector_mark_dirty(cx);
                        }
                    }))
                    .child("Remote / Cloud Sync"),
            );
        body = body.child(simple_field("Storage Mode", mode_selector));

        if is_remote {
            let server_input = form.server_url.clone();
            body = body.child(simple_field(
                "Remote Server URL *",
                div()
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .rounded_md()
                    .bg(surface())
                    .p_2()
                    .child(server_input),
            ));
        }

        // Passphrase
        let pass_input = form.passphrase.clone();
        body = body.child(simple_field(
            "Passphrase *",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(pass_input),
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
                AppButton::new("vault-inspector-cancel-btn", "Cancel", ButtonVariant::Ghost)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.inspector_back(cx);
                    })),
            )
            .child(
                AppButton::new(
                    "vault-inspector-save-btn",
                    if is_saving { "Saving…" } else { "Save" },
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(f) = &app.vault_form {
                        if !f.saving {
                            app.save_vault_inspector(cx);
                        }
                    }
                })),
            );

        InspectorShell::new(
            header,
            body,
            Some(footer),
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Open vault inspector in a specific mode.
    pub(crate) fn open_vault_inspector_mode(
        &mut self,
        vault_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_context = vault_id
            .clone()
            .unwrap_or_else(|| self.active_vault_id.clone());
        let target = InspectorTarget::Vault(vault_id.clone());
        if self
            .inspector_coordinator
            .open(vault_context, target, mode, None)
        {
            if mode == InspectorMode::Edit || mode == InspectorMode::Create {
                let existing = vault_id
                    .as_deref()
                    .and_then(|id| self.available_vaults.iter().find(|v| v.id == id));
                let name_val = existing.map(|v| v.name.as_str()).unwrap_or("");
                let is_rem = existing.map(|v| v.is_remote).unwrap_or(false);
                let url_val = existing
                    .and_then(|v| v.server_url.as_deref())
                    .unwrap_or("https://sync.shellumi.internal");

                let name = cx.new(|cx| TextInput::new(cx, name_val, "Vault name"));
                let server_url = cx.new(|cx| TextInput::new(cx, url_val, "https://example.com"));
                let passphrase = cx.new(|cx| TextInput::new(cx, "", "Encryption passphrase"));

                cx.observe(&name, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&server_url, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&passphrase, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();

                self.vault_form = Some(VaultFormState {
                    vault_id,
                    name,
                    is_remote: is_rem,
                    server_url,
                    passphrase,
                    error: None,
                    saving: false,
                });
            } else {
                self.vault_form = None;
            }
        }
        cx.notify();
    }

    /// Save vault form.
    pub(crate) fn save_vault_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.vault_form else {
            return;
        };

        let name = form.name.read(cx).value().trim().to_string();
        let is_remote = form.is_remote;
        let server_url = form.server_url.read(cx).value().trim().to_string();
        let _passphrase = form.passphrase.read(cx).value().trim().to_string();
        let existing_vault_id = form.vault_id.clone();

        if name.is_empty() {
            form.error = Some("Vault name is required.".into());
            cx.notify();
            return;
        }

        if is_remote && server_url.is_empty() {
            form.error = Some("Remote server URL is required for remote vaults.".into());
            cx.notify();
            return;
        }

        form.saving = true;
        form.error = None;
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
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
                let target_id = existing_vault_id
                    .unwrap_or_else(|| format!("vault-{}", uuid::Uuid::new_v4().as_simple()));
                let vid = shellumi_core::VaultId::new(&target_id).map_err(|e| e.to_string())?;

                let location = if is_remote {
                    let conn_id = shellumi_core::ServerConnectionId::new(&server_url)
                        .map_err(|e| e.to_string())?;
                    let remote_id =
                        shellumi_core::VaultId::new(&target_id).map_err(|e| e.to_string())?;
                    shellumi_core::VaultLocation::Remote {
                        server: conn_id,
                        server_vault_id: remote_id,
                    }
                } else {
                    shellumi_core::VaultLocation::Local
                };

                let context = shellumi_core::VaultContext::new(vid, location);
                store
                    .register_vault_context(&name, &context)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<String, String>(target_id)
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.vault_form {
                    f.saving = false;
                }
                match res {
                    Ok(saved_id) => {
                        app.vault_form = None;
                        app.load_vault_list(cx);
                        app.inspector_coordinator.set_dirty(false);
                        app.open_vault_inspector_mode(
                            Some(saved_id),
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                        app.desktop.message =
                            Some(format!("Vault '{}' saved successfully", name_for_msg));
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.vault_form {
                            f.error = Some(err);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete a vault by unregistering its context.
    pub(crate) fn delete_vault_inspector(&mut self, id: &str, cx: &mut Context<Self>) {
        if id == self.active_vault_id {
            self.desktop.message = Some("Cannot delete the currently active vault.".into());
            cx.notify();
            return;
        }

        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let vault_key = self.active_vault_key();
        let id_string = id.to_string();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .unregister_vault_context(&id_string)
                    .await
                    .map_err(|e| e.to_string())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(_) => {
                        app.load_vault_list(cx);
                        app.inspector_close(cx);
                        app.desktop.message = Some("Vault removed successfully".into());
                    }
                    Err(e) => {
                        app.desktop.message = Some(format!("Failed to delete vault: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
