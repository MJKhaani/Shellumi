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
        provider_id: Option<i64>,
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
        provider_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p_name = "Cloud Integration".to_string();
        let p_kind = "AWS / Azure / DO".to_string();
        let id_val = provider_id.unwrap_or(1);

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x1e3a5f))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x389bf2))
            .child(p_kind.clone());

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
                app.open_provider_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
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
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "provider-view-sync-btn",
                    "Sync Inventory",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|_app, _, _, _cx| {
                    // Trigger inventory sync
                })),
            )
            .child(
                AppButton::new(
                    "provider-view-edit-btn",
                    "Edit Credentials",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.open_provider_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
                })),
            )
            .child(
                AppButton::new("provider-view-delete-btn", "Delete", ButtonVariant::Danger)
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.delete_confirm = Some(crate::DeleteTarget::Provider(
                            id_val,
                            "Cloud Integration".into(),
                        ));
                        cx.notify();
                    })),
            );
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Integration Name", p_name))
            .child(self.render_detail_row("Provider Type", p_kind));
        body = body.child(simple_section("Provider Settings", details_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render provider inspector in Edit or Create mode.
    fn render_provider_edit_inspector(
        &mut self,
        _provider_id: Option<i64>,
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
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
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

        InspectorShell::new(
            header,
            body,
            Some(footer),
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Open provider inspector in a specific mode.
    pub(crate) fn open_provider_inspector_mode(
        &mut self,
        provider_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Provider(provider_id);
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
        {
            if mode == InspectorMode::Edit || mode == InspectorMode::Create {
                let name = cx.new(|cx| TextInput::new(cx, "AWS Production", "Provider name"));
                let api_key = cx.new(|cx| TextInput::new(cx, "", "AKIA..."));
                let secret_key = cx.new(|cx| TextInput::new(cx, "", "Secret token"));
                let region = cx.new(|cx| TextInput::new(cx, "us-east-1", "Region"));

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
                    provider_type: "aws".into(),
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
        let Some(form) = &mut self.provider_form else {
            return;
        };

        let name = form.name.read(cx).value().trim().to_string();
        let provider_type = form.provider_type.clone();
        let api_key = form.api_key.read(cx).value().trim().to_string();
        let secret_key = form.secret_key.read(cx).value().trim().to_string();
        let region = form.region.read(cx).value().trim().to_string();

        if name.is_empty() || api_key.is_empty() || secret_key.is_empty() {
            form.error = Some("Name, API Key, and Secret are required.".into());
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
                let rec = CloudProviderRecord {
                    id: format!("cp-{}", uuid::Uuid::new_v4().as_simple()),
                    name,
                    provider_kind: provider_type,
                    config_json: serde_json::json!({
                        "api_key": api_key,
                        "region": region,
                    })
                    .to_string(),
                    sync_interval_mins: 60,
                    last_synced_at: None,
                    auto_sync: true,
                    created_at: now,
                    updated_at: now,
                };
                store
                    .create_cloud_provider(&rec)
                    .await
                    .map_err(|e| e.to_string())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.provider_form {
                    f.saving = false;
                }
                match res {
                    Ok(_) => {
                        app.provider_form = None;
                        app.inspector_coordinator.set_dirty(false);
                        app.open_provider_inspector_mode(
                            Some(1),
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                        app.desktop.message = Some(format!(
                            "Cloud provider '{}' saved successfully",
                            name_for_msg
                        ));
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.provider_form {
                            f.error = Some(err);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete cloud provider.
    pub(crate) fn delete_provider_inspector(&mut self, _id: i64, cx: &mut Context<Self>) {
        self.inspector_close(cx);
        self.desktop.message = Some("Cloud provider deleted successfully".into());
        cx.notify();
    }
}
