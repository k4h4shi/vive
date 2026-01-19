//! Vive - Entry point for the TUI application.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::Event,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;

use vive::{
    App, Config, EventSource, ProductionApp, RealEventSource, RealProjectDiscovery, event::Action,
    tmux::TmuxOrchestrator,
};

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    // Load configuration
    let config = Config::load().unwrap_or_default();

    // Create the app
    let mut app: ProductionApp<io::Stdout> = App::new(
        terminal,
        RealEventSource,
        TmuxOrchestrator::new(),
        RealProjectDiscovery,
        config,
    );

    // Run the application
    let result = run_with_terminal_control(&mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(app.terminal_mut().backend_mut(), LeaveAlternateScreen)?;
    app.terminal_mut().show_cursor()?;

    result
}

/// Run the app with special handling for AttachSession which needs terminal control.
fn run_with_terminal_control<W: Write>(app: &mut ProductionApp<W>) -> Result<()> {
    app.init()?;

    loop {
        app.render()?;

        // Poll for events
        if let Some(Event::Key(key)) = app.event_source.poll(Duration::from_millis(100))? {
            let action = app.handle_key_event(key);

            // Handle AttachSession specially since it requires terminal control
            if action == Action::AttachSession {
                handle_attach_session(app)?;
            } else {
                app.handle_action(action)?;
            }
        }

        // Periodic preview update
        if app.last_preview_update.elapsed() >= app.preview_update_interval {
            app.update_pane_preview();
            app.last_preview_update = Instant::now();
        }

        if app.state().should_quit() {
            break;
        }
    }

    Ok(())
}

/// Handle the AttachSession action which requires special terminal handling.
fn handle_attach_session<W: Write>(app: &mut ProductionApp<W>) -> Result<()> {
    if let Some(project) = app.state().selected_project() {
        // Session info resolution with fallback chain:
        // 1. Try selected worktree -> use its session_id (project__branch format)
        // 2. Fall back to default worktree (main/master) -> use its session_id
        // 3. Final fallback: use project name as session_id with project root path
        let session_info = app
            .state()
            .selected_worktree()
            .and_then(|wt| wt.session_id(&project.name).map(|id| (id, wt.path.clone())))
            .or_else(|| {
                // Fallback to default worktree (main or master branch)
                project
                    .default_worktree()
                    .and_then(|wt| wt.session_id(&project.name).map(|id| (id, wt.path.clone())))
            })
            .or_else(|| {
                // Final fallback: use project name as session with project root
                Some((project.name.clone(), project.path.clone()))
            });

        if let Some((session_id, worktree_path)) = session_info {
            let worktree_path_str = worktree_path.to_string_lossy();
            let _ = app
                .tmux
                .ensure_session(&session_id, Some(&worktree_path_str));

            // Restore terminal before exec
            disable_raw_mode()?;
            execute!(app.terminal_mut().backend_mut(), LeaveAlternateScreen)?;
            app.terminal_mut().show_cursor()?;

            // This will replace the current process
            let _ = app.tmux.exec_into_session(&session_id);

            // If we get here, exec failed - restore terminal state
            enable_raw_mode()?;
            execute!(io::stdout(), EnterAlternateScreen)?;
        }
    } else {
        app.state_mut().set_error_message("No project selected");
    }

    Ok(())
}
