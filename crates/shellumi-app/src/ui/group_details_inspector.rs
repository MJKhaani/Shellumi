//! Group details inspector rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_group_details_inspector(
        &self,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let Some(ins) = &self.group_inspector else {
            return div().id("group-details-inspector-empty");
        };

        let orig_name = ins.original_name.clone();
        let current_identity_id = ins.identity_id;

        let mut identities_dropdown = div().flex().flex_col().gap_1().child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                .child("CONNECTION IDENTITY"),
        );

        let none_sel = current_identity_id.is_none();
        let mut id_list = div()
            .id("group-inspector-id-list")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(140.0))
            .overflow_y_scroll()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .rounded_md()
            .p_1();

        id_list = id_list.child(
            div()
                .id("group-id-opt-none")
                .px_2()
                .py_1()
                .rounded_sm()
                .text_xs()
                .bg(if none_sel {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgb(MacTheme::INPUT_BG)
                })
                .text_color(if none_sel {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(ins) = &mut app.group_inspector {
                        ins.identity_id = None;
                        cx.notify();
                    }
                }))
                .child(if none_sel {
                    "✓ None (Inherit defaults)"
                } else {
                    "None (Inherit defaults)"
                }),
        );

        for (idx, id) in self.identities.iter().enumerate() {
            let is_sel = current_identity_id == Some(id.id);
            let id_val = id.id;
            id_list = id_list.child(
                div()
                    .id(("group-id-opt", idx))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_xs()
                    .bg(if is_sel {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(ins) = &mut app.group_inspector {
                            ins.identity_id = Some(id_val);
                            cx.notify();
                        }
                    }))
                    .child(format!("{} {}", if is_sel { "✓" } else { " " }, id.name)),
            );
        }
        identities_dropdown = identities_dropdown.child(id_list);

        let error_view = ins.error.as_ref().map(|err| {
            div()
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(rgb(0x3e1717))
                .border_1()
                .border_color(rgb(MacTheme::DANGER))
                .text_xs()
                .text_color(rgb(MacTheme::DANGER))
                .child(err.clone())
        });

        let delete_name = orig_name.clone();

        div()
            .id("group-details-inspector")
            .w(px(320.0))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(rgb(MacTheme::SURFACE))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_4()
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Group Details"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(if let Some(p) = resolve_icon_path("more", false, 32) {
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("···").into_any_element()
                                    }),
                            )
                            .child(
                                div()
                                    .id("close-group-inspector")
                                    .text_sm()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.group_inspector = None;
                                        cx.notify();
                                    }))
                                    .child("→|"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("group-details-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .children(error_view)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_2p5()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().child("💬"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child("Personal"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("⌵"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("General"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .child(
                                        div()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .rounded_md()
                                            .bg(rgb(MacTheme::BADGE_NAVY))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .font_weight(FontWeight::BOLD)
                                            .text_base()
                                            .child(
                                                if let Some(p) =
                                                    resolve_icon_path("groups", false, 32)
                                                {
                                                    div()
                                                        .w(px(22.0))
                                                        .h(px(22.0))
                                                        .child(gpui::img(p).size_full())
                                                        .into_any_element()
                                                } else {
                                                    div().child("⊞").into_any_element()
                                                },
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::SURFACE))
                                            .border_1()
                                            .border_color(rgb(MacTheme::BORDER))
                                            .px_2p5()
                                            .py_1p5()
                                            .child(ins.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::SURFACE))
                                    .border_1()
                                    .border_color(rgb(MacTheme::BORDER))
                                    .px_2p5()
                                    .py_1p5()
                                    .child(ins.parent_group.clone()),
                            )
                            .child(
                                div()
                                    .id("share-group-btn")
                                    .w_full()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(0x192538))
                                    .border_1()
                                    .border_color(rgb(0x27436b))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x223652)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.open_invite_modal(cx);
                                    }))
                                    .child(
                                        if let Some(p) =
                                            resolve_icon_path("invite_member", false, 48)
                                        {
                                            div()
                                                .w(px(18.0))
                                                .h(px(18.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("👥").into_any_element()
                                        },
                                    )
                                    .child("Share this group"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Cloud Sync")
                                    .child(
                                        div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("🛈"),
                                    ),
                            )
                            .child({
                                let sync_enabled = ins.cloud_sync;
                                div()
                                    .id("cloud-sync-toggle")
                                    .w(px(36.0))
                                    .h(px(20.0))
                                    .rounded_full()
                                    .bg(if sync_enabled {
                                        rgb(MacTheme::ACCENT_BLUE)
                                    } else {
                                        rgb(0x38383a)
                                    })
                                    .flex()
                                    .items_center()
                                    .p_0p5()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        if let Some(ins) = &mut app.group_inspector {
                                            ins.cloud_sync = !ins.cloud_sync;
                                            cx.notify();
                                        }
                                    }))
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .rounded_full()
                                            .bg(rgb(0xffffff))
                                            .when(sync_enabled, |e| e.ml_auto()),
                                    )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child("Protocols & Credentials"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .child("+ Add protocol"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child("DEFAULT USERNAME"),
                                    )
                                    .child(
                                        div()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::SURFACE))
                                            .border_1()
                                            .border_color(rgb(MacTheme::BORDER))
                                            .px_2p5()
                                            .py_1p5()
                                            .child(ins.default_username.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child("DEFAULT PORT"),
                                    )
                                    .child(
                                        div()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::SURFACE))
                                            .border_1()
                                            .border_color(rgb(MacTheme::BORDER))
                                            .px_2p5()
                                            .py_1p5()
                                            .child(ins.default_port.clone()),
                                    ),
                            )
                            .child(identities_dropdown),
                    ),
            )
            .child(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .id("delete-group-inspector-btn")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_xs()
                            .text_color(rgb(MacTheme::DANGER))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x3e1717)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.delete_group_inspector(&delete_name, cx);
                            }))
                            .child("Remove group"),
                    )
                    .child(
                        div()
                            .id("save-group-inspector-btn")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x4098ff)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.save_group_inspector(cx);
                            }))
                            .child("Save"),
                    ),
            )
    }
}
