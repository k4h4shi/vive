use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::state::AppState;

/// Poll for terminal events with a timeout.
pub fn poll_event(timeout: Duration) -> Result<Option<Event>> {
    if event::poll(timeout)? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}

/// Handle a key event by updating the application state.
pub fn handle_key_event(key: KeyEvent, state: &mut AppState) {
    match key.code {
        KeyCode::Char('q') => state.quit(),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Char('j') | KeyCode::Down => state.increment(),
        KeyCode::Char('k') | KeyCode::Up => state.decrement(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_quit_on_q() {
        let mut state = AppState::new();
        handle_key_event(key_event(KeyCode::Char('q')), &mut state);
        assert!(state.should_quit());
    }

    #[test]
    fn test_quit_on_ctrl_c() {
        let mut state = AppState::new();
        handle_key_event(
            key_event_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut state,
        );
        assert!(state.should_quit());
    }

    #[test]
    fn test_increment_on_j() {
        let mut state = AppState::new();
        handle_key_event(key_event(KeyCode::Char('j')), &mut state);
        assert_eq!(state.counter(), 1);
    }

    #[test]
    fn test_increment_on_down() {
        let mut state = AppState::new();
        handle_key_event(key_event(KeyCode::Down), &mut state);
        assert_eq!(state.counter(), 1);
    }

    #[test]
    fn test_decrement_on_k() {
        let mut state = AppState::new();
        state.increment();
        state.increment();
        handle_key_event(key_event(KeyCode::Char('k')), &mut state);
        assert_eq!(state.counter(), 1);
    }

    #[test]
    fn test_decrement_on_up() {
        let mut state = AppState::new();
        state.increment();
        state.increment();
        handle_key_event(key_event(KeyCode::Up), &mut state);
        assert_eq!(state.counter(), 1);
    }

    #[test]
    fn test_unknown_key_does_nothing() {
        let mut state = AppState::new();
        handle_key_event(key_event(KeyCode::Char('x')), &mut state);
        assert!(!state.should_quit());
        assert_eq!(state.counter(), 0);
    }
}
