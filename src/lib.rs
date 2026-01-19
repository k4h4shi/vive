//! Vive - A TUI application for managing Claude Code sessions with tmux.
//!
//! This library provides the core components for the Vive TUI application.
//! It is separated into a library crate to enable integration testing.

pub mod config;
pub mod discovery;
pub mod event;
mod process;
pub mod state;
pub mod tmux;
pub mod ui;

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::Event;
use ratatui::{Terminal, backend::Backend};

pub use crate::config::Config;
pub use crate::discovery::{Project, discover_projects};
pub use crate::event::Action;
pub use crate::state::AppState;
pub use crate::tmux::{RealTmuxExecutor, TmuxExecutor, TmuxOrchestrator};

/// Trait for discovering projects, abstracted for testing.
pub trait ProjectDiscovery: Send + Sync {
    /// Discover projects from the given root directory.
    fn discover(&self, root: &Path, ignored_dirs: &[String]) -> Result<Vec<Project>>;
}

/// Real implementation that uses the filesystem.
#[derive(Debug, Default, Clone)]
pub struct RealProjectDiscovery;

impl ProjectDiscovery for RealProjectDiscovery {
    fn discover(&self, root: &Path, ignored_dirs: &[String]) -> Result<Vec<Project>> {
        discover_projects(root, ignored_dirs)
    }
}

/// Trait for polling events, abstracted for testing.
pub trait EventSource: Send + Sync {
    /// Poll for an event with the given timeout.
    fn poll(&self, timeout: Duration) -> Result<Option<Event>>;
}

/// Real implementation that uses crossterm events.
#[derive(Debug, Default, Clone)]
pub struct RealEventSource;

impl EventSource for RealEventSource {
    fn poll(&self, timeout: Duration) -> Result<Option<Event>> {
        event::poll_event(timeout)
    }
}

/// The main application struct, parameterized for testability.
///
/// Type parameters:
/// - `B`: The backend type for rendering (e.g., `CrosstermBackend`, `TestBackend`)
/// - `E`: The event source type (e.g., `RealEventSource`, mock)
/// - `T`: The tmux executor type (e.g., `RealTmuxExecutor`, mock)
/// - `D`: The project discovery type (e.g., `RealProjectDiscovery`, mock)
pub struct App<B, E, T, D>
where
    B: Backend,
    E: EventSource,
    T: TmuxExecutor,
    D: ProjectDiscovery,
{
    terminal: Terminal<B>,
    pub event_source: E,
    pub tmux: TmuxOrchestrator<T>,
    discovery: D,
    state: AppState,
    config: Config,
    pub last_preview_update: Instant,
    pub preview_update_interval: Duration,
}

impl<B, E, T, D> App<B, E, T, D>
where
    B: Backend,
    E: EventSource,
    T: TmuxExecutor,
    D: ProjectDiscovery,
{
    /// Create a new App with the given components.
    pub fn new(
        terminal: Terminal<B>,
        event_source: E,
        tmux: TmuxOrchestrator<T>,
        discovery: D,
        config: Config,
    ) -> Self {
        let projects_root = config.effective_projects_root();
        let state = AppState::with_projects_root(projects_root);

        Self {
            terminal,
            event_source,
            tmux,
            discovery,
            state,
            config,
            last_preview_update: Instant::now(),
            preview_update_interval: Duration::from_secs(2),
        }
    }

    /// Get a reference to the application state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get a mutable reference to the application state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// Get a reference to the terminal.
    pub fn terminal(&self) -> &Terminal<B> {
        &self.terminal
    }

    /// Get a mutable reference to the terminal.
    pub fn terminal_mut(&mut self) -> &mut Terminal<B> {
        &mut self.terminal
    }

    /// Initialize the application by discovering projects.
    pub fn init(&mut self) -> Result<()> {
        let projects_root = self.config.effective_projects_root();
        if let Ok(projects) = self
            .discovery
            .discover(&projects_root, &self.config.ignored_dirs)
        {
            self.state.set_projects(projects);
        }
        Ok(())
    }

    /// Render the current state to the terminal.
    pub fn render(&mut self) -> Result<()> {
        self.terminal.draw(|frame| ui::render(frame, &self.state))?;
        Ok(())
    }

    /// Process a single tick of the event loop.
    /// Returns `true` if the application should continue, `false` if it should quit.
    pub fn tick(&mut self, poll_timeout: Duration) -> Result<bool> {
        // Poll for events
        if let Some(Event::Key(key)) = self.event_source.poll(poll_timeout)? {
            let action = event::handle_key_event(key, &mut self.state);
            self.handle_action(action)?;
        }

        // Periodic preview update
        if self.last_preview_update.elapsed() >= self.preview_update_interval {
            self.update_pane_preview();
            self.last_preview_update = Instant::now();
        }

        Ok(!self.state.should_quit())
    }

    /// Handle a key event and return the resulting action.
    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Action {
        event::handle_key_event(key, &mut self.state)
    }

    /// Handle an action.
    pub fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::None | Action::Quit => {}

            Action::AttachSession => {
                // Note: In tests, this won't actually exec into tmux
                // For real usage, this is handled specially in run_with_terminal_control
            }

            Action::SendInput(input) => {
                if let (Some(project), Some(worktree)) = (
                    self.state.selected_project(),
                    self.state.selected_worktree(),
                ) && let Some(session_id) = worktree.session_id(&project.name)
                {
                    let _ = self.tmux.send_keys(&session_id, &input, true);
                }
            }

            Action::CreateTask(branch_name) => {
                if let Some(project) = self.state.selected_project().cloned() {
                    let worktree_path = project.path.join(".worktrees").join(&branch_name);
                    let output = std::process::Command::new("git")
                        .args([
                            "worktree",
                            "add",
                            "-b",
                            &branch_name,
                            &worktree_path.to_string_lossy(),
                        ])
                        .current_dir(&project.path)
                        .output();

                    match output {
                        Ok(output) if output.status.success() => {
                            if let Ok(projects) = self
                                .discovery
                                .discover(&self.state.projects_root, &self.config.ignored_dirs)
                            {
                                self.state.set_projects(projects);
                            }
                            self.state
                                .set_success_message(format!("Created worktree '{branch_name}'"));
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let error_msg = stderr.trim();
                            if error_msg.is_empty() {
                                self.state.set_error_message(format!(
                                    "Failed to create worktree '{branch_name}': unknown error"
                                ));
                            } else {
                                self.state.set_error_message(format!(
                                    "Failed to create worktree: {error_msg}"
                                ));
                            }
                        }
                        Err(e) => {
                            self.state
                                .set_error_message(format!("Failed to run git command: {e}"));
                        }
                    }
                } else {
                    self.state.set_error_message("No project selected");
                }
            }

            Action::RefreshPreview => {
                self.update_pane_preview();
            }
        }

        Ok(())
    }

    /// Update the pane preview from tmux.
    pub fn update_pane_preview(&mut self) {
        if let (Some(project), Some(worktree)) = (
            self.state.selected_project(),
            self.state.selected_worktree(),
        ) && let Some(session_id) = worktree.session_id(&project.name)
            && self.tmux.has_session(&session_id).unwrap_or(false)
            && let Ok(content) = self.tmux.capture_pane(&session_id, 50)
        {
            self.state.set_pane_preview(content);
            return;
        }
        self.state.set_pane_preview(String::new());
    }

    /// Run the main application loop.
    pub fn run(&mut self) -> Result<()> {
        self.init()?;

        loop {
            self.render()?;

            if !self.tick(Duration::from_millis(100))? {
                break;
            }
        }

        Ok(())
    }
}

/// Type alias for the standard production App.
pub type ProductionApp<W> = App<
    ratatui::backend::CrosstermBackend<W>,
    RealEventSource,
    RealTmuxExecutor,
    RealProjectDiscovery,
>;
