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
        let Some(inspector) = &self.group_inspector else {
            return;
        };
        let new_name = inspector.name.read(cx).value().trim().to_string();
        if new_name.is_empty() {
            if let Some(ins) = &mut self.group_inspector {
                ins.error = Some("Group name cannot be empty".into());
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

        let Some(db_path) = self.database.clone() else {
            return;
        };

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
                    cx.notify();
                }
                return;
            }
        };

        let is_rename = orig_name != new_name && !orig_name.is_empty();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                if store
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
                if is_rename {
                    let _ = store.delete_group(&orig_name).await;
                }
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(err) = res {
                    if let Some(ins) = &mut app.group_inspector {
                        ins.error = Some(err);
                        cx.notify();
                    }
                } else {
                    app.group_inspector = None;
                    app.load_hosts(cx);
                    app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                    app.desktop.message = Some(format!("Group '{}' saved successfully", new_name));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn delete_group_inspector(&mut self, name: &str, cx: &mut Context<Self>) {
        let name_str = name.to_string();
        let target_name = name_str.clone();
        let Some(db_path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                store
                    .delete_group(&target_name)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })
        });
        cx.spawn(async move |app, cx| {
            let _ = task.await;
            let _ = app.update(cx, |app, cx| {
                app.group_inspector = None;
                app.group_context_menu = None;
                app.load_hosts(cx);
                app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                app.desktop.message = Some(format!("Group '{}' removed", name_str));
                cx.notify();
            });
        })
        .detach();
    }
}
