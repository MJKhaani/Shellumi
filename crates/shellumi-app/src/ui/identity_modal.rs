//! Identity modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_identity_modal(
        &self,
        modal: &IdentityModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_edit = modal.editing_id.is_some();
        let title = if is_edit {
            "Edit Identity"
        } else {
            "New Identity"
        };

        let error_view = modal.error.as_ref().map(|err| {
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

        let overlay = div()
            .id("identity-modal-overlay")
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

        let modal_box = div()
            .id("identity-modal-box")
            .w(px(500.0))
            .rounded_2xl()
            .bg(rgb(0x1c1e28))
            .border_1()
            .border_color(rgb(0x2d3245))
            .p_6()
            .shadow_xl()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Reusable SSH credential and private key path"),
                            ),
                    )
                    .child(
                        div()
                            .id("identity-modal-close-btn")
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.identity_modal = None;
                                cx.notify();
                            }))
                            .child("✕"),
                    ),
            )
            .children(error_view)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("IDENTITY NAME"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.name.clone()),
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
                                    .child("USERNAME"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.username.clone()),
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
                                    .child("PASSWORD (OPTIONAL)"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.password.clone()),
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
                                    .child("PRIVATE KEY PATH (OPTIONAL)"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.key_path.clone()),
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
                                    .child("PASSPHRASE (OPTIONAL)"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.passphrase.clone()),
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
                                    .child("TAGS (COMMA-SEPARATED)"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.tags.clone()),
                            ),
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
                            .id("identity-modal-cancel")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x252a36))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x323a4b)).text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.identity_modal = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("identity-modal-save")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x4098ff)))
                            .on_click(cx.listener(|app, _, _, cx| app.save_identity_modal(cx)))
                            .child("Save Identity"),
                    ),
            );

        overlay.child(modal_box)
    }
}
