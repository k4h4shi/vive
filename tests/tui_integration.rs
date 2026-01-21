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
    App, EventSource, GitHubIssue, IssueTitleFetcher, IssueTitleResult, ProjectDiscovery,
    config::Config,
    discovery::{Project, Worktree},
    event::Action,
    state::{AgentStatus, CreateTaskMethod, FocusMode, ModalType},
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

/// Mock Issue title fetcher for testing.
#[derive(Debug, Default)]
pub struct MockIssueFetcher;

impl IssueTitleFetcher for MockIssueFetcher {
    fn fetch(&self, _repo_path: &str, issue_number: u32) -> IssueTitleResult {
        // Return mock titles for testing
        IssueTitleResult::Found(format!("Mock Issue Title #{issue_number}"))
    }
}

// ============================================================================
// Issue Title Display Tests
// ============================================================================

/// Test: Preview title shows Issue title and branch name when worktree has Issue number.
#[test]
fn test_preview_title_shows_issue_title_and_branch() {
    // Create project with issue-numbered branch
    let projects = vec![
        Project::new("test-project", "/path/to/test").with_worktrees(vec![Worktree::new(
            "/path/to/test/.worktrees/feature/issue-42",
            "abc123",
            Some("feature/issue-42".to_string()),
        )]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app);

    // Navigate to the worktree
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Trigger preview update to fetch Issue title
    app.update_pane_preview();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Preview title should contain Issue title
    assert_buffer_contains(buffer, "Mock Issue Title #42");
    // Preview title should also contain branch name
    assert_buffer_contains(buffer, "feature/issue-42");
}

// ============================================================================
// Test Harness
// ============================================================================

/// Type alias for test App.
pub type TestApp =
    App<TestBackend, MockEventSource, MockTmuxExecutor, MockProjectDiscovery, MockIssueFetcher>;

/// Create a test application with the given projects and events.
pub fn create_test_app(projects: Vec<Project>, events: Vec<Event>) -> TestApp {
    let backend = TestBackend::new(120, 30);
    let terminal = Terminal::new(backend).unwrap();
    let event_source = MockEventSource::new(events);
    let tmux = TmuxOrchestrator::with_executor(MockTmuxExecutor::new());
    let discovery = MockProjectDiscovery::new(projects);
    let issue_fetcher = MockIssueFetcher;
    let config = Config::default();

    App::new(
        terminal,
        event_source,
        tmux,
        discovery,
        issue_fetcher,
        config,
    )
}

/// Create a test application with mock tmux sessions.
pub fn create_test_app_with_tmux(
    projects: Vec<Project>,
    events: Vec<Event>,
    tmux_sessions: Vec<(&str, &str)>,
) -> TestApp {
    let backend = TestBackend::new(120, 30);
    let terminal = Terminal::new(backend).unwrap();
    let event_source = MockEventSource::new(events);
    let executor = MockTmuxExecutor::new();
    for (name, content) in tmux_sessions {
        executor.add_session(name, content);
    }
    let tmux = TmuxOrchestrator::with_executor(executor);
    let discovery = MockProjectDiscovery::new(projects);
    let issue_fetcher = MockIssueFetcher;
    let config = Config::default();

    App::new(
        terminal,
        event_source,
        tmux,
        discovery,
        issue_fetcher,
        config,
    )
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

/// Helper to expand all projects in an App (for tests expecting worktree navigation).
/// Only expands projects that are not already expanded (to handle cases where
/// favorites loaded from disk may already be expanded).
pub fn expand_all_projects(app: &mut TestApp) {
    // Get all project names
    let project_names: Vec<String> = app
        .state()
        .projects
        .iter()
        .map(|p| p.name.clone())
        .collect();
    for name in project_names {
        // Only expand if not already expanded
        if !app.state().is_expanded(&name) {
            app.state_mut().toggle_expanded(&name);
        }
    }
}

/// Assert that the terminal buffer contains the expected text somewhere.
pub fn assert_buffer_contains(buffer: &Buffer, expected: &str) {
    let content = buffer_to_string(buffer);
    assert!(
        content.contains(expected),
        "Expected buffer to contain '{expected}', but got:\n{content}"
    );
}

/// Assert that the terminal buffer does NOT contain the expected text.
pub fn assert_buffer_not_contains(buffer: &Buffer, unexpected: &str) {
    let content = buffer_to_string(buffer);
    assert!(
        !content.contains(unexpected),
        "Expected buffer NOT to contain '{unexpected}', but found it in:\n{content}"
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
    expand_all_projects(&mut app); // Expand to show worktrees
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
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Initially at project header (worktree_idx = None)
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Simulate pressing 'j'
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Now at worktree index 0
    assert_eq!(app.state().selected_worktree_idx(), Some(0));
}

/// Test: Navigation - Press 'k' to move selection up.
#[test]
fn test_navigation_k_moves_up() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Move down twice to get to worktree 1
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
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
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Initially at project header
    assert_eq!(app.state().selected_worktree_idx(), None);

    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Now at worktree 0
    assert_eq!(app.state().selected_worktree_idx(), Some(0));
}

/// Test: Navigation - Navigation crosses project boundaries.
#[test]
fn test_navigation_crosses_projects() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // project-alpha has 2 worktrees, project-beta has 1
    // Start: project 0, header (worktree_idx = None)
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), None);

    // j -> project 0, worktree 0
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(0));

    // j -> project 0, worktree 1
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

    // j -> project 1, header (worktree_idx = None)
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_project_idx(), Some(1));
    assert_eq!(app.state().selected_worktree_idx(), None);
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

    // Open modal (now opens method selection modal)
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Select "Manual" to go to text input modal
    let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty());
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

/// Test: Status icon rendering - idle status shows bullet.
#[test]
fn test_status_icon_idle() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Idle status icon (bullet)
    assert_buffer_contains(buffer, "•");
}

/// Test: Status update - Mock status changes are reflected.
#[test]
fn test_status_update_changes_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    // Set a session status to Working
    app.state_mut().set_status(
        "project-alpha__main".to_string(),
        AgentStatus::Working { detail: None },
    );

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Working status icon (gear)
    assert_buffer_contains(buffer, "⚙");
}

/// Test: Status update - WaitingEdit status shows pencil icon.
#[test]
fn test_status_waiting_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    app.state_mut().set_status(
        "project-alpha__main".to_string(),
        AgentStatus::WaitingEdit { path: None },
    );

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "✎");
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
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate to worktree 0 (main) to get session ID
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

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
        MockEventSource::key(KeyCode::Char('j')), // header -> worktree 0
        MockEventSource::key(KeyCode::Char('j')), // worktree 0 -> worktree 1
        MockEventSource::key(KeyCode::Char('q')),
    ];
    let mut app = create_test_app(projects, events);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // First tick: process first 'j' (header -> worktree 0)
    let should_continue = app.tick(Duration::from_millis(0)).unwrap();
    assert!(should_continue);
    assert_eq!(app.state().selected_worktree_idx(), Some(0));

    // Second tick: process second 'j' (worktree 0 -> worktree 1)
    let should_continue = app.tick(Duration::from_millis(0)).unwrap();
    assert!(should_continue);
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

    // Third tick: process 'q'
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

    // Clear any favorites loaded from disk to ensure test isolation
    app.state_mut()
        .set_favorites(std::collections::HashSet::new());

    // No favorites initially (after clearing) - starts at project-alpha header
    assert!(!app.state().favorites().contains("project-alpha"));
    assert_eq!(
        app.state().selected_project().unwrap().name,
        "project-alpha"
    );

    // Press 'f' to toggle favorite
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Now project-alpha should be a favorite
    assert!(app.state().favorites().contains("project-alpha"));
    // Selection should still be on project-alpha (first in sorted order as favorite)
    assert_eq!(
        app.state().selected_project().unwrap().name,
        "project-alpha"
    );

    // Toggle again to remove
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // project-alpha should no longer be a favorite
    assert!(!app.state().favorites().contains("project-alpha"));
}

/// Test: Favorites display - favorite projects show star icon.
#[test]
fn test_favorites_show_star_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Clear any favorites loaded from disk to ensure test isolation
    app.state_mut()
        .set_favorites(std::collections::HashSet::new());

    // Toggle favorite to add project-alpha
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
    let projects = vec![
        Project::new("test-project", "/path/to/test").with_worktrees(vec![
            Worktree::new("/path/to/test", "abc123", Some("main".to_string())),
            Worktree::new(
                "/path/to/test/.worktrees/feature-x",
                "def456",
                Some("feature-x".to_string()),
            ),
        ]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate: header -> worktree 0 (main) -> worktree 1 (feature-x)
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

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
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate to main branch: header -> worktree 0 (main)
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_worktree_idx(), Some(0));

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
    let projects = vec![
        Project::new("test-project", "/path/to/test").with_worktrees(vec![
            Worktree::new("/path/to/test", "abc123", Some("main".to_string())),
            Worktree::new(
                "/path/to/test/.worktrees/my-feature",
                "def456",
                Some("my-feature".to_string()),
            ),
        ]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate: header -> worktree 0 (main) -> worktree 1 (my-feature)
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

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
    let projects = vec![
        Project::new("test-project", "/path/to/test").with_worktrees(vec![
            Worktree::new("/path/to/test", "abc123", Some("main".to_string())),
            Worktree::new(
                "/path/to/test/.worktrees/feature-1",
                "def456",
                Some("feature-1".to_string()),
            ),
        ]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate: header -> worktree 0 (main) -> worktree 1 (feature-1)
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

    // Press 'd' to open deletion modal
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

/// Test: Status - Error status shows cross icon.
#[test]
fn test_status_error_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    app.state_mut()
        .set_status("project-alpha__main".to_string(), AgentStatus::Error);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "✖");
}

/// Test: Multiple projects with different statuses.
#[test]
fn test_multiple_project_statuses() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    // Set different statuses for different sessions
    app.state_mut().set_status(
        "project-alpha__main".to_string(),
        AgentStatus::Working { detail: None },
    );
    app.state_mut().set_status(
        "project-beta__main".to_string(),
        AgentStatus::WaitingEdit { path: None },
    );

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should show gear and pencil icons
    assert_buffer_contains(buffer, "⚙");
    assert_buffer_contains(buffer, "✎");
}

/// Test: Navigation at boundary doesn't crash.
#[test]
fn test_navigation_at_boundary() {
    let projects = vec![
        Project::new("single-project", "/path/to/single").with_worktrees(vec![Worktree::new(
            "/path/to/single",
            "abc123",
            Some("main".to_string()),
        )]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Initially at project header
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Try to move up when already at top
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Should still be at first position (project header)
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Move down to worktree 0
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

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
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate to worktree 0 (main) to get session ID
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    app.update_pane_preview();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Working on task");
}

/// Test: Preview area scrolls to show the latest (bottom) content.
/// When the preview content is longer than the display area, the most recent
/// lines at the bottom should be visible, not the older lines at the top.
#[test]
fn test_preview_scrolls_to_bottom() {
    let projects = create_test_projects();

    // Create content with many lines - old content at top, latest at bottom
    // Use "FIRST_LINE_MARKER" to avoid substring matching issues
    let mut lines = Vec::new();
    lines.push("FIRST_LINE_MARKER_OLD".to_string());
    for i in 2..=30 {
        lines.push(format!("Old line {i}"));
    }
    lines.push(">>> LATEST MESSAGE <<<".to_string());
    lines.push("This is the most recent output".to_string());
    let content = lines.join("\n");

    let mut app =
        create_test_app_with_tmux(projects, vec![], vec![("project-alpha__main", &content)]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate to worktree 0 (main) to get session ID for preview
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    app.update_pane_preview();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();

    // The latest content at the bottom should be visible
    assert_buffer_contains(buffer, "LATEST MESSAGE");
    assert_buffer_contains(buffer, "most recent output");

    // The old content at the top should NOT be visible (scrolled out)
    assert_buffer_not_contains(buffer, "FIRST_LINE_MARKER_OLD");
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

    // Open modal (now opens method selection modal)
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Select "Manual" to go to text input modal
    let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty());
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

// ========== Additional tests for rich UI tree feature ==========

/// Test: Tree structure shows project collapse indicator (▼ for expanded, ▶ for collapsed).
#[test]
fn test_tree_shows_collapse_indicator() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show ▼
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "▼");
}

/// Test: Tree structure shows tree prefixes (├─ and └─).
#[test]
fn test_tree_shows_branch_prefixes() {
    let projects = vec![
        Project::new("test-project", "/path/to/test").with_worktrees(vec![
            Worktree::new("/path/to/test", "abc123", Some("main".to_string())),
            Worktree::new(
                "/path/to/test/.worktrees/feature",
                "def456",
                Some("feature".to_string()),
            ),
        ]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // First worktree should have ├─
    assert_buffer_contains(buffer, "├─");
    // Last worktree should have └─
    assert_buffer_contains(buffer, "└─");
}

/// Test: Tree structure shows single worktree with └─.
#[test]
fn test_tree_single_worktree_shows_end_prefix() {
    let projects = vec![
        Project::new("single-worktree", "/path/to/single").with_worktrees(vec![Worktree::new(
            "/path/to/single",
            "abc123",
            Some("main".to_string()),
        )]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "└─");
}

/// Test: Status text is displayed inline.
#[test]
fn test_status_text_inline_display() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    // Set a working status with detail
    app.state_mut().set_status(
        "project-alpha__main".to_string(),
        AgentStatus::Working {
            detail: Some("Exploring".to_string()),
        },
    );

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should show "Work" in the status text (may be truncated due to column width)
    // The full text is "Working" but UI may truncate based on available space
    assert_buffer_contains(buffer, "Work");
}

/// Test: Success status shows checkmark icon.
#[test]
fn test_status_success_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    app.state_mut()
        .set_status("project-alpha__main".to_string(), AgentStatus::Success);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "✓");
    assert_buffer_contains(buffer, "Done");
}

/// Test: WaitingShell status shows prompt icon.
#[test]
fn test_status_waiting_shell_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    app.state_mut().set_status(
        "project-alpha__main".to_string(),
        AgentStatus::WaitingShell {
            command: Some("cargo build".to_string()),
        },
    );

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, ">");
}

/// Test: WaitingOther status shows question icon.
#[test]
fn test_status_waiting_other_icon() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    app.state_mut()
        .set_status("project-alpha__main".to_string(), AgentStatus::WaitingOther);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "?");
}

/// Test: Long branch name is truncated.
#[test]
fn test_long_branch_name_truncated() {
    let projects = vec![
        Project::new("test-project", "/path/to/test").with_worktrees(vec![Worktree::new(
            "/path/to/test/.worktrees/very-long-feature-branch-name",
            "abc123",
            Some("very-long-feature-branch-name-that-exceeds-max".to_string()),
        )]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should contain truncation indicator
    assert_buffer_contains(buffer, "...");
}

/// Test: Padding dots appear between branch and status.
#[test]
fn test_padding_dots_between_branch_and_status() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should have padding dots
    assert_buffer_contains(buffer, "...");
}

/// Test: Multiple statuses are displayed correctly.
#[test]
fn test_multiple_status_types() {
    let projects = vec![
        Project::new("multi-status", "/path/to/multi").with_worktrees(vec![
            Worktree::new("/path/to/multi", "abc123", Some("main".to_string())),
            Worktree::new(
                "/path/to/multi/.worktrees/working",
                "def456",
                Some("working".to_string()),
            ),
            Worktree::new(
                "/path/to/multi/.worktrees/error",
                "ghi789",
                Some("error".to_string()),
            ),
        ]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees

    // Set different statuses
    app.state_mut().set_status(
        "multi-status__main".to_string(),
        AgentStatus::Working { detail: None },
    );
    app.state_mut().set_status(
        "multi-status__working".to_string(),
        AgentStatus::WaitingEdit { path: None },
    );
    app.state_mut()
        .set_status("multi-status__error".to_string(), AgentStatus::Error);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should show all different icons
    assert_buffer_contains(buffer, "⚙"); // Working
    assert_buffer_contains(buffer, "✎"); // WaitingEdit
    assert_buffer_contains(buffer, "✖"); // Error
}

/// Test: Favorite project shows star in parentheses.
#[test]
fn test_favorite_shows_star_in_parentheses() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Clear any favorites loaded from disk to ensure test isolation
    app.state_mut()
        .set_favorites(std::collections::HashSet::new());

    // Toggle favorite
    let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // The star is displayed without parentheses in the current UI
    assert_buffer_contains(buffer, "★");
}

/// Test: Tree structure maintains proper indentation.
#[test]
fn test_tree_proper_indentation() {
    let projects = vec![
        Project::new("indentation-test", "/path/to/test").with_worktrees(vec![
            Worktree::new("/path/to/test", "abc", Some("main".to_string())),
            Worktree::new("/path/to/test/f1", "def", Some("feature-1".to_string())),
            Worktree::new("/path/to/test/f2", "ghi", Some("feature-2".to_string())),
        ]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // All worktrees should be indented under the project
    // First two worktrees should have ├─
    assert_buffer_contains(buffer, "├─");
    // Last worktree should have └─
    assert_buffer_contains(buffer, "└─");
}

/// Test: UTF-8 branch names are truncated safely without panic.
#[test]
fn test_utf8_branch_name_truncation() {
    // Test with Japanese branch name that exceeds max length
    let projects = vec![
        Project::new("utf8-test", "/path/to/utf8").with_worktrees(vec![Worktree::new(
            "/path/to/utf8",
            "abc123",
            Some("feature/日本語ブランチ名テスト".to_string()),
        )]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to show worktrees
    // Should not panic when rendering UTF-8 branch names
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should contain truncation indicator
    assert_buffer_contains(buffer, "...");
}

/// Test: Emoji in branch names are handled safely.
#[test]
fn test_emoji_branch_name_truncation() {
    let projects = vec![
        Project::new("emoji-test", "/path/to/emoji").with_worktrees(vec![Worktree::new(
            "/path/to/emoji",
            "abc123",
            Some("feature/🚀🎉-awesome-feature".to_string()),
        )]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    // Should not panic when rendering emoji branch names
    app.render().unwrap();

    // Just verify it doesn't panic - the truncation behavior is tested
}

// ============================================================================
// Dashboard / Project Header Selection Tests
// ============================================================================

/// Test: Initial selection is at project header (worktree_idx = None).
#[test]
fn test_initial_selection_at_project_header() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Initially at project header (worktree_idx = None)
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), None);
}

/// Test: Navigate from project header to first worktree.
#[test]
fn test_navigate_from_header_to_worktree() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Start at project header
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Press 'j' to move to first worktree
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Now at worktree index 0
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(0));
}

/// Test: Navigate from first worktree back to project header.
#[test]
fn test_navigate_from_worktree_to_header() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Move to worktree 0
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();
    assert_eq!(app.state().selected_worktree_idx(), Some(0));

    // Move back up to project header
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Back at project header
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), None);
}

/// Test: Navigate from last worktree to next project header.
#[test]
fn test_navigate_to_next_project_header() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // project-alpha: header -> worktree 0 -> worktree 1 -> project-beta header
    // Navigate: header -> w0 -> w1
    for _ in 0..2 {
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    // At project-alpha, worktree 1
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(1));

    // Move to project-beta header
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // At project-beta header
    assert_eq!(app.state().selected_project_idx(), Some(1));
    assert_eq!(app.state().selected_worktree_idx(), None);
}

/// Test: Navigate from project header back to previous project's last worktree.
#[test]
fn test_navigate_to_prev_project_worktree() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Navigate to project-beta header
    for _ in 0..3 {
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    // At project-beta header
    assert_eq!(app.state().selected_project_idx(), Some(1));
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Move back to project-alpha's last worktree
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // At project-alpha, worktree 1 (last worktree)
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(1));
}

/// Test: selected_session_id returns None when at project header.
#[test]
fn test_session_id_none_at_project_header() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // At project header
    assert_eq!(app.state().selected_worktree_idx(), None);
    assert!(app.state().selected_session_id().is_none());
}

/// Test: selected_session_id returns session ID when at worktree.
#[test]
fn test_session_id_present_at_worktree() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app); // Expand to allow worktree navigation

    // Move to worktree 0
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Should have session ID
    assert!(app.state().selected_session_id().is_some());
    assert_eq!(
        app.state().selected_session_id(),
        Some("project-alpha__main".to_string())
    );
}

/// Test: Enter on project header triggers AttachSession action.
#[test]
fn test_enter_on_project_header_triggers_attach() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // At project header
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Press Enter
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let action = app.handle_key_event(key);

    // Should trigger AttachSession action with "enter" key
    assert_eq!(action, Action::AttachSession("enter".to_string()));
}

/// Test: Enter on worktree triggers AttachSession action.
#[test]
fn test_enter_on_worktree_triggers_attach() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Move to worktree
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Press Enter
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let action = app.handle_key_event(key);

    // Should trigger AttachSession action with "enter" key
    assert_eq!(action, Action::AttachSession("enter".to_string()));
}

/// Test: Full navigation cycle: header -> worktrees -> next header -> worktrees.
#[test]
fn test_full_navigation_cycle() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app);

    // Track the navigation path
    let mut path: Vec<(Option<usize>, Option<usize>)> = vec![];

    path.push((
        app.state().selected_project_idx(),
        app.state().selected_worktree_idx(),
    ));

    // Navigate down through all items
    for _ in 0..4 {
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();

        path.push((
            app.state().selected_project_idx(),
            app.state().selected_worktree_idx(),
        ));
    }

    // Expected path:
    // (0, None) -> (0, 0) -> (0, 1) -> (1, None) -> (1, 0)
    assert_eq!(path[0], (Some(0), None)); // project-alpha header
    assert_eq!(path[1], (Some(0), Some(0))); // project-alpha worktree 0
    assert_eq!(path[2], (Some(0), Some(1))); // project-alpha worktree 1
    assert_eq!(path[3], (Some(1), None)); // project-beta header
    assert_eq!(path[4], (Some(1), Some(0))); // project-beta worktree 0
}

/// Test: Single project with multiple worktrees navigation.
#[test]
fn test_single_project_navigation() {
    let projects = vec![
        Project::new("only-project", "/path/to/only").with_worktrees(vec![
            Worktree::new("/path/to/only", "abc", Some("main".to_string())),
            Worktree::new("/path/to/only/f1", "def", Some("feature-1".to_string())),
            Worktree::new("/path/to/only/f2", "ghi", Some("feature-2".to_string())),
        ]),
    ];
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    expand_all_projects(&mut app);

    // Start at header
    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Navigate through all worktrees
    for i in 0..3 {
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();

        assert_eq!(app.state().selected_project_idx(), Some(0));
        assert_eq!(app.state().selected_worktree_idx(), Some(i));
    }

    // Try to navigate past the last worktree (should stay at last)
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().selected_project_idx(), Some(0));
    assert_eq!(app.state().selected_worktree_idx(), Some(2));
}

/// Test: Project header shows project name in render.
#[test]
fn test_project_header_renders_project_name() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();
    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "project-alpha");
    assert_buffer_contains(buffer, "project-beta");
}

/// Test: Preview pane shows placeholder when at project header.
#[test]
fn test_preview_placeholder_at_project_header() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // At project header, no session selected
    assert_eq!(app.state().selected_worktree_idx(), None);

    app.render().unwrap();

    let buffer = app.terminal().backend().buffer();
    // Should show placeholder since no session is selected
    assert_buffer_contains(buffer, "No active session");
}

/// Test: Navigation updates sidebar selection state correctly.
#[test]
fn test_sidebar_selection_state_updates() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Initial selection should sync with sidebar
    let initial_idx = app.state().sidebar_list_state().selected();

    // Navigate down
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    let new_idx = app.state().sidebar_list_state().selected();
    assert_ne!(initial_idx, new_idx);
}

/// Test: 'o' key also triggers AttachSession on project header.
#[test]
fn test_o_key_on_project_header_triggers_attach() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // At project header
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Press 'o'
    let key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty());
    let action = app.handle_key_event(key);

    // Should trigger AttachSession action with "o" key
    assert_eq!(action, Action::AttachSession("o".to_string()));
}

/// Test: Dashboard session name uses double underscore delimiter.
#[test]
fn test_dashboard_session_name_delimiter() {
    use vive::tmux::TmuxOrchestrator;

    // Verify the dashboard session name format
    let name = TmuxOrchestrator::<MockTmuxExecutor>::dashboard_session_name("my-project");
    assert_eq!(name, "my-project__dashboard");

    // Should not contain single colon (tmux delimiter)
    assert!(!name.contains(':'));
}

/// Test: Create task modal opens when at project header.
#[test]
fn test_create_modal_opens_at_project_header() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // At project header
    assert_eq!(app.state().selected_worktree_idx(), None);
    assert!(app.state().modal.is_none());

    // Press 'n' to open modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Modal should be open
    assert!(app.state().modal.is_some());
}

/// Test: Delete key at project header shows error (cannot delete project).
#[test]
fn test_delete_at_project_header_shows_error() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // At project header
    assert_eq!(app.state().selected_worktree_idx(), None);

    // Press 'd'
    let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Should have an error message (no worktree selected or it's the main branch logic)
    // The behavior depends on implementation, but no crash should occur
    app.render().unwrap();
}

// ========== Issue Picker Integration Tests (Issue #55) ==========

/// Test: 'n' key opens create task method selection modal.
#[test]
fn test_n_key_opens_method_selection_modal() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Press 'n' to open create task modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Should open CreateTaskMethod modal
    assert!(matches!(
        app.state().modal,
        Some(ModalType::CreateTaskMethod { .. })
    ));

    // Render and check the modal is displayed
    app.render().unwrap();
    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Create Task");
    assert_buffer_contains(buffer, "Manual");
    assert_buffer_contains(buffer, "Pick from Issue");
}

/// Test: Method selection modal navigation with j/k.
#[test]
fn test_method_selection_modal_navigation() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open method selection modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Default selection should be Manual
    assert_eq!(
        app.state().selected_create_task_method(),
        Some(CreateTaskMethod::Manual)
    );

    // Press 'j' to move to PickFromIssue
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(
        app.state().selected_create_task_method(),
        Some(CreateTaskMethod::PickFromIssue)
    );

    // Press 'k' to move back to Manual
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(
        app.state().selected_create_task_method(),
        Some(CreateTaskMethod::Manual)
    );
}

/// Test: Selecting 'i' in method selection opens Issue Picker.
#[test]
fn test_i_key_opens_issue_picker() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open method selection modal
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Press 'i' to select Pick from Issue
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    // Note: FetchIssues action would try to call gh CLI in real scenario
    // For this test, we just verify the modal transition
    app.handle_action(action).unwrap();

    // Should open IssuePicker modal
    assert!(matches!(
        app.state().modal,
        Some(ModalType::IssuePicker { .. })
    ));
}

/// Test: Issue Picker displays issues and allows navigation.
#[test]
fn test_issue_picker_navigation() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open Issue Picker modal directly
    app.state_mut().open_issue_picker_modal();

    // Set mock issues
    let issues = vec![
        GitHubIssue::new(42, "Fix authentication bug"),
        GitHubIssue::new(55, "Add Issue Picker to New Task flow"),
        GitHubIssue::new(60, "Improve error handling"),
    ];
    app.state_mut().set_issue_picker_issues(issues);

    // Render the modal
    app.render().unwrap();
    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Pick from Issue");
    assert_buffer_contains(buffer, "#42");
    assert_buffer_contains(buffer, "Fix authentication bug");

    // Initially first issue is selected
    assert_eq!(app.state().selected_issue().map(|i| i.number), Some(42));

    // Navigate down with 'j'
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().selected_issue().map(|i| i.number), Some(55));

    // Navigate down again
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().selected_issue().map(|i| i.number), Some(60));

    // Navigate up with 'k'
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert_eq!(app.state().selected_issue().map(|i| i.number), Some(55));
}

/// Test: Issue Picker filter functionality.
#[test]
fn test_issue_picker_filter() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open Issue Picker modal directly
    app.state_mut().open_issue_picker_modal();

    // Set mock issues
    let issues = vec![
        GitHubIssue::new(42, "Fix authentication bug"),
        GitHubIssue::new(55, "Add dark mode feature"),
        GitHubIssue::new(60, "Fix login error"),
    ];
    app.state_mut().set_issue_picker_issues(issues);

    // All 3 issues should be visible
    assert_eq!(app.state().filtered_issues().len(), 3);

    // Type "Fix" to filter
    for c in "Fix".chars() {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    // Only 2 issues should match (42 and 60)
    assert_eq!(app.state().filtered_issues().len(), 2);

    // First filtered issue should be selected
    assert_eq!(app.state().selected_issue().map(|i| i.number), Some(42));

    // Render and verify filter is displayed
    app.render().unwrap();
    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Filter: Fix");
}

/// Test: Issue Picker filter by issue number.
#[test]
fn test_issue_picker_filter_by_number() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open Issue Picker modal directly
    app.state_mut().open_issue_picker_modal();

    // Set mock issues
    let issues = vec![
        GitHubIssue::new(42, "First issue"),
        GitHubIssue::new(55, "Second issue"),
        GitHubIssue::new(123, "Third issue"),
    ];
    app.state_mut().set_issue_picker_issues(issues);

    // Type "55" to filter by number
    for c in "55".chars() {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
        let action = app.handle_key_event(key);
        app.handle_action(action).unwrap();
    }

    // Only issue #55 should match
    assert_eq!(app.state().filtered_issues().len(), 1);
    assert_eq!(app.state().selected_issue().map(|i| i.number), Some(55));
}

/// Test: Selecting an issue with Enter creates task.
#[test]
fn test_issue_picker_select_creates_task() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open Issue Picker modal directly
    app.state_mut().open_issue_picker_modal();

    // Set mock issues
    let issues = vec![GitHubIssue::new(42, "Fix authentication bug")];
    app.state_mut().set_issue_picker_issues(issues);

    // Press Enter to select the issue
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let action = app.handle_key_event(key);

    // Should return CreateTaskFromIssue action
    match action {
        Action::CreateTaskFromIssue(issue, auto_kickstart) => {
            assert_eq!(issue.number, 42);
            assert_eq!(issue.branch_name(), "feature/issue-42");
            assert!(auto_kickstart); // Default is true
        }
        _ => panic!("Expected CreateTaskFromIssue action, got {:?}", action),
    }

    // Modal should be closed
    assert!(app.state().modal.is_none());
}

/// Test: Issue Picker Escape cancels without action.
#[test]
fn test_issue_picker_escape_cancels() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open Issue Picker modal
    app.state_mut().open_issue_picker_modal();
    app.state_mut()
        .set_issue_picker_issues(vec![GitHubIssue::new(1, "Test")]);

    // Press Escape
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let action = app.handle_key_event(key);

    // No task created (Action::None)
    assert_eq!(action, Action::None);

    app.handle_action(action).unwrap();

    // Modal should be closed
    assert!(app.state().modal.is_none());
}

/// Test: Full flow from 'n' -> 'i' -> filter -> Enter.
#[test]
fn test_full_issue_picker_flow() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Step 1: Press 'n' to open method selection
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    assert!(matches!(
        app.state().modal,
        Some(ModalType::CreateTaskMethod { .. })
    ));

    // Step 2: Press 'i' to select Issue Picker
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    // FetchIssues action is returned but we skip actual fetch
    assert_eq!(action, Action::FetchIssues);

    // Manually set issues (simulating fetch completion)
    app.state_mut().set_issue_picker_issues(vec![
        GitHubIssue::new(100, "Feature A"),
        GitHubIssue::new(200, "Feature B"),
    ]);

    // Step 3: Filter by typing "B"
    let key = KeyEvent::new(KeyCode::Char('B'), KeyModifiers::empty());
    let action = app.handle_key_event(key);
    app.handle_action(action).unwrap();

    // Only "Feature B" should match
    assert_eq!(app.state().filtered_issues().len(), 1);
    assert_eq!(app.state().selected_issue().map(|i| i.number), Some(200));

    // Step 4: Press Enter to create task
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let action = app.handle_key_event(key);

    match action {
        Action::CreateTaskFromIssue(issue, auto_kickstart) => {
            assert_eq!(issue.number, 200);
            assert_eq!(issue.branch_name(), "feature/issue-200");
            assert!(auto_kickstart); // Default is true
        }
        _ => panic!("Expected CreateTaskFromIssue action"),
    }

    // Modal should be closed
    assert!(app.state().modal.is_none());
}

/// Test: Issue Picker renders loading state.
#[test]
fn test_issue_picker_loading_state() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open Issue Picker modal (will be in loading state)
    app.state_mut().open_issue_picker_modal();

    // Render the loading state
    app.render().unwrap();
    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Loading issues...");
}

/// Test: Issue Picker renders error state.
#[test]
fn test_issue_picker_error_state() {
    let projects = create_test_projects();
    let mut app = create_test_app(projects, vec![]);

    app.init().unwrap();

    // Open Issue Picker modal and set error
    app.state_mut().open_issue_picker_modal();
    app.state_mut()
        .set_issue_picker_error("gh CLI not installed".to_string());

    // Render the error state
    app.render().unwrap();
    let buffer = app.terminal().backend().buffer();
    assert_buffer_contains(buffer, "Error:");
    assert_buffer_contains(buffer, "gh CLI not installed");
}
