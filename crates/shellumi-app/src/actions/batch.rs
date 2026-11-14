//! Batch actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_batch_modal(&mut self, snippet_id: i64, cx: &mut Context<Self>) {
        let Some(snippet) = self.snippets.iter().find(|s| s.id == snippet_id).cloned() else {
            return;
        };

        let variables = snippet
            .variables
            .iter()
            .map(|v| {
                let default_val = v.default_value.clone();
                let initial_val = default_val.clone().unwrap_or_default();
                let placeholder = default_val.as_deref().unwrap_or("Value required");
                let input = cx.new(|cx| TextInput::new(cx, &initial_val, placeholder));
                SnippetModalVariable {
                    name: v.name.clone(),
                    is_secret: v.is_secret,
                    default_value: default_val,
                    input,
                }
            })
            .collect();

        let hosts = match &self.hosts {
            HostState::Loaded(saved_hosts) => saved_hosts
                .iter()
                .map(|h| BatchHostItem {
                    id: h.id,
                    name: h.name.clone(),
                    destination: h.destination.clone(),
                    group: h.group_name.clone(),
                    tags: h.tags.clone(),
                    selected: true,
                    status: BatchHostStatus::Queued,
                    exit_code: None,
                    output: String::new(),
                    duration_ms: None,
                })
                .collect(),
            _ => Vec::new(),
        };

        let search_input = cx
            .new(|cx| TextInput::new(cx, "", "Filter target hosts by name, tag, or destination…"));

        self.batch_modal = Some(BatchModalState {
            snippet_id: snippet.id,
            snippet_name: snippet.name,
            command_template: snippet.command,
            policy: BatchExecutionPolicyChoice::Parallel,
            concurrency: 5,
            variables,
            hosts,
            search_input,
            dry_run: false,
            is_running: false,
            summary: None,
            error: None,
            expanded_host_output: None,
        });
        cx.notify();
    }

    pub(crate) fn dismiss_batch_modal(&mut self, cx: &mut Context<Self>) {
        self.batch_modal = None;
        cx.notify();
    }

    pub(crate) fn toggle_batch_host_selection(&mut self, host_id: i64, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            if let Some(item) = modal.hosts.iter_mut().find(|h| h.id == host_id) {
                item.selected = !item.selected;
                cx.notify();
            }
        }
    }

    pub(crate) fn batch_select_all_hosts(&mut self, selected: bool, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            for h in &mut modal.hosts {
                h.selected = selected;
            }
            cx.notify();
        }
    }

    pub(crate) fn toggle_batch_dry_run(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            modal.dry_run = !modal.dry_run;
            cx.notify();
        }
    }

    pub(crate) fn set_batch_policy(
        &mut self,
        policy: BatchExecutionPolicyChoice,
        cx: &mut Context<Self>,
    ) {
        if let Some(modal) = &mut self.batch_modal {
            modal.policy = policy;
            cx.notify();
        }
    }

    pub(crate) fn toggle_batch_host_output(&mut self, host_id: i64, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            if modal.expanded_host_output == Some(host_id) {
                modal.expanded_host_output = None;
            } else {
                modal.expanded_host_output = Some(host_id);
            }
            cx.notify();
        }
    }

    pub(crate) fn execute_batch(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &mut self.batch_modal else {
            return;
        };
        if modal.is_running {
            return;
        }

        let mut global_variables = std::collections::HashMap::new();
        for var in &modal.variables {
            let val = var.input.read(cx).value().trim().to_string();
            if !val.is_empty() {
                global_variables.insert(var.name.clone(), val);
            }
        }

        let selected_targets: Vec<BatchHostTarget> = modal
            .hosts
            .iter()
            .filter(|h| h.selected)
            .map(|h| {
                let mut target = BatchHostTarget::new(h.id, h.name.clone(), h.destination.clone())
                    .with_tags(h.tags.clone());
                if let Some(grp) = &h.group {
                    target = target.with_group(grp.clone());
                }
                target
            })
            .collect();

        if selected_targets.is_empty() {
            modal.error = Some("No hosts selected for batch execution.".into());
            cx.notify();
            return;
        }

        let plan_policy = match modal.policy {
            BatchExecutionPolicyChoice::Sequential => BatchPolicy::Sequential,
            BatchExecutionPolicyChoice::Parallel => BatchPolicy::Parallel {
                max_concurrency: modal.concurrency,
            },
        };

        let plan = BatchExecutionPlan::new(
            modal.snippet_id,
            &modal.snippet_name,
            &modal.command_template,
            selected_targets,
        )
        .with_policy(plan_policy)
        .with_global_variables(global_variables);

        if modal.dry_run {
            let previews = plan.preview_all();
            for (target, rendered_res) in previews {
                if let Some(item) = modal.hosts.iter_mut().find(|h| h.id == target.id) {
                    match rendered_res {
                        Ok(cmd) => {
                            item.status = BatchHostStatus::Succeeded;
                            item.output = format!("[DRY RUN PREVIEW]\n$ {cmd}");
                            item.exit_code = Some(0);
                            item.duration_ms = Some(0);
                        }
                        Err(err) => {
                            item.status = BatchHostStatus::Failed;
                            item.output = format!("[DRY RUN ERROR] {err}");
                            item.exit_code = Some(1);
                            item.duration_ms = Some(0);
                        }
                    }
                }
            }
            let total = modal.hosts.iter().filter(|h| h.selected).count();
            let succeeded = modal
                .hosts
                .iter()
                .filter(|h| h.selected && h.status == BatchHostStatus::Succeeded)
                .count();
            let failed = modal
                .hosts
                .iter()
                .filter(|h| h.selected && h.status == BatchHostStatus::Failed)
                .count();
            modal.summary = Some(BatchExecutionSummary {
                total,
                succeeded,
                failed,
                cancelled: 0,
                in_progress: 0,
                queued: 0,
                is_complete: true,
                total_duration_ms: 0,
            });
            modal.is_running = false;
            modal.error = None;
            cx.notify();
            return;
        }

        modal.is_running = true;
        modal.error = None;
        for h in &mut modal.hosts {
            if h.selected {
                h.status = BatchHostStatus::InProgress;
                h.output = "Executing snippet...".into();
            }
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let mut runner = BatchRunner::new(plan);
            let summary = runner.execute_sync(|target, cmd| {
                let mut proc = std::process::Command::new("ssh");
                proc.args(target.destination.openssh_arguments());
                proc.arg(cmd);
                match proc.output() {
                    Ok(output) => {
                        let code = output.status.code().unwrap_or(-1);
                        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                        Ok((code, stdout, stderr))
                    }
                    Err(err) => Err(err.to_string()),
                }
            });
            (runner.results().to_vec(), summary)
        });

        cx.spawn(async move |app, cx| {
            let (results, summary) = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.batch_modal {
                    modal.is_running = false;
                    modal.summary = Some(summary);
                    for res in results {
                        if let Some(h) = modal.hosts.iter_mut().find(|h| h.id == res.host_id) {
                            h.status = res.status;
                            h.exit_code = res.exit_code;
                            h.duration_ms = Some(res.duration_ms);
                            let mut full_output = String::new();
                            if !res.command_rendered.is_empty() {
                                full_output.push_str(&format!("$ {}\n", res.command_rendered));
                            }
                            if !res.stdout.is_empty() {
                                full_output.push_str(&res.stdout);
                            }
                            if !res.stderr.is_empty() {
                                if !full_output.is_empty() && !full_output.ends_with('\n') {
                                    full_output.push('\n');
                                }
                                full_output.push_str(&format!("[stderr]\n{}", res.stderr));
                            }
                            if let Some(err) = res.error {
                                if !full_output.is_empty() && !full_output.ends_with('\n') {
                                    full_output.push('\n');
                                }
                                full_output.push_str(&format!("[error: {}]", err));
                            }
                            h.output = full_output;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
