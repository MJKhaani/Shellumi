#![allow(dead_code, unused_imports)]
//! Unified Inspector state machine, coordinator, and typed drafts.

pub mod coordinator;
pub mod drafts;
pub mod group;
pub mod host;

pub use coordinator::{
    AsyncOperationState, InspectorCoordinator, InspectorMode, InspectorState, InspectorTarget,
    NavigationRequest,
};
pub use drafts::*;

use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::Shellumi;
use gpui::{div, prelude::*, Context, Div, IntoElement, ParentElement, Window};

impl Shellumi {
    /// Open the host inspector with target host ID and mode.
    pub(crate) fn open_host_inspector(
        &mut self,
        host_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Host(host_id);
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
        {
            self.selected_host_id = host_id;
            self.group_inspector = None;
            if mode == InspectorMode::Edit || mode == InspectorMode::Create {
                let host_to_edit = if let Some(id) = host_id {
                    let hosts = match &self.hosts {
                        crate::HostState::Loaded(hosts) => hosts.as_slice(),
                        _ => &[],
                    };
                    hosts.iter().find(|h| h.id == id).cloned()
                } else {
                    None
                };
                self.edit_host(host_to_edit.as_ref(), cx);
            }
        }
        cx.notify();
    }

    /// Select a host from grid or list, replacing whatever inspector was open.
    pub(crate) fn inspector_select_host(&mut self, host_id: i64, cx: &mut Context<Self>) {
        let target = InspectorTarget::Host(Some(host_id));
        if self.inspector_coordinator.is_open()
            && self.inspector_coordinator.active().map(|a| &a.target) == Some(&target)
        {
            if self.inspector_coordinator.is_dirty() {
                let _ = self.inspector_coordinator.request_close();
            } else {
                self.inspector_coordinator.close();
                self.selected_host_id = None;
            }
        } else {
            let vault_id = self.active_vault_id.clone();
            if self
                .inspector_coordinator
                .open(vault_id, target, InspectorMode::View, None)
            {
                self.selected_host_id = Some(host_id);
                self.group_inspector = None;
            }
        }
        cx.notify();
    }

    /// Select a group, replacing whatever inspector was open.
    pub(crate) fn inspector_select_group(&mut self, group_name: &str, cx: &mut Context<Self>) {
        let target = InspectorTarget::Group(Some(group_name.to_string()));
        let vault_id = self.active_vault_id.clone();
        if self
            .inspector_coordinator
            .open(vault_id, target, InspectorMode::View, None)
        {
            self.selected_host_id = None;
            self.open_group_inspector(group_name, cx);
        }
        cx.notify();
    }

    /// Open inspector to create a new host.
    pub(crate) fn inspector_new_host(&mut self, cx: &mut Context<Self>) {
        let target = InspectorTarget::Host(None);
        let vault_id = self.active_vault_id.clone();
        if self
            .inspector_coordinator
            .open(vault_id, target, InspectorMode::Create, None)
        {
            self.selected_host_id = None;
            self.edit_host(None, cx);
        }
        cx.notify();
    }

    /// Open inspector to create a new group.
    pub(crate) fn inspector_new_group(&mut self, cx: &mut Context<Self>) {
        let target = InspectorTarget::Group(None);
        let vault_id = self.active_vault_id.clone();
        if self
            .inspector_coordinator
            .open(vault_id, target, InspectorMode::Create, None)
        {
            self.selected_host_id = None;
            self.open_group_inspector("", cx);
        }
        cx.notify();
    }

    /// Close inspector (guarded if dirty).
    pub(crate) fn inspector_close(&mut self, cx: &mut Context<Self>) {
        if self.inspector_coordinator.request_close() {
            self.selected_host_id = None;
            self.form = None;
            self.group_inspector = None;
        }
        cx.notify();
    }

    /// Render the unified inspector based on current active target and mode.
    pub(crate) fn render_unified_inspector(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let (target, mode) = if let Some(active) = self.inspector_coordinator.active() {
            (active.target.clone(), active.mode)
        } else {
            return div();
        };

        match target {
            InspectorTarget::Host(host_id) => self.render_host_inspector(host_id, mode, window, cx),
            InspectorTarget::Group(group_name) => self.render_group_inspector(group_name, mode, cx),
            _ => div(),
        }
    }

    /// Render delete confirmation dialog if requested.
    pub(crate) fn render_delete_confirm_dialog(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let target = self.delete_confirm.clone()?;
        let (obj_type, obj_name) = match &target {
            crate::DeleteTarget::Host(_, name) => ("host", name.clone()),
            crate::DeleteTarget::Group(name) => ("group", name.clone()),
        };

        let confirm_btn = AppButton::new(
            "confirm-delete-alert-btn",
            format!("Delete {}", obj_type),
            ButtonVariant::Danger,
        )
        .on_click(cx.listener(move |app, _, _, cx| {
            match &target {
                crate::DeleteTarget::Host(id, _) => {
                    app.delete_host(*id, cx);
                    app.inspector_close(cx);
                }
                crate::DeleteTarget::Group(name) => {
                    app.delete_group_inspector(name, cx);
                    app.inspector_close(cx);
                }
            }
            app.delete_confirm = None;
            cx.notify();
        }));

        let cancel_btn = AppButton::new("cancel-delete-alert-btn", "Cancel", ButtonVariant::Ghost)
            .on_click(cx.listener(|app, _, _, cx| {
                app.delete_confirm = None;
                cx.notify();
            }));

        Some(crate::ui::components::delete_confirmation_dialog(
            obj_type,
            &obj_name,
            confirm_btn,
            cancel_btn,
        ))
    }

    /// Render unsaved changes guard dialog if active.
    pub(crate) fn render_unsaved_changes_guard(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.inspector_coordinator.is_guard_active() {
            return None;
        }

        let obj_name = self
            .inspector_coordinator
            .active()
            .map(|s| match &s.target {
                InspectorTarget::Host(Some(id)) => {
                    let hosts = match &self.hosts {
                        crate::HostState::Loaded(hosts) => hosts.as_slice(),
                        _ => &[],
                    };
                    hosts
                        .iter()
                        .find(|h| h.id == *id)
                        .map(|h| h.name.clone())
                        .unwrap_or_else(|| "Host".into())
                }
                InspectorTarget::Host(None) => "New Host".into(),
                InspectorTarget::Group(Some(name)) => name.clone(),
                InspectorTarget::Group(None) => "New Group".into(),
                _ => "Item".into(),
            })
            .unwrap_or_else(|| "Item".into());

        let keep_btn = AppButton::new(
            "guard-keep-editing-btn",
            "Keep Editing",
            ButtonVariant::Ghost,
        )
        .on_click(cx.listener(|app, _, _, cx| {
            app.inspector_coordinator.cancel_guard();
            cx.notify();
        }));

        let discard_btn = AppButton::new("guard-discard-btn", "Discard", ButtonVariant::Secondary)
            .on_click(cx.listener(|app, _, _, cx| {
                app.inspector_confirm_discard(cx);
            }));

        let save_btn = AppButton::new("guard-save-btn", "Save", ButtonVariant::Primary).on_click(
            cx.listener(|app, _, _, cx| {
                app.inspector_save(cx);
            }),
        );

        Some(crate::ui::components::unsaved_changes_guard_dialog(
            obj_name,
            keep_btn,
            discard_btn,
            save_btn,
        ))
    }

    /// Save whatever is currently dirty in inspector.
    pub(crate) fn inspector_save(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = self.inspector_coordinator.active() {
            match &active.target {
                InspectorTarget::Host(_) => self.save_host(cx),
                InspectorTarget::Group(_) => self.save_group_inspector(cx),
                _ => {}
            }
        }
    }

    /// Confirm discard of unsaved changes and navigate to requested target.
    pub(crate) fn inspector_confirm_discard(&mut self, cx: &mut Context<Self>) {
        if let Some(req) = self.inspector_coordinator.pending_guard().cloned() {
            self.inspector_coordinator.confirm_discard();
            match req {
                crate::ui::inspector::NavigationRequest::SwitchTarget(target, mode) => {
                    let vault_id = self.active_vault_id.clone();
                    let _ = self
                        .inspector_coordinator
                        .open(vault_id, target.clone(), mode, None);
                    match target {
                        InspectorTarget::Host(Some(id)) => {
                            self.selected_host_id = Some(id);
                            self.group_inspector = None;
                        }
                        InspectorTarget::Host(None) => {
                            self.selected_host_id = None;
                            self.edit_host(None, cx);
                        }
                        InspectorTarget::Group(Some(name)) => {
                            self.selected_host_id = None;
                            self.open_group_inspector(&name, cx);
                        }
                        InspectorTarget::Group(None) => {
                            self.selected_host_id = None;
                            self.open_group_inspector("", cx);
                        }
                        _ => {
                            self.selected_host_id = None;
                        }
                    }
                }
                crate::ui::inspector::NavigationRequest::CloseInspector => {
                    self.selected_host_id = None;
                    self.form = None;
                    self.group_inspector = None;
                }
                crate::ui::inspector::NavigationRequest::SwitchVault(new_vault_id) => {
                    self.selected_host_id = None;
                    self.form = None;
                    self.group_inspector = None;
                    self.active_vault_id = new_vault_id;
                    self.load_hosts(cx);
                }
                crate::ui::inspector::NavigationRequest::SwitchPage(_) => {
                    self.selected_host_id = None;
                    self.form = None;
                    self.group_inspector = None;
                }
            }
        }
        cx.notify();
    }
}
