//! Snippet modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_snippet_modal(
        &self,
        modal: &SnippetModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let action_name = if modal.execute {
            "Run Snippet"
        } else {
            "Paste Snippet"
        };
        let action_bg = if modal.execute {
            rgb(MacTheme::SUCCESS)
        } else {
            rgb(MacTheme::ACCENT_BLUE)
        };

        let overlay = div()
            .id("snippet-modal-overlay")
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
            .id("snippet-modal-container")
            .w(px(580.0))
            .max_h(px(520.0))
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
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(format!("📜 {action_name}: {}", modal.snippet_name)),
                ),
            )
            .child(
                div()
                    .id("dismiss-snippet-modal-btn")
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
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_snippet_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Command template card
        let template_card = div()
            .p_2()
            .rounded_md()
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .font_family(MONOSPACE_FONT)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(format!("Template: {}", modal.command_template)),
            );
        modal_box = modal_box.child(template_card);

        // Variables fields
        let mut vars_list = div()
            .id("snippet-vars-list")
            .flex()
            .flex_col()
            .gap_3()
            .overflow_y_scroll()
            .max_h(px(240.0));
        for var in &modal.variables {
            let mut var_header = div().flex().items_center().gap_2();
            var_header = var_header.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(var.name.clone()),
            );
            if var.is_secret {
                var_header = var_header.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x4a1818))
                        .text_xs()
                        .text_color(rgb(MacTheme::DANGER))
                        .child("SECRET"),
                );
            }
            if let Some(def) = &var.default_value {
                var_header = var_header.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x183048))
                        .text_xs()
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child(format!("default: {def}")),
                );
            }

            let field = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(var_header)
                .child(var.input.clone());
            vars_list = vars_list.child(field);
        }
        modal_box = modal_box.child(vars_list);

        // Error message if any
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠ {err}")),
            );
        }

        // Action buttons
        let actions = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pt_3()
            .child(
                div()
                    .id("cancel-snippet-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_snippet_modal(cx)))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("batch-from-snippet-modal-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener({
                        let snip_id = modal.snippet_id;
                        move |app, _, _, cx| {
                            app.dismiss_snippet_modal(cx);
                            app.open_batch_modal(snip_id, cx);
                        }
                    }))
                    .child("⚡ Multi-Host"),
            )
            .child(
                div()
                    .id("apply-snippet-btn")
                    .px_4()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(action_bg)
                    .bg(action_bg)
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.apply_snippet_modal(cx)))
                    .child(if modal.execute {
                        "Run (↵)"
                    } else {
                        "Paste into Terminal"
                    }),
            );
        modal_box = modal_box.child(actions);

        overlay.child(modal_box)
    }
}
