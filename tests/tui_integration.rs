//! TUI Integration Tests
//!
//! This module provides integration tests for the Vive TUI using ratatui's TestBackend.
//! It tests UI logic (rendering, navigation, state updates) without running the actual binary.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use vive::{
    App, EventSource, ProjectDiscovery,
    config::Config,
    discovery::{Project, Worktree},
    event::Action,
    state::{AgentStatus, FocusMode},
    tmux::{TmuxCommandResult, TmuxExecutor, TmuxOrchestrator},
};

// ============================================================================
// Mock Implementations
// ============================================================================

/// Mock event source that returns pre-configured events.
#[derive(Debug, Default)]
pub struct MockEventSource {
    events: Mutex<Vec<Event>>,
}

impl MockEventSource {
    /// Create a new mock event source with the given events.
    /// Events will be returned in order (first pushed = first returned).
    pub fn new(events: Vec<Event>) -> Self {
        Self {
            events: Mutex::new(events),
        }
    }

    /// Push a new event to return.
    pub fn push_event(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }

    /// Create a key event.
    pub fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
    }

    /// Create a key event with modifiers.
    pub fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }
}

impl EventSource for MockEventSource {
    fn poll(&self, _timeout: Duration) -> Result<Option<Event>> {
        let mut events = self.events.lock().unwrap();
        if events.is_empty() {
            Ok(None)
        } else {
            Ok(Some(events.remove(0)))
        }
    }
}

/// Mock tmux executor for testing.
#[derive(Debug, Default)]
pub struct MockTmuxExecutor {
    sessions: Mutex<std::collections::HashMap<String, String>>,
    sent_keys: Mutex<Vec<(String, String)>>,
}

impl MockTmuxExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a mock session with content.
    pub fn add_session(&self, name: &str, content: &str) {
        self.sessions
            .lock()
            .unwrap()
            .insert(name.to_string(), content.to_string());
    }

    /// Get the keys that were sent.
    pub fn get_sent_keys(&self) -> Vec<(String, String)> {
        self.sent_keys.lock().unwrap().clone()
    }
}

impl TmuxExecutor for MockTmuxExecutor {
    fn execute(&self, args: Vec<String>) -> Result<TmuxCommandResult> {
        let cmd = args.first().map(String::as_str).unwrap_or("");

        match cmd {
            "has-session" => {
                let session = args.get(2).map(String::as_str).unwrap_or("");
                let exists = self.sessions.lock().unwrap().contains_key(session);
                Ok(TmuxCommandResult {
                    success: exists,
                    stdout: String::new(),
                    stderr: if exists {
                        String::new()
                    } else {
                        "session not found".to_string()
                    },
                })
            }
            "capture-pane" => {
                let session = args.get(2).map(String::as_str).unwrap_or("");
                let content = self
                    .sessions
                    .lock()
                    .unwrap()
                    .get(session)
                    .cloned()
                    .unwrap_or_default();
                Ok(TmuxCommandResult {
                    success: true,
                    stdout: content,
                    stderr: String::new(),
                })
            }
            "send-keys" => {
                let target = args.get(2).map(String::as_str).unwrap_or("").to_string();
                let keys = args.get(3).map(String::as_str).unwrap_or("").to_string();
                self.sent_keys.lock().unwrap().push((target, keys));
                Ok(TmuxCommandResult {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                })
            }
            "new-session" => Ok(TmuxCommandResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            }),
            _ => Ok(TmuxCommandResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            }),
        }
    }
}

/// Mock project discovery that returns pre-configured projects.
#[derive(Debug, Default)]
pub struct MockProjectDiscovery {
    projects: Vec<Project>,
}

impl MockProjectDiscovery {
    pub fn new(projects: Vec<Project>) -> Self {
        Self { projects }
    }
}

impl ProjectDiscovery for MockProjectDiscovery {
    fn discover(&self, _root: &Path, _ignored_dirs: &[String]) -> Result<Vec<Project>> {
        Ok(self.projects.clone())
    }
}

// ============================================================================
// Test Harness
// ============================================================================

/// Type alias for test App.
pub type TestApp = App<TestBackend, MockEventSource, MockTmuxExecutor, MockProjectDiscovery>;

/// Create a test application with the given projects and events.
pub fn create_test_app(projects: Vec<Project>, events: Vec<Event>) -> TestApp {
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).unwrap();
    let event_source = MockEventSource::new(events);
    let tmux = TmuxOrchestrator::with_executor(MockTmuxExecutor::new());
    let discovery = MockProjectDiscovery::new(projects);
    let config = Config::default();

    App::new(terminal, event_source, tmux, discovery, config)
}

/// Create a test application with mock tmux sessions.
pub fn create_test_app_with_tmux(
    projects: Vec<Project>,
    events: Vec<Event>,
    tmux_sessions: Vec<(&str, &str)>,
) -> TestApp {
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).unwrap();
    let event_source = MockEventSource::new(events);
    let executor = MockTmuxExecutor::new();
    for (name, content) in tmux_sessions {
        executor.add_session(name, content);
    }
    let tmux = TmuxOrchestrator::with_executor(executor);
    let discovery = MockProjectDiscovery::new(projects);
    let config = Config::default();

    App::new(terminal, event_source, tmux, discovery, config)
}

/// Helper function to create test projects.
pub fn create_test_projects() -> Vec<Project> {
    vec![
        Project::new("project-alpha", "/path/to/alpha").with_worktrees(vec![
            Worktree::new("/path/to/alpha", "abc123", Some("main".to_string())),
            Worktree::new(
                "/path/to/alpha/.worktrees/feature-1",
                "def456",
                Some("feature-1".to_string()),
            ),
        ]),
        Project::new("project-beta", "/path/to/beta").with_worktrees(vec![Worktree::new(
            "/path/to/beta",
            "ghi789",
            Some("main".to_string()),
        )]),
    ]
}

/// Assert that the terminal buffer contains the expected text somewhere.
pub fn assert_buffer_contains(buffer: &Buffer, expected: &str) {
    let content = buffer_to_string(buffer);
    assert!(
        content.contains(expected),
        "Expected buffer to contain '{}', but got:\n{}",
        expected,
        content
    );
}

/// Assert that the terminal buffer does NOT contain the expected text.
pub fn assert_buffer_not_contains(buffer: &Buffer, unexpected: &str) {
    let content = buffer_to_string(buffer);
    assert!(
        !content.contains(unexpected),
        "Expected buffer NOT to contain '{}', but found it in:\n{}",
        unexpected,
        content
    );
}

/// Convert terminal buffer to string for easier assertions.
pub fn buffer_to_string(buffer: &Buffer) -> String {
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            result.push_str(cell.symbol());
        }
        result.push('\n');
    }
    result
}

// ============================================================================
// Integration Tests
// ============================================================================

/// Test: Startup - Verify project list is rendered correctly.
#[test]
fn test_startup_renders_project_list() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    // Initialize and render
    app.init().unwrap();
    app.render().unwrap();

    // Check the buffer contains project names
    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "project-alpha");
    assert_buffer_contains(buffer, "project-beta");

    // Check worktrees are shown
    assert_buffer_contains(buffer, "main");
    assert_buffer_contains(buffer, "feature-1");
}

/// Test: Startup - Verify header is rendered.
#[test]
fn test_startup_renders_header() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Vive");
}

/// Test: Startup - Empty project list shows "No projects found".
#[test]
fn test_startup_empty_projects() {
    let mut app = create_test_app(vec![], vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "No projects found");
}

/// Test: Navigation - Press 'j' to move selection down.
#[test]
fn test_navigation_j_moves_down() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Initially at worktree index 0
    assert_eq!(app.state().selected_worktree_idx(), Some(0));

    // Simulate pressing 'j'
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Now at worktree index 1
    assert_eq!(app.state().selected_worktree_idx(), Some(1));
}

/// Test: Navigation - Press 'k' to move selection up.
#[test]
fn test_navigation_k_moves_up() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Move down first
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

    // Now move up
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Back at worktree index 0
    assert_eq!(app.state().selected_worktree_idx(), Some(0));
}

/// Test: Navigation - Down arrow also moves selection down.
#[test]
fn test_navigation_down_arrow() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().selected_worktree_idx(), Some(1));
}

/// Test: Navigation - Navigation crosses project boundaries.
#[test]
fn test_navigation_crosses_projects() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // project-alpha has 2 worktrees, project-beta has 1
    // Start: project 0, worktree 0
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(0));

    // j -> project 0, worktree 1
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

    // j -> project 1, worktree 0
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_project_idx(), Some(1));
    assert_eq!(app.state().selected_worktree_idx(), Some(0));
}

/// Test: Modal - Press 'n' to open create task modal.
#[test]
fn test_modal_opens_on_n() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    assert!(app.state().modal.is_none());

    // Press 'n' to open modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert!(app.state().modal.is_some());
}

/// Test: Modal - Renders modal dialog.
#[test]
fn test_modal_renders() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Render
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Create Task");
    assert_buffer_contains(buffer, "branch name");
}

/// Test: Modal - Escape closes the modal.
#[test]
fn test_modal_closes_on_escape() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert!(app.state().modal.is_some());

    // Press Escape
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert!(app.state().modal.is_none());
}

/// Test: Modal - Typing in modal updates input.
#[test]
fn test_modal_typing() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Type "test"
    for c in "test".chars() {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    assert_eq!(app.state().modal_input(), Some("test"));
}

/// Test: Input mode - Press 'i' to enter input mode.
#[test]
fn test_input_mode_on_i() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    assert_eq!(app.state().focus_mode, FocusMode::Normal);

    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().focus_mode, FocusMode::Input);
}

/// Test: Input mode - Escape exits input mode.
#[test]
fn test_input_mode_escape_exits() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Enter input mode
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().focus_mode, FocusMode::Input);

    // Press Escape
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().focus_mode, FocusMode::Normal);
}

/// Test: Quit - Press 'q' to quit.
#[test]
fn test_quit_on_q() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    assert!(!app.state().should_quit());

    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    assert_eq!(action, Action::Quit);

    assert!(app.state().should_quit());
}

/// Test: Quit - Ctrl+C also quits.
#[test]
fn test_quit_on_ctrl_c() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let action = app.handle_key_event(key);
    assert_eq!(action, Action::Quit);

    assert!(app.state().should_quit());
}

/// Test: Status icon rendering - idle status shows white circle.
#[test]
fn test_status_icon_idle() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Idle status icon
    assert_buffer_contains(buffer, "⚪");
}

/// Test: Status update - Mock status changes are reflected.
#[test]
fn test_status_update_changes_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Set a session status to Working
    app.state_mut()
        .set_status("project-alpha__main".to_string(), AgentStatus::Working);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Working status icon (green circle)
    assert_buffer_contains(buffer, "🟢");
}

/// Test: Status update - Waiting status shows yellow circle.
#[test]
fn test_status_waiting_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    app.state_mut()
        .set_status("project-alpha__main".to_string(), AgentStatus::Waiting);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "🟡");
}

/// Test: Preview area updates from tmux capture.
#[test]
fn test_preview_updates_from_tmux() {
    let projects = create_test_projects();
    let mut app = create_test_app_with_tmux(
        projects,
        vec![],
        vec![(
            "project-alpha__main",
            "Claude Code >\nAnalyzing your code...",
        )],
    );

    app.init().unwrap();

    // Update preview
    app.update_pane_preview();

    assert!(app.state().pane_preview.contains("Analyzing your code"));
}

/// Test: Send input sends keys to tmux.
#[test]
fn test_send_input_to_tmux() {
    let projects = create_test_projects();
    let mut app = create_test_app_with_tmux(projects, vec![], vec![("project-alpha__main", "")]);

    app.init().unwrap();

    // Enter input mode
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Type 'y'
    let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Press Enter to send
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let action = app.handle_key_event(key);

    // Handle SendInput action
    assert!(matches!(action, Action::SendInput(ref s) if s == "y"));
    app.handle_action(action).unwrap();
}

/// Test: Footer shows help text.
#[test]
fn test_footer_shows_help() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Nav");
    assert_buffer_contains(buffer, "Quit");
}

/// Test: Error message is displayed in header.
#[test]
fn test_error_message_displayed() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Set an error message
    app.state_mut().set_error_message("Something went wrong");

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "ERROR");
    assert_buffer_contains(buffer, "Something went wrong");
}

/// Test: Success message is displayed in header.
#[test]
fn test_success_message_displayed() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    app.state_mut().set_success_message("Operation completed");

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "OK");
    assert_buffer_contains(buffer, "Operation completed");
}

/// Test: Navigation clears status message.
#[test]
fn test_navigation_clears_status_message() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Set a status message
    app.state_mut().set_error_message("Old error");
    assert!(app.state().status_message.is_some());

    // Navigate
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Status message should be cleared
    assert!(app.state().status_message.is_none());
}

/// Test: Full tick cycle works correctly.
#[test]
fn test_tick_cycle() {
    let projects = create_test_projects();
    let events = vec![
        MockEventSource::key(KeyCode::Char('j')),
        MockEventSource::key(KeyCode::Char('q')),
    ];
    let mut app = create_test_app(projects, events);

    app.init().unwrap();

    // First tick: process 'j'
    let should_continue = app.tick(Duration::from_millis(0)).unwrap();
    assert!(should_continue);
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

    // Second tick: process 'q'
    let should_continue = app.tick(Duration::from_millis(0)).unwrap();
    assert!(!should_continue);
    assert!(app.state().should_quit());
}

/// Test: Preview placeholder shown when no session.
#[test]
fn test_preview_placeholder_no_session() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "No active session");
}

// ============================================================================
// Additional Integration Tests for Test Coverage Expansion
// ============================================================================

/// Test: Favorites toggle - Press 'f' to toggle favorite.
#[test]
fn test_favorites_toggle_on_f() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // No favorites initially
    assert!(!app.state().favorites().contains("project-alpha"));

    // Press 'f' to toggle favorite
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Now project-alpha should be a favorite
    assert!(app.state().favorites().contains("project-alpha"));

    // Toggle again to remove
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert!(!app.state().favorites().contains("project-alpha"));
}

/// Test: Favorites display - favorite projects show star icon.
#[test]
fn test_favorites_show_star_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Toggle favorite
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "★");
}

/// Test: Deletion modal - Press 'd' on non-main branch opens modal.
#[test]
fn test_deletion_modal_opens_on_d() {
    let projects = vec![Project::new("test-project", "/path/to/test").with_worktrees(vec![
        Worktree::new("/path/to/test", "abc123", Some("main".to_string())),
        Worktree::new(
            "/path/to/test/.worktrees/feature-x",
            "def456",
            Some("feature-x".to_string()),
        ),
    ])];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Navigate to feature-x worktree
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Now on feature-x, press 'd'
    let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Modal should be open
    assert!(app.state().modal.is_some());
}

/// Test: Deletion modal - Press 'd' on main branch shows error.
#[test]
fn test_deletion_on_main_shows_error() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Currently on main branch (default selection)
    // Press 'd'
    let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Should have an error message
    assert!(app.state().status_message.is_some());

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Cannot delete");
}

/// Test: Deletion modal renders with branch name.
#[test]
fn test_deletion_modal_renders_branch_name() {
    let projects = vec![Project::new("test-project", "/path/to/test").with_worktrees(vec![
        Worktree::new("/path/to/test", "abc123", Some("main".to_string())),
        Worktree::new(
            "/path/to/test/.worktrees/my-feature",
            "def456",
            Some("my-feature".to_string()),
        ),
    ])];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Navigate to my-feature
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Press 'd' to open deletion modal
    let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "my-feature");
    assert_buffer_contains(buffer, "Delete");
}

/// Test: Deletion modal - 'n' cancels deletion.
#[test]
fn test_deletion_modal_n_cancels() {
    let projects = vec![Project::new("test-project", "/path/to/test").with_worktrees(vec![
        Worktree::new("/path/to/test", "abc123", Some("main".to_string())),
        Worktree::new(
            "/path/to/test/.worktrees/feature-1",
            "def456",
            Some("feature-1".to_string()),
        ),
    ])];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Navigate and open deletion modal
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert!(app.state().modal.is_some());

    // Press 'n' to cancel
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert!(app.state().modal.is_none());
}

/// Test: Status - Error status shows red icon.
#[test]
fn test_status_error_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    app.state_mut()
        .set_status("project-alpha__main".to_string(), AgentStatus::Error);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "🔴");
}

/// Test: Multiple projects with different statuses.
#[test]
fn test_multiple_project_statuses() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Set different statuses for different sessions
    app.state_mut()
        .set_status("project-alpha__main".to_string(), AgentStatus::Working);
    app.state_mut()
        .set_status("project-beta__main".to_string(), AgentStatus::Waiting);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should show both green and yellow circles
    assert_buffer_contains(buffer, "🟢");
    assert_buffer_contains(buffer, "🟡");
}

/// Test: Navigation at boundary doesn't crash.
#[test]
fn test_navigation_at_boundary() {
    let projects = vec![Project::new("single-project", "/path/to/single").with_worktrees(vec![
        Worktree::new("/path/to/single", "abc123", Some("main".to_string())),
    ])];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Try to move up when already at top
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Should still be at first position
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(0));

    // Try to move down when already at bottom
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Should still be at same position
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(0));
}

/// Test: Preview area shows spinner content.
#[test]
fn test_preview_shows_spinner_content() {
    let projects = create_test_projects();
    let mut app = create_test_app_with_tmux(
        projects,
        vec![],
        vec![(
            "project-alpha__main",
            "⠋ Working on task...\nAnalyzing code...",
        )],
    );

    app.init().unwrap();
    app.update_pane_preview();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Working on task");
}

/// Test: Input mode typing updates input buffer.
#[test]
fn test_input_mode_typing_updates_buffer() {
    let projects = create_test_projects();
    let mut app = create_test_app_with_tmux(projects, vec![], vec![("project-alpha__main", "")]);

    app.init().unwrap();

    // Enter input mode
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Type "hello"
    for c in "hello".chars() {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    assert_eq!(&app.state().input_buffer, "hello");
}

/// Test: Input mode backspace removes character.
#[test]
fn test_input_mode_backspace_removes_char() {
    let projects = create_test_projects();
    let mut app = create_test_app_with_tmux(projects, vec![], vec![("project-alpha__main", "")]);

    app.init().unwrap();

    // Enter input mode
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Type "ab"
    for c in "ab".chars() {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    assert_eq!(&app.state().input_buffer, "ab");

    // Press backspace
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(&app.state().input_buffer, "a");
}

/// Test: Modal input with backspace.
#[test]
fn test_modal_backspace() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Type "abc"
    for c in "abc".chars() {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    assert_eq!(app.state().modal_input(), Some("abc"));

    // Press backspace
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().modal_input(), Some("ab"));
}

/// Test: Footer shows delete key hint.
#[test]
fn test_footer_shows_delete_hint() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Del");
}

/// Test: Footer shows favorite key hint.
#[test]
fn test_footer_shows_favorite_hint() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Fav");
}
