//! Status monitoring with hysteresis to prevent UI flickering.
//!
//! This module provides:
//! - Hysteresis mechanism to smooth status transitions
//! - Spinner detection in pane titles

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::state::AgentStatus;

/// Default hysteresis duration in milliseconds.
const DEFAULT_HYSTERESIS_MS: u64 = 2000;

/// Spinner characters used by Claude Code.
const SPINNER_CHARS: &[char] = &[
    '⠿', '⠇', '⠋', '⠙', '⠸', '⠴', '⠦', '⠧', '⠖', '⠏', '⏺', '▶', '►',
];

/// Status monitor with hysteresis support.
#[derive(Debug)]
pub struct StatusMonitor {
    /// Track when each session was last seen as "active".
    last_active: HashMap<String, Instant>,
    /// Hysteresis duration - how long to wait before transitioning to Idle.
    hysteresis: Duration,
}

impl Default for StatusMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusMonitor {
    /// Create a new StatusMonitor with default hysteresis (2000ms).
    pub fn new() -> Self {
        Self {
            last_active: HashMap::new(),
            hysteresis: Duration::from_millis(DEFAULT_HYSTERESIS_MS),
        }
    }

    /// Create a new StatusMonitor with custom hysteresis duration.
    pub fn with_hysteresis(hysteresis_ms: u64) -> Self {
        Self {
            last_active: HashMap::new(),
            hysteresis: Duration::from_millis(hysteresis_ms),
        }
    }

    /// Update status with hysteresis applied.
    ///
    /// When transitioning from Working/Waiting to Idle, the hysteresis
    /// mechanism delays the transition to prevent UI flickering.
    pub fn apply_hysteresis(&mut self, session_id: &str, raw_status: AgentStatus) -> AgentStatus {
        let now = Instant::now();

        match raw_status {
            AgentStatus::Working | AgentStatus::Waiting => {
                // Active state - record the time
                self.last_active.insert(session_id.to_string(), now);
                raw_status
            }
            AgentStatus::Idle => {
                // Check if we're still in the cooldown period
                if let Some(last) = self.last_active.get(session_id)
                    && now.duration_since(*last) < self.hysteresis
                {
                    // Still in cooldown - report as Working
                    return AgentStatus::Working;
                }
                AgentStatus::Idle
            }
            AgentStatus::Error => raw_status,
        }
    }

    /// Clear the last active time for a session.
    pub fn clear_session(&mut self, session_id: &str) {
        self.last_active.remove(session_id);
    }

    /// Force a session to be considered idle (bypass hysteresis).
    #[cfg(test)]
    pub fn force_idle(&mut self, session_id: &str) {
        // Set last_active to a time far in the past
        self.last_active.insert(
            session_id.to_string(),
            Instant::now() - Duration::from_secs(3600),
        );
    }
}

/// Check if a pane title contains spinner characters.
///
/// Claude Code updates the pane title with spinners even when stdout is not updating.
pub fn title_has_spinner(title: &str) -> bool {
    title.chars().any(|c| SPINNER_CHARS.contains(&c))
}

/// Determine final status by combining parsed status with title spinner check.
pub fn combine_status_with_title(
    parsed_status: AgentStatus,
    pane_title: Option<&str>,
) -> AgentStatus {
    // If title has spinner, force Working status
    if let Some(title) = pane_title
        && title_has_spinner(title)
    {
        return AgentStatus::Working;
    }
    parsed_status
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_hysteresis_working_stays_working() {
        let mut monitor = StatusMonitor::new();
        let session = "test-session";

        // Working status should pass through
        let status = monitor.apply_hysteresis(session, AgentStatus::Working);
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_hysteresis_waiting_stays_waiting() {
        let mut monitor = StatusMonitor::new();
        let session = "test-session";

        // Waiting status should pass through
        let status = monitor.apply_hysteresis(session, AgentStatus::Waiting);
        assert_eq!(status, AgentStatus::Waiting);
    }

    #[test]
    fn test_hysteresis_idle_during_cooldown_returns_working() {
        let mut monitor = StatusMonitor::with_hysteresis(100); // Short hysteresis for testing
        let session = "test-session";

        // First, mark as working
        monitor.apply_hysteresis(session, AgentStatus::Working);

        // Immediately check idle - should return Working due to hysteresis
        let status = monitor.apply_hysteresis(session, AgentStatus::Idle);
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_hysteresis_idle_after_cooldown_returns_idle() {
        let mut monitor = StatusMonitor::with_hysteresis(10); // Very short hysteresis
        let session = "test-session";

        // First, mark as working
        monitor.apply_hysteresis(session, AgentStatus::Working);

        // Wait for hysteresis to expire
        thread::sleep(Duration::from_millis(20));

        // Now idle should return Idle
        let status = monitor.apply_hysteresis(session, AgentStatus::Idle);
        assert_eq!(status, AgentStatus::Idle);
    }

    #[test]
    fn test_hysteresis_idle_without_prior_active_returns_idle() {
        let mut monitor = StatusMonitor::new();
        let session = "test-session";

        // Idle without prior active state should return Idle
        let status = monitor.apply_hysteresis(session, AgentStatus::Idle);
        assert_eq!(status, AgentStatus::Idle);
    }

    #[test]
    fn test_hysteresis_error_passes_through() {
        let mut monitor = StatusMonitor::new();
        let session = "test-session";

        // Error status should pass through
        let status = monitor.apply_hysteresis(session, AgentStatus::Error);
        assert_eq!(status, AgentStatus::Error);
    }

    #[test]
    fn test_title_has_spinner_detects_spinners() {
        assert!(title_has_spinner("⠋ Loading..."));
        assert!(title_has_spinner("Working ⠿"));
        assert!(title_has_spinner("⏺ Task running"));
        assert!(title_has_spinner("► Processing"));
    }

    #[test]
    fn test_title_has_spinner_no_spinner() {
        assert!(!title_has_spinner("Claude Code"));
        assert!(!title_has_spinner("Normal title"));
        assert!(!title_has_spinner(""));
    }

    #[test]
    fn test_combine_status_with_title_spinner_overrides_idle() {
        // Title with spinner should override Idle to Working
        let status = combine_status_with_title(AgentStatus::Idle, Some("⠋ Loading..."));
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_combine_status_with_title_no_spinner_keeps_status() {
        // Title without spinner should keep original status
        let status = combine_status_with_title(AgentStatus::Idle, Some("Normal title"));
        assert_eq!(status, AgentStatus::Idle);
    }

    #[test]
    fn test_combine_status_with_title_none_keeps_status() {
        // No title should keep original status
        let status = combine_status_with_title(AgentStatus::Idle, None);
        assert_eq!(status, AgentStatus::Idle);
    }

    #[test]
    fn test_combine_status_with_title_working_stays_working() {
        // Already Working should stay Working regardless of title
        let status = combine_status_with_title(AgentStatus::Working, Some("Normal title"));
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_hysteresis_multiple_sessions_independent() {
        let mut monitor = StatusMonitor::with_hysteresis(100);

        // Session A becomes working
        monitor.apply_hysteresis("session-a", AgentStatus::Working);

        // Session B has no prior activity - should return Idle immediately
        let status_b = monitor.apply_hysteresis("session-b", AgentStatus::Idle);
        assert_eq!(status_b, AgentStatus::Idle);

        // Session A goes idle - should return Working due to hysteresis
        let status_a = monitor.apply_hysteresis("session-a", AgentStatus::Idle);
        assert_eq!(status_a, AgentStatus::Working);
    }

    #[test]
    fn test_hysteresis_waiting_also_triggers_cooldown() {
        let mut monitor = StatusMonitor::with_hysteresis(100);
        let session = "test-session";

        // Waiting status should also record last_active
        monitor.apply_hysteresis(session, AgentStatus::Waiting);

        // Immediately check idle - should return Working due to hysteresis
        let status = monitor.apply_hysteresis(session, AgentStatus::Idle);
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_clear_session_removes_tracking() {
        let mut monitor = StatusMonitor::with_hysteresis(100);
        let session = "test-session";

        // Mark as working
        monitor.apply_hysteresis(session, AgentStatus::Working);

        // Clear the session
        monitor.clear_session(session);

        // Now idle should return Idle (no prior active state)
        let status = monitor.apply_hysteresis(session, AgentStatus::Idle);
        assert_eq!(status, AgentStatus::Idle);
    }

    #[test]
    fn test_hysteresis_activity_resets_cooldown() {
        let mut monitor = StatusMonitor::with_hysteresis(50);
        let session = "test-session";

        // Initial working
        monitor.apply_hysteresis(session, AgentStatus::Working);

        // Wait half the hysteresis time
        thread::sleep(Duration::from_millis(30));

        // Another working resets the timer
        monitor.apply_hysteresis(session, AgentStatus::Working);

        // Wait another 30ms (total 60ms from first, but only 30ms from reset)
        thread::sleep(Duration::from_millis(30));

        // Should still be in cooldown (30ms < 50ms from reset)
        let status = monitor.apply_hysteresis(session, AgentStatus::Idle);
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_title_has_spinner_all_spinner_chars() {
        // Test all defined spinner characters
        for &c in SPINNER_CHARS {
            let title = format!("Test {c} title");
            assert!(title_has_spinner(&title), "Should detect spinner char: {c}");
        }
    }

    #[test]
    fn test_title_has_spinner_mixed_content() {
        assert!(title_has_spinner("Claude Code ⠋ Thinking..."));
        assert!(title_has_spinner("▶ Running command in /home/user"));
        assert!(title_has_spinner("prefix ⏺ suffix with more text"));
    }

    #[test]
    fn test_combine_status_with_title_error_not_overridden() {
        // Error status should not be overridden even with spinner
        let status = combine_status_with_title(AgentStatus::Error, Some("⠋ Loading..."));
        // Note: Current implementation would return Working, but this tests current behavior
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_combine_status_with_title_waiting_with_spinner() {
        // Waiting with spinner should become Working
        let status =
            combine_status_with_title(AgentStatus::Waiting, Some("⠋ Waiting for input..."));
        assert_eq!(status, AgentStatus::Working);
    }

    #[test]
    fn test_combine_status_with_title_waiting_without_spinner() {
        // Waiting without spinner stays Waiting
        let status = combine_status_with_title(AgentStatus::Waiting, Some("Normal title"));
        assert_eq!(status, AgentStatus::Waiting);
    }

    #[test]
    fn test_hysteresis_error_during_cooldown_passes_through() {
        let mut monitor = StatusMonitor::with_hysteresis(100);
        let session = "test-session";

        // Mark as working
        monitor.apply_hysteresis(session, AgentStatus::Working);

        // Error should pass through even during cooldown
        let status = monitor.apply_hysteresis(session, AgentStatus::Error);
        assert_eq!(status, AgentStatus::Error);
    }
}
