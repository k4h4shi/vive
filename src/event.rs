use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::state::{AppState, FocusMode, FocusPane};

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
    /// Toggle expanded state of the selected project.
    ToggleExpanded,
    /// Delete a task/worktree with the given branch name.
    DeleteTask(String),
    /// Fetch issues for the issue picker modal.
    FetchIssues,
    /// Create a task from the selected issue.
    CreateTaskFromIssue(crate::github::GitHubIssue),
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
///
/// Uses the cached sidebar_width and preview_visible_height from state
/// (set during rendering) for layout-aware mouse handling.
pub fn handle_mouse_event(mouse: MouseEvent, state: &mut AppState) -> Action {
    // Ignore mouse events when modal is open or in input mode
    if state.modal.is_some() || state.focus_mode == FocusMode::Input {
        return Action::None;
    }

    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            // Click to switch focus between panes using cached sidebar_width
            let sidebar_width = state.sidebar_width();
            if sidebar_width > 0 {
                state.clear_status_message();
                if mouse.column < sidebar_width {
                    state.focus_sidebar();
                } else {
                    state.focus_preview();
                }
            }
            Action::None
        }
        MouseEventKind::ScrollDown => {
            // Apply debounce to prevent rapid scrolling
            if !state.try_scroll() {
                return Action::None;
            }
            state.clear_status_message();

            // Scroll behavior depends on focused pane
            match state.focus_pane() {
                FocusPane::Sidebar => {
                    state.select_next();
                    Action::RefreshPreview
                }
                FocusPane::Preview => {
                    state.scroll_preview_down();
                    Action::None
                }
            }
        }
        MouseEventKind::ScrollUp => {
            // Apply debounce to prevent rapid scrolling
            if !state.try_scroll() {
                return Action::None;
            }
            state.clear_status_message();

            // Scroll behavior depends on focused pane
            match state.focus_pane() {
                FocusPane::Sidebar => {
                    state.select_prev();
                    Action::RefreshPreview
                }
                FocusPane::Preview => {
                    state.scroll_preview_up();
                    Action::None
                }
            }
        }
        _ => Action::None,
    }
}

fn handle_normal_key_event(key: KeyEvent, state: &mut AppState) -> Action {
    // Global keys that work regardless of focus pane
    match key.code {
        // Quit
        KeyCode::Char('q') => {
            state.quit();
            return Action::Quit;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.quit();
            return Action::Quit;
        }

        // Tab toggles focus between sidebar and preview
        KeyCode::Tab => {
            state.clear_status_message();
            state.toggle_focus_pane();
            return Action::None;
        }

        // h/l also switch focus panes (vim-style left/right)
        KeyCode::Char('h') | KeyCode::Left => {
            state.clear_status_message();
            state.focus_sidebar();
            return Action::None;
        }
        KeyCode::Char('l') | KeyCode::Right => {
            state.clear_status_message();
            state.focus_preview();
            return Action::None;
        }

        _ => {}
    }

    // Dispatch to pane-specific handlers
    match state.focus_pane() {
        FocusPane::Sidebar => handle_sidebar_key_event(key, state),
        FocusPane::Preview => handle_preview_key_event(key, state),
    }
}

/// Handle key events when sidebar pane is focused.
fn handle_sidebar_key_event(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
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

        // Toggle expand/collapse (Space key)
        KeyCode::Char(' ') => {
            state.clear_status_message();
            Action::ToggleExpanded
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

/// Handle key events when preview pane is focused.
/// Scroll calculations use the cached preview_visible_height from state.
fn handle_preview_key_event(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        // j/k scroll the preview content (1 line at a time)
        KeyCode::Char('j') | KeyCode::Down => {
            state.clear_status_message();
            state.scroll_preview_down();
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.clear_status_message();
            state.scroll_preview_up();
            Action::None
        }

        // Ctrl-d/Ctrl-u for half-page scroll
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.clear_status_message();
            state.scroll_preview_page_down();
            Action::None
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.clear_status_message();
            state.scroll_preview_page_up();
            Action::None
        }

        // g/G for top/bottom (vim-style)
        KeyCode::Char('g') => {
            state.clear_status_message();
            state.reset_preview_scroll_to_top();
            Action::None
        }
        KeyCode::Char('G') => {
            state.clear_status_message();
            state.reset_preview_scroll();
            Action::None
        }

        // Enter/o still attaches to session (useful when viewing preview)
        KeyCode::Enter | KeyCode::Char('o') => {
            state.clear_status_message();
            Action::AttachSession
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
        KeyCode::Left => {
            state.input_cursor_left();
            Action::None
        }
        KeyCode::Right => {
            state.input_cursor_right();
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
        Some(ModalType::CreateTaskMethod { .. }) => handle_create_task_method_modal_key(key, state),
        Some(ModalType::CreateTask { .. }) => handle_create_task_modal_key(key, state),
        Some(ModalType::IssuePicker { .. }) => handle_issue_picker_modal_key(key, state),
        Some(ModalType::ConfirmDeletion { .. }) => handle_confirm_deletion_modal_key(key, state),
        None => Action::None,
    }
}

fn handle_create_task_method_modal_key(key: KeyEvent, state: &mut AppState) -> Action {
    use crate::state::CreateTaskMethod;

    match key.code {
        KeyCode::Esc => {
            state.close_modal();
            Action::None
        }
        KeyCode::Enter => {
            let method = state.selected_create_task_method();
            state.close_modal();
            match method {
                Some(CreateTaskMethod::Manual) => {
                    state.open_manual_create_task_modal();
                    Action::None
                }
                Some(CreateTaskMethod::PickFromIssue) => {
                    state.open_issue_picker_modal();
                    Action::FetchIssues
                }
                None => Action::None,
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.toggle_create_task_method();
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.toggle_create_task_method();
            Action::None
        }
        KeyCode::Char('m') | KeyCode::Char('M') => {
            state.close_modal();
            state.open_manual_create_task_modal();
            Action::None
        }
        KeyCode::Char('i') | KeyCode::Char('I') => {
            state.close_modal();
            state.open_issue_picker_modal();
            Action::FetchIssues
        }
        _ => Action::None,
    }
}

fn handle_issue_picker_modal_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.close_modal();
            Action::None
        }
        KeyCode::Enter => {
            if let Some(issue) = state.selected_issue().cloned() {
                state.close_modal();
                Action::CreateTaskFromIssue(issue)
            } else {
                Action::None
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.issue_picker_select_next();
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.issue_picker_select_prev();
            Action::None
        }
        KeyCode::Backspace => {
            state.issue_picker_filter_backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.issue_picker_filter_char(c);
            Action::None
        }
        _ => Action::None,
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
        // Expand both projects for tests that expect worktree navigation
        state.toggle_expanded("project-a");
        state.toggle_expanded("project-b");
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
    fn test_input_mode_cursor_left() {
        let mut state = AppState::new();
        state.enter_input_mode();

        handle_key_event(key_event(KeyCode::Char('a')), &mut state);
        handle_key_event(key_event(KeyCode::Char('b')), &mut state);
        handle_key_event(key_event(KeyCode::Char('c')), &mut state);
        assert_eq!(state.input_cursor(), 3);

        let action = handle_key_event(key_event(KeyCode::Left), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.input_cursor(), 2);

        let action = handle_key_event(key_event(KeyCode::Left), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.input_cursor(), 1);
    }

    #[test]
    fn test_input_mode_cursor_right() {
        let mut state = AppState::new();
        state.enter_input_mode();

        handle_key_event(key_event(KeyCode::Char('a')), &mut state);
        handle_key_event(key_event(KeyCode::Char('b')), &mut state);
        handle_key_event(key_event(KeyCode::Left), &mut state);
        handle_key_event(key_event(KeyCode::Left), &mut state);
        assert_eq!(state.input_cursor(), 0);

        let action = handle_key_event(key_event(KeyCode::Right), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.input_cursor(), 1);
    }

    #[test]
    fn test_input_mode_insert_at_cursor() {
        let mut state = AppState::new();
        state.enter_input_mode();

        // Type "ac"
        handle_key_event(key_event(KeyCode::Char('a')), &mut state);
        handle_key_event(key_event(KeyCode::Char('c')), &mut state);

        // Move cursor left, insert 'b'
        handle_key_event(key_event(KeyCode::Left), &mut state);
        handle_key_event(key_event(KeyCode::Char('b')), &mut state);

        assert_eq!(state.input_buffer, "abc");
    }

    #[test]
    fn test_modal_typing_and_submit() {
        let mut state = AppState::new();
        // Use open_manual_create_task_modal for the direct input test
        state.open_manual_create_task_modal();

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
        state.open_manual_create_task_modal();

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

    // ========== Create Task Method Modal Tests ==========

    #[test]
    fn test_create_task_method_modal_opens_on_n() {
        let mut state = AppState::new();
        let action = handle_key_event(key_event(KeyCode::Char('n')), &mut state);
        assert_eq!(action, Action::None);
        assert!(matches!(
            state.modal,
            Some(crate::state::ModalType::CreateTaskMethod { .. })
        ));
    }

    #[test]
    fn test_create_task_method_modal_toggle_with_j() {
        let mut state = AppState::new();
        state.open_create_task_modal();

        // Initially Manual is selected
        assert_eq!(
            state.selected_create_task_method(),
            Some(crate::state::CreateTaskMethod::Manual)
        );

        // Press j to toggle to PickFromIssue
        let action = handle_key_event(key_event(KeyCode::Char('j')), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(
            state.selected_create_task_method(),
            Some(crate::state::CreateTaskMethod::PickFromIssue)
        );
    }

    #[test]
    fn test_create_task_method_modal_select_manual_with_m() {
        let mut state = AppState::new();
        state.open_create_task_modal();

        // Press m to select Manual directly
        let action = handle_key_event(key_event(KeyCode::Char('m')), &mut state);
        assert_eq!(action, Action::None);
        assert!(matches!(
            state.modal,
            Some(crate::state::ModalType::CreateTask { .. })
        ));
    }

    #[test]
    fn test_create_task_method_modal_select_issue_with_i() {
        let mut state = AppState::new();
        state.open_create_task_modal();

        // Press i to select PickFromIssue
        let action = handle_key_event(key_event(KeyCode::Char('i')), &mut state);
        assert_eq!(action, Action::FetchIssues);
        assert!(matches!(
            state.modal,
            Some(crate::state::ModalType::IssuePicker { .. })
        ));
    }

    #[test]
    fn test_create_task_method_modal_confirm_manual_with_enter() {
        let mut state = AppState::new();
        state.open_create_task_modal();

        // Manual is selected by default, press Enter
        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        assert_eq!(action, Action::None);
        assert!(matches!(
            state.modal,
            Some(crate::state::ModalType::CreateTask { .. })
        ));
    }

    #[test]
    fn test_create_task_method_modal_confirm_issue_with_enter() {
        let mut state = AppState::new();
        state.open_create_task_modal();
        state.toggle_create_task_method(); // Select PickFromIssue

        // Press Enter to confirm
        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        assert_eq!(action, Action::FetchIssues);
        assert!(matches!(
            state.modal,
            Some(crate::state::ModalType::IssuePicker { .. })
        ));
    }

    #[test]
    fn test_create_task_method_modal_cancel_with_esc() {
        let mut state = AppState::new();
        state.open_create_task_modal();

        let action = handle_key_event(key_event(KeyCode::Esc), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.modal.is_none());
    }

    // ========== Issue Picker Modal Tests ==========

    #[test]
    fn test_issue_picker_modal_cancel_with_esc() {
        let mut state = AppState::new();
        state.open_issue_picker_modal();

        let action = handle_key_event(key_event(KeyCode::Esc), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_issue_picker_modal_navigate_with_j_k() {
        use crate::github::GitHubIssue;

        let mut state = AppState::new();
        state.open_issue_picker_modal();
        state.set_issue_picker_issues(vec![
            GitHubIssue::new(1, "First"),
            GitHubIssue::new(2, "Second"),
        ]);

        // Navigate down
        let action = handle_key_event(key_event(KeyCode::Char('j')), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.selected_issue().map(|i| i.number), Some(2));

        // Navigate up
        let action = handle_key_event(key_event(KeyCode::Char('k')), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.selected_issue().map(|i| i.number), Some(1));
    }

    #[test]
    fn test_issue_picker_modal_filter_with_typing() {
        use crate::github::GitHubIssue;

        let mut state = AppState::new();
        state.open_issue_picker_modal();
        state.set_issue_picker_issues(vec![
            GitHubIssue::new(1, "Add feature"),
            GitHubIssue::new(2, "Fix bug"),
        ]);

        // Type filter characters
        let action = handle_key_event(key_event(KeyCode::Char('b')), &mut state);
        assert_eq!(action, Action::None);

        let action = handle_key_event(key_event(KeyCode::Char('u')), &mut state);
        assert_eq!(action, Action::None);

        let action = handle_key_event(key_event(KeyCode::Char('g')), &mut state);
        assert_eq!(action, Action::None);

        // Should filter to just "Fix bug"
        assert_eq!(state.filtered_issues().len(), 1);
        assert_eq!(state.selected_issue().map(|i| i.number), Some(2));
    }

    #[test]
    fn test_issue_picker_modal_select_with_enter() {
        use crate::github::GitHubIssue;

        let mut state = AppState::new();
        state.open_issue_picker_modal();
        state.set_issue_picker_issues(vec![GitHubIssue::new(42, "Test issue")]);

        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        match action {
            Action::CreateTaskFromIssue(issue) => {
                assert_eq!(issue.number, 42);
                assert_eq!(issue.title, "Test issue");
            }
            _ => panic!("Expected CreateTaskFromIssue action"),
        }
        assert!(state.modal.is_none());
    }

    #[test]
    fn test_issue_picker_modal_enter_with_no_issues() {
        let mut state = AppState::new();
        state.open_issue_picker_modal();
        // Don't add any issues

        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        assert_eq!(action, Action::None);
        // Modal should still be open
        assert!(state.modal.is_some());
    }

    #[test]
    fn test_toggle_expanded_on_space() {
        let mut state = create_test_state_with_projects();
        let action = handle_key_event(key_event(KeyCode::Char(' ')), &mut state);
        assert_eq!(action, Action::ToggleExpanded);
    }

    #[test]
    fn test_space_key_toggles_project_expansion() {
        // Use fresh state (not the helper that expands projects)
        let mut state = AppState::new();
        state.set_projects(vec![
            Project::new("project-a", "/path/to/a").with_worktrees(vec![Worktree::new(
                "/path/to/a",
                "abc",
                Some("main".to_string()),
            )]),
        ]);

        // Initially project-a is collapsed (non-favorite)
        assert!(!state.is_expanded("project-a"));

        // Press space returns ToggleExpanded action
        let action = handle_key_event(key_event(KeyCode::Char(' ')), &mut state);
        assert_eq!(action, Action::ToggleExpanded);
        // Note: The actual toggle happens in lib.rs handle_action, not here
    }

    // ========== Focus Pane Key Event Tests ==========

    #[test]
    fn test_tab_toggles_focus_pane() {
        let mut state = create_test_state_with_projects();
        assert!(state.is_sidebar_focused());

        // Tab should toggle focus to preview
        let action = handle_key_event(key_event(KeyCode::Tab), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.is_preview_focused());

        // Tab again should toggle back to sidebar
        let action = handle_key_event(key_event(KeyCode::Tab), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.is_sidebar_focused());
    }

    #[test]
    fn test_h_key_focuses_sidebar() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        assert!(state.is_preview_focused());

        let action = handle_key_event(key_event(KeyCode::Char('h')), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.is_sidebar_focused());
    }

    #[test]
    fn test_l_key_focuses_preview() {
        let mut state = create_test_state_with_projects();
        assert!(state.is_sidebar_focused());

        let action = handle_key_event(key_event(KeyCode::Char('l')), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.is_preview_focused());
    }

    #[test]
    fn test_left_key_focuses_sidebar() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();

        let action = handle_key_event(key_event(KeyCode::Left), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.is_sidebar_focused());
    }

    #[test]
    fn test_right_key_focuses_preview() {
        let mut state = create_test_state_with_projects();

        let action = handle_key_event(key_event(KeyCode::Right), &mut state);
        assert_eq!(action, Action::None);
        assert!(state.is_preview_focused());
    }

    // ========== Sidebar Focus Key Event Tests ==========

    #[test]
    fn test_j_navigates_when_sidebar_focused() {
        let mut state = create_test_state_with_projects();
        assert!(state.is_sidebar_focused());
        assert_eq!(state.selected_worktree_idx(), None); // At project header

        let action = handle_key_event(key_event(KeyCode::Char('j')), &mut state);
        assert_eq!(action, Action::RefreshPreview);
        assert_eq!(state.selected_worktree_idx(), Some(0)); // Moved to worktree
    }

    #[test]
    fn test_k_navigates_when_sidebar_focused() {
        let mut state = create_test_state_with_projects();
        state.select_next(); // Move to worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));

        let action = handle_key_event(key_event(KeyCode::Char('k')), &mut state);
        assert_eq!(action, Action::RefreshPreview);
        assert_eq!(state.selected_worktree_idx(), None); // Back to project header
    }

    // ========== Preview Focus Key Event Tests ==========

    #[test]
    fn test_j_scrolls_when_preview_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        assert!(state.is_preview_focused());

        let initial_offset = state.preview_scroll_offset();
        let action = handle_key_event(key_event(KeyCode::Char('j')), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.preview_scroll_offset(), initial_offset + 5); // Scrolls 5 lines
    }

    #[test]
    fn test_k_scrolls_when_preview_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        state.set_preview_visible_height(20);
        state.scroll_preview_down(); // Move down first (+5)
        let initial_offset = state.preview_scroll_offset();

        let action = handle_key_event(key_event(KeyCode::Char('k')), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(
            state.preview_scroll_offset(),
            initial_offset.saturating_sub(5) // Scrolls 5 lines
        );
    }

    #[test]
    fn test_ctrl_d_scrolls_page_down_when_preview_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        state.set_preview_visible_height(20);

        let action = handle_key_event(
            key_event_with_modifiers(KeyCode::Char('d'), KeyModifiers::CONTROL),
            &mut state,
        );
        assert_eq!(action, Action::None);
        // Should scroll by half a page (10 lines)
        assert!(state.preview_scroll_offset() > 0);
    }

    #[test]
    fn test_ctrl_u_scrolls_page_up_when_preview_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        state.set_preview_visible_height(20);
        // Scroll down first
        for _ in 0..20 {
            state.scroll_preview_down();
        }
        let initial_offset = state.preview_scroll_offset();

        let action = handle_key_event(
            key_event_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL),
            &mut state,
        );
        assert_eq!(action, Action::None);
        assert!(state.preview_scroll_offset() < initial_offset);
    }

    #[test]
    fn test_g_scrolls_to_top_when_preview_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        state.set_preview_visible_height(20);
        state.scroll_preview_down(); // Move down first

        let action = handle_key_event(key_event(KeyCode::Char('g')), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.preview_scroll_offset(), 0);
    }

    #[test]
    fn test_G_scrolls_to_bottom_when_preview_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        state.set_preview_visible_height(20);

        let action = handle_key_event(key_event(KeyCode::Char('G')), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.preview_scroll_offset(), u16::MAX);
    }

    #[test]
    fn test_enter_attaches_when_preview_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();

        let action = handle_key_event(key_event(KeyCode::Enter), &mut state);
        assert_eq!(action, Action::AttachSession);
    }

    // ========== Mouse Scroll Tests with Focus ==========

    #[test]
    fn test_mouse_scroll_down_navigates_sidebar_when_focused() {
        let mut state = create_test_state_with_projects();
        assert!(state.is_sidebar_focused());
        assert_eq!(state.selected_worktree_idx(), None);

        let action = handle_mouse_event(mouse_scroll_down(), &mut state);
        assert_eq!(action, Action::RefreshPreview);
        assert_eq!(state.selected_worktree_idx(), Some(0));
    }

    #[test]
    fn test_mouse_scroll_down_scrolls_preview_when_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        state.set_preview_visible_height(20);
        let initial_offset = state.preview_scroll_offset();

        let action = handle_mouse_event(mouse_scroll_down(), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(state.preview_scroll_offset(), initial_offset + 5); // Scrolls 5 lines
    }

    #[test]
    fn test_mouse_scroll_up_navigates_sidebar_when_focused() {
        let mut state = create_test_state_with_projects();
        state.select_next(); // Move to worktree 0
        assert_eq!(state.selected_worktree_idx(), Some(0));
        state.reset_scroll_debounce();

        let action = handle_mouse_event(mouse_scroll_up(), &mut state);
        assert_eq!(action, Action::RefreshPreview);
        assert_eq!(state.selected_worktree_idx(), None);
    }

    #[test]
    fn test_mouse_scroll_up_scrolls_preview_when_focused() {
        let mut state = create_test_state_with_projects();
        state.focus_preview();
        state.set_preview_line_count(100);
        state.set_preview_visible_height(20);
        state.scroll_preview_down(); // Move down first (+5)
        let initial_offset = state.preview_scroll_offset();

        let action = handle_mouse_event(mouse_scroll_up(), &mut state);
        assert_eq!(action, Action::None);
        assert_eq!(
            state.preview_scroll_offset(),
            initial_offset.saturating_sub(5) // Scrolls 5 lines
        );
    }
}
