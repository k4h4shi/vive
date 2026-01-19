use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::state::{AppState, FocusMode, ModalType};

/// Render the UI based on the current application state.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Main layout: Header, Content, Footer
    let main_chunks = Layout::vertical([
        Constraint::Length(3),  // Header
        Constraint::Min(0),     // Content
        Constraint::Length(3),  // Footer/Input
    ])
    .split(area);

    render_header(frame, main_chunks[0]);
    render_content(frame, main_chunks[1], state);
    render_footer(frame, main_chunks[2], state);

    // Render modal on top if present
    if let Some(modal) = &state.modal {
        render_modal(frame, area, modal);
    }
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new("Vive - Claude Code Cockpit")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).title("Vive"));
    frame.render_widget(header, area);
}

fn render_content(frame: &mut Frame, area: Rect, state: &AppState) {
    // Split content into sidebar and preview
    let content_chunks = Layout::horizontal([
        Constraint::Percentage(30), // Sidebar
        Constraint::Percentage(70), // Preview
    ])
    .split(area);

    render_sidebar(frame, content_chunks[0], state);
    render_preview(frame, content_chunks[1], state);
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut items: Vec<ListItem> = Vec::new();

    for (proj_idx, project) in state.projects.iter().enumerate() {
        // Project header
        let is_selected_project = state.selected_project_idx() == Some(proj_idx);
        let project_style = if is_selected_project && state.selected_worktree_idx().is_none() {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{} ", project.name), project_style),
        ])));

        // Worktrees under the project
        for (wt_idx, worktree) in project.worktrees.iter().enumerate() {
            let is_selected = is_selected_project && state.selected_worktree_idx() == Some(wt_idx);
            let branch_name = worktree
                .branch
                .as_deref()
                .unwrap_or("(detached)");

            // Get status for this worktree
            let status = worktree
                .session_id(&project.name)
                .map(|sid| state.get_status(&sid))
                .unwrap_or_default();

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::Gray)
            };

            items.push(ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::raw(status.icon()),
                Span::raw(" "),
                Span::styled(branch_name, style),
            ])));
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "No projects found",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    let sidebar = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Projects"));
    frame.render_widget(sidebar, area);
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

    let preview = Paragraph::new(content)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(preview, area);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) {
    let content = match state.focus_mode {
        FocusMode::Normal => {
            Line::from(vec![
                Span::styled("j/k", Style::default().fg(Color::Yellow)),
                Span::raw(": Navigate  "),
                Span::styled("Enter/o", Style::default().fg(Color::Yellow)),
                Span::raw(": Attach  "),
                Span::styled("i", Style::default().fg(Color::Yellow)),
                Span::raw(": Input  "),
                Span::styled("n", Style::default().fg(Color::Yellow)),
                Span::raw(": New Task  "),
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(": Quit"),
            ])
        }
        FocusMode::Input => {
            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Green)),
                Span::raw(&state.input_buffer),
                Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
                Span::raw("  "),
                Span::styled("Enter", Style::default().fg(Color::Yellow)),
                Span::raw(": Send  "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(": Cancel"),
            ])
        }
    };

    let footer = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Commands"));
    frame.render_widget(footer, area);
}

fn render_modal(frame: &mut Frame, area: Rect, modal: &ModalType) {
    let ModalType::CreateTask { input } = modal;

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
            Span::raw(input.as_str()),
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
