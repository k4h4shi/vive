//! Vive - A TUI application for managing Claude Code sessions with tmux.
//!
//! This library provides the core components for the Vive TUI application.
//! It is separated into a library crate to enable integration testing.

pub mod config;
pub mod discovery;
pub mod event;
pub mod monitor;
pub mod parser;
mod process;
pub mod state;
pub mod tmux;
pub mod ui;

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;

/// Default number of lines to capture for pane preview.
/// Issue #57: Increased from 50 to 200 to prevent command confirmation prompts
/// from being cut off when there's a long output (like diffs or thought logs).
pub const DEFAULT_PREVIEW_LINES: usize = 200;
use crossterm::event::Event;
use ratatui::{Terminal, backend::Backend};

pub use crate::config::{Config, Favorites};
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
    status_monitor: monitor::StatusMonitor,
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
            status_monitor: monitor::StatusMonitor::new(),
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

    /// Initialize the application by discovering projects and loading favorites.
    pub fn init(&mut self) -> Result<()> {
        // Load favorites from disk - use robust loading that handles errors gracefully
        match Favorites::load() {
            Ok(favorites) => {
                self.state.set_favorites(favorites.projects);
            }
            Err(e) => {
                // Log the error but don't fail - user can still use the app
                // Important: we mark that loading failed so we don't overwrite
                // potentially valid data on the next save
                eprintln!("Warning: Failed to load favorites: {e}");
                self.state.mark_favorites_load_failed();
            }
        }

        // Discover projects
        let projects_root = self.config.effective_projects_root();
        match self
            .discovery
            .discover(&projects_root, &self.config.ignored_dirs)
        {
            Ok(projects) => {
                self.state.set_projects(projects);
            }
            Err(e) => {
                // Log the error but don't fail - user can still use the app
                eprintln!(
                    "Warning: Failed to discover projects in '{}': {e}",
                    projects_root.display()
                );
            }
        }
        Ok(())
    }

    /// Save the current favorites to disk.
    ///
    /// This method is defensive: if favorites failed to load at startup
    /// AND the user hasn't modified favorites, we don't save (to avoid
    /// overwriting potentially valid data). However, if the user has
    /// explicitly modified favorites, we honor their intent and save.
    fn save_favorites(&self) {
        // Don't save if loading failed AND user hasn't modified favorites
        // If user explicitly modified, honor their intent and save
        if self.state.favorites_load_failed() && !self.state.favorites_modified() {
            eprintln!("Warning: Skipping favorites save because loading failed at startup");
            return;
        }

        let favorites = Favorites {
            projects: self.state.favorites().clone(),
        };
        if let Err(e) = favorites.save() {
            // Log error but don't fail - favorites are not critical
            eprintln!("Warning: Failed to save favorites: {e}");
        }
    }

    /// Render the current state to the terminal.
    pub fn render(&mut self) -> Result<()> {
        let state = &mut self.state;
        self.terminal.draw(|frame| ui::render(frame, state))?;
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

    /// Handle a mouse event and return the resulting action.
    pub fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> Action {
        event::handle_mouse_event(mouse, &mut self.state)
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
                            // Refresh project list
                            if let Ok(projects) = self
                                .discovery
                                .discover(&self.state.projects_root, &self.config.ignored_dirs)
                            {
                                self.state.set_projects(projects);
                            }

                            // Add pane to dashboard if it exists
                            let session_id = format!("{}__{}", project.name, branch_name);
                            let worktree_path_str = worktree_path.to_string_lossy();
                            let _ = self
                                .tmux
                                .ensure_session(&session_id, Some(&worktree_path_str));
                            let _ = self.tmux.add_pane_to_dashboard(&project.name, &session_id);

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

            Action::ToggleFavorite => {
                self.state.toggle_favorite_selected();
                self.save_favorites();
            }

            Action::DeleteTask(branch_name) => {
                if let Some(project) = self.state.selected_project().cloned() {
                    let project_name = project.name.clone();

                    // Kill tmux session if it exists
                    let session_id = format!("{}__{}", project.name, branch_name);
                    if self.tmux.has_session(&session_id).unwrap_or(false) {
                        let _ = self.tmux.kill_session(&session_id);
                    }

                    // Remove worktree
                    let worktree_path = project.path.join(".worktrees").join(&branch_name);
                    let worktree_remove = std::process::Command::new("git")
                        .args([
                            "worktree",
                            "remove",
                            "--force",
                            &worktree_path.to_string_lossy(),
                        ])
                        .current_dir(&project.path)
                        .output();

                    match worktree_remove {
                        Ok(output) if output.status.success() => {
                            // Delete the branch
                            let branch_delete = std::process::Command::new("git")
                                .args(["branch", "-D", &branch_name])
                                .current_dir(&project.path)
                                .output();

                            match branch_delete {
                                Ok(output) if output.status.success() => {
                                    self.state.set_success_message(format!(
                                        "Deleted task '{branch_name}'"
                                    ));
                                }
                                Ok(output) => {
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    self.state.set_error_message(format!(
                                        "Worktree removed but failed to delete branch: {}",
                                        stderr.trim()
                                    ));
                                }
                                Err(e) => {
                                    self.state.set_error_message(format!(
                                        "Worktree removed but failed to delete branch: {e}"
                                    ));
                                }
                            }

                            // Refresh project list
                            if let Ok(projects) = self
                                .discovery
                                .discover(&self.state.projects_root, &self.config.ignored_dirs)
                            {
                                // Sync dashboard with updated worktrees before moving projects
                                if let Some(updated_project) =
                                    projects.iter().find(|p| p.name == project_name)
                                {
                                    let worktree_sessions: Vec<(String, String)> = updated_project
                                        .worktrees
                                        .iter()
                                        .filter_map(|wt| {
                                            wt.session_id(&project_name).map(|id| {
                                                (id, wt.path.to_string_lossy().to_string())
                                            })
                                        })
                                        .collect();

                                    if !worktree_sessions.is_empty() {
                                        let _ = self
                                            .tmux
                                            .sync_dashboard(&project_name, &worktree_sessions);
                                    }
                                }

                                self.state.set_projects(projects);
                            }
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            self.state.set_error_message(format!(
                                "Failed to remove worktree: {}",
                                stderr.trim()
                            ));
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
        }

        Ok(())
    }

    /// Update the pane preview from tmux and parse agent status.
    pub fn update_pane_preview(&mut self) {
        // Try worktree session first (when a worktree is selected)
        if let (Some(project), Some(worktree)) = (
            self.state.selected_project(),
            self.state.selected_worktree(),
        ) && let Some(session_id) = worktree.session_id(&project.name)
            && self.tmux.has_session(&session_id).unwrap_or(false)
            && let Ok(content) = self.tmux.capture_pane(&session_id, DEFAULT_PREVIEW_LINES)
        {
            // Parse the content to detect agent status
            let parsed = parser::parse_status(&content);
            let raw_status = state::AgentStatus::from_parsed(&parsed);

            // Get pane title and combine with parsed status
            let pane_title = self.tmux.get_pane_title(&session_id).ok().flatten();
            let title_combined =
                monitor::combine_status_with_title(raw_status, pane_title.as_deref());

            // Apply hysteresis to smooth out transitions
            let final_status = self
                .status_monitor
                .apply_hysteresis(&session_id, title_combined);

            self.state.set_status(session_id.clone(), final_status);
            self.state.set_pane_preview(content);
            return;
        }

        // Try dashboard session (when project header is selected, no worktree)
        if let Some(project) = self.state.selected_project()
            && self.state.selected_worktree_idx().is_none()
        {
            let dashboard_session = TmuxOrchestrator::<T>::dashboard_session_name(&project.name);
            if self.tmux.has_session(&dashboard_session).unwrap_or(false) {
                // Capture individual panes for split preview
                if let Ok(panes) = self.tmux.list_panes(&dashboard_session) {
                    // Get branch names from worktrees (panes are created in worktree order)
                    let branch_names: Vec<String> = project
                        .worktrees
                        .iter()
                        .filter_map(|wt| wt.branch.clone())
                        .collect();

                    let mut pane_contents: Vec<(String, String)> = Vec::new();
                    for (idx, pane) in panes.iter().enumerate() {
                        // Use pane_id for reliable targeting across sessions
                        if let Ok(content) = self.tmux.capture_pane(&pane.pane_id, 20) {
                            let branch_name = branch_names
                                .get(idx)
                                .cloned()
                                .unwrap_or_else(|| format!("Pane {}", idx + 1));
                            pane_contents.push((branch_name, content));
                        }
                    }
                    self.state.set_dashboard_panes(pane_contents);

                    // Also set combined preview for fallback
                    if let Ok(content) = self.tmux.capture_all_panes(&dashboard_session, 15) {
                        self.state.set_pane_preview(content);
                    }
                    return;
                }
            }
        }

        self.state.clear_dashboard_panes();
        self.state.set_pane_preview(String::new());
    }
}

/// Type alias for the standard production App.
pub type ProductionApp<W> = App<
    ratatui::backend::CrosstermBackend<W>,
    RealEventSource,
    RealTmuxExecutor,
    RealProjectDiscovery,
>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #57: Verify preview lines constant is set to 200 (increased from 50)
    #[test]
    fn test_default_preview_lines_is_200() {
        assert_eq!(DEFAULT_PREVIEW_LINES, 200);
    }
}
