use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

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

    render_header(frame, chunks[0]);
    render_main(frame, chunks[1], state);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new("Vive TUI")
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Header"));
    frame.render_widget(header, area);
}

fn render_main(frame: &mut Frame, area: Rect, state: &AppState) {
    let text = vec![
        Line::from(vec![
            Span::raw("Counter: "),
            Span::styled(
                state.counter().to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from("Press 'j' to increment, 'k' to decrement, 'q' to quit."),
    ];

    let main = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Main"));
    frame.render_widget(main, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new("Press 'q' to quit")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(footer, area);
}
