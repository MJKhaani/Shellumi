//! Desktop sessions page.
use super::*;

impl Shellumi {
    pub(super) fn sessions_page(&self, sftp: bool, cx: &mut Context<Self>) -> gpui::Div {
        let mut content = div().flex().flex_col().gap_3();
        if !sftp {
            content =
                content.child(primary("new-local-session", "+ Local terminal").on_click(
                    cx.listener(|app, _, window, cx| app.open_local_terminal(window, cx)),
                ));
        }
        for (index, tab) in self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| !sftp || tab.kind == SessionKind::Sftp)
        {
            content = content.child(button(("session-nav", index), tab.name.clone()).on_click(
                cx.listener(move |app, _, window, cx| app.select_tab(Some(index), window, cx)),
            ));
        }
        if let HostState::Loaded(hosts) = &self.hosts {
            for host in hosts.iter().filter(|host| host.is_ssh()) {
                let host = host.clone();
                content = content.child(
                    panel()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(heading(host.name.clone()))
                        .child(
                            button(
                                ("file-host", host.id as u64),
                                if sftp { "Connect SFTP" } else { "Connect SSH" },
                            )
                            .on_click(cx.listener(
                                move |app, _, window, cx| {
                                    if sftp {
                                        app.connect_sftp(&host, window, cx);
                                    } else {
                                        app.connect(&host, window, cx);
                                    }
                                },
                            )),
                        ),
                );
            }
        }
        content
    }
}
