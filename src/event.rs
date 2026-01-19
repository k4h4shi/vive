use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

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

        // New task modal - clear status message when opening modal
        KeyCode::Char('n') => {
            state.clear_status_message();
            state.open_create_task_modal();
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
        assert_eq!(state.selected_worktree_idx(), Some(0));

        let action = handle_key_event(key_event(KeyCode::Char('j')), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(1));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_navigate_next_on_down() {
        let mut state = create_test_state_with_projects();
        let action = handle_key_event(key_event(KeyCode::Down), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(1));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_navigate_prev_on_k() {
        let mut state = create_test_state_with_projects();
        state.select_next(); // Move to worktree 1
        assert_eq!(state.selected_worktree_idx(), Some(1));

        let action = handle_key_event(key_event(KeyCode::Char('k')), &mut state);
        assert_eq!(state.selected_worktree_idx(), Some(0));
        assert_eq!(action, Action::RefreshPreview);
    }

    #[test]
    fn test_navigate_prev_on_up() {
        let mut state = create_test_state_with_projects();
        state.select_next();
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
}
