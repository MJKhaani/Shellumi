#![allow(dead_code)]
//! Vault inspector rendering and actions for View, Edit, and Create modes.

use crate::data_path;
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
use shellumi_core::{VaultContext, VaultId, VaultLocation, VaultManifest};
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
                self.active_inspector_width(),
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

        InspectorShell::new(header, body, None::<Div>, self.active_inspector_width())
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
                self.active_inspector_width(),
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

        InspectorShell::new(header, body, Some(footer), self.active_inspector_width())
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
                let passphrase =
                    cx.new(|cx| TextInput::new(cx, "", "Encryption passphrase").masked());

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
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };

        let Some(form) = &mut self.vault_form else {
            return;
        };

        let name = form.name.read(cx).value().trim().to_string();
        let is_remote = form.is_remote;
        let server_url = form.server_url.read(cx).value().trim().to_string();
        let passphrase = form.passphrase.read(cx).value().trim().to_string();
        let existing_vault_id = form.vault_id.clone();
        let active_vid = self.active_vault_id.clone();

        if name.is_empty() {
            form.error = Some("Vault name is required.".into());
            self.inspector_coordinator.fail_save(
                &active_vid,
                gen,
                "Vault name is required.".into(),
            );
            cx.notify();
            return;
        }

        if is_remote && server_url.is_empty() {
            form.error = Some("Remote server URL is required for remote vaults.".into());
            self.inspector_coordinator.fail_save(
                &active_vid,
                gen,
                "Remote server URL is required for remote vaults.".into(),
            );
            cx.notify();
            return;
        }

        if existing_vault_id.is_none() && !is_remote {
            if passphrase.is_empty() {
                form.error = Some("Passphrase is required to encrypt a new vault.".into());
                self.inspector_coordinator.fail_save(
                    &active_vid,
                    gen,
                    "Passphrase is required to encrypt a new vault.".into(),
                );
                cx.notify();
                return;
            }
            if let Err(err) = shellumi_core::validate_vault_password(&passphrase) {
                let msg = format!("Password does not meet security requirements: {err}");
                form.error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
        }

        form.saving = true;
        form.error = None;
        let Some(db_path) = self
            .database
            .clone()
            .or_else(|| self.active_vault_db_path())
        else {
            return;
        };
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let name_for_msg = name.clone();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                if let Some(target_id) = existing_vault_id {
                    let vid = VaultId::new(&target_id).map_err(|e| e.to_string())?;
                    let location = if is_remote {
                        let conn_id = shellumi_core::ServerConnectionId::new(&server_url)
                            .map_err(|e| e.to_string())?;
                        let remote_id = VaultId::new(&target_id).map_err(|e| e.to_string())?;
                        VaultLocation::Remote {
                            server: conn_id,
                            server_vault_id: remote_id,
                        }
                    } else {
                        VaultLocation::Local
                    };
                    let reg_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path),
                        shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                    );
                    let mut reg_store = HostStore::open_with_key(
                        reg_db_path
                            .to_str()
                            .ok_or_else(|| "Invalid UTF-8 path".to_string())?,
                        default_key.as_ref(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    let context = VaultContext::new(vid, location);
                    reg_store
                        .register_vault_context(&name, &context)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok::<(String, Option<shellumi_core::VaultKey>), String>((target_id, None))
                } else {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    let target_id = format!("vlt-{}-{}", now.as_secs(), now.subsec_nanos());
                    let local_vault_id = VaultId::new(&target_id).map_err(|e| e.to_string())?;

                    let cred_store = shellumi_core::default_credential_store();
                    let recovery_code = shellumi_core::VaultKey::generate_recovery_code()
                        .map_err(|e| e.to_string())?;
                    let (key, primary_env, recovery_env) =
                        shellumi_core::VaultKeyManager::provision_new_vault(
                            cred_store.as_ref(),
                            &local_vault_id,
                            &passphrase,
                            false,
                            Some(&recovery_code),
                        )
                        .map_err(|e| e.to_string())?;

                    let manifest = VaultManifest::new(
                        local_vault_id.as_str().to_string(),
                        name.clone(),
                        primary_env.clone(),
                        recovery_env.clone(),
                        false,
                    );
                    let manifest_path = data_path::resolve_manifest_file(
                        std::path::Path::new(&db_path),
                        local_vault_id.as_str(),
                    );
                    manifest
                        .save_to_path(&manifest_path)
                        .map_err(|e| format!("Failed to save vault manifest: {e}"))?;

                    let new_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path),
                        local_vault_id.as_str(),
                    );
                    if let Err(e) = data_path::prepare_parent(&new_db_path) {
                        let _ = std::fs::remove_file(&manifest_path);
                        return Err(e);
                    }
                    let new_db_str = new_db_path
                        .to_str()
                        .ok_or_else(|| "Invalid UTF-8 path for new vault database".to_string())?;
                    let mut new_store = match HostStore::open_with_key(new_db_str, Some(&key)).await
                    {
                        Ok(s) => s,
                        Err(err) => {
                            let _ = std::fs::remove_file(&manifest_path);
                            let _ = std::fs::remove_file(&new_db_path);
                            return Err(format!("Failed to initialize encrypted database: {err}"));
                        }
                    };
                    new_store.set_vault_id(local_vault_id.clone());

                    let reg_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path),
                        shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                    );
                    let mut reg_store = match HostStore::open_with_key(
                        reg_db_path.to_str().unwrap(),
                        default_key.as_ref(),
                    )
                    .await
                    {
                        Ok(s) => s,
                        Err(err) => {
                            let _ = std::fs::remove_file(&manifest_path);
                            let _ = std::fs::remove_file(&new_db_path);
                            return Err(format!("Failed to open default registry database: {err}"));
                        }
                    };

                    let location = if is_remote {
                        let conn_id = shellumi_core::ServerConnectionId::new(&server_url)
                            .map_err(|e| e.to_string())?;
                        let remote_id = VaultId::new(&target_id).map_err(|e| e.to_string())?;
                        VaultLocation::Remote {
                            server: conn_id,
                            server_vault_id: remote_id,
                        }
                    } else {
                        VaultLocation::Local
                    };

                    let context = VaultContext::new(local_vault_id.clone(), location);
                    if let Err(e) = reg_store.register_vault_context(&name, &context).await {
                        let _ = std::fs::remove_file(&manifest_path);
                        let _ = std::fs::remove_file(&new_db_path);
                        return Err(format!("Failed to register vault context: {e}"));
                    }

                    if let Ok(env_json) = serde_json::to_string(&primary_env) {
                        let _ = new_store
                            .save_vault_recovery_envelope(local_vault_id.as_str(), &env_json)
                            .await;
                    }

                    Ok::<(String, Option<shellumi_core::VaultKey>), String>((target_id, Some(key)))
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.vault_form {
                    f.saving = false;
                }
                match res {
                    Ok((saved_id, opt_key)) => {
                        if let Some(key) = opt_key {
                            app.active_vault_keys.insert(saved_id.clone(), key);
                        }
                        app.vault_form = None;
                        app.load_vault_list(cx);
                        let target = InspectorTarget::Vault(Some(saved_id.clone()));
                        let active_vid = app.active_vault_id.clone();
                        let ok = app
                            .inspector_coordinator
                            .complete_save(&active_vid, gen, target);
                        app.open_vault_inspector_mode(
                            Some(saved_id),
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                        app.desktop.message =
                            Some(format!("Vault '{}' saved successfully", name_for_msg));
                        if ok {
                            if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                                app.apply_navigation_request(req, None, cx);
                            }
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.vault_form {
                            f.error = Some(err.clone());
                        }
                        let active_vid = app.active_vault_id.clone();
                        app.inspector_coordinator.fail_save(&active_vid, gen, err);
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
