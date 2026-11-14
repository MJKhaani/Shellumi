//! SFTP browser UI.
use super::*;

impl Shellumi {
    pub(crate) fn render_sftp_redesigned_dual_pane(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut dual_pane = div()
            .relative()
            .size_full()
            .flex()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(self.render_sftp_pane(true, cx))
            .child(self.render_sftp_pane(false, cx));

        if let Some(ctx_menu) = &self.sftp_context_menu {
            dual_pane = dual_pane.child(self.render_sftp_context_menu_overlay(ctx_menu, cx));
        }

        if let Some(modal) = &self.sftp_modal {
            dual_pane = dual_pane.child(self.render_sftp_modal_overlay(modal, cx));
        }

        dual_pane
    }

    fn render_sftp_pane(&self, is_left: bool, cx: &mut Context<Self>) -> gpui::Div {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };
        let mut pane_box = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden();

        if is_left {
            pane_box = pane_box.border_r_1().border_color(rgb(MacTheme::BORDER));
        }

        if pane.is_selecting_host {
            pane_box.child(self.render_sftp_select_host(is_left, cx))
        } else if pane.host.is_none() {
            pane_box.child(self.render_sftp_connect_to_host(is_left, cx))
        } else {
            pane_box.child(self.render_sftp_connected_browser(is_left, cx))
        }
    }
}
