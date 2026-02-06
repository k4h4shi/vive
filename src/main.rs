//! Vive - Entry point for the TUI application.

use std::io::{self, Write};
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;

use vive::{
    App, Config, EventSource, GhIssueFetcher, LaunchStrategy, ProductionApp, RealEventSource,
    RealProjectDiscovery,
    event::Action,
    mcp::{ViveStateSnapshot, run_mcp_server},
    tmux::TmuxOrchestrator,
};

fn main() -> Result<()> {
    // Check for flags
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--mcp-server") {
        return run_mcp_server_mode();
    }

    let list_only = args.iter().any(|arg| arg == "--list-only" || arg == "-l");

    // Normal TUI mode
    run_tui_mode(list_only)
}

/// Run the application in MCP server mode.
///
/// In this mode, the application exposes Vive's state via the Model Context Protocol
/// over stdio. This allows external tools like Claude Code to query Vive's state.
fn run_mcp_server_mode() -> Result<()> {
    // Create the tokio runtime for the MCP server
    let rt = tokio::runtime::Runtime::new()?;

    // Create a shared state that will be populated with real data
    let shared_state = Arc::new(RwLock::new(ViveStateSnapshot::default()));

    // Discover projects and populate the state
    let config = Config::load().unwrap_or_default();
    let projects_root = config.effective_projects_root();
    if let Ok(projects) = vive::discover_projects(&projects_root, &config.ignored_dirs) {
        let mut state = shared_state.write().unwrap();
        state.projects = projects
            .iter()
            .map(vive::mcp::ProjectSnapshot::from)
            .collect();
    }

    // Run the MCP server
    rt.block_on(run_mcp_server(shared_state))?;

    Ok(())
}

/// Run the application in normal TUI mode.
fn run_tui_mode(list_only: bool) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
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
        GhIssueFetcher,
        config.clone(),
    );

    // Apply list-only mode
    if list_only {
        app.state_mut().set_list_only(true);
    }

    // Run the application
    let result = run_with_terminal_control(&mut app, &config);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        app.terminal_mut().backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    app.terminal_mut().show_cursor()?;

    result
}

/// Run the app with special handling for AttachSession which needs terminal control.
fn run_with_terminal_control<W: Write>(app: &mut ProductionApp<W>, config: &Config) -> Result<()> {
    app.init()?;

    loop {
        app.render()?;

        // Poll for events
        if let Some(event) = app.event_source.poll(Duration::from_millis(100))? {
            let action = match event {
                Event::Key(key) => app.handle_key_event(key),
                Event::Mouse(mouse) => app.handle_mouse_event(mouse),
                _ => Action::None,
            };

            // Handle AttachSession specially since it requires terminal control
            if let Action::AttachSession(key) = action {
                handle_attach_session(app, config, &key)?;
            } else {
                app.handle_action(action)?;
            }
        }

        // Periodic preview and status update
        if app.last_preview_update.elapsed() >= app.preview_update_interval {
            app.update_pane_preview();
            app.update_all_statuses();
            app.last_preview_update = Instant::now();
        }

        if app.state().should_quit() {
            break;
        }
    }

    Ok(())
}

/// Handle the AttachSession action which requires special terminal handling.
///
/// The `key` parameter specifies which key triggered the action (e.g., "enter", "o").
/// If a custom keybinding is configured for that key, the command is executed.
/// Otherwise, falls back to legacy terminal config behavior.
fn handle_attach_session<W: Write>(
    app: &mut ProductionApp<W>,
    config: &Config,
    key: &str,
) -> Result<()> {
    if let Some(project) = app.state().selected_project() {
        let project_name = project.name.clone();

        // Check if we're on project header (no worktree selected) -> dashboard mode
        if app.state().selected_worktree_idx().is_none() {
            // Project-level selection: create/attach to dashboard session
            handle_dashboard_attach(app, &project_name, config, key)?;
        } else {
            // Worktree-level selection: attach to specific worktree session
            handle_worktree_attach(app, &project_name, config, key)?;
        }
    } else {
        app.state_mut().set_error_message("No project selected");
    }

    Ok(())
}

/// Handle attaching to a dashboard session for project-level selection.
fn handle_dashboard_attach<W: Write>(
    app: &mut ProductionApp<W>,
    project_name: &str,
    config: &Config,
    key: &str,
) -> Result<()> {
    use vive::tmux::TmuxOrchestrator;

    let project = app
        .state()
        .selected_project()
        .expect("Project should exist");

    // Collect all worktree window info
    let worktree_windows: Vec<(String, String)> = project
        .worktrees
        .iter()
        .filter_map(|wt| {
            wt.window_name()
                .map(|name| (name, wt.path.to_string_lossy().to_string()))
        })
        .collect();

    if worktree_windows.is_empty() {
        let project_path = project.path.to_string_lossy().to_string();
        let _ = app.tmux.ensure_session(project_name, Some(&project_path));
        execute_launch(app, config, project_name, Some(&project_path), key)?;
        return Ok(());
    }

    // Ensure project session exists (use project root as start directory)
    let project_path = project.path.to_string_lossy().to_string();
    let _ = app.tmux.ensure_session(project_name, Some(&project_path));

    // Ensure all worktree windows exist
    for (window_name, worktree_path) in &worktree_windows {
        let _ = app.tmux.ensure_session_with_window(
            project_name,
            window_name,
            Some(worktree_path),
            None,
        );
    }

    // Create or refresh dashboard session (ensures panes are properly attached)
    let dashboard_session =
        TmuxOrchestrator::<vive::tmux::RealTmuxExecutor>::dashboard_session_name(project_name);
    let _ = app
        .tmux
        .ensure_dashboard_session(project_name, &worktree_windows);

    // For dashboard, path is the project root
    execute_launch(app, config, &dashboard_session, Some(&project_path), key)?;

    Ok(())
}

/// Handle attaching to a specific worktree session.
fn handle_worktree_attach<W: Write>(
    app: &mut ProductionApp<W>,
    project_name: &str,
    config: &Config,
    key: &str,
) -> Result<()> {
    let project = app
        .state()
        .selected_project()
        .expect("Project should exist");

    // Window resolution with fallback chain:
    // 1. Try selected worktree -> use its window name
    // 2. Fall back to default worktree (main/master) -> use its window name
    // 3. Final fallback: attach to project session with project root
    let window_info = app
        .state()
        .selected_worktree()
        .and_then(|wt| wt.window_name().map(|name| (name, wt.path.clone())))
        .or_else(|| {
            project
                .default_worktree()
                .and_then(|wt| wt.window_name().map(|name| (name, wt.path.clone())))
        });

    let session_name = project_name.to_string();
    if let Some((window_name, worktree_path)) = window_info {
        let worktree_path_str = worktree_path.to_string_lossy().to_string();
        let _ = app.tmux.ensure_session_with_window(
            &session_name,
            &window_name,
            Some(&worktree_path_str),
            None,
        );

        let target = format!("{session_name}:{window_name}");
        execute_launch(app, config, &target, Some(&worktree_path_str), key)?;
    } else {
        let project_path = project.path.to_string_lossy().to_string();
        let _ = app.tmux.ensure_session(&session_name, Some(&project_path));
        execute_launch(app, config, &session_name, Some(&project_path), key)?;
    }

    Ok(())
}

/// Execute the launch strategy for attaching to a tmux session.
///
/// This helper function first checks for custom keybindings, then falls back
/// to legacy terminal config:
/// - **Custom keybinding**: Execute the configured shell command
/// - **Inline** (legacy): Suspends TUI and replaces the process with tmux attach
/// - **Spawn** (legacy): Launches external terminal without suspending TUI
fn execute_launch<W: Write>(
    app: &mut ProductionApp<W>,
    config: &Config,
    session_id: &str,
    worktree_path: Option<&str>,
    key: &str,
) -> Result<()> {
    // Check for custom keybinding first
    if let Some(command) =
        config.build_keybinding_command_with_placeholders(key, session_id, worktree_path)
    {
        return execute_keybinding_command(app, &command);
    }

    // Fall back to legacy terminal config behavior
    match config.terminal.strategy {
        LaunchStrategy::Spawn => {
            spawn_terminal_session(&config.terminal, session_id, app)?;
        }
        LaunchStrategy::Inline => {
            // Restore terminal before exec (inline mode replaces the process)
            disable_raw_mode()?;
            execute!(app.terminal_mut().backend_mut(), LeaveAlternateScreen)?;
            app.terminal_mut().show_cursor()?;

            // This will replace the current process
            let _ = app.tmux.exec_into_session(session_id);

            // If we get here, exec failed - restore terminal state
            enable_raw_mode()?;
            execute!(io::stdout(), EnterAlternateScreen)?;
        }
    }
    Ok(())
}

/// Execute a custom keybinding command using the shell.
///
/// This function runs the command via `sh -c` to allow for complex shell commands
/// including pipes, redirects, and variable expansion.
fn execute_keybinding_command<W: Write>(app: &mut ProductionApp<W>, command: &str) -> Result<()> {
    match Command::new("sh").args(["-c", command]).spawn() {
        Ok(mut child) => {
            // Spawn a background thread to reap the child process when it exits.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            app.state_mut()
                .set_success_message(format!("Executed: {}", truncate_command(command)));
        }
        Err(e) => {
            app.state_mut()
                .set_error_message(format!("Failed to execute command: {e}"));
        }
    }
    Ok(())
}

/// Truncate a command for display in status messages.
///
/// Uses character count (not byte count) to safely handle UTF-8 strings.
fn truncate_command(command: &str) -> String {
    const MAX_CHARS: usize = 50;
    let char_count = command.chars().count();
    if char_count <= MAX_CHARS {
        command.to_string()
    } else {
        // Find the byte index at the (MAX_CHARS - 3)th character
        let truncate_at = MAX_CHARS - 3;
        let byte_idx = command
            .char_indices()
            .nth(truncate_at)
            .map(|(i, _)| i)
            .unwrap_or(command.len());
        format!("{}...", &command[..byte_idx])
    }
}

/// Spawn an external terminal attached to the given session.
///
/// This function launches a new terminal process without suspending the TUI.
/// The terminal command is configured via `[terminal]` section in config.toml.
///
/// A background thread is spawned to reap the child process when it exits,
/// preventing zombie processes from accumulating in long-running TUI sessions.
fn spawn_terminal_session<W: Write>(
    terminal_config: &vive::TerminalConfig,
    session_id: &str,
    app: &mut ProductionApp<W>,
) -> Result<()> {
    if let Some((cmd, args)) = terminal_config.build_spawn_command(session_id) {
        match Command::new(&cmd).args(&args).spawn() {
            Ok(mut child) => {
                // Spawn a background thread to reap the child process when it exits.
                // This prevents zombie processes from accumulating when users open/close
                // many external terminals in a long-running TUI session.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                app.state_mut()
                    .set_success_message(format!("Launched terminal for session '{session_id}'"));
            }
            Err(e) => {
                app.state_mut()
                    .set_error_message(format!("Failed to spawn terminal '{cmd}': {e}"));
            }
        }
    } else {
        app.state_mut()
            .set_error_message("Spawn strategy requires terminal.command to be set in config");
    }
    Ok(())
}
