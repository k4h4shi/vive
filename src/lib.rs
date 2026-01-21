//! Vive - A TUI application for managing Claude Code sessions with tmux.
//!
//! This library provides the core components for the Vive TUI application.
//! It is separated into a library crate to enable integration testing.

pub mod config;
pub mod discovery;
pub mod event;
pub mod github;
pub mod mcp;
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

pub use crate::config::{Config, Favorites, LaunchStrategy, TerminalConfig};
pub use crate::discovery::{Project, discover_projects};
pub use crate::event::Action;
pub use crate::github::{
    GhIssueFetcher, GitHubIssue, IssueListFetcher, IssueListResult, IssueTitleFetcher,
    IssueTitleResult, fetch_issue_list_sync,
};
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
/// - `I`: The Issue title fetcher type (e.g., `GhIssueFetcher`, mock)
pub struct App<B, E, T, D, I = GhIssueFetcher>
where
    B: Backend,
    E: EventSource,
    T: TmuxExecutor,
    D: ProjectDiscovery,
    I: IssueTitleFetcher,
{
    terminal: Terminal<B>,
    pub event_source: E,
    pub tmux: TmuxOrchestrator<T>,
    discovery: D,
    issue_fetcher: I,
    state: AppState,
    config: Config,
    pub last_preview_update: Instant,
    pub preview_update_interval: Duration,
    status_monitor: monitor::StatusMonitor,
}

impl<B, E, T, D, I> App<B, E, T, D, I>
where
    B: Backend,
    E: EventSource,
    T: TmuxExecutor,
    D: ProjectDiscovery,
    I: IssueTitleFetcher,
{
    /// Create a new App with the given components.
    pub fn new(
        terminal: Terminal<B>,
        event_source: E,
        tmux: TmuxOrchestrator<T>,
        discovery: D,
        issue_fetcher: I,
        config: Config,
    ) -> Self {
        let projects_root = config.effective_projects_root();
        let state = AppState::with_projects_root(projects_root);

        Self {
            terminal,
            event_source,
            tmux,
            discovery,
            issue_fetcher,
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

    /// Fetch Issue title for the currently selected worktree (if needed).
    fn fetch_selected_issue_title(&mut self) {
        if let (Some(project), Some(worktree)) = (
            self.state.selected_project(),
            self.state.selected_worktree(),
        ) && let Some(issue_number) = worktree.issue_number()
        {
            let repo_path = project.path.to_string_lossy().to_string();
            if self.state.needs_issue_title_fetch(&repo_path, issue_number) {
                let result = self.issue_fetcher.fetch(&repo_path, issue_number);
                self.state.set_issue_title(repo_path, issue_number, result);
            }
        }
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

            Action::AttachSession(_key) => {
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

            Action::CreateTask(branch_name, auto_kickstart) => {
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

                            // Auto-kickstart: send initial command if enabled
                            if auto_kickstart {
                                let command = "claude --print-architecture";
                                let _ = self.tmux.send_keys(&session_id, command, true);
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

            Action::ToggleFavorite => {
                self.state.toggle_favorite_selected();
                self.save_favorites();
            }

            Action::FetchIssues => {
                if let Some(project) = self.state.selected_project() {
                    let repo_path = project.path.to_string_lossy().to_string();
                    match fetch_issue_list_sync(&repo_path) {
                        IssueListResult::Found(issues) => {
                            self.state.set_issue_picker_issues(issues);
                        }
                        IssueListResult::Empty => {
                            self.state
                                .set_issue_picker_error("No open issues found".to_string());
                        }
                        IssueListResult::Error(err) => {
                            self.state.set_issue_picker_error(err);
                        }
                    }
                } else {
                    self.state.close_modal();
                    self.state.set_error_message("No project selected");
                }
            }

            Action::CreateTaskFromIssue(issue, auto_kickstart) => {
                let branch_name = issue.branch_name();
                // Reuse the CreateTask logic
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

                            // Auto-kickstart: start Claude and send /fix command
                            if auto_kickstart {
                                // First, start Claude CLI
                                let _ = self.tmux.send_keys(&session_id, "claude", true);
                                // Then send /fix command after a brief delay for Claude to start
                                // Note: The /fix command will be queued and processed once Claude is ready
                                let command = format!("/fix {}", issue.number);
                                let _ = self.tmux.send_keys(&session_id, &command, true);
                            }

                            self.state.set_success_message(format!(
                                "Created worktree '{}' for Issue #{}",
                                branch_name, issue.number
                            ));
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

            Action::ToggleExpanded => {
                self.state.toggle_expanded_selected();
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
        // Fetch Issue title for selected worktree (lazy loading)
        self.fetch_selected_issue_title();

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

        // Try dashboard mode (when project header is selected, no worktree)
        // Issue #65: Capture from underlying worktree sessions directly for full history.
        if let Some(project) = self.state.selected_project()
            && self.state.selected_worktree_idx().is_none()
        {
            let dashboard_session = TmuxOrchestrator::<T>::dashboard_session_name(&project.name);
            if self.tmux.has_session(&dashboard_session).unwrap_or(false) {
                // Capture from underlying worktree sessions directly
                let mut pane_contents: Vec<(String, String)> = Vec::new();
                let mut combined_preview = String::new();

                for worktree in &project.worktrees {
                    if let Some(branch) = &worktree.branch {
                        // Skip main/master branches from dashboard preview
                        if branch == "main" || branch == "master" {
                            continue;
                        }
                        if let Some(sid) = worktree.session_id(&project.name) {
                            // Check if the worktree session exists before capturing
                            #[allow(clippy::collapsible_if)]
                            if self.tmux.has_session(&sid).unwrap_or(false) {
                                if let Ok(content) =
                                    self.tmux.capture_pane(&sid, DEFAULT_PREVIEW_LINES)
                                {
                                    pane_contents.push((branch.clone(), content.clone()));
                                    if !combined_preview.is_empty() {
                                        combined_preview.push_str("\n--- ");
                                        combined_preview.push_str(branch);
                                        combined_preview.push_str(" ---\n");
                                    }
                                    combined_preview.push_str(&content);
                                }
                            }
                        }
                    }
                }

                if !pane_contents.is_empty() {
                    self.state.set_dashboard_panes(pane_contents);
                    self.state.set_pane_preview(combined_preview);
                    return;
                }
            }
        }

        self.state.clear_dashboard_panes();
        self.state.set_pane_preview(String::new());
    }

    /// Update status for all sessions that have tmux sessions.
    /// This allows status indicators to update even for non-selected items.
    pub fn update_all_statuses(&mut self) {
        // Collect session info to avoid borrow conflicts
        let sessions_to_check: Vec<String> = self
            .state
            .projects
            .iter()
            .flat_map(|project| {
                project
                    .worktrees
                    .iter()
                    .filter_map(|worktree| worktree.session_id(&project.name))
            })
            .collect();

        for session_id in sessions_to_check {
            // Skip if session doesn't exist
            if !self.tmux.has_session(&session_id).unwrap_or(false) {
                continue;
            }

            // Capture a small amount of content for status detection (faster)
            if let Ok(content) = self.tmux.capture_pane(&session_id, 30) {
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

                self.state.set_status(session_id, final_status);
            }
        }
    }
}

/// Type alias for the standard production App.
pub type ProductionApp<W> = App<
    ratatui::backend::CrosstermBackend<W>,
    RealEventSource,
    RealTmuxExecutor,
    RealProjectDiscovery,
    GhIssueFetcher,
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
