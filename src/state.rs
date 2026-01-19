//! Application state management.
//!
//! This module contains the central state for the TUI application,
//! including project data, selection state, and UI mode tracking.

// Allow dead code during development - some APIs are for future features
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

use crate::discovery::Project;

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
        // Auto-select first project if available
        if !self.projects.is_empty() {
            self.selected_project_idx = Some(0);
            if !self.projects[0].worktrees.is_empty() {
                self.selected_worktree_idx = Some(0);
            }
        }
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

    /// Navigate to the next item in the sidebar.
    pub fn select_next(&mut self) {
        if self.projects.is_empty() {
            return;
        }

        let Some(proj_idx) = self.selected_project_idx else {
            self.selected_project_idx = Some(0);
            self.selected_worktree_idx = if self.projects[0].worktrees.is_empty() {
                None
            } else {
                Some(0)
            };
            return;
        };

        let project = &self.projects[proj_idx];
        let wt_idx = self.selected_worktree_idx.unwrap_or(0);

        // Try to move to next worktree in current project
        if wt_idx + 1 < project.worktrees.len() {
            self.selected_worktree_idx = Some(wt_idx + 1);
        } else if proj_idx + 1 < self.projects.len() {
            // Move to next project
            self.selected_project_idx = Some(proj_idx + 1);
            self.selected_worktree_idx = if self.projects[proj_idx + 1].worktrees.is_empty() {
                None
            } else {
                Some(0)
            };
        }
        // Otherwise, stay at current position (at the end)
    }

    /// Navigate to the previous item in the sidebar.
    pub fn select_prev(&mut self) {
        if self.projects.is_empty() {
            return;
        }

        let Some(proj_idx) = self.selected_project_idx else {
            return;
        };

        let wt_idx = self.selected_worktree_idx.unwrap_or(0);

        // Try to move to previous worktree in current project
        if wt_idx > 0 {
            self.selected_worktree_idx = Some(wt_idx - 1);
        } else if proj_idx > 0 {
            // Move to previous project's last worktree
            let prev_proj_idx = proj_idx - 1;
            self.selected_project_idx = Some(prev_proj_idx);
            let prev_project = &self.projects[prev_proj_idx];
            self.selected_worktree_idx = if prev_project.worktrees.is_empty() {
                None
            } else {
                Some(prev_project.worktrees.len() - 1)
            };
        }
        // Otherwise, stay at current position (at the beginning)
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
    fn test_set_projects_auto_selects_first() {
        let mut state = AppState::new();
        let projects = create_test_projects();
        state.set_projects(projects);

        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(state.selected_project().unwrap().name, "project-a");
    }

    #[test]
    fn test_select_next_within_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        // Should start at project-a, worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));

        state.select_next();
        // Now at project-a, worktree 1 (feature-1)
        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), Some(1));
    }

    #[test]
    fn test_select_next_moves_to_next_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        state.select_next(); // project-a, worktree 1
        state.select_next(); // project-b, worktree 0

        assert_eq!(state.selected_project_idx(), Some(1));
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(state.selected_project().unwrap().name, "project-b");
    }

    #[test]
    fn test_select_prev_within_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        state.select_next(); // Move to worktree 1
        assert_eq!(state.selected_worktree_idx(), Some(1));

        state.select_prev(); // Back to worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));
    }

    #[test]
    fn test_select_prev_moves_to_prev_project() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

        state.select_next(); // project-a, worktree 1
        state.select_next(); // project-b, worktree 0
        state.select_prev(); // project-a, worktree 1 (last worktree of prev project)

        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), Some(1));
    }

    #[test]
    fn test_selected_session_id() {
        let mut state = AppState::new();
        state.set_projects(create_test_projects());

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
}
