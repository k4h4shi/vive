use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthChar;

use crate::state::{
    AppState, CreateTaskMethod, FocusMode, FocusPane, ModalType, StatusMessageType,
};

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

    // Cache layout info in state for mouse event handling
    state.set_sidebar_width(content_chunks[0].width);

    render_sidebar(frame, content_chunks[0], state);
    render_preview(frame, content_chunks[1], state);
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    // Build list items first (immutable borrow of state)
    let items = build_sidebar_items(state);

    // Border style based on focus
    let border_style = if state.focus_pane() == FocusPane::Sidebar {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let sidebar = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Projects")
                .border_style(border_style),
        )
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
        let is_expanded = state.is_expanded(&project.name);

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

        // Project header with expand/collapse indicator and ★ favorite indicator
        // ▼ for expanded, ▶ for collapsed
        let expand_indicator = if is_expanded { "▼" } else { "▶" };
        let favorite_indicator = if is_favorite { " (★)" } else { "" };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                format!("{expand_indicator} "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(project.name.clone(), project_style),
            Span::styled(
                favorite_indicator.to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ])));

        // Only show worktrees if project is expanded
        if is_expanded {
            // Worktrees under the project with tree structure
            let worktree_count = project.worktrees.len();
            for (wt_idx, worktree) in project.worktrees.iter().enumerate() {
                let is_selected =
                    is_selected_project && state.selected_worktree_idx() == Some(wt_idx);
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

fn render_preview(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let title = if let Some(project) = state.selected_project() {
        if let Some(worktree) = state.selected_worktree() {
            let branch = worktree.branch.as_deref().unwrap_or("(detached)");
            // Try to get Issue title if available
            let issue_title = worktree.issue_number().and_then(|num| {
                let repo_path = project.path.to_string_lossy();
                state.get_cached_issue_title(&repo_path, num)
            });
            if let Some(title) = issue_title {
                format!("{} - {} ({})", project.name, title, branch)
            } else {
                format!("{} - {}", project.name, branch)
            }
        } else {
            format!("Dashboard - {}", project.name)
        }
    } else {
        "Preview".to_string()
    };

    // Dashboard mode: show split view with panes
    if state.is_dashboard_mode() && !state.dashboard_panes.is_empty() {
        render_dashboard_preview(frame, area, state, &title);
        return;
    }

    let (text, line_count) = if state.pane_preview.is_empty() {
        let msg = "No active session. Press Enter to attach.\n\nSelect a worktree and press Enter/o to switch to that tmux session.";
        (Text::raw(msg), msg.lines().count())
    } else {
        // Clone pane_preview to avoid borrow conflicts
        let preview_content = state.pane_preview.clone();
        // Parse ANSI escape sequences to preserve Claude Code colors
        let text = preview_content
            .as_bytes()
            .into_text()
            .unwrap_or_else(|_| Text::raw(preview_content.clone()));
        let line_count = text.lines.len();
        (text, line_count)
    };

    // Update line count in state for scroll calculations
    state.set_preview_line_count(line_count);

    // Calculate scroll offset
    // Account for borders (2 lines) when calculating visible height
    let visible_height = area.height.saturating_sub(2);

    // Cache visible height in state for scroll calculations in event handling
    state.set_preview_visible_height(visible_height);

    let visible_height = visible_height as usize;

    // When sidebar is focused, auto-scroll to bottom (show most recent content)
    // When preview is focused, use user's scroll offset
    let scroll_offset = if state.is_preview_focused() {
        // Use user's scroll offset, but ensure it's valid
        let max_scroll = line_count.saturating_sub(visible_height) as u16;
        let user_offset = state.preview_scroll_offset();
        if user_offset == u16::MAX {
            // User requested to go to bottom
            max_scroll
        } else {
            user_offset.min(max_scroll)
        }
    } else {
        // Auto-scroll to bottom when sidebar is focused
        line_count.saturating_sub(visible_height) as u16
    };

    // Border style based on focus
    let border_style = if state.focus_pane() == FocusPane::Preview {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Issue #65: Disable wrap to fix scroll calculation.
    // With Wrap enabled, long lines become multiple screen lines, causing
    // scroll_offset (based on logical lines) to be incorrect.
    // Long lines will be truncated at the preview edge instead of wrapping.
    let preview = Paragraph::new(text).scroll((scroll_offset, 0)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    frame.render_widget(preview, area);
}

/// Render split dashboard preview with summary and pane contents.
fn render_dashboard_preview(frame: &mut Frame, area: Rect, state: &AppState, title: &str) {
    let pane_count = state.dashboard_panes.len();

    // Split into summary area and panes area
    let chunks = Layout::vertical([
        Constraint::Length(4), // Summary (with border)
        Constraint::Min(0),    // Panes
    ])
    .split(area);

    // Render summary
    let summary_content =
        format!("Active panes: {pane_count}  |  Press Enter to attach to dashboard");
    let summary = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(summary_content, Style::default().fg(Color::Cyan)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_string()),
    );
    frame.render_widget(summary, chunks[0]);

    // Render panes in a grid layout
    if pane_count == 0 {
        return;
    }

    // Determine layout: horizontal split for 2-4 panes
    let pane_area = chunks[1];
    let constraints: Vec<Constraint> = state
        .dashboard_panes
        .iter()
        .map(|_| Constraint::Ratio(1, pane_count as u32))
        .collect();

    let pane_chunks = Layout::horizontal(constraints).split(pane_area);

    // Get project info for Issue title lookup
    let project = state.selected_project();

    for ((branch_name, content), chunk) in state.dashboard_panes.iter().zip(pane_chunks.iter()) {
        // Parse ANSI escape sequences to preserve Claude Code colors
        let text = content
            .as_bytes()
            .into_text()
            .unwrap_or_else(|_| Text::raw(content));
        let line_count = text.lines.len();

        // Calculate scroll to show bottom content
        let visible_height = chunk.height.saturating_sub(2) as usize;
        let scroll_offset = line_count.saturating_sub(visible_height) as u16;

        // Build pane title with Issue title if available
        let pane_title = if let Some(proj) = project {
            // Find worktree by branch name and get Issue title
            let issue_title = proj
                .worktrees
                .iter()
                .find(|wt| wt.branch.as_deref() == Some(branch_name.as_str()))
                .and_then(|wt| wt.issue_number())
                .and_then(|num| {
                    let repo_path = proj.path.to_string_lossy();
                    state.get_cached_issue_title(&repo_path, num)
                });
            if let Some(title) = issue_title {
                format!("{title} ({branch_name})")
            } else {
                branch_name.clone()
            }
        } else {
            branch_name.clone()
        };

        // Issue #65: Disable wrap for consistent scroll calculation
        let pane_widget = Paragraph::new(text)
            .scroll((scroll_offset, 0))
            .block(Block::default().borders(Borders::ALL).title(pane_title));
        frame.render_widget(pane_widget, *chunk);
    }
}

/// Render the footer and return the cursor position if in input mode.
fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) -> Option<(u16, u16)> {
    let mut cursor_position = None;

    let content = match state.focus_mode {
        FocusMode::Normal => {
            // Show different help based on which pane is focused
            match state.focus_pane() {
                FocusPane::Sidebar => Line::from(vec![
                    Span::styled("j/k", Style::default().fg(Color::Yellow)),
                    Span::raw(": Nav  "),
                    Span::styled("Tab", Style::default().fg(Color::Yellow)),
                    Span::raw(": Switch  "),
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
                FocusPane::Preview => Line::from(vec![
                    Span::styled("j/k", Style::default().fg(Color::Yellow)),
                    Span::raw(": Scroll  "),
                    Span::styled("^d/^u", Style::default().fg(Color::Yellow)),
                    Span::raw(": Page  "),
                    Span::styled("g/G", Style::default().fg(Color::Yellow)),
                    Span::raw(": Top/Bot  "),
                    Span::styled("Tab", Style::default().fg(Color::Yellow)),
                    Span::raw(": Switch  "),
                    Span::styled("o", Style::default().fg(Color::Yellow)),
                    Span::raw(": Attach  "),
                    Span::styled("q", Style::default().fg(Color::Yellow)),
                    Span::raw(": Quit"),
                ]),
            }
        }
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
        ModalType::CreateTaskMethod {
            selected,
            auto_kickstart,
        } => render_create_task_method_modal(frame, area, *selected, *auto_kickstart),
        ModalType::CreateTask {
            input,
            auto_kickstart,
        } => render_create_task_modal(frame, area, input, *auto_kickstart),
        ModalType::IssuePicker {
            issues,
            selected_idx,
            filter,
            error,
            loading,
            auto_kickstart,
        } => render_issue_picker_modal(
            frame,
            area,
            issues,
            *selected_idx,
            filter,
            error,
            *loading,
            *auto_kickstart,
        ),
        ModalType::ConfirmDeletion { branch_name } => {
            render_confirm_deletion_modal(frame, area, branch_name)
        }
    }
}

fn render_create_task_method_modal(
    frame: &mut Frame,
    area: Rect,
    selected: CreateTaskMethod,
    auto_kickstart: bool,
) {
    // Center the modal
    let modal_width = 55.min(area.width.saturating_sub(4));
    let modal_height = 12;
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    let manual_style = if selected == CreateTaskMethod::Manual {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(Color::White)
    };

    let issue_style = if selected == CreateTaskMethod::PickFromIssue {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(Color::White)
    };

    let checkbox = if auto_kickstart { "[x]" } else { "[ ]" };
    let checkbox_style = Style::default().fg(Color::Cyan);

    let content = vec![
        Line::from(""),
        Line::from("How would you like to create a task?"),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("[M] Manual - Enter branch name", manual_style),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("[I] Pick from Issue", issue_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(checkbox, checkbox_style),
            Span::styled(
                " Auto-Kickstart (Tab to toggle)",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::DarkGray)),
            Span::styled(": Select  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(Color::DarkGray)),
            Span::styled(": Toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::DarkGray)),
            Span::styled(": Confirm  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::styled(": Cancel", Style::default().fg(Color::DarkGray)),
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

#[allow(clippy::too_many_arguments)]
fn render_issue_picker_modal(
    frame: &mut Frame,
    area: Rect,
    issues: &[crate::github::GitHubIssue],
    selected_idx: usize,
    filter: &str,
    error: &Option<String>,
    loading: bool,
    auto_kickstart: bool,
) {
    // Center the modal
    let modal_width = 70.min(area.width.saturating_sub(4));
    let modal_height = 22.min(area.height.saturating_sub(4));
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    let mut content: Vec<Line> = Vec::new();

    // Auto-kickstart checkbox
    let checkbox = if auto_kickstart { "[x]" } else { "[ ]" };
    content.push(Line::from(vec![
        Span::styled(checkbox, Style::default().fg(Color::Cyan)),
        Span::styled(" Auto-Kickstart (Tab)", Style::default().fg(Color::White)),
    ]));
    content.push(Line::from(""));

    // Filter input line
    content.push(Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(Color::Cyan)),
        Span::raw(filter),
        Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
    ]));
    content.push(Line::from(""));

    if loading {
        content.push(Line::from(Span::styled(
            "Loading issues...",
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(err) = error {
        content.push(Line::from(Span::styled(
            format!("Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else {
        // Filter issues
        let filter_lower = filter.to_lowercase();
        let filtered_issues: Vec<_> = if filter.is_empty() {
            issues.iter().collect()
        } else {
            issues
                .iter()
                .filter(|issue| {
                    issue.title.to_lowercase().contains(&filter_lower)
                        || issue.number.to_string().contains(&filter_lower)
                })
                .collect()
        };

        if filtered_issues.is_empty() {
            content.push(Line::from(Span::styled(
                "No issues found",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            // Calculate visible window (max ~12 items)
            let max_visible = (modal_height as usize).saturating_sub(6);
            let total = filtered_issues.len();

            let (start, end) = if total <= max_visible {
                (0, total)
            } else {
                // Center the selected item in the visible window
                let half_visible = max_visible / 2;
                let start = selected_idx.saturating_sub(half_visible);
                let end = (start + max_visible).min(total);
                let start = if end == total {
                    total.saturating_sub(max_visible)
                } else {
                    start
                };
                (start, end)
            };

            // Show scroll indicator if needed
            if start > 0 {
                content.push(Line::from(Span::styled(
                    "  ↑ more issues above",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            for (display_idx, issue) in filtered_issues[start..end].iter().enumerate() {
                let actual_idx = start + display_idx;
                let is_selected = actual_idx == selected_idx;

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::White)
                };

                // Truncate title if too long
                // Use saturating_sub to prevent underflow on narrow terminals
                let max_title_len = (modal_width as usize).saturating_sub(12);
                let title = if max_title_len <= 3 {
                    // Too narrow to show any title, just show ellipsis
                    "...".to_string()
                } else if issue.title.chars().count() > max_title_len {
                    let truncated: String = issue
                        .title
                        .chars()
                        .take(max_title_len.saturating_sub(3))
                        .collect();
                    format!("{truncated}...")
                } else {
                    issue.title.clone()
                };

                content.push(Line::from(vec![
                    Span::styled(
                        if is_selected { "> " } else { "  " },
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(
                        format!("#{:<4} ", issue.number),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(title, style),
                ]));
            }

            // Show scroll indicator if needed
            if end < total {
                content.push(Line::from(Span::styled(
                    "  ↓ more issues below",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }

    // Help line at the bottom
    content.push(Line::from(""));
    content.push(Line::from(vec![
        Span::styled("j/k", Style::default().fg(Color::DarkGray)),
        Span::styled(": Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tab", Style::default().fg(Color::DarkGray)),
        Span::styled(": Toggle  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::styled(": Select  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::styled(": Cancel", Style::default().fg(Color::DarkGray)),
    ]));

    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Pick from Issue")
            .style(Style::default().bg(Color::DarkGray)),
    );
    frame.render_widget(modal_widget, modal_area);
}

fn render_create_task_modal(frame: &mut Frame, area: Rect, input: &str, auto_kickstart: bool) {
    // Center the modal
    let modal_width = 55.min(area.width.saturating_sub(4));
    let modal_height = 10;
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    let checkbox = if auto_kickstart { "[x]" } else { "[ ]" };

    let content = vec![
        Line::from(""),
        Line::from("Enter branch name for new task:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::raw(input),
            Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(checkbox, Style::default().fg(Color::Cyan)),
            Span::styled(
                " Auto-Kickstart (Tab to toggle)",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::DarkGray)),
            Span::styled(": Toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::DarkGray)),
            Span::styled(": Create  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::styled(": Cancel", Style::default().fg(Color::DarkGray)),
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

    // ========== Title Truncation Tests (Issue #55 / Codex P2 Fix) ==========

    /// Helper function to truncate issue title (mirrors logic in render_issue_picker_modal)
    fn truncate_issue_title(title: &str, modal_width: u16) -> String {
        let max_title_len = (modal_width as usize).saturating_sub(12);
        if max_title_len <= 3 {
            "...".to_string()
        } else if title.chars().count() > max_title_len {
            let truncated: String = title
                .chars()
                .take(max_title_len.saturating_sub(3))
                .collect();
            format!("{truncated}...")
        } else {
            title.to_string()
        }
    }

    #[test]
    fn test_title_truncation_normal_width() {
        // Normal width (70 chars) - title fits
        let title = "Short title";
        let result = truncate_issue_title(title, 70);
        assert_eq!(result, "Short title");
    }

    #[test]
    fn test_title_truncation_long_title() {
        // Normal width but long title - should truncate
        let title = "This is a very long issue title that exceeds the maximum length allowed";
        let result = truncate_issue_title(title, 50);
        // max_title_len = 50 - 12 = 38, truncate at 38 - 3 = 35
        assert!(result.ends_with("..."));
        assert!(result.len() < title.len());
    }

    #[test]
    fn test_title_truncation_narrow_width() {
        // Very narrow width (15 chars) - should not underflow
        let title = "Some title";
        let result = truncate_issue_title(title, 15);
        // max_title_len = 15 - 12 = 3, which is <= 3, so returns "..."
        assert_eq!(result, "...");
    }

    #[test]
    fn test_title_truncation_minimal_width() {
        // Minimal width (10 chars) - should not underflow
        let title = "Test";
        let result = truncate_issue_title(title, 10);
        // max_title_len = 10 - 12 = 0 (saturating), which is <= 3
        assert_eq!(result, "...");
    }

    #[test]
    fn test_title_truncation_zero_width() {
        // Zero width - should not panic
        let title = "Test";
        let result = truncate_issue_title(title, 0);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_title_truncation_exact_boundary() {
        // Width where max_title_len is exactly 4 (just above the guard)
        // modal_width = 16, max_title_len = 16 - 12 = 4
        let title = "ABCD"; // 4 chars, exactly max_title_len
        let result = truncate_issue_title(title, 16);
        assert_eq!(result, "ABCD"); // No truncation needed

        // 5 chars with max_title_len = 4 should truncate
        let title = "ABCDE";
        let result = truncate_issue_title(title, 16);
        // truncate at 4 - 3 = 1, so "A..."
        assert_eq!(result, "A...");
    }

    #[test]
    fn test_title_truncation_utf8() {
        // UTF-8 characters (Japanese)
        let title = "日本語のイシュータイトル";
        let result = truncate_issue_title(title, 50);
        // max_title_len = 38, title is 12 chars, no truncation
        assert_eq!(result, "日本語のイシュータイトル");

        // With narrow width forcing truncation (width = 22 -> max_title_len = 10)
        // Title has 12 chars, which exceeds 10, so truncation happens
        let result = truncate_issue_title(title, 22);
        // max_title_len = 10, truncate at 10 - 3 = 7, so "日本語のイシュ..."
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 10); // 7 chars + "..."
    }
}
