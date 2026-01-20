use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::state::{AppState, FocusMode};

/// Action to be performed by the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// No action to perform.
    None,
    /// Quit the application.
    Quit,
    /// Attach to the selected tmux session.
    AttachSession,
    /// Send input to the current tmux pane.
    SendInput(String),
    /// Create a new task/worktree with the given branch name.
    CreateTask(String),
    /// Refresh pane preview for the selected worktree.
    RefreshPreview,
    /// Toggle favorite status of the selected project.
    ToggleFavorite,
    /// Delete a task/worktree with the given branch name.
    DeleteTask(String),
}

/// Poll for terminal events with a timeout.
pub fn poll_event(timeout: Duration) -> Result<Option<Event>> {
    if event::poll(timeout)? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}

/// Handle a key event by updating the application state.
/// Returns an Action that the main loop should perform.
pub fn handle_key_event(key: KeyEvent, state: &mut AppState) -> Action {
    // Handle modal input if a modal is open
    if state.modal.is_some() {
        return handle_modal_key_event(key, state);
    }

    // Handle based on focus mode
    match state.focus_mode {
        FocusMode::Normal => handle_normal_key_event(key, state),
        FocusMode::Input => handle_input_key_event(key, state),
    }
}

/// Handle a mouse event by updating the application state.
/// Returns an Action that the main loop should perform.
pub fn handle_mouse_event(mouse: MouseEvent, state: &mut AppState) -> Action {
    // Ignore mouse events when modal is open or in input mode
    if state.modal.is_some() || state.focus_mode == FocusMode::Input {
        return Action::None;
    }

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            // Apply debounce to prevent rapid scrolling
            if !state.try_scroll() {
                return Action::None;
            }
            state.clear_status_message();
            state.select_next();
            Action::RefreshPreview
        }
        MouseEventKind::ScrollUp => {
            // Apply debounce to prevent rapid scrolling
            if !state.try_scroll() {
                return Action::None;
            }
            state.clear_status_message();
            state.select_prev();
            Action::RefreshPreview
        }
        _ => Action::None,
    }
}

fn handle_normal_key_event(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        // Quit
        KeyCode::Char('q') => {
            state.quit();
            Action::Quit
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.quit();
            Action::Quit
        }

        // Navigation - clear status message when navigating
        KeyCode::Char('j') | KeyCode::Down => {
            state.clear_status_message();
            state.select_next();
            Action::RefreshPreview
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.clear_status_message();
            state.select_prev();
            Action::RefreshPreview
        }

        // Attach to session
        KeyCode::Enter | KeyCode::Char('o') => {
            state.clear_status_message();
            Action::AttachSession
        }

        // Input mode
        KeyCode::Char('i') => {
            state.clear_status_message();
            state.enter_input_mode();
            Action::None
        }

        // Toggle favorite
        KeyCode::Char('f') => {
            state.clear_status_message();
            Action::ToggleFavorite
        }

        // New task modal - clear status message when opening modal
        KeyCode::Char('n') => {
            state.clear_status_message();
            state.open_create_task_modal();
            Action::None
        }

        // Delete task - only for non-main/master worktrees (d or D)
        KeyCode::Char('d') | KeyCode::Char('D') => {
            state.clear_status_message();
            if let Some(worktree) = state.selected_worktree() {
                if let Some(branch) = &worktree.branch {
                    // Don't allow deleting main or master branches
                    if branch == "main" || branch == "master" {
                        state.set_error_message(format!("Cannot delete '{branch}' branch"));
                    } else {
                        state.open_confirm_deletion_modal(branch.clone());
                    }
                } else {
                    state.set_error_message("Cannot delete detached worktree");
                }
            } else {
                state.set_error_message("Select a worktree to delete");
            }
            Action::None
        }

        _ => Action::None,
    }
}

fn handle_input_key_event(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.exit_input_mode();
            Action::None
        }
        KeyCode::Enter => {
            let input = state.take_input();
            state.exit_input_mode();
            if input.is_empty() {
                Action::None
            } else {
                Action::SendInput(input)
            }
        }
        KeyCode::Backspace => {
            state.input_backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.input_char(c);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_modal_key_event(key: KeyEvent, state: &mut AppState) -> Action {
    use crate::state::ModalType;

    // Check which modal is open and handle accordingly
    match &state.modal {
        Some(ModalType::CreateTask { .. }) => handle_create_task_modal_key(key, state),
        Some(ModalType::ConfirmDeletion { .. }) => handle_confirm_deletion_modal_key(key, state),
        None => Action::None,
    }
}

fn handle_create_task_modal_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.close_modal();
            Action::None
        }
        KeyCode::Enter => {
            let input = state.modal_input().unwrap_or("").to_string();
            state.close_modal();
            if input.is_empty() {
                Action::None
            } else {
                Action::CreateTask(input)
            }
        }
        KeyCode::Backspace => {
            state.modal_input_backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.modal_input_char(c);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_confirm_deletion_modal_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            state.close_modal();
            Action::None
        }
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let branch_name = state.deletion_branch_name().unwrap_or("").to_string();
            state.close_modal();
            if branch_name.is_empty() {
                Action::None
            } else {
                Action::DeleteTask(branch_name)
            }
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{Project, Worktree};

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn create_test_state_with_projects() -> AppState {
        let mut state = AppState::new();
        state.set_projects(vec![
            Project::new("project-a", "/path/to/a").with_worktrees(vec![
                Worktree::new("/path/to/a", "abc", Some("main".to_string())),
                Worktree::new("/path/to/a/feature", "def", Some("feature".to_string())),
            ]),
            Project::new("project-b", "/path/to/b").with_worktrees(vec![Worktree::new(
                "/path/to/b",
                "ghi",
                Some("main".to_string()),
            )]),
        ]);
        state
    }

    #[test]
    fn test_quit_on_q() {
        let mut state = AppState::new();
        let action = handle_key_event(key_event(KeyCode::Char('q')), &mut state);
        assert!(state.should_quit());
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn test_quit_on_ctrl_c() {
        let mut state = AppState::new();
        let action = handle_key_event(
            key_event_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut state,
        );
        assert!(state.should_quit());
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn test_navigate_next_on_j() {
        let mut state = create_test_state_with_projects();
        // Starts at project header (worktree_idx = None)
        assert_eq!(state.selected_worktree_idx(), None);

        let action = handle_key_event(key_event(KeyCode::Char('j')), &mut state);
        // Now at worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_navigate_next_on_down() {
        let mut state = create_test_state_with_projects();
        // Starts at project header
        assert_eq!(state.selected_worktree_idx(), None);

        let action = handle_key_event(key_event(KeyCode::Down), &mut state);
        // Now at worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_navigate_prev_on_k() {
        let mut state = create_test_state_with_projects();
        state.select_next(); // Move to worktree 0
        state.select_next(); // Move to worktree 1
        assert_eq!(state.selected_worktree_idx(), Some(1));

        let action = handle_key_event(key_event(KeyCode::Char('k')), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_navigate_prev_on_up() {
        let mut state = create_test_state_with_projects();
        state.select_next(); // worktree 0
        state.select_next(); // worktree 1
        let action = handle_key_event(key_event(KeyCode::Up), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_attach_session_on_enter() {
        let mut state = create_test_state_with_projects();
        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        assert_eq!(action, Action::AttachSession);
    }

    #[test]
    fn test_attach_session_on_o() {
        let mut state = create_test_state_with_projects();
        let action = handle_key_event(key_event(KeyCode::Char('o')), &mut state);
        assert_eq!(action, Action::AttachSession);
    }

    #[test]
    fn test_enter_input_mode_on_i() {
        let mut state = AppState::new();
        let action = handle_key_event(key_event(KeyCode::Char('i')), &mut state);
        assert_eq!(state.focus_mode, FocusMode::Input);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_open_create_modal_on_n() {
        let mut state = AppState::new();
        let action = handle_key_event(key_event(KeyCode::Char('n')), &mut state);
        assert!(state.modal.is_some());
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_input_mode_typing() {
        let mut state = AppState::new();
        state.enter_input_mode();

        handle_key_event(key_event(KeyCode::Char('h')), &mut state);
        handle_key_event(key_event(KeyCode::Char('i')), &mut state);
        assert_eq!(state.input_buffer, "hi");
    }

    #[test]
    fn test_input_mode_backspace() {
        let mut state = AppState::new();
        state.enter_input_mode();

        handle_key_event(key_event(KeyCode::Char('h')), &mut state);
        handle_key_event(key_event(KeyCode::Char('i')), &mut state);
        handle_key_event(key_event(KeyCode::Backspace), &mut state);
        assert_eq!(state.input_buffer, "h");
    }

    #[test]
    fn test_input_mode_submit() {
        let mut state = AppState::new();
        state.enter_input_mode();

        handle_key_event(key_event(KeyCode::Char('c')), &mut state);
        handle_key_event(key_event(KeyCode::Char('m')), &mut state);
        handle_key_event(key_event(KeyCode::Char('d')), &mut state);

        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        assert_eq!(action, Action::SendInput("cmd".to_string()));
        assert_eq!(state.focus_mode, FocusMode::Normal);
    }

    #[test]
    fn test_input_mode_cancel() {
        let mut state = AppState::new();
        state.enter_input_mode();

        handle_key_event(key_event(KeyCode::Char('t')), &mut state);
        let action = handle_key_event(key_event(KeyCode::Esc), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.focus_mode, FocusMode::Normal);
    }

    #[test]
    fn test_modal_typing_and_submit() {
        let mut state = AppState::new();
        state.open_create_task_modal();

        handle_key_event(key_event(KeyCode::Char('f')), &mut state);
        handle_key_event(key_event(KeyCode::Char('e')), &mut state);
        handle_key_event(key_event(KeyCode::Char('a')), &mut state);
        handle_key_event(key_event(KeyCode::Char('t')), &mut state);

        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        assert_eq!(action, Action::CreateTask("feat".to_string()));
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_modal_cancel() {
        let mut state = AppState::new();
        state.open_create_task_modal();

        handle_key_event(key_event(KeyCode::Char('t')), &mut state);
        let action = handle_key_event(key_event(KeyCode::Esc), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_unknown_key_does_nothing() {
        let mut state = AppState::new();
        let action = handle_key_event(key_event(KeyCode::Char('x')), &mut state);
        assert!(!state.should_quit());
        assert_eq!(action, Action::None);
    }

    fn mouse_scroll_down() -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn mouse_scroll_up() -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn test_mouse_scroll_down_moves_one_item() {
        let mut state = create_test_state_with_projects();
        // Starts at project header
        assert_eq!(state.selected_worktree_idx(), None);

        // Scroll down should move exactly one item (to worktree 0)
        let action = handle_mouse_event(mouse_scroll_down(), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(action, Action::RefreshPreview);

        // Reset debounce for next scroll test
        state.reset_scroll_debounce();

        // Scroll down again (to worktree 1)
        let action = handle_mouse_event(mouse_scroll_down(), &mut state);
        assert_eq!(state.selected_project_idx(), Some(0));
        assert_eq!(state.selected_worktree_idx(), Some(1));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_mouse_scroll_up_moves_one_item() {
        let mut state = create_test_state_with_projects();
        // Move to project-b worktree
        state.select_next(); // worktree 0
        state.select_next(); // worktree 1
        state.select_next(); // project-b header
        state.select_next(); // project-b worktree 0
        assert_eq!(state.selected_project_idx(), Some(1));
        assert_eq!(state.selected_worktree_idx(), Some(0));

        // Scroll up should move exactly one item (to project-b header)
        let action = handle_mouse_event(mouse_scroll_up(), &mut state);
        assert_eq!(state.selected_project_idx(), Some(1));
        assert_eq!(state.selected_worktree_idx(), None); // Project header
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_mouse_scroll_debounce_blocks_rapid_scrolls() {
        let mut state = create_test_state_with_projects();
        // Starts at project header
        assert_eq!(state.selected_worktree_idx(), None);

        // First scroll should work (to worktree 0)
        let action = handle_mouse_event(mouse_scroll_down(), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(action, Action::RefreshPreview);

        // Immediate second scroll should be blocked by debounce
        let action = handle_mouse_event(mouse_scroll_down(), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(0)); // Still at 0, not moved
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_toggle_favorite_on_f() {
        let mut state = create_test_state_with_projects();
        let action = handle_key_event(key_event(KeyCode::Char('f')), &mut state);
        assert_eq!(action, Action::ToggleFavorite);
    }

    #[test]
    fn test_d_key_opens_deletion_modal_for_worktree() {
        let mut state = create_test_state_with_projects();
        // Select a worktree (not main/master) - navigate: header -> wt0 -> wt1(feature)
        state.select_next(); // worktree 0 (main)
        state.select_next(); // worktree 1 (feature)
        assert_eq!(state.selected_worktree_idx(), Some(1));

        let action = handle_key_event(key_event(KeyCode::Char('d')), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.modal.is_some());
    }

    #[test]
    fn test_d_key_does_not_delete_main_branch() {
        let mut state = create_test_state_with_projects();
        // Navigate to main worktree (header -> wt0)
        state.select_next(); // worktree 0 (main)
        assert_eq!(state.selected_worktree_idx(), Some(0));
        let worktree = state.selected_worktree().unwrap();
        assert_eq!(worktree.branch.as_deref(), Some("main"));

        let action = handle_key_event(key_event(KeyCode::Char('d')), &mut state);
        // Should not open modal for main branch
        assert_eq!(action, Action::None);
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_deletion_modal_confirm_with_y() {
        let mut state = create_test_state_with_projects();
        state.select_next(); // Move to feature worktree
        state.open_confirm_deletion_modal("feature".to_string());

        let action = handle_key_event(key_event(KeyCode::Char('y')), &mut state);
        assert_eq!(action, Action::DeleteTask("feature".to_string()));
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_deletion_modal_cancel_with_n() {
        let mut state = create_test_state_with_projects();
        state.open_confirm_deletion_modal("feature".to_string());

        let action = handle_key_event(key_event(KeyCode::Char('n')), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_deletion_modal_cancel_with_esc() {
        let mut state = create_test_state_with_projects();
        state.open_confirm_deletion_modal("feature".to_string());

        let action = handle_key_event(key_event(KeyCode::Esc), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.modal.is_none());
    }
}
