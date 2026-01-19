/// Application state, separated from UI for testability.
#[derive(Debug, Default)]
pub struct AppState {
    /// Whether the application should quit.
    should_quit: bool,
    /// A counter to demonstrate state changes.
    counter: i32,
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

    pub fn counter(&self) -> i32 {
        self.counter
    }

    pub fn increment(&mut self) {
        self.counter = self.counter.saturating_add(1);
    }

    pub fn decrement(&mut self) {
        self.counter = self.counter.saturating_sub(1).max(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_defaults() {
        let state = AppState::new();
        assert!(!state.should_quit());
        assert_eq!(state.counter(), 0);
    }

    #[test]
    fn test_quit() {
        let mut state = AppState::new();
        assert!(!state.should_quit());
        state.quit();
        assert!(state.should_quit());
    }

    #[test]
    fn test_increment() {
        let mut state = AppState::new();
        assert_eq!(state.counter(), 0);
        state.increment();
        assert_eq!(state.counter(), 1);
        state.increment();
        assert_eq!(state.counter(), 2);
    }

    #[test]
    fn test_decrement() {
        let mut state = AppState::new();
        state.increment();
        state.increment();
        assert_eq!(state.counter(), 2);
        state.decrement();
        assert_eq!(state.counter(), 1);
    }

    #[test]
    fn test_decrement_does_not_underflow() {
        let mut state = AppState::new();
        state.decrement();
        assert_eq!(state.counter(), 0);
    }
}
