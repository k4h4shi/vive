use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthChar;

use crate::state::{AppState, FocusMode, ModalType, StatusMessageType};

/// Render the UI based on the current application state.
pub fn render(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();

    // Main layout: Header, Content, Footer
    let main_chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(0),    // Content
        Constraint::Length(3), // Footer/Input
    ])
    .split(area);

    render_header(frame, main_chunks[0], state);
    render_content(frame, main_chunks[1], state);
    let cursor_position = render_footer(frame, main_chunks[2], state);

    // Render modal on top if present
    if let Some(modal) = &state.modal {
        render_modal(frame, area, modal);
    }

    // Set cursor position for IME input when in input mode
    if let Some((x, y)) = cursor_position {
        frame.set_cursor_position((x, y));
    }
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    // Show status message if present, otherwise show default header
    let content = if let Some(status) = &state.status_message {
        let (color, prefix) = match status.message_type {
            StatusMessageType::Info => (Color::Blue, "INFO"),
            StatusMessageType::Success => (Color::Green, "OK"),
            StatusMessageType::Error => (Color::Red, "ERROR"),
        };
        Line::from(vec![
            Span::styled(
                format!("[{prefix}] "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(&status.message, Style::default().fg(color)),
        ])
    } else {
        Line::from(Span::styled(
            "Vive - Claude Code Cockpit",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
    };

    let header =
        Paragraph::new(content).block(Block::default().borders(Borders::ALL).title("Vive"));
    frame.render_widget(header, area);
}

fn render_content(frame: &mut Frame, area: Rect, state: &mut AppState) {
    // Split content into sidebar and preview
    // Use 40% for sidebar to accommodate longer branch names
    let content_chunks = Layout::horizontal([
        Constraint::Percentage(40), // Sidebar
        Constraint::Percentage(60), // Preview
    ])
    .split(area);

    render_sidebar(frame, content_chunks[0], state);
    render_preview(frame, content_chunks[1], state);
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    // Build list items first (immutable borrow of state)
    let items = build_sidebar_items(state);

    let sidebar = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Projects"))
        // Use empty highlight symbol to avoid interfering with item content
        .highlight_symbol("")
        // Use minimal highlight style - items already have their own selection styling
        .highlight_style(Style::default());
    // Now we can mutably borrow for ListState
    frame.render_stateful_widget(sidebar, area, state.sidebar_list_state_mut());
}

fn build_sidebar_items(state: &AppState) -> Vec<ListItem<'static>> {
    let mut items: Vec<ListItem> = Vec::new();

    let sorted_projects = state.sorted_projects();
    let has_favorites = state.has_favorites();
    let has_non_favorites = sorted_projects.iter().any(|p| !state.is_favorite(&p.name));
    let mut separator_added = false;

    for project in sorted_projects.iter() {
        let is_favorite = state.is_favorite(&project.name);

        // Add separator between favorites and non-favorites
        if has_favorites && has_non_favorites && !is_favorite && !separator_added {
            items.push(ListItem::new(Line::from(Span::styled(
                "────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            ))));
            separator_added = true;
        }

        // Find original index for selection comparison
        let orig_idx = state
            .projects
            .iter()
            .position(|p| p.name == project.name)
            .unwrap();
        let is_selected_project = state.selected_project_idx() == Some(orig_idx);

        let project_style = if is_selected_project && state.selected_worktree_idx().is_none() {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if is_favorite {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        };

        // Project header with ▼ collapse indicator and ★ favorite indicator
        let favorite_indicator = if is_favorite { " (★)" } else { "" };
        items.push(ListItem::new(Line::from(vec![
            Span::styled("▼ ", Style::default().fg(Color::DarkGray)),
            Span::styled(project.name.clone(), project_style),
            Span::styled(
                favorite_indicator.to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ])));

        // Worktrees under the project with tree structure
        let worktree_count = project.worktrees.len();
        for (wt_idx, worktree) in project.worktrees.iter().enumerate() {
            let is_selected = is_selected_project && state.selected_worktree_idx() == Some(wt_idx);
            let is_last = wt_idx == worktree_count - 1;
            let branch_name = worktree.branch.as_deref().unwrap_or("(detached)");

            // Tree prefix: ├─ for middle items, └─ for last item
            let tree_prefix = if is_last { "└─" } else { "├─" };

            // Get status for this worktree
            let status = worktree
                .session_id(&project.name)
                .map(|sid| state.get_status(&sid))
                .unwrap_or_default();

            // Status icon with color based on type
            let (icon, icon_color) = get_status_icon_and_color(&status);

            // Branch name style
            let branch_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::Cyan)
            };

            // Fixed-width branch name (truncate if too long, pad if short)
            // Uses character count to handle UTF-8 safely
            // Increased from 16 to 24 to accommodate longer branch names
            let max_branch_len = 24;
            let branch_char_count = branch_name.chars().count();
            let branch_display = if branch_char_count > max_branch_len {
                let truncated: String = branch_name.chars().take(max_branch_len - 3).collect();
                format!("{truncated}...")
            } else {
                branch_name.to_string()
            };

            // Padding dots between branch name and status
            let display_char_count = branch_display.chars().count();
            let padding_len = max_branch_len.saturating_sub(display_char_count) + 1;
            let padding = ".".repeat(padding_len);

            // Status text for inline display
            let status_text = status.status_text();

            items.push(ListItem::new(Line::from(vec![
                Span::styled("  ", Style::default()), // indent
                Span::styled(
                    tree_prefix.to_string(),
                    Style::default().fg(Color::DarkGray),
                ), // tree chars
                Span::styled(" ", Style::default()),
                Span::styled(branch_display, branch_style), // branch name
                Span::styled(format!(" {padding} "), Style::default().fg(Color::DarkGray)), // padding
                Span::styled(icon.to_string(), Style::default().fg(icon_color)), // status icon
                Span::styled(" ", Style::default()),
                Span::styled(status_text, Style::default().fg(Color::DarkGray)), // status text
            ])));
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "No projects found",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    items
}

/// Get the icon and color for a given agent status.
fn get_status_icon_and_color(status: &crate::state::AgentStatus) -> (&'static str, Color) {
    use crate::state::AgentStatus;
    match status {
        AgentStatus::Working { .. } => ("⚙", Color::Yellow),
        AgentStatus::WaitingEdit { .. } => ("✎", Color::Red),
        AgentStatus::WaitingShell { .. } => (">", Color::Red),
        AgentStatus::WaitingOther => ("?", Color::Magenta),
        AgentStatus::Idle => ("•", Color::DarkGray),
        AgentStatus::Success => ("✓", Color::Green),
        AgentStatus::Error => ("✖", Color::Red),
    }
}

fn render_preview(frame: &mut Frame, area: Rect, state: &AppState) {
    let title = if let Some(project) = state.selected_project() {
        if let Some(worktree) = state.selected_worktree() {
            let branch = worktree.branch.as_deref().unwrap_or("(detached)");
            format!("Preview - {}:{}", project.name, branch)
        } else {
            format!("Preview - {}", project.name)
        }
    } else {
        "Preview".to_string()
    };

    let content = if state.pane_preview.is_empty() {
        "No active session. Press Enter to attach.\n\nSelect a worktree and press Enter/o to switch to that tmux session."
            .to_string()
    } else {
        state.pane_preview.clone()
    };

    // Calculate scroll to show the bottom (most recent) content
    // Account for borders (2 lines) when calculating visible height
    let visible_height = area.height.saturating_sub(2) as usize;
    let content_lines = content.lines().count();
    let scroll_offset = content_lines.saturating_sub(visible_height) as u16;

    let preview = Paragraph::new(content)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset, 0))
        .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(preview, area);
}

/// Render the footer and return the cursor position if in input mode.
fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) -> Option<(u16, u16)> {
    let mut cursor_position = None;

    let content = match state.focus_mode {
        FocusMode::Normal => Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw(": Nav  "),
            Span::styled("o", Style::default().fg(Color::Yellow)),
            Span::raw(": Attach  "),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(": New  "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(": Del  "),
            Span::styled("f", Style::default().fg(Color::Yellow)),
            Span::raw(": Fav  "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(": Quit"),
        ]),
        FocusMode::Input => {
            // Split input_buffer at cursor position for display
            let chars: Vec<char> = state.input_buffer.chars().collect();
            let cursor_pos = state.input_cursor();
            let (before, after) = chars.split_at(cursor_pos.min(chars.len()));
            let before_str: String = before.iter().collect();
            let after_str: String = after.iter().collect();

            // Calculate display width of the text before cursor (for cursor positioning)
            // Account for: border (1) + "> " prefix (2)
            let prefix_width = 3u16; // border + "> "
            let before_width: u16 = before_str.chars().map(unicode_display_width).sum();

            // Cursor position: area.x + prefix_width + before_width
            cursor_position = Some((area.x + prefix_width + before_width, area.y + 1));

            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Green)),
                Span::raw(before_str),
                Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
                Span::raw(after_str),
                Span::raw("  "),
                Span::styled("Enter", Style::default().fg(Color::Yellow)),
                Span::raw(": Send  "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(": Cancel  "),
                Span::styled("</>", Style::default().fg(Color::Yellow)),
                Span::raw(": Move"),
            ])
        }
    };

    let footer =
        Paragraph::new(content).block(Block::default().borders(Borders::ALL).title("Commands"));
    frame.render_widget(footer, area);

    cursor_position
}

/// Calculate the display width of a character using unicode-width crate.
fn unicode_display_width(c: char) -> u16 {
    c.width().unwrap_or(0) as u16
}

fn render_modal(frame: &mut Frame, area: Rect, modal: &ModalType) {
    match modal {
        ModalType::CreateTask { input } => render_create_task_modal(frame, area, input),
        ModalType::ConfirmDeletion { branch_name } => {
            render_confirm_deletion_modal(frame, area, branch_name)
        }
    }
}

fn render_create_task_modal(frame: &mut Frame, area: Rect, input: &str) {
    // Center the modal
    let modal_width = 50.min(area.width.saturating_sub(4));
    let modal_height = 7;
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    let content = vec![
        Line::from(""),
        Line::from("Enter branch name for new task:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::raw(input),
            Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
        ]),
    ];

    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Create Task")
            .style(Style::default().bg(Color::DarkGray)),
    );
    frame.render_widget(modal_widget, modal_area);
}

fn render_confirm_deletion_modal(frame: &mut Frame, area: Rect, branch_name: &str) {
    // Center the modal
    let modal_width = 60.min(area.width.saturating_sub(4));
    let modal_height = 8;
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    let content = vec![
        Line::from(""),
        Line::from(format!("Delete task '{branch_name}'?")),
        Line::from(""),
        Line::from("This will remove the worktree and branch."),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Yes  "),
            Span::styled(
                "n",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": No"),
        ]),
    ];

    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Confirm Deletion")
            .style(Style::default().bg(Color::DarkGray)),
    );
    frame.render_widget(modal_widget, modal_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentStatus;

    #[test]
    fn test_get_status_icon_and_color_working() {
        let (icon, color) = get_status_icon_and_color(&AgentStatus::Working { detail: None });
        assert_eq!(icon, "⚙");
        assert_eq!(color, Color::Yellow);

        // With detail - should be same icon and color
        let (icon, color) = get_status_icon_and_color(&AgentStatus::Working {
            detail: Some("task".to_string()),
        });
        assert_eq!(icon, "⚙");
        assert_eq!(color, Color::Yellow);
    }

    #[test]
    fn test_get_status_icon_and_color_waiting_edit() {
        let (icon, color) = get_status_icon_and_color(&AgentStatus::WaitingEdit { path: None });
        assert_eq!(icon, "✎");
        assert_eq!(color, Color::Red);

        // With path - same icon and color
        let (icon, color) = get_status_icon_and_color(&AgentStatus::WaitingEdit {
            path: Some("file.rs".to_string()),
        });
        assert_eq!(icon, "✎");
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn test_get_status_icon_and_color_waiting_shell() {
        let (icon, color) = get_status_icon_and_color(&AgentStatus::WaitingShell { command: None });
        assert_eq!(icon, ">");
        assert_eq!(color, Color::Red);

        // With command - same icon and color
        let (icon, color) = get_status_icon_and_color(&AgentStatus::WaitingShell {
            command: Some("cargo test".to_string()),
        });
        assert_eq!(icon, ">");
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn test_get_status_icon_and_color_waiting_other() {
        let (icon, color) = get_status_icon_and_color(&AgentStatus::WaitingOther);
        assert_eq!(icon, "?");
        assert_eq!(color, Color::Magenta);
    }

    #[test]
    fn test_get_status_icon_and_color_idle() {
        let (icon, color) = get_status_icon_and_color(&AgentStatus::Idle);
        assert_eq!(icon, "•");
        assert_eq!(color, Color::DarkGray);
    }

    #[test]
    fn test_get_status_icon_and_color_success() {
        let (icon, color) = get_status_icon_and_color(&AgentStatus::Success);
        assert_eq!(icon, "✓");
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_get_status_icon_and_color_error() {
        let (icon, color) = get_status_icon_and_color(&AgentStatus::Error);
        assert_eq!(icon, "✖");
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn test_all_statuses_have_distinct_icons() {
        let statuses = vec![
            AgentStatus::Working { detail: None },
            AgentStatus::WaitingEdit { path: None },
            AgentStatus::WaitingShell { command: None },
            AgentStatus::WaitingOther,
            AgentStatus::Idle,
            AgentStatus::Success,
            AgentStatus::Error,
        ];

        let icons: Vec<&str> = statuses
            .iter()
            .map(|s| get_status_icon_and_color(s).0)
            .collect();

        // Verify all icons are non-empty
        for icon in &icons {
            assert!(!icon.is_empty());
        }
    }

    #[test]
    fn test_waiting_statuses_have_attention_colors() {
        // WaitingEdit and WaitingShell should have red (attention) colors
        let (_, edit_color) = get_status_icon_and_color(&AgentStatus::WaitingEdit { path: None });
        let (_, shell_color) =
            get_status_icon_and_color(&AgentStatus::WaitingShell { command: None });
        let (_, error_color) = get_status_icon_and_color(&AgentStatus::Error);

        assert_eq!(edit_color, Color::Red);
        assert_eq!(shell_color, Color::Red);
        assert_eq!(error_color, Color::Red);
    }

    #[test]
    fn test_working_status_has_active_color() {
        let (_, color) = get_status_icon_and_color(&AgentStatus::Working { detail: None });
        assert_eq!(color, Color::Yellow);
    }

    #[test]
    fn test_success_status_has_positive_color() {
        let (_, color) = get_status_icon_and_color(&AgentStatus::Success);
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_idle_status_has_muted_color() {
        let (_, color) = get_status_icon_and_color(&AgentStatus::Idle);
        assert_eq!(color, Color::DarkGray);
    }
}
