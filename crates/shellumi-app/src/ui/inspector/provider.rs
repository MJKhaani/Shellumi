#![allow(dead_code)]
//! Infrastructure provider inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout::{self, InspectorPresentation};
use crate::{ProviderFormState, Shellumi, TextInput};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};
use shellumi_storage::{CloudProviderRecord, HostStore};

impl Shellumi {
    /// Render the unified Cloud Provider Inspector for viewing or editing infrastructure sync.
    pub(crate) fn render_provider_inspector(
        &mut self,
        provider_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_provider_view_inspector(provider_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_provider_edit_inspector(provider_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render provider inspector in View mode.
    fn render_provider_view_inspector(
        &mut self,
        provider_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let provider = provider_id
            .as_deref()
            .and_then(|id| self.cloud_providers.iter().find(|p| p.id == id))
            .or_else(|| self.cloud_providers.first())
            .cloned();

        let Some(provider) = provider else {
            let close_btn = InspectorShell::close_button(
                "close-provider-inspector-btn",
                false,
                |app, _, _, cx| {
                    app.inspector_close(cx);
                },
                cx,
            );
            let header = InspectorShell::header(
                "Cloud Provider".to_string(),
                Some("Infrastructure Provider"),
                None,
                None,
                Some(close_btn),
            );
            return InspectorShell::new(
                header,
                div()
                    .p_4()
                    .text_color(text_secondary())
                    .child("No cloud provider selected or configured yet."),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let p_name = provider.name.clone();
        let p_kind = provider.provider_kind.to_uppercase();
        let id_val = provider.id.clone();

        let cfg: serde_json::Value =
            serde_json::from_str(&provider.config_json).unwrap_or_default();
        let region = cfg
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("us-east-1")
            .to_string();
        let api_key = cfg
            .get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let masked_api = if api_key.len() > 6 {
            format!("{}...{}", &api_key[..3], &api_key[api_key.len() - 3..])
        } else {
            "••••••••".to_string()
        };

        let last_sync_str = match provider.last_synced_at {
            Some(ts) => format!("Synced (epoch {})", ts),
            None => "Never synced".to_string(),
        };
        let auto_sync_str = if provider.auto_sync {
            format!("Enabled (every {}m)", provider.sync_interval_mins)
        } else {
            "Disabled".to_string()
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x1e3a5f))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x389bf2))
            .child(p_kind.clone());

        let edit_id = id_val.clone();
        let edit_btn = div()
            .id("inspector-edit-provider-btn")
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
                app.open_provider_inspector_mode(Some(edit_id.clone()), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-provider-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            p_name.clone(),
            Some("Infrastructure Provider"),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let sync_name = p_name.clone();
        let sync_kind = p_kind.clone();
        let edit_id_action = id_val.clone();
        let delete_id_action = id_val.clone();
        let delete_name = p_name.clone();

        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "provider-view-sync-btn",
                    "Sync Inventory",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.desktop.message = Some(format!(
                        "Inventory sync initiated for '{}' ({}). Discovered hosts will appear in Discovered tab.",
                        sync_name, sync_kind
                    ));
                    cx.notify();
                })),
            )
            .child(
                AppButton::new(
                    "provider-view-edit-btn",
                    "Edit Credentials",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.open_provider_inspector_mode(Some(edit_id_action.clone()), InspectorMode::Edit, cx);
                })),
            )
            .child(
                AppButton::new("provider-view-delete-btn", "Delete", ButtonVariant::Danger)
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.delete_confirm = Some(crate::DeleteTarget::Provider(
                            delete_id_action.clone(),
                            delete_name.clone(),
                        ));
                        cx.notify();
                    })),
            );
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Integration Name", p_name))
            .child(self.render_detail_row("Provider Type", p_kind))
            .child(self.render_detail_row("Region", region))
            .child(self.render_detail_row("API Key / Access Key", masked_api))
            .child(self.render_detail_row("Sync Status", last_sync_str))
            .child(self.render_detail_row("Automatic Sync", auto_sync_str));
        body = body.child(simple_section("Provider Settings", details_items));

        InspectorShell::new(header, body, None::<Div>, self.active_inspector_width())
    }

    /// Render provider inspector in Edit or Create mode.
    fn render_provider_edit_inspector(
        &mut self,
        _provider_id: Option<String>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create {
            "New Cloud Provider"
        } else {
            "Edit Cloud Provider"
        };

        let close_btn = InspectorShell::close_button(
            "close-provider-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            title,
            Some("Configure cloud credentials for automated host discovery"),
            None,
            None,
            Some(close_btn),
        );

        let Some(form) = &self.provider_form else {
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

        // Provider Name
        let name_input = form.name.clone();
        body = body.child(simple_field(
            "Provider Name *",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(name_input),
        ));

        // Provider Type selector
        let current_type = form.provider_type.clone();
        let mut type_selector = div().flex().gap_1p5().py_1();
        for (idx, (pt, label)) in [
            ("aws", "AWS EC2"),
            ("azure", "Azure VM"),
            ("digitalocean", "DigitalOcean Droplets"),
        ]
        .iter()
        .enumerate()
        {
            let is_sel = current_type == *pt;
            let pt_val = pt.to_string();
            type_selector = type_selector.child(
                div()
                    .id(("provider-type", idx))
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
                        if let Some(ref mut f) = app.provider_form {
                            f.provider_type = pt_val.clone();
                            app.inspector_mark_dirty(cx);
                        }
                    }))
                    .child(*label),
            );
        }
        body = body.child(simple_field("Cloud Provider", type_selector));

        // API Key / Client ID
        let api_input = form.api_key.clone();
        body = body.child(simple_field(
            "API Key / Access Key ID *",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(api_input),
        ));

        // Secret Key / Token
        let secret_input = form.secret_key.clone();
        body = body.child(simple_field(
            "API Secret / Token *",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(secret_input),
        ));

        // Region
        let region_input = form.region.clone();
        body = body.child(simple_field(
            "Region (e.g. us-east-1)",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(region_input),
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
                    "provider-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_back(cx);
                })),
            )
            .child(
                AppButton::new(
                    "provider-inspector-save-btn",
                    if is_saving { "Saving…" } else { "Save" },
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(f) = &app.provider_form {
                        if !f.saving {
                            app.save_provider_inspector(cx);
                        }
                    }
                })),
            );

        InspectorShell::new(header, body, Some(footer), self.active_inspector_width())
    }

    /// Open provider inspector in a specific mode.
    pub(crate) fn open_provider_inspector_mode(
        &mut self,
        provider_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Provider(provider_id.clone());
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
        {
            if mode == InspectorMode::Edit || mode == InspectorMode::Create {
                let existing = provider_id
                    .as_deref()
                    .and_then(|id| self.cloud_providers.iter().find(|p| p.id == id));

                let (initial_name, initial_type, initial_api, initial_secret, initial_region) =
                    match existing {
                        Some(rec) => {
                            let cfg: serde_json::Value =
                                serde_json::from_str(&rec.config_json).unwrap_or_default();
                            (
                                rec.name.clone(),
                                rec.provider_kind.to_lowercase(),
                                cfg.get("api_key")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                cfg.get("secret_key")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                cfg.get("region")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("us-east-1")
                                    .to_string(),
                            )
                        }
                        None => (
                            "AWS Production".to_string(),
                            "aws".to_string(),
                            String::new(),
                            String::new(),
                            "us-east-1".to_string(),
                        ),
                    };

                let name = cx.new(|cx| TextInput::new(cx, &initial_name, "Provider name"));
                let api_key = cx.new(|cx| TextInput::new(cx, &initial_api, "AKIA..."));
                let secret_key =
                    cx.new(|cx| TextInput::new(cx, &initial_secret, "Secret token").masked());
                let region = cx.new(|cx| TextInput::new(cx, &initial_region, "Region"));

                cx.observe(&name, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&api_key, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&secret_key, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&region, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();

                self.provider_form = Some(ProviderFormState {
                    id: provider_id,
                    name,
                    provider_type: initial_type,
                    api_key,
                    secret_key,
                    region,
                    error: None,
                    saving: false,
                });
            } else {
                self.provider_form = None;
            }
        }
        cx.notify();
    }

    /// Save provider inspector form.
    pub(crate) fn save_provider_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };

        let Some(form) = &mut self.provider_form else {
            return;
        };

        let name = form.name.read(cx).value().trim().to_string();
        let provider_type = form.provider_type.clone();
        let api_key = form.api_key.read(cx).value().trim().to_string();
        let secret_key = form.secret_key.read(cx).value().trim().to_string();
        let region = form.region.read(cx).value().trim().to_string();
        let existing_id = form.id.clone();

        if name.is_empty() || api_key.is_empty() || secret_key.is_empty() {
            form.error = Some("Name, API Key, and Secret are required.".into());
            let active_vault_id = self.active_vault_id.clone();
            self.inspector_coordinator.fail_save(
                &active_vault_id,
                gen,
                "Name, API Key, and Secret are required.".into(),
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
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                    store.set_vault_id(vid);
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let target_id = existing_id
                    .unwrap_or_else(|| format!("cp-{}", uuid::Uuid::new_v4().as_simple()));
                let rec = CloudProviderRecord {
                    id: target_id.clone(),
                    name,
                    provider_kind: provider_type,
                    config_json: serde_json::json!({
                        "api_key": api_key,
                        "secret_key": secret_key,
                        "region": region,
                    })
                    .to_string(),
                    sync_interval_mins: 60,
                    last_synced_at: None,
                    auto_sync: true,
                    created_at: now,
                    updated_at: now,
                };
                if store
                    .get_cloud_provider(&target_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .is_some()
                {
                    store
                        .update_cloud_provider(&rec)
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    store
                        .create_cloud_provider(&rec)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                Ok::<String, String>(target_id)
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.provider_form {
                    f.saving = false;
                }
                match res {
                    Ok(saved_id) => {
                        app.provider_form = None;
                        let target = InspectorTarget::Provider(Some(saved_id.clone()));
                        let active_vid = app.active_vault_id.clone();
                        let ok = app
                            .inspector_coordinator
                            .complete_save(&active_vid, gen, target);
                        app.load_cloud_providers(cx);
                        app.open_provider_inspector_mode(
                            Some(saved_id),
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                        app.desktop.message = Some(format!(
                            "Cloud provider '{}' saved successfully",
                            name_for_msg
                        ));
                        if ok {
                            if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                                app.apply_navigation_request(req, None, cx);
                            }
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.provider_form {
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

    /// Delete cloud provider.
    pub(crate) fn delete_provider_inspector(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let key = self.active_vault_key();
        let id_clone = id.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .delete_cloud_provider(&id_clone)
                    .await
                    .map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(_) => {
                        app.cloud_providers.retain(|p| p.id != id);
                        app.inspector_close(cx);
                        app.desktop.message = Some("Cloud provider deleted successfully".into());
                    }
                    Err(e) => {
                        if let Some(ref mut f) = app.provider_form {
                            f.error = Some(e);
                        } else {
                            app.desktop.message = Some(format!("Failed to delete provider: {e}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
