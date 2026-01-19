mod discovery;
mod event;
mod state;
mod ui;

use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::Event,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::state::AppState;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the application
    let result = run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut state = AppState::new();

    loop {
        terminal.draw(|frame| ui::render(frame, &state))?;

        if let Some(Event::Key(key)) = event::poll_event(Duration::from_millis(100))? {
            event::handle_key_event(key, &mut state);
        }

        if state.should_quit() {
            break;
        }
    }

    Ok(())
}
