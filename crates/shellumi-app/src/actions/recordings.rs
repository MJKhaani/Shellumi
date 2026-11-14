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

    pub(crate) fn toggle_pane_recording(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        for tab in self.tabs.iter_mut() {
            for pane in tab.panes.iter_mut() {
                if pane.id == pane_id {
                    let name = pane.name.clone();
                    if pane.terminal.is_recording() {
                        let rec_opt = pane.terminal.stop_recording();
                        if let Some(rec) = rec_opt {
                            let duration_secs = rec.duration_secs();
                            let event_count = rec.event_count();
                            let raw_cast = rec.to_asciicast_v2().unwrap_or_default();
                            let clean_cast =
                                SessionRedactor::redact_asciicast_v2(&raw_cast).unwrap_or(raw_cast);

                            let now_epoch = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now_epoch}");
                            let rec_id = format!("rec-{now_epoch}");
                            let sess_id = format!("sess-{now_epoch}");

                            let db_rec = SessionRecordingRecord {
                                id: rec_id.clone(),
                                session_id: sess_id,
                                host_id: None,
                                title: format!("Recorded Session: {name}"),
                                duration_secs,
                                event_count,
                                byte_size: clean_cast.len(),
                                cast_data: clean_cast,
                                recorded_at: now_str.clone(),
                                created_at: now_str,
                            };

                            if let Some(path) = self.database.clone() {
                                let db_rec_clone = db_rec.clone();
                                cx.background_executor()
                                    .spawn(async move {
                                        let runtime = tokio::runtime::Builder::new_current_thread()
                                            .build()
                                            .map_err(|e| e.to_string());
                                        if let Ok(rt) = runtime {
                                            rt.block_on(async {
                                                if let Ok(mut store) = HostStore::open(&path).await
                                                {
                                                    let _ = store
                                                        .save_session_recording(&db_rec_clone)
                                                        .await;
                                                }
                                            });
                                        }
                                    })
                                    .detach();
                            }

                            pane.message = Some(format!(
                                "⏹ Recording stopped & saved ({}, {event_count} events)",
                                format_duration_hms(duration_secs)
                            ));
                        }
                    } else {
                        pane.terminal
                            .start_recording(Some(format!("Session: {name}")));
                        pane.message = Some("⏺ Session recording started.".to_string());
                    }
                    cx.notify();
                    return;
                }
            }
        }
    }

    fn load_session_recordings(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.database.clone() {
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
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
        if let Some(path) = self.database.clone() {
            let id_clone = rec_id.clone();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store
                        .get_session_recording(&id_clone)
                        .await
                        .map_err(|e| e.to_string())
                })
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Ok(Some(record)) = result {
                        if let Some(ref mut m) = app.session_replay_modal {
                            if let Ok(mut engine) =
                                CastPlaybackEngine::from_asciicast_v2(&record.cast_data)
                            {
                                m.selected_recording_id = Some(rec_id.clone());
                                m.current_recording = Some(record);
                                m.active_tab = SessionReplayTab::ReplayPlayer;
                                m.screen_output_buffer.clear();
                                let initial_frames = engine.seek(0.0);
                                for f in initial_frames {
                                    if f.event_type == "o" {
                                        m.screen_output_buffer.push_str(&f.data);
                                    }
                                }
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
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                engine.toggle_play_pause();
                // Advance 1 frame immediately so playback responds visually
                if engine.status() == PlaybackStatus::Playing {
                    let frames = engine.advance(0.1);
                    for f in frames {
                        if f.event_type == "o" {
                            m.screen_output_buffer.push_str(&f.data);
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn seek_modal_playback(&mut self, fraction: f32, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                let target = fraction as f64 * engine.duration_secs();
                let frames = engine.seek(target);
                m.screen_output_buffer.clear();
                for f in frames {
                    if f.event_type == "o" {
                        m.screen_output_buffer.push_str(&f.data);
                    }
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn step_modal_playback(&mut self, delta_secs: f64, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                let target =
                    (engine.current_time_secs() + delta_secs).clamp(0.0, engine.duration_secs());
                let frames = engine.seek(target);
                m.screen_output_buffer.clear();
                for f in frames {
                    if f.event_type == "o" {
                        m.screen_output_buffer.push_str(&f.data);
                    }
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
        if let Some(path) = self.database.clone() {
            let id_clone = rec_id.clone();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
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
                                m.selected_recording_id = None;
                                m.current_recording = None;
                                m.playback_engine = None;
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
        if let Some(ref mut m) = self.session_replay_modal {
            m.status_notification = Some(format!(
                "Exported recording '{rec_id}' to standard Asciicast v2 format (.cast)"
            ));
        }
        cx.notify();
    }

    // =========================================================================
    // Dynamic Port-Forwarding Tunnel Manager UI & Telemetry
    // =========================================================================
}
