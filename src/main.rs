mod discovery;
mod event;
mod state;
mod tmux;
mod ui;

use std::path::PathBuf;
use std::{env, io};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::Event,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::discovery::discover_projects;
use crate::event::Action;
use crate::state::AppState;
use crate::tmux::{TmuxOrchestrator, RealTmuxExecutor};

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
    // Determine project root (default to home directory)
    let projects_root = env::var("VIVE_PROJECTS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_home()
                .map(|h| h.join("src"))
                .unwrap_or_else(|| PathBuf::from("."))
        });

    let mut state = AppState::with_projects_root(projects_root.clone());

    // Discover projects at startup
    if let Ok(projects) = discover_projects(&projects_root) {
        state.set_projects(projects);
    }

    // Create tmux orchestrator
    let tmux = TmuxOrchestrator::new();

    // Track time for periodic updates
    let mut last_preview_update = Instant::now();
    let preview_update_interval = Duration::from_secs(2);

    loop {
        terminal.draw(|frame| ui::render(frame, &state))?;

        // Poll for events
        if let Some(Event::Key(key)) = event::poll_event(Duration::from_millis(100))? {
            let action = event::handle_key_event(key, &mut state);
            handle_action(action, &mut state, &tmux, terminal)?;
        }

        // Periodic preview update
        if last_preview_update.elapsed() >= preview_update_interval {
            update_pane_preview(&mut state, &tmux);
            last_preview_update = Instant::now();
        }

        if state.should_quit() {
            break;
        }
    }

    Ok(())
}

fn handle_action(
    action: Action,
    state: &mut AppState,
    tmux: &TmuxOrchestrator<RealTmuxExecutor>,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    match action {
        Action::None | Action::Quit => {}

        Action::AttachSession => {
            if let (Some(project), Some(worktree)) =
                (state.selected_project(), state.selected_worktree())
            {
                if let Some(session_id) = worktree.session_id(&project.name) {
                    // Ensure session exists before attaching
                    let worktree_path = worktree.path.to_string_lossy();
                    let _ = tmux.ensure_session(&session_id, Some(&worktree_path));

                    // Restore terminal before exec
                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                    terminal.show_cursor()?;

                    // This will replace the current process
                    let _ = tmux.exec_into_session(&session_id);

                    // If we get here, exec failed - restore terminal state
                    enable_raw_mode()?;
                    execute!(io::stdout(), EnterAlternateScreen)?;
                }
            }
        }

        Action::SendInput(input) => {
            if let (Some(project), Some(worktree)) =
                (state.selected_project(), state.selected_worktree())
            {
                if let Some(session_id) = worktree.session_id(&project.name) {
                    // Send keys to the session
                    let _ = tmux.send_keys(&session_id, &input, true);
                }
            }
        }

        Action::CreateTask(branch_name) => {
            if let Some(project) = state.selected_project().cloned() {
                // Create a new worktree using git
                let worktree_path = project.path.join(".worktrees").join(&branch_name);
                let result = std::process::Command::new("git")
                    .args([
                        "worktree",
                        "add",
                        "-b",
                        &branch_name,
                        &worktree_path.to_string_lossy(),
                    ])
                    .current_dir(&project.path)
                    .output();

                if result.is_ok() {
                    // Refresh project list
                    if let Ok(projects) = discover_projects(&state.projects_root) {
                        state.set_projects(projects);
                    }
                }
            }
        }

        Action::RefreshPreview => {
            update_pane_preview(state, tmux);
        }
    }

    Ok(())
}

fn update_pane_preview(state: &mut AppState, tmux: &TmuxOrchestrator<RealTmuxExecutor>) {
    if let (Some(project), Some(worktree)) = (state.selected_project(), state.selected_worktree()) {
        if let Some(session_id) = worktree.session_id(&project.name) {
            // Check if session exists and capture pane content
            if tmux.has_session(&session_id).unwrap_or(false) {
                if let Ok(content) = tmux.capture_pane(&session_id, 50) {
                    state.set_pane_preview(content);
                    return;
                }
            }
        }
    }
    // Clear preview if no valid session
    state.set_pane_preview(String::new());
}

/// Get the user's home directory.
fn dirs_home() -> Option<PathBuf> {
    env::var("HOME").ok().map(PathBuf::from)
}
