//! Groups actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_group_inspector(&mut self, group_name: &str, cx: &mut Context<Self>) {
        let existing = self.desktop.groups.iter().find(|g| g.name == group_name);
        let default_user = existing
            .and_then(|g| g.default_username.clone())
            .unwrap_or_default();
        let default_port = existing
            .and_then(|g| g.default_port)
            .map(|p| p.get().to_string())
            .unwrap_or_else(|| "22".into());
        let identity_id = existing.and_then(|g| g.identity_id);

        let name = cx.new(|cx| TextInput::new(cx, group_name, "Group name"));
        let parent_group = cx.new(|cx| TextInput::new(cx, "", "Parent Group"));
        let default_username = cx.new(|cx| TextInput::new(cx, &default_user, "Default username"));
        let default_port = cx.new(|cx| TextInput::new(cx, &default_port, "22"));

        cx.observe(&name, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&parent_group, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&default_username, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&default_port, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();

        self.group_inspector = Some(GroupInspectorState {
            original_name: group_name.to_string(),
            name,
            parent_group,
            default_username,
            default_port,
            identity_id,
            cloud_sync: true,
            error: None,
        });
        self.group_context_menu = None;
        cx.notify();
    }

    pub(crate) fn save_group_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };

        let Some(inspector) = &self.group_inspector else {
            return;
        };
        let new_name = inspector.name.read(cx).value().trim().to_string();
        let active_vid = self.active_vault_id.clone();
        if new_name.is_empty() {
            if let Some(ins) = &mut self.group_inspector {
                ins.error = Some("Group name cannot be empty".into());
                self.inspector_coordinator.fail_save(
                    &active_vid,
                    gen,
                    "Group name cannot be empty".into(),
                );
                cx.notify();
            }
            return;
        }
        let orig_name = inspector.original_name.clone();
        let default_user = inspector
            .default_username
            .read(cx)
            .value()
            .trim()
            .to_string();
        let default_port_str = inspector.default_port.read(cx).value().trim().to_string();
        let default_port = default_port_str.parse::<u16>().ok();
        let identity_id = inspector.identity_id;

        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let vault_key = self.active_vault_key();

        let group_res = shellumi_core::HostGroup::new(
            &new_name,
            if default_user.is_empty() {
                None
            } else {
                Some(&default_user)
            },
            default_port,
            &[],
        );

        let group = match group_res {
            Ok(g) => g.with_identity_id(identity_id),
            Err(e) => {
                if let Some(ins) = &mut self.group_inspector {
                    ins.error = Some(e.to_string());
                    self.inspector_coordinator
                        .fail_save(&active_vid, gen, e.to_string());
                    cx.notify();
                }
                return;
            }
        };

        let is_rename = orig_name != new_name && !orig_name.is_empty();
        let target_group_name = new_name.clone();
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
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
                if is_rename {
                    store
                        .rename_group(&orig_name, &group)
                        .await
                        .map_err(|e| e.to_string())?;
                } else if store
                    .get_group(&group.name)
                    .await
                    .map_err(|e| e.to_string())?
                    .is_some()
                {
                    store
                        .update_group(&group)
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    store.add_group(&group).await.map_err(|e| e.to_string())?;
                }
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| match res {
                Err(err) => {
                    if let Some(ins) = &mut app.group_inspector {
                        ins.error = Some(err.clone());
                        cx.notify();
                    }
                    app.inspector_coordinator
                        .fail_save(&active_vault_id, gen, err);
                }
                Ok(()) => {
                    let target = crate::ui::inspector::coordinator::InspectorTarget::Group(Some(
                        target_group_name.clone(),
                    ));
                    let ok = app
                        .inspector_coordinator
                        .complete_save(&active_vault_id, gen, target);
                    if !ok {
                        tracing::warn!(
                            originating_vault = %active_vault_id,
                            current_vault = %app.active_vault_id,
                            gen = gen,
                            "Discarded stale save completion for group"
                        );
                        return;
                    }

                    app.group_inspector = None;
                    app.load_hosts(cx);
                    app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                    app.desktop.message =
                        Some(format!("Group '{}' saved successfully", target_group_name));

                    if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                        app.apply_navigation_request(req, None, cx);
                    } else {
                        app.open_group_inspector_mode(
                            &target_group_name,
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn delete_group_inspector(&mut self, name: &str, cx: &mut Context<Self>) {
        let name_str = name.to_string();
        let target_name = name_str.clone();
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
                let deleted = store
                    .delete_group(&target_name)
                    .await
                    .map_err(|e| e.to_string())?;
                if !deleted {
                    return Err(format!("Group '{}' not found", target_name));
                }
                Ok::<(), String>(())
            })
        });
        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(()) => {
                        app.group_inspector = None;
                        app.group_context_menu = None;
                        app.load_hosts(cx);
                        app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                        app.desktop.message = Some(format!("Group '{}' removed", name_str));
                        app.inspector_coordinator.close();
                    }
                    Err(err) => {
                        if let Some(ins) = &mut app.group_inspector {
                            ins.error = Some(format!("Failed to delete group: {err}"));
                        }
                        app.desktop.message =
                            Some(format!("Failed to delete group '{}': {}", name_str, err));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
