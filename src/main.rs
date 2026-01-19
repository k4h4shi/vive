mod discovery;
mod event;
mod process;
mod state;
mod tmux;
mod ui;

use std::{
    io,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::Event,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::process::{ProcessMonitor, RealProcessDetector};
use crate::state::{AppState, WindowState};
use crate::tmux::{RealTmuxExecutor, TmuxOrchestrator};

/// Polling interval for updating agent states.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

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
    let tmux = TmuxOrchestrator::<RealTmuxExecutor>::new();
    let process_monitor = ProcessMonitor::<RealProcessDetector>::new();

    // Try to detect session name from current directory
    let session_name = detect_session_name();
    if let Some(name) = &session_name {
        state.set_session_name(name.clone());
    }

    let mut last_poll = Instant::now();

    // Initial refresh
    if let Some(name) = &session_name {
        refresh_windows(&mut state, name, &tmux, &process_monitor);
    }

    loop {
        terminal.draw(|frame| ui::render(frame, &state))?;

        if let Some(Event::Key(key)) = event::poll_event(Duration::from_millis(100))? {
            event::handle_key_event(key, &mut state);
        }

        // Periodic refresh of agent states
        if last_poll.elapsed() >= POLL_INTERVAL {
            if let Some(name) = &session_name {
                refresh_agent_states(&mut state, name, &process_monitor);
            }
            last_poll = Instant::now();
        }

        if state.should_quit() {
            break;
        }
    }

    Ok(())
}

/// Detect the session name from the current directory (project name).
fn detect_session_name() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
}

/// Refresh the window list and their states.
fn refresh_windows(
    state: &mut AppState,
    session_name: &str,
    tmux: &TmuxOrchestrator<RealTmuxExecutor>,
    process_monitor: &ProcessMonitor<RealProcessDetector>,
) {
    let windows = match tmux.list_windows(session_name) {
        Ok(windows) => windows,
        Err(_) => return,
    };

    let window_states: Vec<WindowState> = windows
        .into_iter()
        .map(|w| {
            let agent_state = process_monitor
                .get_agent_state(session_name, &w.name)
                .unwrap_or_default();
            let cpu = process_monitor
                .get_process_info(session_name, &w.name)
                .ok()
                .flatten()
                .map(|info| info.cpu);

            WindowState::new(w.name)
                .with_state(agent_state)
                .with_cpu(cpu.unwrap_or(0.0))
        })
        .collect();

    state.set_windows(window_states);
}

/// Refresh only the agent states (faster than full refresh).
fn refresh_agent_states(
    state: &mut AppState,
    session_name: &str,
    process_monitor: &ProcessMonitor<RealProcessDetector>,
) {
    // Collect window names first to avoid borrow conflicts
    let window_names: Vec<String> = state.windows().iter().map(|w| w.name.clone()).collect();

    for name in window_names {
        let agent_state = process_monitor
            .get_agent_state(session_name, &name)
            .unwrap_or_default();
        let cpu = process_monitor
            .get_process_info(session_name, &name)
            .ok()
            .flatten()
            .map(|info| info.cpu);

        state.update_window_state(&name, agent_state, cpu);
    }
}
