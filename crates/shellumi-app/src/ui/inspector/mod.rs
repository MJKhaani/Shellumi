#![allow(dead_code, unused_imports)]
//! Unified Inspector state machine, coordinator, and typed drafts.

pub mod coordinator;
pub mod drafts;
pub mod group;
pub mod host;
pub mod identity;
pub mod provider;
pub mod session;
pub mod snippet;
pub mod team;
pub mod tunnel;
pub mod vault;
pub mod workspace;

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
            self.snippet_form = None;
            self.tunnel_form = None;
            self.workspace_form = None;
            self.vault_form = None;
            self.provider_form = None;
            self.team_form = None;
        }
        cx.notify();
    }

    /// Return from Edit mode to View mode, or close inspector (guarded if dirty).
    pub(crate) fn inspector_back(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = self.inspector_coordinator.active().cloned() {
            match active.mode {
                InspectorMode::Edit => {
                    let vault_id = self.active_vault_id.clone();
                    if self.inspector_coordinator.open(
                        vault_id,
                        active.target.clone(),
                        InspectorMode::View,
                        None,
                    ) {
                        self.form = None;
                        self.group_inspector = None;
                        self.snippet_form = None;
                        self.tunnel_form = None;
                        self.workspace_form = None;
                        self.vault_form = None;
                        self.provider_form = None;
                        self.team_form = None;
                    }
                }
                InspectorMode::Create | InspectorMode::View => {
                    self.inspector_close(cx);
                }
            }
        } else {
            self.inspector_close(cx);
        }
        cx.notify();
    }

    /// Mark inspector coordinator draft dirty and refresh UI.
    pub(crate) fn inspector_mark_dirty(&mut self, cx: &mut Context<Self>) {
        self.inspector_coordinator.set_dirty(true);
        cx.notify();
    }

    /// Return active inspector width, or None when in FocusedContent mode (takes w_full).
    pub(crate) fn active_inspector_width(&self) -> Option<f32> {
        self.current_inspector_width
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

        let bounds = window.bounds();
        let total_w = bounds.size.width.to_f64() as f32;
        let sidebar_w = if self.sidebar_visible {
            crate::ui::layout::DEFAULT_SIDEBAR_WIDTH
        } else {
            0.0
        };
        let geom = crate::ui::layout::calculate_layout(total_w, sidebar_w, true, None);
        self.current_inspector_width =
            if geom.presentation == crate::ui::layout::InspectorPresentation::FocusedContent {
                None
            } else {
                Some(geom.inspector_width)
            };

        match target {
            InspectorTarget::Host(host_id) => self.render_host_inspector(host_id, mode, window, cx),
            InspectorTarget::Group(group_name) => self.render_group_inspector(group_name, mode, cx),
            InspectorTarget::Identity(identity_id) => {
                self.render_identity_inspector(identity_id, mode, cx)
            }
            InspectorTarget::Snippet(snippet_id) => {
                self.render_snippet_inspector(snippet_id, mode, cx)
            }
            InspectorTarget::Tunnel(tunnel_id) => self.render_tunnel_inspector(tunnel_id, mode, cx),
            InspectorTarget::Workspace(workspace_id) => {
                self.render_workspace_inspector(workspace_id, mode, cx)
            }
            InspectorTarget::Vault(vault_id) => self.render_vault_inspector(vault_id, mode, cx),
            InspectorTarget::Provider(provider_id) => {
                self.render_provider_inspector(provider_id, mode, cx)
            }
            InspectorTarget::TeamMember(member_id) => {
                self.render_team_inspector(member_id, mode, cx)
            }
            InspectorTarget::Session(session_id) => {
                self.render_session_inspector(session_id, window, cx)
            }
        }
    }

    /// Render delete confirmation dialog if requested.
    pub(crate) fn render_delete_confirm_dialog(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let target = self.delete_confirm.clone()?;
        let (obj_type, obj_name) = match &target {
            crate::DeleteTarget::Host(_, name) => ("host", name.clone()),
            crate::DeleteTarget::Group(name) => ("group", name.clone()),
            crate::DeleteTarget::Identity(_, name) => ("identity", name.clone()),
            crate::DeleteTarget::Snippet(_, name) => ("snippet", name.clone()),
            crate::DeleteTarget::Tunnel(_, name) => ("tunnel", name.clone()),
            crate::DeleteTarget::Workspace(_, name) => ("workspace", name.clone()),
            crate::DeleteTarget::Vault(_, name) => ("vault", name.clone()),
            crate::DeleteTarget::Provider(_, name) => ("cloud provider", name.clone()),
            crate::DeleteTarget::TeamMember(_, name) => ("team member", name.clone()),
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
                }
                crate::DeleteTarget::Group(name) => {
                    app.delete_group_inspector(name, cx);
                }
                crate::DeleteTarget::Identity(id, _) => {
                    app.delete_identity(*id, cx);
                }
                crate::DeleteTarget::Snippet(id, _) => {
                    app.delete_snippet_inspector(*id, cx);
                }
                crate::DeleteTarget::Tunnel(id, _) => {
                    app.delete_tunnel_inspector(*id, cx);
                }
                crate::DeleteTarget::Workspace(id, _) => {
                    app.delete_workspace_inspector(*id, cx);
                }
                crate::DeleteTarget::Vault(id, _) => {
                    app.delete_vault_inspector(id, cx);
                }
                crate::DeleteTarget::Provider(id, _) => {
                    app.delete_provider_inspector(id.clone(), cx);
                }
                crate::DeleteTarget::TeamMember(id, _) => {
                    app.delete_team_member_inspector(id, cx);
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
            .on_click(cx.listener(|app, _, window, cx| {
                app.inspector_confirm_discard(Some(window), cx);
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
                InspectorTarget::Identity(_) => self.save_identity_modal(cx),
                InspectorTarget::Snippet(_) => self.save_snippet_inspector(cx),
                InspectorTarget::Tunnel(_) => self.save_tunnel_inspector(cx),
                InspectorTarget::Workspace(_) => self.save_workspace_inspector(cx),
                InspectorTarget::Vault(_) => self.save_vault_inspector(cx),
                InspectorTarget::Provider(_) => self.save_provider_inspector(cx),
                InspectorTarget::TeamMember(_) => self.save_team_inspector(cx),
                _ => {}
            }
        }
    }

    /// Apply confirmed navigation request (target, close, vault, page).
    pub(crate) fn apply_navigation_request(
        &mut self,
        req: crate::ui::inspector::NavigationRequest,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
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
                self.snippet_form = None;
                self.tunnel_form = None;
                self.workspace_form = None;
                self.vault_form = None;
                self.provider_form = None;
                self.team_form = None;
            }
            crate::ui::inspector::NavigationRequest::SwitchVault(new_vault_id) => {
                let vname = self
                    .available_vaults
                    .iter()
                    .find(|v| v.id == new_vault_id)
                    .map(|v| v.name.clone())
                    .unwrap_or_else(|| new_vault_id.clone());
                self.switch_vault(new_vault_id, vname, window, cx);
            }
            crate::ui::inspector::NavigationRequest::SwitchPage(page_name) => {
                let leaked: &'static str = Box::leak(page_name.into_boxed_str());
                self.navigate_page(leaked, window, cx);
            }
            crate::ui::inspector::NavigationRequest::SwitchTab(idx) => {
                if let Some(win) = window {
                    self.select_tab(idx, win, cx);
                } else {
                    self.page = None;
                    self.active_tab_idx = idx;
                    cx.notify();
                }
            }
            crate::ui::inspector::NavigationRequest::CloseTab(tab_id) => {
                if let Some(win) = window {
                    self.close_tab(tab_id, win, cx);
                }
            }
            crate::ui::inspector::NavigationRequest::CloseWindow => {
                self.inspector_coordinator.close();
                cx.notify();
            }
        }
    }

    /// Confirm discard of unsaved changes and navigate to requested target.
    pub(crate) fn inspector_confirm_discard(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some(req) = self.inspector_coordinator.pending_guard().cloned() {
            self.inspector_coordinator.confirm_discard();
            self.apply_navigation_request(req, window, cx);
        }
        cx.notify();
    }
}
