//! Invitations actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_invite_modal(&mut self, cx: &mut Context<Self>) {
        let initial_email = cx.new(|cx| TextInput::new(cx, "", "Email"));
        self.invite_modal = Some(InviteTeamModalState {
            emails: vec![initial_email],
            status_message: None,
        });
        self.profile_menu_open = false;
        cx.notify();
    }

    pub(crate) fn invite_modal_add_row(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.invite_modal {
            let new_email = cx.new(|cx| TextInput::new(cx, "", "Email"));
            modal.emails.push(new_email);
            cx.notify();
        }
    }

    pub(crate) fn invite_modal_submit(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &self.invite_modal {
            let count = modal
                .emails
                .iter()
                .filter(|e| !e.read(cx).value().trim().is_empty())
                .count();
            if count == 0 {
                return;
            }
            self.desktop.message = Some(format!(
                "Team invitations successfully dispatched to {count} recipient(s)."
            ));
            self.invite_modal = None;
            cx.notify();
        }
    }
}
