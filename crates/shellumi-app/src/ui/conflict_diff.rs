//! Specialized diff visualizers for Snippets and Tunnels in sync conflict resolution.

use super::*;
use shellumi_core::{ConflictItemView, ConflictResolutionChoice};

/// Classification of a single diff line between local and remote scripts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Unchanged,
    LocalOnly,
    RemoteOnly,
}

/// A line in a unified or side-by-side script diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub local_line_no: Option<usize>,
    pub remote_line_no: Option<usize>,
    pub content: String,
}

/// Computes a line-by-line diff between local and remote text using Longest Common Subsequence (LCS).
pub fn compute_line_diff(local_text: &str, remote_text: &str) -> Vec<DiffLine> {
    let local_lines: Vec<&str> = local_text.lines().collect();
    let remote_lines: Vec<&str> = remote_text.lines().collect();

    let m = local_lines.len();
    let n = remote_lines.len();

    // DP table for LCS
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..m {
        for j in 0..n {
            if local_lines[i] == remote_lines[j] {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // Backtrack from (m, n) to build diff
    let mut i = m;
    let mut j = n;
    let mut diff = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && local_lines[i - 1] == remote_lines[j - 1] {
            diff.push(DiffLine {
                kind: DiffLineKind::Unchanged,
                local_line_no: Some(i),
                remote_line_no: Some(j),
                content: local_lines[i - 1].to_string(),
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            diff.push(DiffLine {
                kind: DiffLineKind::RemoteOnly,
                local_line_no: None,
                remote_line_no: Some(j),
                content: remote_lines[j - 1].to_string(),
            });
            j -= 1;
        } else if i > 0 && (j == 0 || dp[i][j - 1] < dp[i - 1][j]) {
            diff.push(DiffLine {
                kind: DiffLineKind::LocalOnly,
                local_line_no: Some(i),
                remote_line_no: None,
                content: local_lines[i - 1].to_string(),
            });
            i -= 1;
        }
    }

    diff.reverse();
    diff
}

fn extract_command_from_summary(summary: &str) -> String {
    if let Some(start) = summary.find('`') {
        if let Some(end) = summary[start + 1..].rfind('`') {
            return summary[start + 1..start + 1 + end].to_string();
        }
    }
    summary.to_string()
}

/// Parsed model of a snippet conflict comparing local and remote attributes.
#[derive(Clone, Debug)]
pub struct SnippetConflictModel {
    pub local_name: String,
    pub remote_name: String,
    pub local_command: String,
    pub remote_command: String,
    pub local_description: Option<String>,
    pub remote_description: Option<String>,
    pub local_tags: Vec<String>,
    pub remote_tags: Vec<String>,
    pub diff_lines: Vec<DiffLine>,
}

impl SnippetConflictModel {
    pub fn from_conflict_item(item: &ConflictItemView) -> Self {
        let local_val: Option<serde_json::Value> = item
            .local_raw_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok());
        let remote_val: Option<serde_json::Value> = item
            .remote_raw_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok());

        let local_name = local_val
            .as_ref()
            .and_then(|v| v.get("name").and_then(|s| s.as_str()).map(str::to_string))
            .unwrap_or_else(|| item.object_id.clone());
        let remote_name = remote_val
            .as_ref()
            .and_then(|v| v.get("name").and_then(|s| s.as_str()).map(str::to_string))
            .unwrap_or_else(|| item.object_id.clone());

        let local_command = local_val
            .as_ref()
            .and_then(|v| {
                v.get("command")
                    .and_then(|s| s.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| extract_command_from_summary(&item.local_summary));
        let remote_command = remote_val
            .as_ref()
            .and_then(|v| {
                v.get("command")
                    .and_then(|s| s.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| extract_command_from_summary(&item.remote_summary));

        let local_description = local_val.as_ref().and_then(|v| {
            v.get("description")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        });
        let remote_description = remote_val.as_ref().and_then(|v| {
            v.get("description")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        });

        let local_tags = local_val
            .as_ref()
            .and_then(|v| {
                v.get("tags").and_then(|t| {
                    t.as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                })
            })
            .unwrap_or_default();
        let remote_tags = remote_val
            .as_ref()
            .and_then(|v| {
                v.get("tags").and_then(|t| {
                    t.as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                })
            })
            .unwrap_or_default();

        let diff_lines = compute_line_diff(&local_command, &remote_command);

        Self {
            local_name,
            remote_name,
            local_command,
            remote_command,
            local_description,
            remote_description,
            local_tags,
            remote_tags,
            diff_lines,
        }
    }
}

fn format_cli_flag(
    ftype: &str,
    bind_address: Option<&str>,
    bind_port: u16,
    target_host: Option<&str>,
    target_port: Option<u16>,
) -> String {
    let flag = match ftype.to_lowercase().as_str() {
        "remote" => "-R",
        "dynamic" => "-D",
        _ => "-L",
    };

    if ftype.eq_ignore_ascii_case("dynamic") {
        if let Some(addr) = bind_address {
            format!("ssh {flag} {addr}:{bind_port}")
        } else {
            format!("ssh {flag} {bind_port}")
        }
    } else {
        let thost = target_host.unwrap_or("localhost");
        let tport = target_port.unwrap_or(80);
        if let Some(addr) = bind_address {
            format!("ssh {flag} {addr}:{bind_port}:{thost}:{tport}")
        } else {
            format!("ssh {flag} {bind_port}:{thost}:{tport}")
        }
    }
}

/// Parsed model of a tunnel conflict comparing local and remote forwarding configuration.
#[derive(Clone, Debug)]
pub struct TunnelConflictModel {
    pub local_name: String,
    pub remote_name: String,
    pub local_type: String,
    pub remote_type: String,
    pub local_bind_address: Option<String>,
    pub remote_bind_address: Option<String>,
    pub local_bind_port: u16,
    pub remote_bind_port: u16,
    pub local_target_host: Option<String>,
    pub remote_target_host: Option<String>,
    pub local_target_port: Option<u16>,
    pub remote_target_port: Option<u16>,
    pub local_auto_start: bool,
    pub remote_auto_start: bool,
    pub local_cli_flag: String,
    pub remote_cli_flag: String,
}

impl TunnelConflictModel {
    pub fn from_conflict_item(item: &ConflictItemView) -> Self {
        let local_val: Option<serde_json::Value> = item
            .local_raw_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok());
        let remote_val: Option<serde_json::Value> = item
            .remote_raw_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok());

        let local_name = local_val
            .as_ref()
            .and_then(|v| v.get("name").and_then(|s| s.as_str()).map(str::to_string))
            .unwrap_or_else(|| item.object_id.clone());
        let remote_name = remote_val
            .as_ref()
            .and_then(|v| v.get("name").and_then(|s| s.as_str()).map(str::to_string))
            .unwrap_or_else(|| item.object_id.clone());

        let local_type = local_val
            .as_ref()
            .and_then(|v| {
                v.get("forwarding_type")
                    .and_then(|s| s.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "Local".into());
        let remote_type = remote_val
            .as_ref()
            .and_then(|v| {
                v.get("forwarding_type")
                    .and_then(|s| s.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "Local".into());

        let local_bind_address = local_val.as_ref().and_then(|v| {
            v.get("bind_address")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        });
        let remote_bind_address = remote_val.as_ref().and_then(|v| {
            v.get("bind_address")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        });

        let local_bind_port = local_val
            .as_ref()
            .and_then(|v| {
                v.get("bind_port")
                    .and_then(|s| s.as_u64())
                    .map(|p| p as u16)
            })
            .unwrap_or(0);
        let remote_bind_port = remote_val
            .as_ref()
            .and_then(|v| {
                v.get("bind_port")
                    .and_then(|s| s.as_u64())
                    .map(|p| p as u16)
            })
            .unwrap_or(0);

        let local_target_host = local_val.as_ref().and_then(|v| {
            v.get("target_host")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        });
        let remote_target_host = remote_val.as_ref().and_then(|v| {
            v.get("target_host")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        });

        let local_target_port = local_val.as_ref().and_then(|v| {
            v.get("target_port")
                .and_then(|s| s.as_u64())
                .map(|p| p as u16)
        });
        let remote_target_port = remote_val.as_ref().and_then(|v| {
            v.get("target_port")
                .and_then(|s| s.as_u64())
                .map(|p| p as u16)
        });

        let local_auto_start = local_val
            .as_ref()
            .and_then(|v| v.get("auto_start").and_then(|s| s.as_bool()))
            .unwrap_or(false);
        let remote_auto_start = remote_val
            .as_ref()
            .and_then(|v| v.get("auto_start").and_then(|s| s.as_bool()))
            .unwrap_or(false);

        let local_cli_flag = format_cli_flag(
            &local_type,
            local_bind_address.as_deref(),
            local_bind_port,
            local_target_host.as_deref(),
            local_target_port,
        );
        let remote_cli_flag = format_cli_flag(
            &remote_type,
            remote_bind_address.as_deref(),
            remote_bind_port,
            remote_target_host.as_deref(),
            remote_target_port,
        );

        Self {
            local_name,
            remote_name,
            local_type,
            remote_type,
            local_bind_address,
            remote_bind_address,
            local_bind_port,
            remote_bind_port,
            local_target_host,
            remote_target_host,
            local_target_port,
            remote_target_port,
            local_auto_start,
            remote_auto_start,
            local_cli_flag,
            remote_cli_flag,
        }
    }
}

impl Shellumi {
    /// Renders an interactive snippet script and metadata diff for conflict resolution.
    pub(crate) fn render_snippet_conflict_diff(
        &self,
        item: &ConflictItemView,
        current_choice: ConflictResolutionChoice,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let model = SnippetConflictModel::from_conflict_item(item);
        let is_local_chosen = current_choice == ConflictResolutionChoice::KeepLocal;
        let is_remote_chosen = current_choice == ConflictResolutionChoice::AcceptRemote;

        let mut container = div().flex().flex_col().gap_3().flex_1();

        // 1. Metadata differences banner
        let mut meta_bar = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_2p5()
            .rounded_md()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR_SUBTLE));

        let name_match = model.local_name == model.remote_name;
        meta_bar = meta_bar.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Name:"),
                        )
                        .child(
                            div()
                                .text_color(if name_match {
                                    rgb(MacTheme::TEXT_PRIMARY)
                                } else {
                                    rgb(MacTheme::WARNING)
                                })
                                .child(format!(
                                    "Local: '{}'  vs  Remote: '{}'",
                                    model.local_name, model.remote_name
                                )),
                        ),
                )
                .child(if name_match {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(MacTheme::BADGE_DEEP_BLUE))
                        .text_xs()
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child("Matches")
                } else {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x5c3c10))
                        .text_xs()
                        .text_color(rgb(MacTheme::WARNING))
                        .child("Modified")
                }),
        );

        if model.local_description.is_some() || model.remote_description.is_some() {
            let desc_match = model.local_description == model.remote_description;
            let local_d = model.local_description.as_deref().unwrap_or("—");
            let remote_d = model.remote_description.as_deref().unwrap_or("—");
            meta_bar = meta_bar.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Description:"),
                            )
                            .child(
                                div()
                                    .text_color(if desc_match {
                                        rgb(MacTheme::TEXT_PRIMARY)
                                    } else {
                                        rgb(MacTheme::WARNING)
                                    })
                                    .child(format!("Local: '{local_d}'  vs  Remote: '{remote_d}'")),
                            ),
                    )
                    .child(if desc_match {
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::BADGE_DEEP_BLUE))
                            .text_xs()
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .child("Matches")
                    } else {
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x5c3c10))
                            .text_xs()
                            .text_color(rgb(MacTheme::WARNING))
                            .child("Modified")
                    }),
            );
        }

        // Tags diff chips
        let mut tags_row = div().flex().items_center().gap_1p5().text_xs().child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child("Tags:"),
        );

        if model.local_tags.is_empty() && model.remote_tags.is_empty() {
            tags_row = tags_row.child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("None"));
        } else {
            // Local tags
            for tag in &model.local_tags {
                let in_remote = model.remote_tags.contains(tag);
                tags_row = tags_row.child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(if in_remote {
                            rgb(MacTheme::SURFACE_ACTIVE)
                        } else {
                            rgb(0x4a1e1e)
                        })
                        .text_color(if in_remote {
                            rgb(MacTheme::TEXT_PRIMARY)
                        } else {
                            rgb(MacTheme::DANGER)
                        })
                        .child(if in_remote {
                            format!("#{tag}")
                        } else {
                            format!("- #{tag} (local only)")
                        }),
                );
            }
            // Remote-only tags
            for tag in &model.remote_tags {
                if !model.local_tags.contains(tag) {
                    tags_row = tags_row.child(
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x1b3b2b))
                            .text_color(rgb(MacTheme::SUCCESS))
                            .child(format!("+ #{tag} (remote only)")),
                    );
                }
            }
        }
        meta_bar = meta_bar.child(tags_row);
        container = container.child(meta_bar);

        // 2. Visual Script / Command Diff Box
        let mut diff_box = div()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .bg(rgb(MacTheme::TERMINAL_BG))
            .overflow_hidden();

        // Diff header
        let local_line_count = model.local_command.lines().count();
        let remote_line_count = model.remote_command.lines().count();
        let diff_header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_1p5()
            .bg(rgb(0x1e1e24))
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR_SUBTLE))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("📜 Command Script Diff"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child(format!(
                            "({local_line_count} local lines / {remote_line_count} remote lines)"
                        )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .text_xs()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(rgb(MacTheme::DANGER)),
                            )
                            .child(div().text_color(rgb(MacTheme::DANGER)).child("- Local")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(rgb(MacTheme::SUCCESS)),
                            )
                            .child(div().text_color(rgb(MacTheme::SUCCESS)).child("+ Remote")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(rgb(MacTheme::TEXT_TERTIARY)),
                            )
                            .child(
                                div()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Shared"),
                            ),
                    ),
            );
        diff_box = diff_box.child(diff_header);

        // Diff line list
        let mut lines_scroll = div()
            .id("snippet-diff-lines-scroll")
            .flex()
            .flex_col()
            .max_h(px(220.0))
            .overflow_y_scroll()
            .p_2()
            .font_family(".AppleSystemUIFontMonospaced")
            .text_xs();

        if model.diff_lines.is_empty() {
            lines_scroll = lines_scroll.child(
                div()
                    .p_2()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("No command content or empty script."),
            );
        } else {
            for (idx, line) in model.diff_lines.iter().enumerate() {
                let (row_bg, text_color, sign) = match line.kind {
                    DiffLineKind::Unchanged => {
                        (rgb(MacTheme::TERMINAL_BG), rgb(MacTheme::TEXT_PRIMARY), " ")
                    }
                    DiffLineKind::LocalOnly => (rgb(0x351818), rgb(0xff8888), "-"),
                    DiffLineKind::RemoteOnly => (rgb(0x153520), rgb(0x4ade80), "+"),
                };

                let loc_no = line
                    .local_line_no
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                let rem_no = line
                    .remote_line_no
                    .map(|n| n.to_string())
                    .unwrap_or_default();

                lines_scroll = lines_scroll.child(
                    div()
                        .id(("diff-line", idx as u64))
                        .flex()
                        .items_center()
                        .bg(row_bg)
                        .px_1p5()
                        .py_0p5()
                        .gap_2()
                        .child(
                            div()
                                .w(px(24.0))
                                .text_right()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(loc_no),
                        )
                        .child(
                            div()
                                .w(px(24.0))
                                .text_right()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(rem_no),
                        )
                        .child(
                            div()
                                .w(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_color)
                                .child(sign),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(text_color)
                                .child(line.content.clone()),
                        ),
                );
            }
        }
        diff_box = diff_box.child(lines_scroll);
        container = container.child(diff_box);

        // 3. Selection Choice Action Cards
        let mut comp_cards = div().flex().gap_3().mt_auto();

        // Local Selection Card
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
                            .child("🖥 Local Snippet"),
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
                    .id("choose-local-snippet-btn")
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
                        app.set_conflict_choice(ConflictResolutionChoice::KeepLocal, cx);
                    }))
                    .child(if is_local_chosen {
                        "✓ Selected (Keep Local Snippet)"
                    } else {
                        "Keep Local Snippet"
                    }),
            );
        comp_cards = comp_cards.child(local_card);

        // Remote Selection Card
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
                            .child("☁ Remote Snippet"),
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
                    .id("choose-remote-snippet-btn")
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
                        app.set_conflict_choice(ConflictResolutionChoice::AcceptRemote, cx);
                    }))
                    .child(if is_remote_chosen {
                        "✓ Selected (Accept Remote Snippet)"
                    } else {
                        "Accept Remote Snippet"
                    }),
            );
        comp_cards = comp_cards.child(remote_card);
        container = container.child(comp_cards);

        container
    }

    /// Renders an interactive port-forwarding topology and attribute diff for tunnel conflicts.
    pub(crate) fn render_tunnel_conflict_diff(
        &self,
        item: &ConflictItemView,
        current_choice: ConflictResolutionChoice,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let model = TunnelConflictModel::from_conflict_item(item);
        let is_local_chosen = current_choice == ConflictResolutionChoice::KeepLocal;
        let is_remote_chosen = current_choice == ConflictResolutionChoice::AcceptRemote;

        let mut container = div().flex().flex_col().gap_3().flex_1();

        // 1. Topology comparison banner
        let mut topo_box = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2p5()
            .rounded_md()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR_SUBTLE));

        // Topology header
        let tunnel_name_display = if model.local_name == model.remote_name {
            format!("Tunnel: {}", model.local_name)
        } else {
            format!("Tunnel: {} vs {}", model.local_name, model.remote_name)
        };
        topo_box = topo_box.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child("🚇 Port Forwarding Route Comparison")
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(tunnel_name_display),
                ),
        );

        // Local vs Remote routes
        let local_bind_str = format!(
            "{}:{}",
            model.local_bind_address.as_deref().unwrap_or("127.0.0.1"),
            model.local_bind_port
        );
        let local_target_str = format!(
            "{}:{}",
            model.local_target_host.as_deref().unwrap_or("localhost"),
            model.local_target_port.unwrap_or(0)
        );

        let remote_bind_str = format!(
            "{}:{}",
            model.remote_bind_address.as_deref().unwrap_or("127.0.0.1"),
            model.remote_bind_port
        );
        let remote_target_str = format!(
            "{}:{}",
            model.remote_target_host.as_deref().unwrap_or("localhost"),
            model.remote_target_port.unwrap_or(0)
        );

        let routes_grid = div()
            .flex()
            .gap_2()
            .child(
                // Local route box
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x222226))
                    .border_1()
                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .child("🖥 Local Config"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::BADGE_NAVY))
                                    .text_color(rgb(0xffffff))
                                    .child(local_bind_str),
                            )
                            .child(
                                div()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(format!("─({})─>", model.local_type)),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x353538))
                                    .text_color(rgb(0xffffff))
                                    .child(local_target_str),
                            ),
                    ),
            )
            .child(
                // Remote route box
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x222226))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::SUCCESS))
                            .child("☁ Remote Config"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x1b3b2b))
                                    .text_color(rgb(0xffffff))
                                    .child(remote_bind_str),
                            )
                            .child(
                                div()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(format!("─({})─>", model.remote_type)),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x353538))
                                    .text_color(rgb(0xffffff))
                                    .child(remote_target_str),
                            ),
                    ),
            );

        topo_box = topo_box.child(routes_grid);
        container = container.child(topo_box);

        // 2. Structured Attribute Comparison Table
        let mut table_box = div()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .bg(rgb(MacTheme::SURFACE))
            .overflow_hidden();

        let table_header = div()
            .flex()
            .items_center()
            .px_3()
            .py_1p5()
            .bg(rgb(0x1e1e24))
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR_SUBTLE))
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .child(
                div()
                    .w(px(130.0))
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Attribute"),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .child("Local Version"),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child("Remote Version"),
            )
            .child(
                div()
                    .w(px(70.0))
                    .text_right()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Status"),
            );
        table_box = table_box.child(table_header);

        let rows_data: Vec<(&'static str, String, String, bool)> = vec![
            (
                "Tunnel Name",
                model.local_name.clone(),
                model.remote_name.clone(),
                model.local_name == model.remote_name,
            ),
            (
                "Forwarding Type",
                model.local_type.clone(),
                model.remote_type.clone(),
                model.local_type == model.remote_type,
            ),
            (
                "Bind Port",
                model.local_bind_port.to_string(),
                model.remote_bind_port.to_string(),
                model.local_bind_port == model.remote_bind_port,
            ),
            (
                "Bind Address",
                model
                    .local_bind_address
                    .clone()
                    .unwrap_or_else(|| "default (127.0.0.1)".into()),
                model
                    .remote_bind_address
                    .clone()
                    .unwrap_or_else(|| "default (127.0.0.1)".into()),
                model.local_bind_address == model.remote_bind_address,
            ),
            (
                "Target Host",
                model
                    .local_target_host
                    .clone()
                    .unwrap_or_else(|| "none".into()),
                model
                    .remote_target_host
                    .clone()
                    .unwrap_or_else(|| "none".into()),
                model.local_target_host == model.remote_target_host,
            ),
            (
                "Target Port",
                model
                    .local_target_port
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "none".into()),
                model
                    .remote_target_port
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "none".into()),
                model.local_target_port == model.remote_target_port,
            ),
            (
                "Auto-Start",
                if model.local_auto_start {
                    "Enabled".into()
                } else {
                    "Disabled".into()
                },
                if model.remote_auto_start {
                    "Enabled".into()
                } else {
                    "Disabled".into()
                },
                model.local_auto_start == model.remote_auto_start,
            ),
            (
                "OpenSSH CLI",
                model.local_cli_flag.clone(),
                model.remote_cli_flag.clone(),
                model.local_cli_flag == model.remote_cli_flag,
            ),
        ];

        for (idx, (attr, loc_val, rem_val, is_match)) in rows_data.into_iter().enumerate() {
            let row_bg = if idx % 2 == 0 {
                rgb(MacTheme::INPUT_BG)
            } else {
                rgb(0x27272a)
            };

            table_box = table_box.child(
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .bg(row_bg)
                    .border_b_1()
                    .border_color(rgb(MacTheme::SEPARATOR_SUBTLE))
                    .text_xs()
                    .child(
                        div()
                            .w(px(130.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(attr),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(if is_match {
                                rgb(MacTheme::TEXT_PRIMARY)
                            } else {
                                rgb(MacTheme::ACCENT_BLUE)
                            })
                            .child(loc_val),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(if is_match {
                                rgb(MacTheme::TEXT_PRIMARY)
                            } else {
                                rgb(MacTheme::SUCCESS)
                            })
                            .child(rem_val),
                    )
                    .child(div().w(px(70.0)).flex().justify_end().child(if is_match {
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x1b3b2b))
                            .text_color(rgb(MacTheme::SUCCESS))
                            .child("Match")
                    } else {
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x5c3c10))
                            .text_color(rgb(MacTheme::WARNING))
                            .child("Diff")
                    })),
            );
        }

        container = container.child(table_box);

        // 3. Selection Choice Action Cards
        let mut comp_cards = div().flex().gap_3().mt_auto();

        // Local Selection Card
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
                            .child("🖥 Local Tunnel"),
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
                    .id("choose-local-tunnel-btn")
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
                        app.set_conflict_choice(ConflictResolutionChoice::KeepLocal, cx);
                    }))
                    .child(if is_local_chosen {
                        "✓ Selected (Keep Local Tunnel)"
                    } else {
                        "Keep Local Tunnel"
                    }),
            );
        comp_cards = comp_cards.child(local_card);

        // Remote Selection Card
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
                            .child("☁ Remote Tunnel"),
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
                    .id("choose-remote-tunnel-btn")
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
                        app.set_conflict_choice(ConflictResolutionChoice::AcceptRemote, cx);
                    }))
                    .child(if is_remote_chosen {
                        "✓ Selected (Accept Remote Tunnel)"
                    } else {
                        "Accept Remote Tunnel"
                    }),
            );
        comp_cards = comp_cards.child(remote_card);
        container = container.child(comp_cards);

        container
    }
}
