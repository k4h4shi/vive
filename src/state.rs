use crate::process::AgentState;

/// Represents a window (task) in the session.
#[derive(Debug, Clone)]
pub struct WindowState {
    /// Name of the window (e.g., "issue-123").
    pub name: String,
    /// Current agent state.
    pub agent_state: AgentState,
    /// CPU usage percentage (if available).
    pub cpu: Option<f32>,
}

impl WindowState {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            agent_state: AgentState::Idle,
            cpu: None,
        }
    }

    pub fn with_state(mut self, state: AgentState) -> Self {
        self.agent_state = state;
        self
    }

    pub fn with_cpu(mut self, cpu: f32) -> Self {
        self.cpu = Some(cpu);
        self
    }
}

/// Application state, separated from UI for testability.
#[derive(Debug, Default)]
pub struct AppState {
    /// Whether the application should quit.
    should_quit: bool,
    /// Current session name.
    session_name: Option<String>,
    /// Windows in the current session.
    windows: Vec<WindowState>,
    /// Currently selected window index.
    selected_index: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Set the session name.
    pub fn set_session_name(&mut self, name: impl Into<String>) {
        self.session_name = Some(name.into());
    }

    /// Get the session name.
    pub fn session_name(&self) -> Option<&str> {
        self.session_name.as_deref()
    }

    /// Get the windows.
    pub fn windows(&self) -> &[WindowState] {
        &self.windows
    }

    /// Set the windows.
    pub fn set_windows(&mut self, windows: Vec<WindowState>) {
        self.windows = windows;
        // Ensure selected index is valid
        if self.selected_index >= self.windows.len() && !self.windows.is_empty() {
            self.selected_index = self.windows.len() - 1;
        }
    }

    /// Update a window's state by name.
    pub fn update_window_state(&mut self, name: &str, state: AgentState, cpu: Option<f32>) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.name == name) {
            window.agent_state = state;
            window.cpu = cpu;
        }
    }

    /// Get the currently selected window index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.windows.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.windows.len();
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        if !self.windows.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.windows.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Get the currently selected window.
    #[allow(dead_code)]
    pub fn selected_window(&self) -> Option<&WindowState> {
        self.windows.get(self.selected_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_defaults() {
        let state = AppState::new();
        assert!(!state.should_quit());
        assert!(state.session_name().is_none());
        assert!(state.windows().is_empty());
    }

    #[test]
    fn test_quit() {
        let mut state = AppState::new();
        assert!(!state.should_quit());
        state.quit();
        assert!(state.should_quit());
    }

    #[test]
    fn test_session_name() {
        let mut state = AppState::new();
        state.set_session_name("my-project");
        assert_eq!(state.session_name(), Some("my-project"));
    }

    #[test]
    fn test_set_windows() {
        let mut state = AppState::new();
        let windows = vec![
            WindowState::new("issue-1"),
            WindowState::new("issue-2"),
        ];
        state.set_windows(windows);
        assert_eq!(state.windows().len(), 2);
    }

    #[test]
    fn test_select_next() {
        let mut state = AppState::new();
        state.set_windows(vec![
            WindowState::new("a"),
            WindowState::new("b"),
            WindowState::new("c"),
        ]);

        assert_eq!(state.selected_index(), 0);
        state.select_next();
        assert_eq!(state.selected_index(), 1);
        state.select_next();
        assert_eq!(state.selected_index(), 2);
        state.select_next();
        assert_eq!(state.selected_index(), 0); // Wraps around
    }

    #[test]
    fn test_select_previous() {
        let mut state = AppState::new();
        state.set_windows(vec![
            WindowState::new("a"),
            WindowState::new("b"),
            WindowState::new("c"),
        ]);

        assert_eq!(state.selected_index(), 0);
        state.select_previous();
        assert_eq!(state.selected_index(), 2); // Wraps around
        state.select_previous();
        assert_eq!(state.selected_index(), 1);
    }

    #[test]
    fn test_update_window_state() {
        let mut state = AppState::new();
        state.set_windows(vec![
            WindowState::new("issue-1"),
            WindowState::new("issue-2"),
        ]);

        state.update_window_state("issue-1", AgentState::Working, Some(25.0));

        let window = &state.windows()[0];
        assert_eq!(window.agent_state, AgentState::Working);
        assert_eq!(window.cpu, Some(25.0));
    }

    #[test]
    fn test_selected_window() {
        let mut state = AppState::new();
        state.set_windows(vec![
            WindowState::new("issue-1"),
            WindowState::new("issue-2"),
        ]);

        assert_eq!(state.selected_window().unwrap().name, "issue-1");
        state.select_next();
        assert_eq!(state.selected_window().unwrap().name, "issue-2");
    }

    #[test]
    fn test_window_state_builder() {
        let window = WindowState::new("issue-1")
            .with_state(AgentState::Working)
            .with_cpu(50.0);

        assert_eq!(window.name, "issue-1");
        assert_eq!(window.agent_state, AgentState::Working);
        assert_eq!(window.cpu, Some(50.0));
    }
}
