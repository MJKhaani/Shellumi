//! Invite team modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_invite_team_modal(
        &self,
        modal: &InviteTeamModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("invite-modal-overlay")
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

        let mut emails_col = div().w_full().flex().flex_col().gap_2();
        for email_in in &modal.emails {
            emails_col = emails_col.child(
                div()
                    .w_full()
                    .rounded_lg()
                    .bg(rgb(0x161820))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .px_3()
                    .py_2()
                    .child(email_in.clone()),
            );
        }

        let modal_box = div()
            .id("invite-modal-box")
            .w(px(480.0))
            .rounded_2xl()
            .bg(rgb(0x1c1e28))
            .border_1()
            .border_color(rgb(0x2d3245))
            .p_6()
            .shadow_xl()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(52.0))
                    .h(px(52.0))
                    .rounded_full()
                    .bg(rgb(0x1a2942))
                    .border_1()
                    .border_color(rgb(0x284775))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if let Some(p) = resolve_icon_path("invite_member", false, 48) {
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().text_2xl().child("👥").into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Invite members to join your team"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .text_center()
                            .child("Securely share data with team vaults and collaborate in real time within terminal sessions."),
                    ),
            )
            .child(emails_col)
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_start()
                    .child(
                        div()
                            .id("invite-modal-add-another")
                            .text_xs()
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener(|app, _, _, cx| app.invite_modal_add_row(cx)))
                            .child("+ Add another"),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_3()
                    .mt_2()
                    .child(
                        div()
                            .id("invite-modal-cancel")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x252a36))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x323a4b)).text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.invite_modal = None;
                                cx.notify();
                            }))
                            .child("Later"),
                    )
                    .child(
                        div()
                            .id("invite-modal-submit")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x4098ff)))
                            .on_click(cx.listener(|app, _, _, cx| app.invite_modal_submit(cx)))
                            .child("Invite"),
                    ),
            );

        overlay.child(modal_box)
    }
}
