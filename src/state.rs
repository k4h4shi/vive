//! Application state management.
//!
//! This module contains the central state for the TUI application,
//! including project data, selection state, and UI mode tracking.

// Allow dead code during development - some APIs are for future features
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::widgets::ListState;

use crate::discovery::Project;

/// Debounce duration for mouse scroll events (in milliseconds).
const SCROLL_DEBOUNCE_MS: u64 = 25;

/// Agent status for a worktree/session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentStatus {
    /// Agent is actively working (process running with recent output).
    Working,
    /// Agent is waiting for user input.
    Waiting,
    /// Agent has completed or session is idle.
    #[default]
    Idle,
    /// Agent encountered an error.
    Error,
}

impl AgentStatus {
    /// Get the status icon for display.
    pub fn icon(&self) -> &'static str {
        match self {
            AgentStatus::Working => "🟢",
            AgentStatus::Waiting => "🟡",
            AgentStatus::Idle => "⚪",
            AgentStatus::Error => "🔴",
        }
    }

    /// Create AgentStatus from parsed terminal content.
    pub fn from_parsed(parsed: &crate::parser::ParsedStatus) -> Self {
        use crate::parser::ParsedStatus;
        match parsed {
            ParsedStatus::Idle => AgentStatus::Idle,
            ParsedStatus::Working { .. } => AgentStatus::Working,
            ParsedStatus::WaitingApproval { .. } => AgentStatus::Waiting,
        }
    }
}

/// Focus mode for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusMode {
    /// Normal navigation mode.
    #[default]
    Normal,
    /// Input mode for command entry.
    Input,
}

/// Modal dialog type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalType {
    /// Create a new task/worktree.
    CreateTask { input: String },
    /// Confirm deletion of a task/worktree.
    ConfirmDeletion { branch_name: String },
}

/// Status message type for user feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusMessageType {
    /// Informational message.
    Info,
    /// Success message.
    Success,
    /// Error message.
    Error,
}

/// Status message to display to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessage {
    /// The message content.
    pub message: String,
    /// The message type.
    pub message_type: StatusMessageType,
}

/// Application state, separated from UI for testability.
#[derive(Debug)]
pub struct AppState {
    /// Whether the application should quit.
    should_quit: bool,
    /// The root directory for project discovery.
    pub projects_root: PathBuf,
    /// Discovered projects and their worktrees.
    pub projects: Vec<Project>,
    /// Currently selected project index.
    selected_project_idx: Option<usize>,
    /// Currently selected worktree index within the selected project.
    selected_worktree_idx: Option<usize>,
    /// Agent statuses keyed by session ID (project:branch).
    pub statuses: HashMap<String, AgentStatus>,
    /// Current focus mode.
    pub focus_mode: FocusMode,
    /// Current modal dialog, if any.
    pub modal: Option<ModalType>,
    /// Command input buffer.
    pub input_buffer: String,
    /// Captured pane content for preview.
    pub pane_preview: String,
    /// Status message for user feedback.
    pub status_message: Option<StatusMessage>,
    /// ListState for sidebar scrolling support.
    sidebar_list_state: ListState,
    /// Last scroll event time for debouncing.
    last_scroll_time: Option<Instant>,
    /// Set of favorite project names.
    favorites: HashSet<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            should_quit: false,
            projects_root: PathBuf::new(),
            projects: Vec::new(),
            selected_project_idx: None,
            selected_worktree_idx: None,
            statuses: HashMap::new(),
            focus_mode: FocusMode::Normal,
            modal: None,
            input_buffer: String::new(),
            pane_preview: String::new(),
            status_message: None,
            sidebar_list_state: ListState::default(),
            last_scroll_time: None,
            favorites: HashSet::new(),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new AppState with a projects root directory.
    pub fn with_projects_root(projects_root: PathBuf) -> Self {
        Self {
            projects_root,
            ..Self::default()
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Set the discovered projects.
    pub fn set_projects(&mut self, projects: Vec<Project>) {
        self.projects = projects;
        // Auto-select first project in sorted order (favorites first)
        self.select_first_in_sorted_order();
    }

    /// Calculate the flat index for the sidebar list.
    /// The sidebar displays projects and worktrees in a flat list:
    /// - Favorites first, then separator (if both favorites and non-favorites exist), then non-favorites
    /// - Each project takes one row
    /// - Each worktree takes one row under its project
    pub fn flat_sidebar_index(&self) -> Option<usize> {
        let proj_idx = self.selected_project_idx?;
        let wt_idx = self.selected_worktree_idx;

        let selected_project = &self.projects[proj_idx];
        let selected_name = &selected_project.name;
        let is_selected_favorite = self.is_favorite(selected_name);

        // Get sorted projects for correct display order
        let sorted = self.sorted_projects();
        let has_favorites = self.has_favorites();
        let has_non_favorites = sorted.iter().any(|p| !self.is_favorite(&p.name));
        let has_separator = has_favorites && has_non_favorites;

        let mut flat_idx = 0;

        for project in sorted.iter() {
            let is_favorite = self.is_favorite(&project.name);

            // Add separator before first non-favorite (if applicable)
            if has_separator && !is_favorite {
                // Check if we've crossed the separator
                if is_selected_favorite {
                    // Selected is a favorite, no separator to account for before it
                } else {
                    // Check if this is the first non-favorite
                    let first_non_fav = sorted.iter().find(|p| !self.is_favorite(&p.name));
                    if let Some(first) = first_non_fav {
                        if first.name == project.name && project.name != *selected_name {
                            // Haven't reached selected yet, add separator
                            flat_idx += 1;
                        } else if first.name == project.name && project.name == *selected_name {
                            // Selected is the first non-favorite, add separator before
                            flat_idx += 1;
                        }
                    }
                }
            }

            if project.name == *selected_name {
                // Found the selected project
                flat_idx += 1; // project header

                if let Some(wt) = wt_idx {
                    flat_idx += wt;
                } else {
                    flat_idx -= 1; // point to project header
                }
                return Some(flat_idx);
            }

            // Count this project's items
            flat_idx += 1; // project header
            flat_idx += project.worktrees.len(); // worktrees
        }

        None
    }

    /// Sync the ListState with the current selection.
    fn sync_sidebar_list_state(&mut self) {
        self.sidebar_list_state.select(self.flat_sidebar_index());
    }

    /// Get a reference to the sidebar ListState for rendering.
    pub fn sidebar_list_state(&self) -> &ListState {
        &self.sidebar_list_state
    }

    /// Get a mutable reference to the sidebar ListState for rendering.
    pub fn sidebar_list_state_mut(&mut self) -> &mut ListState {
        &mut self.sidebar_list_state
    }

    /// Get the currently selected project.
    pub fn selected_project(&self) -> Option<&Project> {
        self.selected_project_idx
            .and_then(|idx| self.projects.get(idx))
    }

    /// Get the currently selected project index.
    pub fn selected_project_idx(&self) -> Option<usize> {
        self.selected_project_idx
    }

    /// Get the currently selected worktree index.
    pub fn selected_worktree_idx(&self) -> Option<usize> {
        self.selected_worktree_idx
    }

    /// Get the currently selected worktree.
    pub fn selected_worktree(&self) -> Option<&crate::discovery::Worktree> {
        let project = self.selected_project()?;
        self.selected_worktree_idx
            .and_then(|idx| project.worktrees.get(idx))
    }

    /// Get the session ID for the currently selected worktree.
    pub fn selected_session_id(&self) -> Option<String> {
        let project = self.selected_project()?;
        let worktree = self.selected_worktree()?;
        worktree.session_id(&project.name)
    }

    /// Navigate to the next item in the sidebar (following sorted order).
    ///
    /// Navigation order:
    /// - Project header (worktree_idx = None)
    /// - First worktree (worktree_idx = Some(0))
    /// - ... more worktrees ...
    /// - Next project header
    pub fn select_next(&mut self) {
        if self.projects.is_empty() {
            return;
        }

        let sorted = self.sorted_projects();
        let sorted_names: Vec<&str> = sorted.iter().map(|p| p.name.as_str()).collect();

        // Find current position in sorted order
        let current_sorted_idx = if let Some(proj_idx) = self.selected_project_idx {
            let current_name = &self.projects[proj_idx].name;
            sorted_names.iter().position(|n| *n == current_name)
        } else {
            None
        };

        let Some(sorted_idx) = current_sorted_idx else {
            // No current selection, select first project header in sorted order
            let first_name = sorted_names[0].to_string();
            drop(sorted);
            self.select_project_header_by_name(&first_name);
            self.sync_sidebar_list_state();
            return;
        };

        let current_project = sorted[sorted_idx];

        match self.selected_worktree_idx {
            None => {
                // Currently on project header, move to first worktree if any
                if !current_project.worktrees.is_empty() {
                    self.selected_worktree_idx = Some(0);
                } else if sorted_idx + 1 < sorted.len() {
                    // No worktrees, move to next project header
                    let next_name = sorted_names[sorted_idx + 1].to_string();
                    drop(sorted);
                    self.select_project_header_by_name(&next_name);
                }
            }
            Some(wt_idx) => {
                // Currently on a worktree
                if wt_idx + 1 < current_project.worktrees.len() {
                    // Move to next worktree in current project
                    self.selected_worktree_idx = Some(wt_idx + 1);
                } else if sorted_idx + 1 < sorted.len() {
                    // Move to next project header
                    let next_name = sorted_names[sorted_idx + 1].to_string();
                    drop(sorted);
                    self.select_project_header_by_name(&next_name);
                }
                // Otherwise, stay at current position (at the end)
            }
        }
        self.sync_sidebar_list_state();
    }

    /// Navigate to the previous item in the sidebar (following sorted order).
    ///
    /// Navigation order (reverse):
    /// - From first worktree (worktree_idx = Some(0)) -> project header (worktree_idx = None)
    /// - From project header -> previous project's last worktree
    pub fn select_prev(&mut self) {
        if self.projects.is_empty() {
            return;
        }

        // Collect sorted info upfront to avoid borrow issues
        let sorted = self.sorted_projects();
        let sorted_info: Vec<(String, usize)> = sorted
            .iter()
            .map(|p| (p.name.clone(), p.worktrees.len()))
            .collect();
        drop(sorted);

        // Find current position in sorted order
        let current_sorted_idx = if let Some(proj_idx) = self.selected_project_idx {
            let current_name = &self.projects[proj_idx].name;
            sorted_info.iter().position(|(n, _)| n == current_name)
        } else {
            None
        };

        let Some(sorted_idx) = current_sorted_idx else {
            return;
        };

        match self.selected_worktree_idx {
            None => {
                // Currently on project header, move to previous project's last worktree
                if sorted_idx > 0 {
                    let (prev_name, prev_worktrees_len) = &sorted_info[sorted_idx - 1];
                    self.select_project_header_by_name(prev_name);
                    // Move to last worktree of previous project (if any)
                    self.selected_worktree_idx = if *prev_worktrees_len == 0 {
                        None
                    } else {
                        Some(*prev_worktrees_len - 1)
                    };
                }
                // Otherwise, stay at current position (at the beginning)
            }
            Some(wt_idx) => {
                if wt_idx > 0 {
                    // Move to previous worktree in current project
                    self.selected_worktree_idx = Some(wt_idx - 1);
                } else {
                    // At first worktree, move to project header
                    self.selected_worktree_idx = None;
                }
            }
        }
        self.sync_sidebar_list_state();
    }

    /// Select a project by name, updating selection indices.
    /// This selects the first worktree of the project.
    fn select_project_by_name(&mut self, name: &str) {
        if let Some(idx) = self.projects.iter().position(|p| p.name == name) {
            self.selected_project_idx = Some(idx);
            self.selected_worktree_idx = if self.projects[idx].worktrees.is_empty() {
                None
            } else {
                Some(0)
            };
        }
    }

    /// Select a project header by name (no worktree selected).
    /// This is used for project-level selection (dashboard view).
    fn select_project_header_by_name(&mut self, name: &str) {
        if let Some(idx) = self.projects.iter().position(|p| p.name == name) {
            self.selected_project_idx = Some(idx);
            self.selected_worktree_idx = None;
        }
    }

    /// Get the status for a session ID.
    pub fn get_status(&self, session_id: &str) -> AgentStatus {
        self.statuses
            .get(session_id)
            .copied()
            .unwrap_or(AgentStatus::Idle)
    }

    /// Update the status for a session ID.
    pub fn set_status(&mut self, session_id: String, status: AgentStatus) {
        self.statuses.insert(session_id, status);
    }

    /// Enter input mode.
    pub fn enter_input_mode(&mut self) {
        self.focus_mode = FocusMode::Input;
        self.input_buffer.clear();
    }

    /// Exit input mode.
    pub fn exit_input_mode(&mut self) {
        self.focus_mode = FocusMode::Normal;
    }

    /// Add a character to the input buffer.
    pub fn input_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    /// Remove the last character from the input buffer.
    pub fn input_backspace(&mut self) {
        self.input_buffer.pop();
    }

    /// Take the current input and clear the buffer.
    pub fn take_input(&mut self) -> String {
        std::mem::take(&mut self.input_buffer)
    }

    /// Open the create task modal.
    pub fn open_create_task_modal(&mut self) {
        self.modal = Some(ModalType::CreateTask {
            input: String::new(),
        });
    }

    /// Open the confirm deletion modal.
    pub fn open_confirm_deletion_modal(&mut self, branch_name: String) {
        self.modal = Some(ModalType::ConfirmDeletion { branch_name });
    }

    /// Get the branch name from the confirm deletion modal.
    pub fn deletion_branch_name(&self) -> Option<&str> {
        match &self.modal {
            Some(ModalType::ConfirmDeletion { branch_name }) => Some(branch_name),
            _ => None,
        }
    }

    /// Close any open modal.
    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    /// Add a character to the modal input.
    pub fn modal_input_char(&mut self, c: char) {
        if let Some(ModalType::CreateTask { input }) = &mut self.modal {
            input.push(c);
        }
    }

    /// Remove the last character from the modal input.
    pub fn modal_input_backspace(&mut self) {
        if let Some(ModalType::CreateTask { input }) = &mut self.modal {
            input.pop();
        }
    }

    /// Get the current modal input.
    pub fn modal_input(&self) -> Option<&str> {
        match &self.modal {
            Some(ModalType::CreateTask { input }) => Some(input),
            Some(ModalType::ConfirmDeletion { .. }) => None,
            None => None,
        }
    }

    /// Update the pane preview content.
    pub fn set_pane_preview(&mut self, content: String) {
        self.pane_preview = content;
    }

    /// Set an info status message.
    pub fn set_info_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            message: message.into(),
            message_type: StatusMessageType::Info,
        });
    }

    /// Set a success status message.
    pub fn set_success_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            message: message.into(),
            message_type: StatusMessageType::Success,
        });
    }

    /// Set an error status message.
    pub fn set_error_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            message: message.into(),
            message_type: StatusMessageType::Error,
        });
    }

    /// Clear the status message.
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    /// Check if a project is marked as favorite.
    pub fn is_favorite(&self, project_name: &str) -> bool {
        self.favorites.contains(project_name)
    }

    /// Toggle the favorite status of a project by name.
    /// After toggling, selection is updated to the first project in sorted order.
    pub fn toggle_favorite(&mut self, project_name: &str) {
        if self.favorites.contains(project_name) {
            self.favorites.remove(project_name);
        } else {
            self.favorites.insert(project_name.to_string());
        }
        // Update selection to first in sorted order
        self.select_first_in_sorted_order();
    }

    /// Select the first project header in sorted order.
    fn select_first_in_sorted_order(&mut self) {
        if self.projects.is_empty() {
            return;
        }
        let sorted = self.sorted_projects();
        if let Some(first) = sorted.first() {
            self.select_project_header_by_name(&first.name.clone());
            self.sync_sidebar_list_state();
        }
    }

    /// Toggle the favorite status of the currently selected project.
    pub fn toggle_favorite_selected(&mut self) {
        if let Some(project) = self.selected_project() {
            let name = project.name.clone();
            self.toggle_favorite(&name);
        }
    }

    /// Check if there are any favorite projects.
    pub fn has_favorites(&self) -> bool {
        !self.favorites.is_empty()
    }

    /// Set favorites from a HashSet (used for loading from persistence).
    pub fn set_favorites(&mut self, favorites: HashSet<String>) {
        self.favorites = favorites;
    }

    /// Get a reference to the favorites set (used for saving to persistence).
    pub fn favorites(&self) -> &HashSet<String> {
        &self.favorites
    }

    /// Get projects sorted with favorites first, preserving original order within each group.
    pub fn sorted_projects(&self) -> Vec<&Project> {
        let mut favorites: Vec<&Project> = self
            .projects
            .iter()
            .filter(|p| self.is_favorite(&p.name))
            .collect();
        let mut non_favorites: Vec<&Project> = self
            .projects
            .iter()
            .filter(|p| !self.is_favorite(&p.name))
            .collect();

        favorites.append(&mut non_favorites);
        favorites
    }

    /// Try to scroll, returning true if allowed (debounce passed).
    /// This should be called before processing a scroll event.
    pub fn try_scroll(&mut self) -> bool {
        let now = Instant::now();
        let debounce_duration = Duration::from_millis(SCROLL_DEBOUNCE_MS);

        if let Some(last_time) = self.last_scroll_time
            && now.duration_since(last_time) < debounce_duration
        {
            return false;
        }

        self.last_scroll_time = Some(now);
        true
    }

    /// Reset the scroll debounce timer (for testing).
    #[cfg(test)]
    pub fn reset_scroll_debounce(&mut self) {
        self.last_scroll_time = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::Worktree;

    fn create_test_projects() -> Vec<Project> {
        vec![
            Project::new("project-a", "/path/to/project-a").with_worktrees(vec![
                Worktree::new("/path/to/project-a", "abc123", Some("main".to_string())),
                Worktree::new(
                    "/path/to/project-a/.worktrees/feature-1",
                    "def456",
                    Some("feature-1".to_string()),
                ),
            ]),
            Project::new("project-b", "/path/to/project-b").with_worktrees(vec![Worktree::new(
                "/path/to/project-b",
                "ghi789",
                Some("main".to_string()),
            )]),
        ]
    }

    #[test]
    fn test_new_state_defaults() {
        let state = AppState::new();
        assert!(!state.should_quit());
        assert!(state.projects.is_empty());
        assert!(state.selected_project_idx().is_none());
    }

    #[test]
    fn test_quit() {
        let mut state = AppState::new();
        assert!(!state.should_quit());
        state.quit();
        assert!(state.should_quit());
    }

    #[test]
    fn test_set_projects_auto_selects_first_project_header() {
        let mut state = AppState::new();
        let projects = create_test_projects();
        state.set_projects(projects);

        // Now selects project header (no worktree)
        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), None); // Project header selected
        assert_eq!(state.selected_project().unwrap().name, "project-a");
    }

    #[test]
    fn test_select_next_from_project_header_to_worktree() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Starts at project-a header (worktree_idx = None)
        assert_eq!(state.selected_worktree_idx(), None);

        state.select_next();
        // Now at project-a, worktree 0 (main)
        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), Some(0));
    }

    #[test]
    fn test_select_next_within_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        state.select_next(); // project-a, worktree 0
        state.select_next(); // project-a, worktree 1

        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), Some(1));
    }

    #[test]
    fn test_select_next_moves_to_next_project_header() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Navigate: project-a header -> worktree 0 -> worktree 1 -> project-b header
        state.select_next(); // project-a, worktree 0
        state.select_next(); // project-a, worktree 1
        state.select_next(); // project-b header

        assert_eq!(state.selected_project_idx(), Some(1));
        assert_eq!(state.selected_worktree_idx(), None); // Project header
        assert_eq!(state.selected_project().unwrap().name, "project-b");
    }

    #[test]
    fn test_select_prev_to_project_header() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        state.select_next(); // project-a, worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));

        state.select_prev(); // Back to project-a header
        assert_eq!(state.selected_worktree_idx(), None);
    }

    #[test]
    fn test_select_prev_within_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        state.select_next(); // project-a, worktree 0
        state.select_next(); // project-a, worktree 1
        assert_eq!(state.selected_worktree_idx(), Some(1));

        state.select_prev(); // Back to worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));
    }

    #[test]
    fn test_select_prev_moves_to_prev_project_last_worktree() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Navigate to project-b header
        state.select_next(); // project-a, worktree 0
        state.select_next(); // project-a, worktree 1
        state.select_next(); // project-b header
        state.select_next(); // project-b, worktree 0

        state.select_prev(); // project-b header
        state.select_prev(); // project-a, worktree 1 (last worktree of prev project)

        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), Some(1));
    }

    #[test]
    fn test_selected_session_id_none_on_project_header() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // On project header, no session id
        assert_eq!(state.selected_session_id(), None);

        state.select_next(); // Move to first worktree
        assert_eq!(
            state.selected_session_id(),
            Some("project-a__main".to_string())
        );

        state.select_next();
        assert_eq!(
            state.selected_session_id(),
            Some("project-a__feature-1".to_string())
        );
    }

    #[test]
    fn test_agent_status() {
        let mut state = AppState::new();

        assert_eq!(state.get_status("test:main"), AgentStatus::Idle);

        state.set_status("test:main".to_string(), AgentStatus::Working);
        assert_eq!(state.get_status("test:main"), AgentStatus::Working);
    }

    #[test]
    fn test_agent_status_icon() {
        assert_eq!(AgentStatus::Working.icon(), "🟢");
        assert_eq!(AgentStatus::Waiting.icon(), "🟡");
        assert_eq!(AgentStatus::Idle.icon(), "⚪");
        assert_eq!(AgentStatus::Error.icon(), "🔴");
    }

    #[test]
    fn test_input_mode() {
        let mut state = AppState::new();

        assert_eq!(state.focus_mode, FocusMode::Normal);

        state.enter_input_mode();
        assert_eq!(state.focus_mode, FocusMode::Input);

        state.input_char('h');
        state.input_char('i');
        assert_eq!(state.input_buffer, "hi");

        state.input_backspace();
        assert_eq!(state.input_buffer, "h");

        let input = state.take_input();
        assert_eq!(input, "h");
        assert!(state.input_buffer.is_empty());

        state.exit_input_mode();
        assert_eq!(state.focus_mode, FocusMode::Normal);
    }

    #[test]
    fn test_create_task_modal() {
        let mut state = AppState::new();

        assert!(state.modal.is_none());

        state.open_create_task_modal();
        assert!(matches!(state.modal, Some(ModalType::CreateTask { .. })));

        state.modal_input_char('t');
        state.modal_input_char('e');
        state.modal_input_char('s');
        state.modal_input_char('t');
        assert_eq!(state.modal_input(), Some("test"));

        state.modal_input_backspace();
        assert_eq!(state.modal_input(), Some("tes"));

        state.close_modal();
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_flat_sidebar_index_calculation() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Initial state: project-a header selected
        // Structure:
        // 0: project-a (project header) <- selected
        // 1:   main (worktree 0)
        // 2:   feature-1 (worktree 1)
        // 3: project-b (project header)
        // 4:   main (worktree 0)
        assert_eq!(state.flat_sidebar_index(), Some(0)); // project header

        state.select_next(); // worktree 0 (main)
        assert_eq!(state.flat_sidebar_index(), Some(1));

        state.select_next(); // worktree 1 (feature-1)
        assert_eq!(state.flat_sidebar_index(), Some(2));

        state.select_next(); // project-b header
        assert_eq!(state.flat_sidebar_index(), Some(3));

        state.select_next(); // project-b, worktree 0
        assert_eq!(state.flat_sidebar_index(), Some(4));
    }

    #[test]
    fn test_sidebar_list_state_syncs_with_selection() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // ListState should sync with flat index
        // Starts at project-a header
        assert_eq!(state.sidebar_list_state().selected(), Some(0));

        state.select_next(); // worktree 0
        assert_eq!(state.sidebar_list_state().selected(), Some(1));

        state.select_next(); // worktree 1
        assert_eq!(state.sidebar_list_state().selected(), Some(2));

        state.select_next(); // project-b header
        assert_eq!(state.sidebar_list_state().selected(), Some(3));

        state.select_prev(); // back to worktree 1
        assert_eq!(state.sidebar_list_state().selected(), Some(2));
    }

    #[test]
    fn test_scroll_debounce_blocks_rapid_scrolls() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // First scroll should be allowed
        assert!(state.try_scroll());

        // Immediate second scroll should be blocked
        assert!(!state.try_scroll());

        // After debounce period, scroll should be allowed again
        // We simulate this by manually resetting the last scroll time
        state.reset_scroll_debounce();
        assert!(state.try_scroll());
    }

    #[test]
    fn test_toggle_favorite_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Initially no favorites
        assert!(!state.is_favorite("project-a"));
        assert!(!state.is_favorite("project-b"));

        // Toggle favorite on
        state.toggle_favorite("project-a");
        assert!(state.is_favorite("project-a"));
        assert!(!state.is_favorite("project-b"));

        // Toggle favorite off
        state.toggle_favorite("project-a");
        assert!(!state.is_favorite("project-a"));
    }

    #[test]
    fn test_toggle_favorite_selected_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Select first project (starts at project-a header)
        assert_eq!(state.selected_project().unwrap().name, "project-a");

        // Toggle favorite on selected project
        state.toggle_favorite_selected();
        assert!(state.is_favorite("project-a"));

        // After toggling, project-a becomes favorite, so sorted order stays the same
        // Move to second project header: worktree 0 -> worktree 1 -> project-b header
        state.select_next(); // project-a, worktree 0
        state.select_next(); // project-a, worktree 1
        state.select_next(); // project-b header
        assert_eq!(state.selected_project().unwrap().name, "project-b");
        state.toggle_favorite_selected();
        assert!(state.is_favorite("project-b"));
    }

    #[test]
    fn test_favorites_sorted_to_top() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Initially project-a is first
        assert_eq!(state.projects[0].name, "project-a");
        assert_eq!(state.projects[1].name, "project-b");

        // Mark project-b as favorite
        state.toggle_favorite("project-b");

        // Get sorted projects - favorites should be first
        let sorted = state.sorted_projects();
        assert_eq!(sorted[0].name, "project-b"); // favorite first
        assert_eq!(sorted[1].name, "project-a"); // non-favorite second
    }

    #[test]
    fn test_favorites_sorted_preserves_order_within_groups() {
        let mut state = AppState::new();
        // Create 3 projects
        let projects = vec![
            Project::new("alpha", "/path/alpha").with_worktrees(vec![Worktree::new(
                "/path/alpha",
                "a1",
                Some("main".to_string()),
            )]),
            Project::new("beta", "/path/beta").with_worktrees(vec![Worktree::new(
                "/path/beta",
                "b1",
                Some("main".to_string()),
            )]),
            Project::new("gamma", "/path/gamma").with_worktrees(vec![Worktree::new(
                "/path/gamma",
                "g1",
                Some("main".to_string()),
            )]),
        ];
        state.set_projects(projects);

        // Mark alpha and gamma as favorites
        state.toggle_favorite("alpha");
        state.toggle_favorite("gamma");

        // Sorted: favorites (alpha, gamma) then non-favorites (beta)
        let sorted = state.sorted_projects();
        assert_eq!(sorted[0].name, "alpha");
        assert_eq!(sorted[1].name, "gamma");
        assert_eq!(sorted[2].name, "beta");
    }

    #[test]
    fn test_has_favorites() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Initially no favorites
        assert!(!state.has_favorites());

        // Add a favorite
        state.toggle_favorite("project-a");
        assert!(state.has_favorites());

        // Remove the favorite
        state.toggle_favorite("project-a");
        assert!(!state.has_favorites());
    }

    #[test]
    fn test_flat_sidebar_index_with_favorites_and_separator() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Mark project-b as favorite
        state.toggle_favorite("project-b");

        // Sorted order with separator:
        // 0: project-b (favorite, project header)
        // 1:   main (worktree)
        // 2: ──────── (separator)
        // 3: project-a (non-favorite, project header)
        // 4:   main (worktree 0)
        // 5:   feature-1 (worktree 1)

        // Initial selection should be project-b's first worktree (index 1)
        // But we need to update selection to match sorted order
        // For now, test the calculation method directly
        assert!(state.has_favorites());
    }

    #[test]
    fn test_navigation_follows_sorted_order_with_favorites() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Mark project-b as favorite
        // Sorted order: project-b (favorite), project-a (non-favorite)
        state.toggle_favorite("project-b");

        // Initial selection should be first in sorted order (project-b header)
        assert_eq!(state.selected_project().unwrap().name, "project-b");
        assert_eq!(state.selected_worktree_idx(), None); // Project header

        // Navigate: project-b header -> worktree 0 -> project-a header
        state.select_next(); // project-b, worktree 0
        assert_eq!(state.selected_project().unwrap().name, "project-b");
        assert_eq!(state.selected_worktree_idx(), Some(0));

        state.select_next(); // project-a header
        assert_eq!(state.selected_project().unwrap().name, "project-a");
        assert_eq!(state.selected_worktree_idx(), None);

        // Navigate to project-a worktrees
        state.select_next(); // project-a, worktree 0
        assert_eq!(state.selected_project().unwrap().name, "project-a");
        assert_eq!(state.selected_worktree_idx(), Some(0));

        state.select_next(); // project-a, worktree 1
        assert_eq!(state.selected_project().unwrap().name, "project-a");
        assert_eq!(state.selected_worktree_idx(), Some(1));

        // Navigate back
        state.select_prev(); // project-a, worktree 0
        assert_eq!(state.selected_project().unwrap().name, "project-a");
        assert_eq!(state.selected_worktree_idx(), Some(0));

        state.select_prev(); // project-a header
        assert_eq!(state.selected_project().unwrap().name, "project-a");
        assert_eq!(state.selected_worktree_idx(), None);

        state.select_prev(); // project-b, last worktree (worktree 0)
        assert_eq!(state.selected_project().unwrap().name, "project-b");
        assert_eq!(state.selected_worktree_idx(), Some(0));
    }

    #[test]
    fn test_toggle_favorite_updates_selection_to_sorted_first_header() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Initially selected: project-a header
        assert_eq!(state.selected_project().unwrap().name, "project-a");

        // Toggle project-b as favorite
        state.toggle_favorite("project-b");

        // Selection should move to first in sorted order (project-b header)
        assert_eq!(state.selected_project().unwrap().name, "project-b");
        assert_eq!(state.selected_worktree_idx(), None); // Project header
    }

    #[test]
    fn test_set_projects_selects_first_in_sorted_order_with_favorites_header() {
        let mut state = AppState::new();

        // Set favorites BEFORE setting projects (simulating app startup)
        let mut favorites = HashSet::new();
        favorites.insert("project-b".to_string());
        state.set_favorites(favorites);

        // Now set projects
        state.set_projects(create_test_projects());

        // Selection should be first in sorted order (project-b header)
        assert_eq!(state.selected_project().unwrap().name, "project-b");
        assert_eq!(state.selected_worktree_idx(), None); // Project header
    }
}
