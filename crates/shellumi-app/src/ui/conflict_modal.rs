//! Conflict modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_conflict_modal(
        &self,
        modal: &ConflictModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let total = modal.items.len();
        let selected_idx = modal.selected_index.min(total.saturating_sub(1));
        let selected_item = modal.items.get(selected_idx);
        let current_choice = modal
            .resolutions
            .get(selected_idx)
            .copied()
            .unwrap_or(ConflictResolutionChoice::KeepLocal);

        let overlay = div()
            .id("conflict-modal-overlay")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("conflict-modal-container")
            .w(px(880.0))
            .max_h(px(640.0))
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::WARNING))
                            .child("⚠️ Sync Conflict Resolution"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Concurrent modifications detected between local storage and remote vault. Choose which version to retain."),
                    ),
            )
            .child(
                div()
                    .id("close-conflict-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_conflict_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Conflict items sidebar / tabs
        let mut list_row = div().flex().gap_4().flex_1();
        let mut items_col = div().w(px(240.0)).flex().flex_col().gap_1();
        for (i, item) in modal.items.iter().enumerate() {
            let is_sel = i == selected_idx;
            let choice = modal
                .resolutions
                .get(i)
                .copied()
                .unwrap_or(ConflictResolutionChoice::KeepLocal);
            let pill_color = match choice {
                ConflictResolutionChoice::KeepLocal => rgb(MacTheme::ACCENT_BLUE),
                ConflictResolutionChoice::AcceptRemote => rgb(MacTheme::SUCCESS),
            };

            items_col = items_col.child(
                div()
                    .id(("conflict-item", i as u64))
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .border_1()
                    .border_color(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| app.select_conflict_item(i, cx)))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_sel {
                                        rgb(0xffffff)
                                    } else {
                                        rgb(MacTheme::TEXT_PRIMARY)
                                    })
                                    .child(item.object_id.clone()),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(pill_color)
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .child(choice.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if is_sel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .child(format!("Type: {}", item.entity_type)),
                    ),
            );
        }
        list_row = list_row.child(items_col);

        // Details of selected item
        if let Some(item) = selected_item {
            let detail_element: gpui::AnyElement = match item.entity_type {
                shellumi_core::SyncEntityType::Snippet => self
                    .render_snippet_conflict_diff(item, current_choice, cx)
                    .into_any_element(),
                shellumi_core::SyncEntityType::Tunnel => self
                    .render_tunnel_conflict_diff(item, current_choice, cx)
                    .into_any_element(),
                _ => {
                    let mut detail_col = div().flex_1().flex().flex_col().gap_3();

                    // Local vs Remote comparison cards
                    let mut comp_cards = div().flex().gap_3();

                    // Local card
                    let is_local_chosen = current_choice == ConflictResolutionChoice::KeepLocal;
                    let local_card = div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .rounded_md()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_2()
                        .border_color(if is_local_chosen {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SEPARATOR)
                        })
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                                        .child("🖥 Local Version"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(format!("v{}", item.local_version)),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(item.local_summary.clone()),
                        )
                        .child(
                            div()
                                .id("choose-local-btn")
                                .mt_auto()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(if is_local_chosen {
                                    rgb(MacTheme::ACCENT_BLUE)
                                } else {
                                    rgb(MacTheme::SEPARATOR)
                                })
                                .cursor_pointer()
                                .bg(if is_local_chosen {
                                    rgb(MacTheme::ACCENT_BLUE)
                                } else {
                                    rgb(0x323236)
                                })
                                .text_color(if is_local_chosen {
                                    rgb(0xffffff)
                                } else {
                                    rgb(MacTheme::TEXT_PRIMARY)
                                })
                                .text_xs()
                                .hover(move |s| {
                                    if is_local_chosen {
                                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                    } else {
                                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    }
                                })
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.set_conflict_choice(
                                        ConflictResolutionChoice::KeepLocal,
                                        cx,
                                    );
                                }))
                                .child(if is_local_chosen {
                                    "✓ Selected (Keep Local)"
                                } else {
                                    "Keep Local"
                                }),
                        );
                    comp_cards = comp_cards.child(local_card);

                    // Remote card
                    let is_remote_chosen = current_choice == ConflictResolutionChoice::AcceptRemote;
                    let remote_card = div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .rounded_md()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_2()
                        .border_color(if is_remote_chosen {
                            rgb(MacTheme::SUCCESS)
                        } else {
                            rgb(MacTheme::SEPARATOR)
                        })
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::SUCCESS))
                                        .child("☁ Remote Vault"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(format!("v{}", item.remote_version)),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(item.remote_summary.clone()),
                        )
                        .child(
                            div()
                                .id("choose-remote-btn")
                                .mt_auto()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(if is_remote_chosen {
                                    rgb(MacTheme::SUCCESS)
                                } else {
                                    rgb(MacTheme::SEPARATOR)
                                })
                                .cursor_pointer()
                                .bg(if is_remote_chosen {
                                    rgb(MacTheme::SUCCESS)
                                } else {
                                    rgb(0x323236)
                                })
                                .text_color(if is_remote_chosen {
                                    rgb(0xffffff)
                                } else {
                                    rgb(MacTheme::TEXT_PRIMARY)
                                })
                                .text_xs()
                                .hover(move |s| {
                                    if is_remote_chosen {
                                        s.bg(rgb(MacTheme::SUCCESS))
                                    } else {
                                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    }
                                })
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.set_conflict_choice(
                                        ConflictResolutionChoice::AcceptRemote,
                                        cx,
                                    );
                                }))
                                .child(if is_remote_chosen {
                                    "✓ Selected (Accept Remote)"
                                } else {
                                    "Accept Remote"
                                }),
                        );
                    comp_cards = comp_cards.child(remote_card);

                    detail_col = detail_col.child(comp_cards);
                    detail_col.into_any_element()
                }
            };
            list_row = list_row.child(detail_element);
        }

        modal_box = modal_box.child(list_row);

        // Footer buttons
        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .pt_3()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .id("cancel-resolutions-btn")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_conflict_modal(cx)))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("apply-resolutions-btn")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .text_color(rgb(0xffffff))
                    .font_weight(FontWeight::BOLD)
                    .text_xs()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.apply_conflict_resolutions(cx)))
                    .child(format!("Apply All Resolutions ({total})")),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }
}
