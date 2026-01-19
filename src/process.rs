//! Process Monitor module for detecting Claude agent states.
//!
//! This module provides functionality to:
//! - Detect Claude processes running in tmux panes via TTY
//! - Determine the state of Claude agents (Working, Waiting, Error, Idle)
//! - Poll for state updates

// Allow dead code during development - these APIs will be used by main app
#![allow(dead_code)]

use std::process::Command;

use anyhow::{Context, Result};

use crate::state::AgentStatus;

/// Information about a Claude process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,
    /// Process state (e.g., "S+", "R+", "T").
    pub stat: String,
    /// CPU usage percentage.
    pub cpu: f32,
}

/// Trait for detecting processes, allowing for mocking in tests.
pub trait ProcessDetector: Send + Sync {
    /// Get the TTY for a tmux window.
    fn get_window_tty(&self, session_name: &str, window_name: &str) -> Result<Option<String>>;

    /// Get Claude process info for a given TTY.
    fn get_claude_process(&self, tty: &str) -> Result<Option<ProcessInfo>>;
}

/// Default implementation that executes actual system commands.
#[derive(Debug, Default, Clone)]
pub struct RealProcessDetector;

impl ProcessDetector for RealProcessDetector {
    fn get_window_tty(&self, session_name: &str, window_name: &str) -> Result<Option<String>> {
        let target = format!("{session_name}:{window_name}");
        let output = Command::new("tmux")
            .args(["list-panes", "-t", &target, "-F", "#{pane_tty}"])
            .output()
            .context("Failed to execute tmux list-panes")?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let tty = stdout.lines().next().map(|s| s.trim().to_string());

        Ok(tty.filter(|s| !s.is_empty()))
    }

    fn get_claude_process(&self, tty: &str) -> Result<Option<ProcessInfo>> {
        // Strip /dev/ prefix if present
        let tty_short = tty.strip_prefix("/dev/").unwrap_or(tty);

        let output = Command::new("ps")
            .args(["-t", tty_short, "-o", "pid=,stat=,%cpu=,comm="])
            .output()
            .context("Failed to execute ps command")?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Find the claude process line
        for line in stdout.lines() {
            let line = line.trim();
            if line.ends_with("claude") {
                return parse_process_line(line);
            }
        }

        Ok(None)
    }
}

/// Parse a process info line from ps output.
fn parse_process_line(line: &str) -> Result<Option<ProcessInfo>> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return Ok(None);
    }

    let pid = parts[0].parse::<u32>().unwrap_or(0);
    let stat = parts[1].to_string();
    let cpu = parts[2].parse::<f32>().unwrap_or(0.0);

    Ok(Some(ProcessInfo { pid, stat, cpu }))
}

/// Determines the agent state based on process information.
pub fn determine_agent_state(process_info: Option<&ProcessInfo>) -> AgentStatus {
    let Some(info) = process_info else {
        return AgentStatus::Idle;
    };

    // Check if process is stopped or zombie
    if info.stat.contains('T') || info.stat.contains('Z') {
        return AgentStatus::Error;
    }

    // Check if process is running with high CPU (actively working)
    if info.stat.contains('R') || info.cpu > 5.0 {
        return AgentStatus::Working;
    }

    // Process is sleeping (S) - likely waiting for input or idle
    // If it's a foreground process (+), it's waiting for input
    if info.stat.contains('+') {
        return AgentStatus::Waiting;
    }

    AgentStatus::Idle
}

/// A process monitor that tracks Claude agent states.
pub struct ProcessMonitor<D: ProcessDetector = RealProcessDetector> {
    detector: D,
}

impl ProcessMonitor<RealProcessDetector> {
    /// Create a new ProcessMonitor with the default detector.
    pub fn new() -> Self {
        Self {
            detector: RealProcessDetector,
        }
    }
}

impl Default for ProcessMonitor<RealProcessDetector> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: ProcessDetector> ProcessMonitor<D> {
    /// Create a new ProcessMonitor with a custom detector.
    pub fn with_detector(detector: D) -> Self {
        Self { detector }
    }

    /// Get the agent state for a tmux window.
    pub fn get_agent_state(&self, session_name: &str, window_name: &str) -> Result<AgentStatus> {
        let Some(tty) = self.detector.get_window_tty(session_name, window_name)? else {
            return Ok(AgentStatus::Error);
        };

        let process_info = self.detector.get_claude_process(&tty)?;
        Ok(determine_agent_state(process_info.as_ref()))
    }

    /// Get detailed process info for a tmux window.
    pub fn get_process_info(
        &self,
        session_name: &str,
        window_name: &str,
    ) -> Result<Option<ProcessInfo>> {
        let Some(tty) = self.detector.get_window_tty(session_name, window_name)? else {
            return Ok(None);
        };

        self.detector.get_claude_process(&tty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_icon() {
        assert_eq!(AgentStatus::Working.icon(), "🟢");
        assert_eq!(AgentStatus::Waiting.icon(), "🟡");
        assert_eq!(AgentStatus::Error.icon(), "🔴");
        assert_eq!(AgentStatus::Idle.icon(), "⚪");
    }

    #[test]
    fn test_determine_agent_state_idle() {
        assert_eq!(determine_agent_state(None), AgentStatus::Idle);
    }

    #[test]
    fn test_determine_agent_state_working_running() {
        let info = ProcessInfo {
            pid: 1234,
            stat: "R+".to_string(),
            cpu: 1.0,
        };
        assert_eq!(determine_agent_state(Some(&info)), AgentStatus::Working);
    }

    #[test]
    fn test_determine_agent_state_working_high_cpu() {
        let info = ProcessInfo {
            pid: 1234,
            stat: "S+".to_string(),
            cpu: 25.0,
        };
        assert_eq!(determine_agent_state(Some(&info)), AgentStatus::Working);
    }

    #[test]
    fn test_determine_agent_state_input_needed() {
        let info = ProcessInfo {
            pid: 1234,
            stat: "S+".to_string(),
            cpu: 0.5,
        };
        assert_eq!(determine_agent_state(Some(&info)), AgentStatus::Waiting);
    }

    #[test]
    fn test_determine_agent_state_stopped() {
        let info = ProcessInfo {
            pid: 1234,
            stat: "T".to_string(),
            cpu: 0.0,
        };
        assert_eq!(determine_agent_state(Some(&info)), AgentStatus::Error);
    }

    #[test]
    fn test_determine_agent_state_zombie() {
        let info = ProcessInfo {
            pid: 1234,
            stat: "Z".to_string(),
            cpu: 0.0,
        };
        assert_eq!(determine_agent_state(Some(&info)), AgentStatus::Error);
    }

    #[test]
    fn test_determine_agent_state_background() {
        let info = ProcessInfo {
            pid: 1234,
            stat: "S".to_string(),
            cpu: 0.5,
        };
        assert_eq!(determine_agent_state(Some(&info)), AgentStatus::Idle);
    }

    #[test]
    fn test_parse_process_line() {
        let line = "1234 S+  12.5 claude";
        let result = parse_process_line(line).unwrap();
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.pid, 1234);
        assert_eq!(info.stat, "S+");
        assert!((info.cpu - 12.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_process_line_invalid() {
        let line = "invalid";
        let result = parse_process_line(line).unwrap();
        assert!(result.is_none());
    }
}
