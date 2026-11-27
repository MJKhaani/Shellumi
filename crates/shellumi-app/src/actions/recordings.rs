//! Recordings actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_session_replay_modal(&mut self, cx: &mut Context<Self>) {
        self.session_replay_modal = Some(SessionReplayModalState {
            active_tab: SessionReplayTab::RecordingsList,
            recordings: Vec::new(),
            selected_recording_id: None,
            current_recording: None,
            playback_engine: None,
            replay_terminal: None,
            replay_generation: 0,
            screen_output_buffer: String::new(),
            status_notification: None,
            search_query: String::new(),
        });
        self.load_session_recordings(cx);
        cx.notify();
    }

    pub(crate) fn dismiss_session_replay_modal(&mut self, cx: &mut Context<Self>) {
        self.session_replay_modal = None;
        cx.notify();
    }

    pub(crate) fn set_session_replay_tab(&mut self, tab: SessionReplayTab, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            m.active_tab = tab;
        }
        cx.notify();
    }

    fn recording_recovery_journal(&self) -> Option<shellumi_core::RecordingRecoveryJournal> {
        let db_path = self.database.as_ref()?;
        let parent = std::path::Path::new(db_path).parent()?;
        Some(shellumi_core::RecordingRecoveryJournal::new(
            parent.join("recording_recovery"),
        ))
    }

    pub(crate) fn toggle_pane_recording(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        let mut is_currently_recording = false;
        let mut found = false;
        for tab in &self.tabs {
            for pane in &tab.panes {
                if pane.id == pane_id {
                    found = true;
                    is_currently_recording = pane.terminal.is_recording();
                    break;
                }
            }
        }
        if !found {
            return;
        }
        if is_currently_recording {
            self.finalize_pane_recording(pane_id, "Stopped by user", cx);
        } else {
            self.start_pane_recording(pane_id, cx);
        }
    }

    pub(crate) fn start_pane_recording(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        let active_vault_id = self.active_vault_id.clone();
        if self.vault_key_for(&active_vault_id).is_none() && self.database.is_some() {
            for tab in &mut self.tabs {
                if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
                    pane.message =
                        Some("Cannot start recording while vault is locked.".to_string());
                }
            }
            cx.notify();
            return;
        }
        for tab in self.tabs.iter_mut() {
            for pane in tab.panes.iter_mut() {
                if pane.id == pane_id {
                    if pane.session.is_none() {
                        pane.message = Some("Cannot record an inactive session.".to_string());
                        cx.notify();
                        return;
                    }
                    let rec_id = uuid::Uuid::new_v4().to_string();
                    let started_at =
                        shellumi_core::format_rfc3339_utc(std::time::SystemTime::now());
                    pane.recording_id = Some(rec_id);
                    pane.recording_started_at = Some(started_at);
                    pane.recording_state = shellumi_core::RecordingLifecycleState::Recording;
                    pane.terminal
                        .start_recording(Some(format!("Session: {}", pane.name)));
                    pane.message = Some("⏺ Session recording started.".to_string());
                    cx.notify();
                    return;
                }
            }
        }
    }

    pub(crate) fn finalize_all_active_recordings(&mut self, reason: &str, cx: &mut Context<Self>) {
        let active_ids: Vec<u64> = self
            .tabs
            .iter()
            .flat_map(|t| t.panes.iter())
            .filter(|p| p.terminal.is_recording())
            .map(|p| p.id)
            .collect();
        for pane_id in active_ids {
            self.finalize_pane_recording(pane_id, reason, cx);
        }
    }

    pub(crate) fn finalize_pane_recording(
        &mut self,
        pane_id: u64,
        reason: &str,
        cx: &mut Context<Self>,
    ) {
        let mut extracted = None;
        for tab in self.tabs.iter_mut() {
            for pane in tab.panes.iter_mut() {
                if pane.id == pane_id && pane.terminal.is_recording() {
                    if let Some(rec) = pane.terminal.stop_recording() {
                        pane.recording_state = shellumi_core::RecordingLifecycleState::Finalizing;
                        let rec_id = pane
                            .recording_id
                            .take()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                        let started_at = pane.recording_started_at.take().unwrap_or_else(|| {
                            shellumi_core::format_rfc3339_utc(std::time::SystemTime::now())
                        });
                        extracted = Some((
                            rec_id,
                            pane.session_uuid.clone(),
                            pane.host_id,
                            pane.name.clone(),
                            pane.vault_id.clone(),
                            format!("{:?}", pane.kind),
                            started_at,
                            rec,
                        ));
                    }
                    break;
                }
            }
        }
        let Some((rec_id, sess_id, host_id, name, pane_vault_id, protocol, started_at, rec)) =
            extracted
        else {
            return;
        };

        let duration_secs = rec.duration_secs();
        let event_count = rec.event_count();
        let truncated = rec.truncated;
        let now_str = shellumi_core::format_rfc3339_utc(std::time::SystemTime::now());

        let raw_cast = match rec.to_asciicast_v2() {
            Ok(cast) => cast,
            Err(err) => {
                tracing::error!(recording_id = %rec_id, error = %err, "Failed to serialize recording");
                self.set_pane_recording_error(pane_id, &rec_id, &err, cx);
                return;
            }
        };

        // Fail-closed redaction: NEVER fall back to unredacted raw_cast.
        let clean_cast = match SessionRedactor::redact_asciicast_v2(&raw_cast) {
            Ok(redacted) => redacted,
            Err(err) => {
                tracing::error!(recording_id = %rec_id, error = %err, "Recording redaction failed; refusing fail-open");
                self.set_pane_recording_error(
                    pane_id,
                    &rec_id,
                    &format!("Redaction failed: {err}"),
                    cx,
                );
                return;
            }
        };

        let db_rec = SessionRecordingRecord {
            id: rec_id.clone(),
            session_id: sess_id.clone(),
            host_id,
            title: format!("Recorded Session: {name}"),
            duration_secs,
            event_count,
            byte_size: clean_cast.len(),
            cast_data: clean_cast.clone(),
            recorded_at: started_at.clone(),
            created_at: now_str,
        };

        // Persist encrypted crash-recovery checkpoint before attempting DB commit.
        let vault_key = self.vault_key_for(&pane_vault_id);
        let journal = self.recording_recovery_journal();
        if let (Some(ref j), Some(ref key)) = (&journal, &vault_key) {
            if let Ok(vid) = shellumi_core::VaultId::new(&pane_vault_id) {
                let cp = shellumi_core::RecordingCheckpoint {
                    recording_id: rec_id.clone(),
                    vault_id: pane_vault_id.clone(),
                    session_id: sess_id,
                    pane_id,
                    host_id,
                    protocol,
                    title: db_rec.title.clone(),
                    started_at,
                    duration_secs,
                    event_count,
                    byte_size: db_rec.byte_size,
                    cast_data: clean_cast,
                    truncated,
                };
                if let Err(err) = j.save_checkpoint(&cp, &vid, key) {
                    tracing::warn!(recording_id = %rec_id, error = %err, "Failed to write encrypted recovery checkpoint");
                }
            }
        }

        let Some(path) = self
            .vault_db_path(&pane_vault_id)
            .or_else(|| self.active_vault_db_path())
        else {
            self.failed_recordings.push(FailedRecoverableRecording {
                record: db_rec,
                vault_id: pane_vault_id,
                error: "No active vault database path".to_string(),
            });
            self.set_pane_recording_error(pane_id, &rec_id, "No active vault database path", cx);
            return;
        };

        let db_rec_clone = db_rec.clone();
        let expected_gen = self.vault_load_generation;
        let expected_vault = pane_vault_id.clone();
        let reason_owned = reason.to_string();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .save_session_recording(&db_rec_clone)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(()) => {
                        if let Some(j) = app.recording_recovery_journal() {
                            let _ = j.remove_checkpoint(&rec_id);
                        }
                        app.failed_recordings.retain(|f| f.record.id != rec_id);
                        if app.active_vault_id == expected_vault
                            && app.vault_load_generation == expected_gen
                        {
                            for tab in &mut app.tabs {
                                if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
                                    pane.recording_state =
                                        shellumi_core::RecordingLifecycleState::Saved {
                                            recording_id: rec_id.clone(),
                                        };
                                    let trunc_note = if truncated { " [truncated]" } else { "" };
                                    pane.message = Some(format!(
                                        "⏹ Recording saved ({}, {event_count} events{trunc_note}) — {reason_owned}",
                                        format_duration_hms(duration_secs)
                                    ));
                                }
                            }
                            if app.session_replay_modal.is_some() {
                                app.load_session_recordings(cx);
                            }
                        }
                    }
                    Err(err) => {
                        tracing::error!(recording_id = %rec_id, error = %err, "Recording DB commit failed; retaining recoverable state");
                        app.failed_recordings.push(FailedRecoverableRecording {
                            record: db_rec,
                            vault_id: expected_vault.clone(),
                            error: err.clone(),
                        });
                        if app.active_vault_id == expected_vault
                            && app.vault_load_generation == expected_gen
                        {
                            app.set_pane_recording_error(pane_id, &rec_id, &err, cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn set_pane_recording_error(
        &mut self,
        pane_id: u64,
        rec_id: &str,
        err: &str,
        cx: &mut Context<Self>,
    ) {
        for tab in &mut self.tabs {
            if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
                pane.recording_state = shellumi_core::RecordingLifecycleState::FailedRecoverable {
                    recording_id: rec_id.to_string(),
                    error: err.to_string(),
                };
                pane.message = Some(format!("⚠ Recording save failed (recoverable): {err}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn retry_failed_recording(&mut self, rec_id: String, cx: &mut Context<Self>) {
        let Some(pos) = self
            .failed_recordings
            .iter()
            .position(|f| f.record.id == rec_id)
        else {
            return;
        };
        let failed = self.failed_recordings[pos].clone();
        if failed.vault_id != self.active_vault_id {
            return;
        }
        let Some(path) = self.vault_db_path(&failed.vault_id) else {
            return;
        };
        let vault_key = self.vault_key_for(&failed.vault_id);
        let expected_gen = self.vault_load_generation;
        let expected_vault = failed.vault_id.clone();
        let rec_clone = failed.record.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .save_session_recording(&rec_clone)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })
        });
        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if res.is_ok() {
                    if let Some(j) = app.recording_recovery_journal() {
                        let _ = j.remove_checkpoint(&rec_id);
                    }
                    app.failed_recordings.retain(|f| f.record.id != rec_id);
                    if app.active_vault_id == expected_vault
                        && app.vault_load_generation == expected_gen
                    {
                        if let Some(ref mut m) = app.session_replay_modal {
                            m.status_notification =
                                Some(format!("Recovered and saved recording '{rec_id}'"));
                            app.load_session_recordings(cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn discard_failed_recording(&mut self, rec_id: String, cx: &mut Context<Self>) {
        self.failed_recordings.retain(|f| f.record.id != rec_id);
        if let Some(j) = self.recording_recovery_journal() {
            let _ = j.remove_checkpoint(&rec_id);
        }
        cx.notify();
    }

    fn load_session_recordings(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.active_vault_db_path() {
            let vault_key = self.active_vault_key();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open_with_key(&path, vault_key.as_ref())
                        .await
                        .map_err(|e| e.to_string())?;
                    store
                        .list_session_recordings(None)
                        .await
                        .map_err(|e| e.to_string())
                })
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Ok(recordings) = result {
                        if let Some(ref mut m) = app.session_replay_modal {
                            m.recordings = recordings;
                        }
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    pub(crate) fn select_and_load_recording_for_playback(
        &mut self,
        rec_id: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self.active_vault_db_path() {
            let id_clone = rec_id.clone();
            let vault_key = self.active_vault_key();
            let expected_vault = self.active_vault_id.clone();
            let expected_gen = self.vault_load_generation;
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open_with_key(&path, vault_key.as_ref())
                        .await
                        .map_err(|e| e.to_string())?;
                    store
                        .get_session_recording(&id_clone)
                        .await
                        .map_err(|e| e.to_string())
                })
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if app.active_vault_id != expected_vault
                        || app.vault_load_generation != expected_gen
                    {
                        return;
                    }
                    if let Ok(Some(record)) = result {
                        if let Some(ref mut m) = app.session_replay_modal {
                            if let Ok(mut engine) =
                                CastPlaybackEngine::from_asciicast_v2(&record.cast_data)
                            {
                                m.replay_generation = m.replay_generation.wrapping_add(1);
                                m.selected_recording_id = Some(rec_id.clone());
                                let initial_frames = engine.seek(0.0);
                                let term =
                                    rebuild_replay_terminal(&record.cast_data, &initial_frames);
                                m.screen_output_buffer = term.screen_to_string();
                                m.replay_terminal = Some(term);
                                m.current_recording = Some(record);
                                m.active_tab = SessionReplayTab::ReplayPlayer;
                                m.playback_engine = Some(engine);
                            }
                        }
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    pub(crate) fn toggle_modal_playback(&mut self, cx: &mut Context<Self>) {
        let mut should_schedule = false;
        if let Some(ref mut m) = self.session_replay_modal {
            m.replay_generation = m.replay_generation.wrapping_add(1);
            if let Some(ref mut engine) = m.playback_engine {
                engine.toggle_play_pause();
                if engine.status() == PlaybackStatus::Playing {
                    if engine.current_time_secs() == 0.0 {
                        if let Some(ref rec) = m.current_recording {
                            let term = rebuild_replay_terminal(&rec.cast_data, &[]);
                            m.screen_output_buffer = term.screen_to_string();
                            m.replay_terminal = Some(term);
                        }
                    }
                    should_schedule = true;
                }
            }
        }
        cx.notify();
        if should_schedule {
            self.start_replay_scheduler(cx);
        }
    }

    fn start_replay_scheduler(&mut self, cx: &mut Context<Self>) {
        let Some(ref m) = self.session_replay_modal else {
            return;
        };
        let gen = m.replay_generation;
        let expected_vault = self.active_vault_id.clone();
        let expected_vault_gen = self.vault_load_generation;
        cx.spawn(async move |app, cx| {
            let mut last_tick = std::time::Instant::now();
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(60))
                    .await;
                let now = std::time::Instant::now();
                let dt = now.duration_since(last_tick).as_secs_f64();
                last_tick = now;

                let mut keep_going = false;
                let updated = app.update(cx, |app, cx| {
                    if app.active_vault_id != expected_vault
                        || app.vault_load_generation != expected_vault_gen
                    {
                        return;
                    }
                    let Some(ref mut m) = app.session_replay_modal else {
                        return;
                    };
                    if m.replay_generation != gen {
                        return;
                    }
                    let Some(ref mut engine) = m.playback_engine else {
                        return;
                    };
                    if engine.status() != PlaybackStatus::Playing {
                        return;
                    }
                    let frames = engine.advance(dt);
                    if !frames.is_empty() {
                        if let Some(ref mut term) = m.replay_terminal {
                            apply_replay_frames_to_terminal(term, &frames);
                            m.screen_output_buffer = term.screen_to_string();
                        }
                    }
                    keep_going = engine.status() == PlaybackStatus::Playing;
                    cx.notify();
                });
                if updated.is_err() || !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn seek_modal_playback(&mut self, fraction: f32, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            m.replay_generation = m.replay_generation.wrapping_add(1);
            if let Some(ref mut engine) = m.playback_engine {
                let target = fraction as f64 * engine.duration_secs();
                let frames = engine.seek(target);
                if let Some(ref rec) = m.current_recording {
                    let term = rebuild_replay_terminal(&rec.cast_data, &frames);
                    m.screen_output_buffer = term.screen_to_string();
                    m.replay_terminal = Some(term);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn step_modal_playback(&mut self, delta_secs: f64, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            m.replay_generation = m.replay_generation.wrapping_add(1);
            if let Some(ref mut engine) = m.playback_engine {
                let target =
                    (engine.current_time_secs() + delta_secs).clamp(0.0, engine.duration_secs());
                let frames = engine.seek(target);
                if let Some(ref rec) = m.current_recording {
                    let term = rebuild_replay_terminal(&rec.cast_data, &frames);
                    m.screen_output_buffer = term.screen_to_string();
                    m.replay_terminal = Some(term);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn cycle_modal_playback_speed(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                engine.cycle_speed();
            }
        }
        cx.notify();
    }

    pub(crate) fn delete_session_recording_ui(&mut self, rec_id: String, cx: &mut Context<Self>) {
        if let Some(path) = self.active_vault_db_path() {
            let id_clone = rec_id.clone();
            let vault_key = self.active_vault_key();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let mut store = HostStore::open_with_key(&path, vault_key.as_ref())
                        .await
                        .map_err(|e| e.to_string())?;
                    store
                        .delete_session_recording(&id_clone)
                        .await
                        .map_err(|e| e.to_string())
                })
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Ok(true) = result {
                        if let Some(ref mut m) = app.session_replay_modal {
                            m.recordings.retain(|r| r.id != rec_id);
                            if m.selected_recording_id.as_deref() == Some(&rec_id) {
                                m.replay_generation = m.replay_generation.wrapping_add(1);
                                m.selected_recording_id = None;
                                m.current_recording = None;
                                m.playback_engine = None;
                                m.replay_terminal = None;
                                m.screen_output_buffer.clear();
                                m.active_tab = SessionReplayTab::RecordingsList;
                            }
                            m.status_notification =
                                Some(format!("Deleted session recording '{rec_id}'"));
                        }
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    pub(crate) fn export_session_recording_ui(&mut self, rec_id: String, cx: &mut Context<Self>) {
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let vault_key = self.active_vault_key();
        let expected_vault = self.active_vault_id.clone();
        let expected_gen = self.vault_load_generation;
        let default_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let suggested = format!("{rec_id}.cast");
        let path_future = cx.prompt_for_new_path(&default_dir, Some(&suggested));

        cx.spawn(async move |app, cx| {
            let Ok(Ok(Some(dest_path))) = path_future.await else {
                return;
            };
            let id_clone = rec_id.clone();
            let export_res = cx
                .background_executor()
                .spawn(async move {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .build()
                        .map_err(|e| e.to_string())?;
                    let record = runtime.block_on(async {
                        let store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                            .await
                            .map_err(|e| e.to_string())?;
                        store
                            .get_session_recording(&id_clone)
                            .await
                            .map_err(|e| e.to_string())
                    })?;
                    let record = record.ok_or_else(|| "Recording not found".to_string())?;
                    let redacted = SessionRedactor::redact_asciicast_v2(&record.cast_data)?;
                    atomic_write_restricted_cast(&dest_path, redacted.as_bytes())?;
                    Ok::<std::path::PathBuf, String>(dest_path)
                })
                .await;

            let _ = app.update(cx, |app, cx| {
                if app.active_vault_id != expected_vault
                    || app.vault_load_generation != expected_gen
                {
                    return;
                }
                if let Some(ref mut m) = app.session_replay_modal {
                    match export_res {
                        Ok(path) => {
                            m.status_notification = Some(format!(
                                "Exported recording '{rec_id}' to {}",
                                path.display()
                            ));
                        }
                        Err(err) => {
                            m.status_notification = Some(format!("Export failed: {err}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[allow(dead_code)]
    pub(crate) fn import_session_recording_ui(&mut self, cx: &mut Context<Self>) {
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let vault_key = self.active_vault_key();
        let expected_vault = self.active_vault_id.clone();
        let expected_gen = self.vault_load_generation;
        let path_future = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import Asciicast v2 (.cast) Recording".into()),
        });

        cx.spawn(async move |app, cx| {
            let Ok(Ok(Some(paths))) = path_future.await else {
                return;
            };
            let Some(src_path) = paths.into_iter().next() else {
                return;
            };
            let import_res = cx
                .background_executor()
                .spawn(async move {
                    let meta = std::fs::metadata(&src_path)
                        .map_err(|e| format!("Failed to stat cast file: {e}"))?;
                    if meta.len() as usize > shellumi_terminal::recording::MAX_CAST_PARSE_BYTES {
                        return Err("Cast file exceeds maximum allowed import size".to_string());
                    }
                    let raw = std::fs::read_to_string(&src_path)
                        .map_err(|e| format!("Failed to read cast file: {e}"))?;
                    let parsed = shellumi_terminal::SessionRecording::parse_asciicast_v2(&raw)?;
                    let redacted = SessionRedactor::redact_asciicast_v2(&raw)?;
                    let now_str = shellumi_core::format_rfc3339_utc(std::time::SystemTime::now());
                    let title = parsed.header.title.clone().unwrap_or_else(|| {
                        src_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Imported Recording")
                            .to_string()
                    });
                    let db_rec = SessionRecordingRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: uuid::Uuid::new_v4().to_string(),
                        host_id: None,
                        title,
                        duration_secs: parsed.duration_secs(),
                        event_count: parsed.event_count(),
                        byte_size: redacted.len(),
                        cast_data: redacted,
                        recorded_at: now_str.clone(),
                        created_at: now_str,
                    };
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .build()
                        .map_err(|e| e.to_string())?;
                    runtime.block_on(async {
                        let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                            .await
                            .map_err(|e| e.to_string())?;
                        store
                            .save_session_recording(&db_rec)
                            .await
                            .map_err(|e| e.to_string())
                    })?;
                    Ok::<String, String>(db_rec.title)
                })
                .await;

            let _ = app.update(cx, |app, cx| {
                if app.active_vault_id != expected_vault
                    || app.vault_load_generation != expected_gen
                {
                    return;
                }
                if let Some(ref mut m) = app.session_replay_modal {
                    match import_res {
                        Ok(title) => {
                            m.status_notification = Some(format!("Imported recording '{title}'"));
                            app.load_session_recordings(cx);
                        }
                        Err(err) => {
                            m.status_notification = Some(format!("Import failed: {err}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

pub(crate) fn rebuild_replay_terminal(
    cast_data: &str,
    frames: &[shellumi_core::PlaybackFrame],
) -> Terminal {
    let (rows, cols) = shellumi_terminal::SessionRecording::parse_asciicast_v2(cast_data)
        .map(|r| (r.header.height.clamp(1, 500), r.header.width.clamp(1, 500)))
        .unwrap_or((24, 80));
    let mut term = Terminal::new(rows, cols);
    apply_replay_frames_to_terminal(&mut term, frames);
    term
}

fn apply_replay_frames_to_terminal(term: &mut Terminal, frames: &[shellumi_core::PlaybackFrame]) {
    for f in frames {
        match f.event_type.as_str() {
            "o" => {
                term.process_bytes(f.data.as_bytes());
                // Discard all side-effect events (Bell, ProtocolReply, ClipboardWriteRequest, etc.) during replay
                let _ = term.drain_events();
            }
            "r" => {
                if let Some((c_str, r_str)) = f.data.split_once('x') {
                    if let (Ok(cols), Ok(rows)) = (c_str.parse::<usize>(), r_str.parse::<usize>()) {
                        term.resize(rows.clamp(1, 500), cols.clamp(1, 500));
                    }
                }
            }
            _ => {}
        }
    }
}

fn atomic_write_restricted_cast(dest: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = dest
        .parent()
        .ok_or_else(|| "Invalid export destination path".to_string())?;
    let tmp_name = format!(".shellumi-export-{}.tmp", uuid::Uuid::new_v4());
    let tmp = parent.join(tmp_name);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| format!("Failed to create temp export file: {e}"))?;
        if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Failed to write export file: {e}"));
        }
    }
    #[cfg(not(unix))]
    {
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| format!("Failed to create temp export file: {e}"))?;
        if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Failed to write export file: {e}"));
        }
    }

    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Failed to finalize export file: {e}")
    })
}
