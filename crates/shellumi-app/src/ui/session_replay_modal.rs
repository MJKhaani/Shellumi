//! Session replay modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_session_replay_modal(
        &self,
        modal: &SessionReplayModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("session-replay-modal-overlay")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("session-replay-modal-box")
            .w(px(840.0))
            .max_h(px(660.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(if let Some(p) = resolve_icon_path("log", false, 48) {
                                div().w(px(22.0)).h(px(22.0)).child(gpui::img(p).size_full()).into_any_element()
                            } else {
                                div().child("⏱").into_any_element()
                            })
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Session logs"),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(
                                "Asciicast v2 audit logging, automated secret redaction, and timestamped terminal playback.",
                            ),
                    ),
            )
            .child(
                div()
                    .id("dismiss-session-replay-modal-btn").when(self.page == Some("Session Logs"), |element|element.hidden())
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_session_replay_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Status notification if present
        if let Some(ref note) = modal.status_notification {
            modal_box = modal_box.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(note.clone()),
            );
        }

        // Tab Navigation Bar
        let tabs = [
            (
                SessionReplayTab::RecordingsList,
                format!("📼 Recordings List ({})", modal.recordings.len()),
            ),
            (
                SessionReplayTab::ReplayPlayer,
                "▶ Replay Viewer".to_string(),
            ),
        ];

        let mut tab_bar = div().flex().items_center().gap_2();
        for (tab, label) in tabs {
            let is_active = modal.active_tab == tab;
            tab_bar = tab_bar.child(
                div()
                    .id(("session-replay-tab", tab as usize))
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(if is_active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.set_session_replay_tab(tab, cx);
                    }))
                    .child(label),
            );
        }
        modal_box = modal_box.child(tab_bar);

        // Tab Content
        match modal.active_tab {
            SessionReplayTab::RecordingsList => {
                let mut list_body = div().flex().flex_col().gap_3();

                if modal.recordings.is_empty() {
                    list_body = list_body.child(
                        div()
                            .id("recordings-empty-state")
                            .p_6()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("No session recordings found"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(
                                        "Click [⏺ Record] in any active terminal pane header to record your session.",
                                    ),
                            ),
                    );
                } else {
                    let mut scroll_list = div()
                        .id("recordings-scroll-list")
                        .max_h(px(380.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_2();

                    for (idx, rec) in modal.recordings.iter().enumerate() {
                        let is_selected = modal.selected_recording_id.as_deref() == Some(&rec.id);
                        let rec_id_play = rec.id.clone();
                        let rec_id_exp = rec.id.clone();
                        let rec_id_del = rec.id.clone();

                        let item = div()
                            .id(("rec-row", idx))
                            .p_3()
                            .rounded_lg()
                            .bg(if is_selected {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .border_1()
                            .border_color(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                    .child(rec.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(MacTheme::SURFACE))
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                                    .child(format!(
                                                        "⏱ {}",
                                                        rec.formatted_duration()
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(MacTheme::SURFACE))
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                    .child(rec.formatted_size()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!(
                                                "ID: {} • {} events • Recorded at {}",
                                                rec.id, rec.event_count, rec.recorded_at
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id(("rec-play-btn", idx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(MacTheme::ACCENT_BLUE))
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.select_and_load_recording_for_playback(
                                                    rec_id_play.clone(),
                                                    cx,
                                                );
                                            }))
                                            .child("▶ Play"),
                                    )
                                    .child(
                                        div()
                                            .id(("rec-export-btn", idx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(MacTheme::SURFACE_HOVER))
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.export_session_recording_ui(
                                                    rec_id_exp.clone(),
                                                    cx,
                                                );
                                            }))
                                            .child("💾 Export"),
                                    )
                                    .child(
                                        div()
                                            .id(("rec-delete-btn", idx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(MacTheme::DANGER))
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.delete_session_recording_ui(
                                                    rec_id_del.clone(),
                                                    cx,
                                                );
                                            }))
                                            .child("🗑 Delete"),
                                    ),
                            );

                        scroll_list = scroll_list.child(item);
                    }
                    list_body = list_body.child(scroll_list);
                }

                modal_box = modal_box.child(list_body);
            }
            SessionReplayTab::ReplayPlayer => {
                let mut player_body = div().flex().flex_col().gap_3();

                if let Some(ref cur) = modal.current_recording {
                    let engine_opt = modal.playback_engine.as_ref();
                    let status_badge = if let Some(e) = engine_opt {
                        match e.status() {
                            PlaybackStatus::Playing => div()
                                .bg(rgb(MacTheme::SUCCESS))
                                .text_color(rgb(0xffffff))
                                .child("▶ PLAYING"),
                            PlaybackStatus::Paused => div()
                                .bg(rgb(MacTheme::WARNING))
                                .text_color(rgb(0x000000))
                                .child("⏸ PAUSED"),
                            PlaybackStatus::Completed => div()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_color(rgb(0xffffff))
                                .child("✓ COMPLETED"),
                            PlaybackStatus::Stopped => div()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("⏹ STOPPED"),
                        }
                    } else {
                        div()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("⏹ STOPPED")
                    };

                    // Player Header Strip
                    let p_header = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(cur.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                        .child(format!(
                                            "Duration: {} • {} events • Recorded at {}",
                                            format_duration_hms(cur.duration_secs),
                                            cur.event_count,
                                            cur.recorded_at
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1()
                                .rounded_sm()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .child(status_badge),
                        );
                    player_body = player_body.child(p_header);

                    // Scrubber and Controls Bar
                    let (time_label, speed_label, is_playing) = if let Some(e) = engine_opt {
                        (
                            e.formatted_time(),
                            e.speed().display_label(),
                            e.status() == PlaybackStatus::Playing,
                        )
                    } else {
                        ("00:00 / 00:00".to_string(), "1.0x", false)
                    };

                    let mut controls_bar = div()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .flex()
                        .items_center()
                        .justify_between();

                    let playback_btns = div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("player-toggle-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_sm()
                                .bg(if is_playing {
                                    rgb(MacTheme::WARNING)
                                } else {
                                    rgb(MacTheme::SUCCESS)
                                })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_playing {
                                    rgb(0x000000)
                                } else {
                                    rgb(0xffffff)
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.toggle_modal_playback(cx);
                                }))
                                .child(if is_playing { "⏸ Pause" } else { "▶ Play" }),
                        )
                        .child(
                            div()
                                .id("player-back-5s-btn")
                                .px_2p5()
                                .py_1p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.step_modal_playback(-5.0, cx);
                                }))
                                .child("⏪ -5s"),
                        )
                        .child(
                            div()
                                .id("player-fwd-5s-btn")
                                .px_2p5()
                                .py_1p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.step_modal_playback(5.0, cx);
                                }))
                                .child("⏩ +5s"),
                        );

                    let seek_pills = div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id("player-seek-0")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.0, cx);
                                }))
                                .child("0%"),
                        )
                        .child(
                            div()
                                .id("player-seek-25")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.25, cx);
                                }))
                                .child("25%"),
                        )
                        .child(
                            div()
                                .id("player-seek-50")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.5, cx);
                                }))
                                .child("50%"),
                        )
                        .child(
                            div()
                                .id("player-seek-75")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.75, cx);
                                }))
                                .child("75%"),
                        )
                        .child(
                            div()
                                .id("player-seek-100")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(1.0, cx);
                                }))
                                .child("100%"),
                        );

                    let speed_and_time = div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .id("player-cycle-speed-btn")
                                .px_2p5()
                                .py_1p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.cycle_modal_playback_speed(cx);
                                }))
                                .child(format!("⚡ {speed_label}")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(time_label),
                        );

                    controls_bar = controls_bar
                        .child(playback_btns)
                        .child(seek_pills)
                        .child(speed_and_time);
                    player_body = player_body.child(controls_bar);

                    // Replay Terminal Screen
                    let mut screen_div = div()
                        .id("player-screen-box")
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(MacTheme::CONTENT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .min_h(px(240.0))
                        .max_h(px(320.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_1();

                    if modal.screen_output_buffer.is_empty() {
                        screen_div = screen_div.child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("[Ready to play. Click ▶ Play or seek to a timestamp.]"),
                        );
                    } else {
                        for (l_idx, line) in modal.screen_output_buffer.lines().enumerate() {
                            screen_div = screen_div.child(
                                div()
                                    .id(("screen-line", l_idx))
                                    .text_xs()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(if line.is_empty() { " " } else { line }.to_string()),
                            );
                        }
                    }
                    player_body = player_body.child(screen_div);
                } else {
                    player_body = player_body.child(
                        div()
                            .id("player-empty-prompt")
                            .p_6()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("No recording selected for playback"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(
                                        "Select a session from the [Recordings List] tab to start frame-by-frame replay.",
                                    ),
                            ),
                    );
                }

                modal_box = modal_box.child(player_body);
            }
        }

        // Footer
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .pt_3()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Compatible with Asciicast v2 (.cast) • Automated redaction enabled"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("CLI: shellumi session list / play / export"),
            );
        modal_box = modal_box.child(footer);

        if self.page == Some("Session Logs") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
    }
}
