use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::process::AgentState;
use crate::state::AppState;

/// Render the UI based on the current application state.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    render_header(frame, chunks[0], state);
    render_main(frame, chunks[1], state);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let title = match state.session_name() {
        Some(name) => format!("Vive - Session: {name}"),
        None => "Vive - No Session".to_string(),
    };
    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Header"));
    frame.render_widget(header, area);
}

fn render_main(frame: &mut Frame, area: Rect, state: &AppState) {
    let windows = state.windows();

    if windows.is_empty() {
        let text = vec![
            Line::from("No windows found."),
            Line::from(""),
            Line::from("Use 'vive start <issue>' to create a window."),
        ];
        let main =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Windows"));
        frame.render_widget(main, area);
        return;
    }

    let items: Vec<ListItem> = windows
        .iter()
        .enumerate()
        .map(|(i, window)| {
            let indicator = window.agent_state.indicator();
            let description = window.agent_state.description();

            let cpu_str = match window.cpu {
                Some(cpu) => format!("{cpu:.1}%"),
                None => "-".to_string(),
            };

            let content = Line::from(vec![
                Span::raw(format!("{indicator} ")),
                Span::styled(
                    format!("{:<20}", window.name),
                    state_to_style(window.agent_state),
                ),
                Span::raw(format!(" {description:<15} {cpu_str:>8}")),
            ]);

            let style = if i == state.selected_index() {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Windows"));
    frame.render_widget(list, area);
}

fn state_to_style(state: AgentState) -> Style {
    match state {
        AgentState::Working => Style::default().fg(Color::Green),
        AgentState::InputNeeded => Style::default().fg(Color::Yellow),
        AgentState::Stopped => Style::default().fg(Color::Red),
        AgentState::Idle => Style::default().fg(Color::Gray),
    }
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let legend = format!(
        "Legend: {} Working  {} Input Needed  {} Stopped  {} Idle  |  j/k: navigate  q: quit",
        AgentState::Working.indicator(),
        AgentState::InputNeeded.indicator(),
        AgentState::Stopped.indicator(),
        AgentState::Idle.indicator(),
    );
    let footer = Paragraph::new(legend)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(footer, area);
}
