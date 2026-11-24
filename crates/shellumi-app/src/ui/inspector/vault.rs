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
use shellumi_core::{VaultContext, VaultId, VaultKey, VaultLocation, VaultManifest};
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

        let is_default =
            v_id == shellumi_storage::LEGACY_LOCAL_VAULT_ID || data_path::is_default_vault(&v_id);
        if !is_active && !is_default {
            let unregister_id = v_id.clone();
            actions_row = actions_row
                .child(
                    AppButton::new(
                        "vault-view-unregister-btn",
                        "Unregister",
                        ButtonVariant::Secondary,
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.unregister_vault_inspector(&unregister_id, cx);
                    })),
                )
                .child({
                    let delete_id = v_id.clone();
                    let delete_name = v_name.clone();
                    AppButton::new(
                        "vault-view-delete-btn",
                        "Delete (Archive)",
                        ButtonVariant::Danger,
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.delete_confirm = Some(crate::DeleteTarget::Vault(
                            delete_id.clone(),
                            delete_name.clone(),
                        ));
                        cx.notify();
                    }))
                });
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
        let pass_label = if is_create {
            "Passphrase *"
        } else {
            "New Passphrase (leave blank to keep current)"
        };
        body = body.child(simple_field(
            pass_label,
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(pass_input),
        ));

        // Emergency Recovery Code presentation for new vaults
        if is_create {
            if let Some(ref rec_code) = form.generated_recovery_code {
                let rec_code_str = rec_code.clone();
                let is_confirmed = form.recovery_code_confirmed;
                let recovery_box = div()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x1e1500))
                    .border_1()
                    .border_color(rgb(0x8a6d1b))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xf5a623))
                            .child("Emergency Recovery Code (Save This Now)"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary())
                            .child("If you forget your master passphrase, this recovery code is the ONLY way to regain access to your vault. Store it securely offline."),
                    )
                    .child(
                        div()
                            .id("vault-recovery-code-display")
                            .p_3()
                            .rounded_md()
                            .bg(surface())
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(accent_blue())
                            .child(rec_code_str),
                    )
                    .child(
                        div()
                            .id("vault-recovery-confirm-checkbox")
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(ref mut f) = app.vault_form {
                                    f.recovery_code_confirmed = !f.recovery_code_confirmed;
                                    cx.notify();
                                }
                            }))
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(if is_confirmed {
                                        accent_blue()
                                    } else {
                                        rgb(MacTheme::BORDER)
                                    })
                                    .bg(if is_confirmed {
                                        accent_blue()
                                    } else {
                                        surface()
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(if is_confirmed { "✓" } else { "" }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(text_primary())
                                    .child("I have saved my emergency recovery code in a safe place"),
                            ),
                    );
                body = body.child(recovery_box);
            }
        }

        let is_saving = form.saving;
        let save_label = if is_saving {
            if is_create && form.generated_recovery_code.is_some() {
                "Creating Vault…"
            } else {
                "Saving…"
            }
        } else if is_create {
            if form.generated_recovery_code.is_none() {
                "Continue to Recovery Code"
            } else {
                "Create Vault"
            }
        } else {
            "Save"
        };

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
                    save_label,
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
                    generated_recovery_code: None,
                    recovery_code_confirmed: false,
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
        let (name, is_remote, server_url, passphrase, existing_vault_id, gen_rec, rec_confirmed) = {
            let Some(form) = &self.vault_form else {
                return;
            };
            (
                form.name.read(cx).value().trim().to_string(),
                form.is_remote,
                form.server_url.read(cx).value().trim().to_string(),
                form.passphrase.read(cx).value().trim().to_string(),
                form.vault_id.clone(),
                form.generated_recovery_code.clone(),
                form.recovery_code_confirmed,
            )
        };
        let active_vid = self.active_vault_id.clone();

        if name.is_empty() {
            if let Some(f) = &mut self.vault_form {
                f.error = Some("Vault name is required.".into());
            }
            cx.notify();
            return;
        }

        if is_remote && server_url.is_empty() {
            if let Some(f) = &mut self.vault_form {
                f.error = Some("Remote server URL is required for remote vaults.".into());
            }
            cx.notify();
            return;
        }

        // STEP 1 for CREATE: Generate Recovery Code and prompt confirmation before persisting
        if existing_vault_id.is_none() && !is_remote {
            if gen_rec.is_none() {
                if passphrase.is_empty() {
                    if let Some(f) = &mut self.vault_form {
                        f.error = Some("Passphrase is required to encrypt a new vault.".into());
                    }
                    cx.notify();
                    return;
                }
                if let Err(err) = shellumi_core::validate_vault_password(&passphrase) {
                    let msg = format!("Password does not meet security requirements: {err}");
                    if let Some(f) = &mut self.vault_form {
                        f.error = Some(msg);
                    }
                    cx.notify();
                    return;
                }
                match shellumi_core::VaultKey::generate_recovery_code() {
                    Ok(code) => {
                        if let Some(f) = &mut self.vault_form {
                            f.generated_recovery_code = Some(code);
                            f.recovery_code_confirmed = false;
                            f.error = None;
                        }
                        cx.notify();
                        return;
                    }
                    Err(e) => {
                        if let Some(f) = &mut self.vault_form {
                            f.error = Some(format!("Failed to generate recovery code: {e}"));
                        }
                        cx.notify();
                        return;
                    }
                }
            } else if !rec_confirmed {
                if let Some(f) = &mut self.vault_form {
                    f.error = Some(
                        "Please confirm that you have saved your emergency recovery code before proceeding."
                            .into(),
                    );
                }
                cx.notify();
                return;
            }
        }

        // For existing vault edits: validate new passphrase if entered
        if existing_vault_id.is_some() && !passphrase.is_empty() {
            if let Err(err) = shellumi_core::validate_vault_password(&passphrase) {
                let msg = format!("New password does not meet security requirements: {err}");
                if let Some(f) = &mut self.vault_form {
                    f.error = Some(msg);
                }
                cx.notify();
                return;
            }
            if let Some(ref target_id) = existing_vault_id {
                if self.vault_key_for(target_id).is_none() {
                    if let Some(f) = &mut self.vault_form {
                        f.error = Some(
                            "Cannot rotate password: vault key is locked or not loaded in memory."
                                .into(),
                        );
                    }
                    cx.notify();
                    return;
                }
            }
        }

        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };

        if let Some(f) = &mut self.vault_form {
            f.saving = true;
            f.error = None;
        }
        let Some(db_path) = self
            .database
            .clone()
            .or_else(|| self.active_vault_db_path())
        else {
            return;
        };
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let name_for_msg = name.clone();
        let recovery_code_to_persist = gen_rec;
        let existing_key = existing_vault_id
            .as_deref()
            .and_then(|id| self.vault_key_for(id));
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
                    let context = VaultContext::new(vid.clone(), location);
                    reg_store
                        .register_vault_context(&name, &context)
                        .await
                        .map_err(|e| e.to_string())?;

                    // If a new passphrase was provided, rotate the DEK encryption envelope
                    if !passphrase.is_empty() {
                        if let Some(key) = existing_key {
                            let new_primary = key
                                .wrap_with_password(&vid, &passphrase)
                                .map_err(|e| format!("Failed to re-wrap key: {e}"))?;

                            // Pre-commit verification: verify new password unwraps identical DEK
                            let test_unwrapped = VaultKey::unwrap_with_password(
                                &vid,
                                &passphrase,
                                &new_primary,
                            )
                            .map_err(|e| {
                                format!("Verification of rotated key envelope failed: {e}")
                            })?;
                            if test_unwrapped.expose_bytes() != key.expose_bytes() {
                                return Err(
                                    "Cryptographic verification failed: new password does not decrypt identical DEK"
                                        .into(),
                                );
                            }

                            let manifest_path = data_path::resolve_manifest_file(
                                std::path::Path::new(&db_path),
                                vid.as_str(),
                            );
                            let rotation_journal_path =
                                manifest_path.with_extension("rotation_journal.json");

                            let original_manifest_backup = if manifest_path.exists() {
                                Some(std::fs::read_to_string(&manifest_path).map_err(|e| {
                                    format!("Failed to read manifest for backup: {e}")
                                })?)
                            } else {
                                None
                            };

                            let original_registry_envelope = reg_store
                                .get_vault_recovery_envelope(vid.as_str())
                                .await
                                .unwrap_or(None);

                            let target_db = data_path::resolve_vault_db_file(
                                std::path::Path::new(&db_path),
                                vid.as_str(),
                            );
                            let mut original_target_db_envelope = None;
                            if let Some(target_db_str) = target_db.to_str() {
                                if target_db.exists() {
                                    if let Ok(target_store) =
                                        HostStore::open_with_key(target_db_str, Some(&key)).await
                                    {
                                        original_target_db_envelope = target_store
                                            .get_vault_recovery_envelope(vid.as_str())
                                            .await
                                            .unwrap_or(None);
                                    }
                                }
                            }

                            // Pre-rotation validation: verify that the new passphrase unwraps to the exact same DEK
                            if manifest_path.exists() {
                                let mut test_manifest =
                                    VaultManifest::load_from_path(&manifest_path)
                                        .map_err(|e| format!("Failed to parse manifest: {e}"))?;
                                test_manifest.change_password(&key, &passphrase).map_err(|e| {
                                    format!("Pre-rotation validation failed to change password: {e}")
                                })?;
                                let test_unlocked = test_manifest.unlock(&passphrase).map_err(|e| {
                                    format!("Pre-rotation validation failed: new passphrase cannot unlock derived manifest: {e}")
                                })?;
                                if test_unlocked.expose_bytes() != key.expose_bytes() {
                                    return Err("Pre-rotation validation failed: unlocked key does not match active vault DEK".into());
                                }
                            }

                            let mut rotation_journal = data_path::PasswordRotationJournal {
                                journal_version: 1,
                                vault_id: vid.as_str().to_string(),
                                stage: data_path::PasswordRotationStage::Initiated,
                                original_manifest_backup: original_manifest_backup.clone(),
                                original_registry_envelope: original_registry_envelope.clone(),
                                original_target_db_envelope: original_target_db_envelope.clone(),
                                target_db_path: target_db.to_str().map(|s| s.to_string()),
                                registry_db_path: Some(db_path.clone()),
                                updated_at: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            };
                            data_path::write_rotation_journal(
                                &rotation_journal_path,
                                &rotation_journal,
                            )?;

                            if manifest_path.exists() {
                                let mut manifest = VaultManifest::load_from_path(&manifest_path)
                                    .map_err(|e| format!("Failed to parse manifest: {e}"))?;
                                manifest.name = name.clone();
                                manifest
                                    .change_password(&key, &passphrase)
                                    .map_err(|e| {
                                        format!("Failed to update primary envelope revision: {e}")
                                    })?;
                                let serialized = serde_json::to_string_pretty(&manifest)
                                    .map_err(|e| format!("Failed to serialize manifest: {e}"))?;
                                data_path::write_durable_file(&manifest_path, &serialized)?;

                                rotation_journal.stage =
                                    data_path::PasswordRotationStage::ManifestSaved;
                                let _ = data_path::write_rotation_journal(
                                    &rotation_journal_path,
                                    &rotation_journal,
                                );
                            }

                            let env_json = serde_json::to_string(&new_primary)
                                .map_err(|e| format!("Failed to serialize recovery envelope: {e}"))?;

                            let target_db = data_path::resolve_vault_db_file(
                                std::path::Path::new(&db_path),
                                vid.as_str(),
                            );
                            if let Some(target_db_str) = target_db.to_str() {
                                if target_db.exists() {
                                    match HostStore::open_with_key(target_db_str, Some(&key)).await
                                    {
                                        Ok(mut target_store) => {
                                            if let Err(e) = target_store
                                                .save_vault_recovery_envelope(vid.as_str(), &env_json)
                                                .await
                                            {
                                                if let Some(ref bytes) = original_manifest_backup {
                                                    let _ = data_path::write_durable_file(
                                                        &manifest_path,
                                                        bytes,
                                                    );
                                                }
                                                let _ =
                                                    std::fs::remove_file(&rotation_journal_path);
                                                return Err(format!(
                                                    "Failed to persist recovery envelope in target vault: {e}"
                                                ));
                                            }
                                            rotation_journal.stage =
                                                data_path::PasswordRotationStage::TargetDbUpdated;
                                            let _ = data_path::write_rotation_journal(
                                                &rotation_journal_path,
                                                &rotation_journal,
                                            );
                                        }
                                        Err(e) => {
                                            if let Some(ref bytes) = original_manifest_backup {
                                                let _ = data_path::write_durable_file(
                                                    &manifest_path,
                                                    bytes,
                                                );
                                            }
                                            let _ = std::fs::remove_file(&rotation_journal_path);
                                            return Err(format!(
                                                "Failed to open target database for envelope update: {e}"
                                            ));
                                        }
                                    }
                                }
                            }

                            if let Err(e) = reg_store
                                .save_vault_recovery_envelope(vid.as_str(), &env_json)
                                .await
                            {
                                if let Some(ref bytes) = original_manifest_backup {
                                    let _ = data_path::write_durable_file(&manifest_path, bytes);
                                }
                                if let Some(target_db_str) = target_db.to_str() {
                                    if target_db.exists() {
                                        if let Ok(mut t_store) = HostStore::open_with_key(target_db_str, Some(&key)).await {
                                            if let Some(ref orig_t_env) = original_target_db_envelope {
                                                let _ = t_store.save_vault_recovery_envelope(vid.as_str(), orig_t_env).await;
                                            }
                                        }
                                    }
                                }
                                let _ = std::fs::remove_file(&rotation_journal_path);
                                return Err(format!(
                                    "Failed to persist recovery envelope in registry: {e}; rolled back manifest and target DB"
                                ));
                            }
                            rotation_journal.stage =
                                data_path::PasswordRotationStage::RegistryUpdated;
                            let _ = data_path::write_rotation_journal(
                                &rotation_journal_path,
                                &rotation_journal,
                            );

                            // Synchronize OS credential store and strictly verify outcome
                            let cred_store = shellumi_core::default_credential_store();
                            if let Err(e) = cred_store.set_secret(
                                "shellumi",
                                &format!("vault_passphrase_{}", vid.as_str()),
                                &passphrase,
                            ) {
                                if let Some(ref bytes) = original_manifest_backup {
                                    let _ = data_path::write_durable_file(&manifest_path, bytes);
                                }
                                if let Some(target_db_str) = target_db.to_str() {
                                    if target_db.exists() {
                                        if let Ok(mut t_store) = HostStore::open_with_key(target_db_str, Some(&key)).await {
                                            if let Some(ref orig_t_env) = original_target_db_envelope {
                                                let _ = t_store.save_vault_recovery_envelope(vid.as_str(), orig_t_env).await;
                                            }
                                        }
                                    }
                                }
                                if let Some(ref orig_reg) = original_registry_envelope {
                                    let _ = reg_store
                                        .save_vault_recovery_envelope(vid.as_str(), orig_reg)
                                        .await;
                                }
                                let _ = std::fs::remove_file(&rotation_journal_path);
                                return Err(format!(
                                    "Failed to update OS credential store with new vault passphrase: {e}; rolled back all stores"
                                ));
                            }
                            rotation_journal.stage =
                                data_path::PasswordRotationStage::CredentialStoreUpdated;
                            let _ = data_path::write_rotation_journal(
                                &rotation_journal_path,
                                &rotation_journal,
                            );

                            // Post-rotation validation: verify unlocked DEK matches
                            if manifest_path.exists() {
                                let manifest = VaultManifest::load_from_path(&manifest_path)
                                    .map_err(|e| {
                                        format!(
                                            "Failed to load rotated manifest for validation: {e}"
                                        )
                                    })?;
                                let unlocked = manifest.unlock(&passphrase).map_err(|e| {
                                    format!(
                                        "Validation failed: new passphrase cannot unlock rotated manifest: {e}"
                                    )
                                })?;
                                if unlocked.expose_bytes() != key.expose_bytes() {
                                    return Err(
                                        "Post-rotation validation failed: unlocked key does not match DEK"
                                            .into(),
                                    );
                                }
                            }
                            rotation_journal.stage = data_path::PasswordRotationStage::Verified;
                            let _ = data_path::write_rotation_journal(
                                &rotation_journal_path,
                                &rotation_journal,
                            );

                            // Complete rotation and cleanup temporary journal
                            rotation_journal.stage = data_path::PasswordRotationStage::Completed;
                            let _ = std::fs::remove_file(&rotation_journal_path);
                        }
                    }

                    Ok::<(String, Option<shellumi_core::VaultKey>), String>((target_id, None))
                } else {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    let target_id = format!("vlt-{}-{}", now.as_secs(), now.subsec_nanos());
                    let local_vault_id = VaultId::new(&target_id).map_err(|e| e.to_string())?;

                    // Prevent replacing or damaging existing vaults
                    let manifest_path = data_path::resolve_manifest_file(
                        std::path::Path::new(&db_path),
                        local_vault_id.as_str(),
                    );
                    if manifest_path.exists() {
                        return Err(format!(
                            "A vault manifest with ID '{}' already exists",
                            local_vault_id.as_str()
                        ));
                    }
                    let new_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path),
                        local_vault_id.as_str(),
                    );
                    if new_db_path.exists() {
                        return Err(format!(
                            "A database file for vault '{}' already exists",
                            local_vault_id.as_str()
                        ));
                    }

                    let cred_store = shellumi_core::default_credential_store();
                    let (key, primary_env, recovery_env) =
                        shellumi_core::VaultKeyManager::provision_new_vault(
                            cred_store.as_ref(),
                            &local_vault_id,
                            &passphrase,
                            false,
                            recovery_code_to_persist.as_deref(),
                        )
                        .map_err(|e| e.to_string())?;

                    let manifest = VaultManifest::new(
                        local_vault_id.as_str().to_string(),
                        name.clone(),
                        primary_env.clone(),
                        recovery_env.clone(),
                        false,
                    );
                    if let Err(e) = manifest.save_to_path(&manifest_path) {
                        return Err(format!("Failed to save vault manifest: {e}"));
                    }

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
                        let _ = reg_store
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
                        let target = InspectorTarget::Vault(Some(saved_id.clone()));
                        let ok = app
                            .inspector_coordinator
                            .complete_save(&active_vid, gen, target);
                        if !ok {
                            tracing::warn!(
                                originating_vault = %active_vid,
                                current_vault = %app.active_vault_id,
                                gen = gen,
                                "Discarded stale save completion for vault"
                            );
                            return;
                        }

                        if let Some(key) = opt_key {
                            app.active_vault_keys.insert(saved_id.clone(), key);
                        }
                        app.vault_form = None;
                        app.load_vault_list(cx);
                        app.desktop.message =
                            Some(format!("Vault '{}' saved successfully", name_for_msg));

                        if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                            app.apply_navigation_request(req, None, cx);
                        } else {
                            app.open_vault_inspector_mode(
                                Some(saved_id),
                                crate::ui::inspector::InspectorMode::View,
                                cx,
                            );
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.vault_form {
                            f.error = Some(err.clone());
                        }
                        app.inspector_coordinator.fail_save(&active_vid, gen, err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete a vault by quarantining its files and unregistering its context.
    pub(crate) fn delete_vault_inspector(&mut self, id: &str, cx: &mut Context<Self>) {
        if id == self.active_vault_id {
            self.desktop.message = Some(
                "Cannot delete the currently active vault. Please switch to another vault first."
                    .into(),
            );
            cx.notify();
            return;
        }
        if id == shellumi_storage::LEGACY_LOCAL_VAULT_ID || data_path::is_default_vault(id) {
            self.desktop.message = Some("Cannot delete the default local vault.".into());
            cx.notify();
            return;
        }
        if !data_path::is_safe_vault_id(id) {
            self.desktop.message = Some(format!("Invalid or unsafe vault identifier '{id}'."));
            cx.notify();
            return;
        }

        let Some(db_path) = self
            .database
            .clone()
            .or_else(|| self.active_vault_db_path())
        else {
            return;
        };
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let target_key = self.vault_key_for(id);
        let id_string = id.to_string();
        let id_string_task = id_string.clone();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let reg_db_path = data_path::resolve_vault_db_file(
                    std::path::Path::new(&db_path),
                    shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                );
                let mut reg_store = HostStore::open_with_key(
                    reg_db_path
                        .to_str()
                        .ok_or_else(|| "Invalid UTF-8 path for registry database".to_string())?,
                    default_key.as_ref(),
                )
                .await
                .map_err(|e| e.to_string())?;

                let _ = reg_store
                    .get_vault_context(&id_string_task)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| {
                        format!("Vault context '{}' was not found in registry", id_string_task)
                    })?;

                // Fail-closed check: verify pending unsynced changes before deletion
                let target_db = data_path::resolve_vault_db_file(
                    std::path::Path::new(&db_path),
                    &id_string_task,
                );
                if target_db.exists() {
                    let Some(k) = &target_key else {
                        return Err(
                            "Cannot delete encrypted vault while it is locked. Please unlock the vault first to verify pending mutations and safely decommission it."
                                .to_string(),
                        );
                    };
                    let target_db_str = target_db.to_str().ok_or_else(|| {
                        "Target database path is not valid UTF-8".to_string()
                    })?;
                    let store = HostStore::open_with_key(target_db_str, Some(k))
                        .await
                        .map_err(|e| {
                            format!("Failed to open vault to verify pending mutations: {e}")
                        })?;
                    let mutations = store
                        .get_pending_mutations(&id_string_task)
                        .await
                        .map_err(|e| format!("Failed to check pending mutations: {e}"))?;
                    if !mutations.is_empty() {
                        return Err(
                            "Vault has pending unsynced local mutations. Please sync or export before deleting."
                                .to_string(),
                        );
                    }
                    store
                        .checkpoint_truncate()
                        .await
                        .map_err(|e| format!("Failed to checkpoint vault WAL before decommissioning: {e}"))?;
                    drop(store);
                }

                // 1. Quarantine physical files first (crash-safe staging)
                let q_dir = data_path::quarantine_vault_files(
                    std::path::Path::new(&db_path),
                    &id_string_task,
                    "User requested vault deletion",
                )?;

                // 2. Unregister vault context from registry database
                let unregistered = reg_store
                    .unregister_vault_context(&id_string_task)
                    .await;

                match unregistered {
                    Ok(true) => {
                        // Success: files remain staged in quarantine for disaster recovery
                        Ok::<(), String>(())
                    }
                    Ok(false) => {
                        // Rollback quarantine: restore files back to original paths
                        let _ = data_path::restore_quarantined_vault(&q_dir);
                        Err(format!(
                            "Vault context '{}' was not found in registry; rollback complete",
                            id_string_task
                        ))
                    }
                    Err(e) => {
                        // Rollback quarantine on database failure
                        let _ = data_path::restore_quarantined_vault(&q_dir);
                        Err(format!(
                            "Failed to unregister vault context: {e}; rollback complete"
                        ))
                    }
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(_) => {
                        app.active_vault_keys.remove(&id_string);
                        app.available_vaults.retain(|v| v.id != id_string);
                        app.load_vault_list(cx);
                        app.inspector_close(cx);
                        app.desktop.message = Some(format!(
                            "Vault '{}' moved to quarantine and unregistered successfully",
                            id_string
                        ));
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

    /// Unregister a vault context without modifying or moving disk files.
    pub(crate) fn unregister_vault_inspector(&mut self, id: &str, cx: &mut Context<Self>) {
        if id == self.active_vault_id {
            self.desktop.message = Some(
                "Cannot unregister the currently active vault. Please switch to another vault first."
                    .into(),
            );
            cx.notify();
            return;
        }
        if id == shellumi_storage::LEGACY_LOCAL_VAULT_ID || data_path::is_default_vault(id) {
            self.desktop.message = Some("Cannot unregister the default local vault.".into());
            cx.notify();
            return;
        }

        let Some(db_path) = self
            .database
            .clone()
            .or_else(|| self.active_vault_db_path())
        else {
            return;
        };
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let id_string = id.to_string();
        let id_string_task = id_string.clone();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let reg_db_path = data_path::resolve_vault_db_file(
                    std::path::Path::new(&db_path),
                    shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                );
                let mut reg_store = HostStore::open_with_key(
                    reg_db_path
                        .to_str()
                        .ok_or_else(|| "Invalid UTF-8 path for registry database".to_string())?,
                    default_key.as_ref(),
                )
                .await
                .map_err(|e| e.to_string())?;

                let unregistered = reg_store
                    .unregister_vault_context(&id_string_task)
                    .await
                    .map_err(|e| e.to_string())?;

                if !unregistered {
                    return Err(format!(
                        "Vault context '{}' was not found or already unregistered",
                        id_string_task
                    ));
                }

                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(_) => {
                        app.active_vault_keys.remove(&id_string);
                        app.available_vaults.retain(|v| v.id != id_string);
                        app.load_vault_list(cx);
                        app.inspector_close(cx);
                        app.desktop.message = Some(format!(
                            "Vault '{}' unregistered (data files preserved)",
                            id_string
                        ));
                    }
                    Err(e) => {
                        app.desktop.message = Some(format!("Failed to unregister vault: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
