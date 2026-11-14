//! Group context menu rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_group_context_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let (group_name, menu_x, menu_y) = self.group_context_menu.as_ref()?;
        let name_for_edit = group_name.clone();
        let name_for_del = group_name.clone();

        Some(
            div()
                .id("group-context-menu-backdrop")
                .absolute()
                .inset_0()
                .bg(rgba(0x00000022))
                .on_click(cx.listener(|app, _, _, cx| {
                    app.group_context_menu = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("group-context-menu-popup")
                        .absolute()
                        .top(px(*menu_y))
                        .left(px(*menu_x))
                        .w(px(200.0))
                        .rounded_lg()
                        .bg(rgb(0x181a24))
                        .border_1()
                        .border_color(rgb(0x2d3245))
                        .p_1p5()
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                        }))
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .gap_2()
                                .child("⚡")
                                .child("Quick Connect"),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child("⚡")
                                        .child("Connect"),
                                )
                                .child("›"),
                        )
                        .child(
                            div()
                                .id("ctx-edit-group-details")
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x252a36)))
                                .flex()
                                .items_center()
                                .justify_between()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.open_group_inspector(&name_for_edit, cx);
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            if let Some(p) = resolve_icon_path("edit", false, 32) {
                                                div()
                                                    .w(px(16.0))
                                                    .h(px(16.0))
                                                    .child(gpui::img(p).size_full())
                                                    .into_any_element()
                                            } else {
                                                div().child("✏️").into_any_element()
                                            },
                                        )
                                        .child("Edit Group Details"),
                                )
                                .child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("E")),
                        )
                        .child(
                            div()
                                .id("ctx-collaborate-group")
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x252a36)))
                                .flex()
                                .items_center()
                                .gap_2()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.group_context_menu = None;
                                    app.open_invite_modal(cx);
                                }))
                                .child(
                                    if let Some(p) = resolve_icon_path("invite_member", false, 32) {
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("👥").into_any_element()
                                    },
                                )
                                .child("Collaborate"),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child("📄")
                                        .child("Move to"),
                                )
                                .child("›"),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child("📄")
                                        .child("Copy to"),
                                )
                                .child("›"),
                        )
                        .child(
                            div()
                                .id("ctx-remove-group")
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::DANGER))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x3e1717)))
                                .flex()
                                .items_center()
                                .gap_2()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.delete_group_inspector(&name_for_del, cx);
                                }))
                                .child("🗑️")
                                .child("Remove"),
                        ),
                ),
        )
    }
}
