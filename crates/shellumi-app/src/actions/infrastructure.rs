//! Infrastructure actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_infra_modal(&mut self, cx: &mut Context<Self>) {
        let new_provider_name =
            cx.new(|cx| TextInput::new(cx, "", "Provider name (e.g. AWS Production)"));
        let new_provider_region =
            cx.new(|cx| TextInput::new(cx, "", "Region / Resource Group (e.g. us-east-1)"));
        let new_provider_path =
            cx.new(|cx| TextInput::new(cx, "", "Inventory file path (optional)"));
        let ansible_file_path =
            cx.new(|cx| TextInput::new(cx, "", "Path to Ansible inventory (INI/YAML)"));

        self.infra_modal = Some(InfraModalState {
            active_tab: InfraModalTab::Providers,
            providers: Vec::new(),
            selected_provider_id: None,
            discovered: Vec::new(),
            new_provider_name,
            new_provider_kind: CloudProviderKind::Aws,
            new_provider_region,
            new_provider_path,
            ansible_file_path,
            ansible_is_yaml: false,
            error: None,
            status_message: None,
            is_loading: true,
        });
        cx.notify();
        self.load_infra_providers(cx);
    }

    pub(crate) fn dismiss_infra_modal(&mut self, cx: &mut Context<Self>) {
        self.infra_modal = None;
        cx.notify();
    }

    fn load_infra_providers(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            if let Some(m) = &mut self.infra_modal {
                m.is_loading = false;
                m.error = Some(
                    "No database path configured. Infrastructure integrations require a database."
                        .into(),
                );
            }
            cx.notify();
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let providers = store
                    .list_cloud_providers()
                    .await
                    .map_err(|e| e.to_string())?;
                let discovered = store
                    .list_discovered_hosts(None)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((providers, discovered))
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.infra_modal {
                    modal.is_loading = false;
                    match result {
                        Ok((providers, discovered)) => {
                            let first_id = providers.first().map(|p| p.id.clone());
                            modal.providers = providers;
                            if modal.selected_provider_id.is_none() {
                                modal.selected_provider_id = first_id;
                            }
                            modal.discovered = discovered;
                        }
                        Err(e) => {
                            modal.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn create_infra_provider(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        let (name, kind, region, inv_path) = {
            let Some(m) = &self.infra_modal else {
                return;
            };
            (
                m.new_provider_name.read(cx).value().trim().to_string(),
                m.new_provider_kind,
                m.new_provider_region.read(cx).value().trim().to_string(),
                m.new_provider_path.read(cx).value().trim().to_string(),
            )
        };

        if name.is_empty() {
            if let Some(m) = &mut self.infra_modal {
                m.error = Some("Provider name cannot be empty.".into());
            }
            cx.notify();
            return;
        }

        let mut cfg = CloudProviderConfig::new(kind, &name);
        if !region.is_empty() {
            cfg.region = Some(region);
        }
        if !inv_path.is_empty() {
            cfg.inventory_path = Some(inv_path);
        }

        let prov_id = format!(
            "prov-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let cfg_json = match serde_json::to_string(&cfg) {
            Ok(j) => j,
            Err(e) => {
                if let Some(m) = &mut self.infra_modal {
                    m.error = Some(format!("Failed to serialize config: {e}"));
                }
                cx.notify();
                return;
            }
        };

        let record = CloudProviderRecord {
            id: prov_id,
            name: name.clone(),
            provider_kind: kind.as_str().to_string(),
            config_json: cfg_json,
            sync_interval_mins: 60,
            last_synced_at: None,
            auto_sync: false,
            created_at: now,
            updated_at: now,
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
            m.status_message = Some(format!("Created provider '{name}'."));
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .create_cloud_provider(&record)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(e) = result {
                    if let Some(m) = &mut app.infra_modal {
                        m.error = Some(e);
                        m.is_loading = false;
                    }
                    cx.notify();
                } else {
                    app.load_infra_providers(cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn delete_infra_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let pid = provider_id.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .delete_cloud_provider(&pid)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(e) = result {
                    if let Some(m) = &mut app.infra_modal {
                        m.error = Some(e);
                        m.is_loading = false;
                    }
                    cx.notify();
                } else {
                    if let Some(m) = &mut app.infra_modal {
                        if m.selected_provider_id.as_deref() == Some(&provider_id) {
                            m.selected_provider_id = None;
                        }
                    }
                    app.load_infra_providers(cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn sync_infra_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
            m.status_message = Some("Synchronizing provider inventory...".into());
        }
        cx.notify();

        let pid = provider_id.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let prov = store
                    .get_cloud_provider(&pid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "Provider not found".to_string())?;

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // If Ansible file configured, parse directly
                if let Ok(cfg) = serde_json::from_str::<CloudProviderConfig>(&prov.config_json) {
                    if let Some(ref fpath) = cfg.inventory_path {
                        if let Ok(content) = std::fs::read_to_string(fpath) {
                            let is_yaml = cfg.inventory_format.as_deref() == Some("yaml")
                                || fpath.ends_with(".yml")
                                || fpath.ends_with(".yaml");
                            let hosts_res = if is_yaml {
                                AnsibleInventoryEngine::parse_yaml_inventory(&content)
                            } else {
                                AnsibleInventoryEngine::parse_ini_inventory(&content)
                            };

                            if let Ok(hosts) = hosts_res {
                                let records: Vec<DiscoveredHostRecord> = hosts
                                    .into_iter()
                                    .map(|h| DiscoveredHostRecord {
                                        id: format!(
                                            "dh-{}",
                                            std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_nanos()
                                        ),
                                        provider_id: pid.clone(),
                                        external_id: h.id,
                                        name: h.name,
                                        hostname: h.hostname,
                                        port: h.port as i64,
                                        username: h.username,
                                        state: h.state,
                                        region: h.region,
                                        group_path: h.group_path,
                                        tags_json: serde_json::to_string(&h.tags)
                                            .unwrap_or_default(),
                                        metadata_json: serde_json::to_string(&h.extra_metadata)
                                            .unwrap_or_default(),
                                        imported_host_id: None,
                                        last_seen_at: now,
                                    })
                                    .collect();
                                store
                                    .save_discovered_hosts(&pid, &records)
                                    .await
                                    .map_err(|e| e.to_string())?;
                            }
                        }
                    }
                }

                store
                    .update_cloud_provider_sync_time(&pid, now)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(e) = result {
                    if let Some(m) = &mut app.infra_modal {
                        m.error = Some(e);
                        m.is_loading = false;
                    }
                    cx.notify();
                } else {
                    if let Some(m) = &mut app.infra_modal {
                        m.status_message = Some("Synchronization completed successfully.".into());
                    }
                    app.load_infra_providers(cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn import_discovered_host_into_inventory(
        &mut self,
        host_idx: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.database.clone() else {
            return;
        };

        let host_record = {
            let Some(m) = &self.infra_modal else {
                return;
            };
            let Some(h) = m.discovered.get(host_idx) else {
                return;
            };
            h.clone()
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
            m.status_message = Some(format!("Importing host '{}'...", host_record.name));
        }
        cx.notify();

        let h_rec = host_record.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let u = h_rec.username.as_deref().unwrap_or("root");
                let dest = SshDestination::new(&h_rec.hostname, u, h_rec.port as u16)
                    .map_err(|e| e.to_string())?;

                let tags_map: std::collections::BTreeMap<String, String> =
                    serde_json::from_str(&h_rec.tags_json).unwrap_or_default();
                let tags: Vec<String> = tags_map
                    .into_iter()
                    .map(|(k, v)| if v.is_empty() { k } else { format!("{k}:{v}") })
                    .collect();

                let new_id = store
                    .add(
                        &h_rec.name,
                        &dest,
                        h_rec.group_path.as_deref(),
                        &tags,
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .mark_discovered_host_imported(&h_rec.id, new_id)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(new_id)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.infra_modal {
                    m.is_loading = false;
                    match result {
                        Ok(new_id) => {
                            m.status_message = Some(format!(
                                "Imported host '{}' (ID: {new_id}).",
                                host_record.name
                            ));
                        }
                        Err(e) => {
                            m.error = Some(e);
                        }
                    }
                }
                app.load_hosts(cx);
                app.load_infra_providers(cx);
            });
        })
        .detach();
    }

    pub(crate) fn export_ansible_inventory_file(&mut self, is_yaml: bool, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let hosts = store.list().await.map_err(|e| e.to_string())?;
                let export_items: Vec<HostExportItem> = hosts
                    .into_iter()
                    .map(|h| HostExportItem {
                        name: h.name,
                        hostname: h.destination.host().to_string(),
                        port: h.destination.port().get(),
                        username: Some(h.destination.username().to_string()),
                        group_name: h.group_name,
                        tags: h.tags,
                    })
                    .collect();

                let rendered = if is_yaml {
                    AnsibleInventoryEngine::export_to_yaml(&export_items)
                } else {
                    AnsibleInventoryEngine::export_to_ini(&export_items)
                };
                Ok((export_items.len(), rendered))
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.infra_modal {
                    m.is_loading = false;
                    match result {
                        Ok((count, _rendered)) => {
                            m.status_message = Some(format!(
                                "Exported {count} hosts to Ansible inventory format."
                            ));
                        }
                        Err(e) => {
                            m.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    // =========================================================================
    // System Diagnostics & Health Check Engine
    // =========================================================================
}
