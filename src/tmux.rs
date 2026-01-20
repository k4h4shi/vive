//! Tmux Orchestrator - The "Hands" layer.
//!
//! This module provides an interface to interact with Tmux for session management.
//! It wraps tmux CLI commands and provides higher-level abstractions for:
//! - Session management (create, attach, check existence)
//! - Window management (create, select, list)
//! - Pane management (split, send keys)
//! - Layout management (Cockpit layout strategy)

// Allow dead code during development - module not yet integrated with main app
#![allow(dead_code)]

use std::process::Command;

use anyhow::{Context, Result};

/// Result of a tmux command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxCommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Represents a tmux window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxWindow {
    pub index: u32,
    pub name: String,
    pub active: bool,
}

/// Represents a tmux pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxPane {
    pub index: u32,
    pub active: bool,
    pub current_path: String,
}

/// Information about a pane's layout position and size.
#[derive(Debug, Clone)]
pub struct PaneLayout {
    pub index: u32,
    pub width: u16,
    pub height: u16,
    pub left: u16,
    pub top: u16,
}

/// Dashboard layout with pane positions and content.
#[derive(Debug, Clone)]
pub struct DashboardLayout {
    pub total_width: u16,
    pub total_height: u16,
    pub panes: Vec<(PaneLayout, String)>, // (layout, content)
}

/// Layout strategy for the Cockpit view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CockpitLayout {
    /// Single pane (no split).
    Single,
    /// Two panes side by side (vertical split).
    TwoVertical,
    /// Two panes stacked (horizontal split).
    TwoHorizontal,
    /// Main pane on left, smaller panes stacked on right.
    MainLeft,
    /// Main pane on top, smaller panes below.
    MainTop,
    /// Grid layout (2x2).
    Grid,
}

/// Helper macro to create Vec<String> from string literals.
macro_rules! args {
    ($($arg:expr),* $(,)?) => {
        vec![$($arg.to_string()),*]
    };
}

/// Trait for executing tmux commands.
/// This abstraction allows for mocking in tests.
pub trait TmuxExecutor: Send + Sync {
    /// Execute a tmux command and return the result.
    fn execute(&self, args: Vec<String>) -> Result<TmuxCommandResult>;
}

#[cfg(test)]
mockall::mock! {
    pub TmuxExecutor {}

    impl TmuxExecutor for TmuxExecutor {
        fn execute(&self, args: Vec<String>) -> Result<TmuxCommandResult>;
    }
}

/// Default implementation that executes actual tmux commands.
#[derive(Debug, Default, Clone)]
pub struct RealTmuxExecutor;

impl TmuxExecutor for RealTmuxExecutor {
    fn execute(&self, args: Vec<String>) -> Result<TmuxCommandResult> {
        let output = Command::new("tmux")
            .args(&args)
            .output()
            .context("Failed to execute tmux command")?;

        Ok(TmuxCommandResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

/// The Tmux Orchestrator handles all tmux interactions.
pub struct TmuxOrchestrator<E: TmuxExecutor = RealTmuxExecutor> {
    executor: E,
}

impl TmuxOrchestrator<RealTmuxExecutor> {
    /// Create a new TmuxOrchestrator with the default executor.
    pub fn new() -> Self {
        Self {
            executor: RealTmuxExecutor,
        }
    }
}

impl Default for TmuxOrchestrator<RealTmuxExecutor> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: TmuxExecutor> TmuxOrchestrator<E> {
    /// Create a new TmuxOrchestrator with a custom executor.
    pub fn with_executor(executor: E) -> Self {
        Self { executor }
    }

    // ========================================================================
    // Session Management
    // ========================================================================

    /// Check if a tmux session exists.
    pub fn has_session(&self, session_name: &str) -> Result<bool> {
        let result = self
            .executor
            .execute(args!["has-session", "-t", session_name])?;
        Ok(result.success)
    }

    /// Create a new tmux session.
    ///
    /// # Arguments
    /// * `session_name` - Name of the session to create
    /// * `start_directory` - Optional starting directory for the session
    /// * `detached` - If true, create session in detached mode
    pub fn new_session(
        &self,
        session_name: &str,
        start_directory: Option<&str>,
        detached: bool,
    ) -> Result<()> {
        let mut cmd_args = vec!["new-session".to_string()];

        if detached {
            cmd_args.push("-d".to_string());
        }

        cmd_args.push("-s".to_string());
        cmd_args.push(session_name.to_string());

        if let Some(dir) = start_directory {
            cmd_args.push("-c".to_string());
            cmd_args.push(dir.to_string());
        }

        let result = self.executor.execute(cmd_args)?;

        if !result.success {
            anyhow::bail!(
                "Failed to create session '{}': {}",
                session_name,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Kill a tmux session.
    pub fn kill_session(&self, session_name: &str) -> Result<()> {
        let result = self
            .executor
            .execute(args!["kill-session", "-t", session_name])?;

        if !result.success {
            anyhow::bail!(
                "Failed to kill session '{}': {}",
                session_name,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Ensure a session exists, creating it if necessary.
    ///
    /// Returns true if the session was created, false if it already existed.
    pub fn ensure_session(
        &self,
        session_name: &str,
        start_directory: Option<&str>,
    ) -> Result<bool> {
        if self.has_session(session_name)? {
            Ok(false)
        } else {
            self.new_session(session_name, start_directory, true)?;
            Ok(true)
        }
    }

    /// Build the tmux command arguments for attaching to a session.
    ///
    /// # Arguments
    /// * `session_name` - Name of the session to attach to
    /// * `inside_tmux` - Whether we're already inside a tmux session
    ///
    /// # Returns
    /// Vector of command arguments for tmux.
    pub fn build_attach_command(session_name: &str, inside_tmux: bool) -> Vec<&str> {
        if inside_tmux {
            vec!["switch-client", "-t", session_name]
        } else {
            vec!["attach", "-t", session_name]
        }
    }

    /// Attach to an existing session.
    ///
    /// If already inside tmux, switches to the session.
    /// Otherwise, attaches to the session.
    pub fn attach_session(&self, session_name: &str) -> Result<()> {
        // Check if we're inside tmux
        let inside_tmux = std::env::var("TMUX").is_ok();

        let result = if inside_tmux {
            self.executor
                .execute(args!["switch-client", "-t", session_name])?
        } else {
            self.executor.execute(args!["attach", "-t", session_name])?
        };

        if !result.success {
            anyhow::bail!(
                "Failed to attach to session '{}': {}",
                session_name,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Replace the current process with tmux attached to the session.
    ///
    /// This function uses `exec()` to replace the current process, so it never
    /// returns on success. On Unix systems, this provides a seamless transition
    /// into the tmux session.
    ///
    /// # Arguments
    /// * `session_name` - Name of the session to attach to
    ///
    /// # Returns
    /// This function only returns if there's an error. On success, the current
    /// process is replaced.
    #[cfg(unix)]
    pub fn exec_into_session(&self, session_name: &str) -> Result<()> {
        use std::os::unix::process::CommandExt;

        let inside_tmux = std::env::var("TMUX").is_ok();
        let args = Self::build_attach_command(session_name, inside_tmux);

        let err = Command::new("tmux").args(&args).exec();

        // exec() only returns if there's an error
        Err(anyhow::anyhow!(
            "Failed to exec into session '{session_name}': {err}"
        ))
    }

    // ========================================================================
    // Window Management
    // ========================================================================

    /// Create a new window in a session.
    pub fn new_window(
        &self,
        session_name: &str,
        window_name: &str,
        start_directory: Option<&str>,
    ) -> Result<()> {
        let mut cmd_args = args!["new-window", "-t", session_name, "-n", window_name];

        if let Some(dir) = start_directory {
            cmd_args.push("-c".to_string());
            cmd_args.push(dir.to_string());
        }

        let result = self.executor.execute(cmd_args)?;

        if !result.success {
            anyhow::bail!(
                "Failed to create window '{}' in session '{}': {}",
                window_name,
                session_name,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Select (switch to) a window in a session.
    pub fn select_window(&self, session_name: &str, window_name: &str) -> Result<()> {
        let target = format!("{session_name}:{window_name}");
        let result = self
            .executor
            .execute(args!["select-window", "-t", &target])?;

        if !result.success {
            anyhow::bail!(
                "Failed to select window '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// List windows in a session.
    pub fn list_windows(&self, session_name: &str) -> Result<Vec<TmuxWindow>> {
        let result = self.executor.execute(args![
            "list-windows",
            "-t",
            session_name,
            "-F",
            "#{window_index}:#{window_name}:#{window_active}",
        ])?;

        if !result.success {
            anyhow::bail!(
                "Failed to list windows in session '{}': {}",
                session_name,
                result.stderr.trim()
            );
        }

        let windows = result
            .stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    Some(TmuxWindow {
                        index: parts[0].parse().unwrap_or(0),
                        name: parts[1].to_string(),
                        active: parts[2] == "1",
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(windows)
    }

    /// Check if a window exists in a session.
    pub fn has_window(&self, session_name: &str, window_name: &str) -> Result<bool> {
        let windows = self.list_windows(session_name)?;
        Ok(windows.iter().any(|w| w.name == window_name))
    }

    /// Kill a window in a session.
    pub fn kill_window(&self, session_name: &str, window_name: &str) -> Result<()> {
        let target = format!("{session_name}:{window_name}");
        let result = self.executor.execute(args!["kill-window", "-t", &target])?;

        if !result.success {
            anyhow::bail!(
                "Failed to kill window '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    // ========================================================================
    // Pane Management
    // ========================================================================

    /// Split a window/pane horizontally (creates panes stacked vertically).
    ///
    /// # Arguments
    /// * `target` - The target pane (session:window.pane)
    /// * `start_directory` - Optional directory to start the new pane in
    /// * `command` - Optional command to run in the new pane (instead of default shell)
    pub fn split_window_horizontal(
        &self,
        target: &str,
        start_directory: Option<&str>,
        command: Option<&str>,
    ) -> Result<()> {
        let mut cmd_args = args!["split-window", "-v", "-t", target];

        if let Some(dir) = start_directory {
            cmd_args.push("-c".to_string());
            cmd_args.push(dir.to_string());
        }

        if let Some(cmd) = command {
            cmd_args.push(cmd.to_string());
        }

        let result = self.executor.execute(cmd_args)?;

        if !result.success {
            anyhow::bail!(
                "Failed to split window horizontally '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Split a window/pane vertically (creates panes side by side).
    ///
    /// # Arguments
    /// * `target` - The target pane (session:window.pane)
    /// * `start_directory` - Optional directory to start the new pane in
    /// * `command` - Optional command to run in the new pane (instead of default shell)
    pub fn split_window_vertical(
        &self,
        target: &str,
        start_directory: Option<&str>,
        command: Option<&str>,
    ) -> Result<()> {
        let mut cmd_args = args!["split-window", "-h", "-t", target];

        if let Some(dir) = start_directory {
            cmd_args.push("-c".to_string());
            cmd_args.push(dir.to_string());
        }

        if let Some(cmd) = command {
            cmd_args.push(cmd.to_string());
        }

        let result = self.executor.execute(cmd_args)?;

        if !result.success {
            anyhow::bail!(
                "Failed to split window vertically '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Send keys to a target (session:window.pane).
    ///
    /// When `enter` is true, the input text and Enter key are sent as **separate**
    /// tmux commands to ensure reliable delivery. This follows the tmuxcc approach
    /// where splitting input and Enter prevents timing issues with Claude Code.
    pub fn send_keys(&self, target: &str, keys: &str, enter: bool) -> Result<()> {
        // First, send the input text
        let cmd_args = args!["send-keys", "-t", target, keys];
        let result = self.executor.execute(cmd_args)?;

        if !result.success {
            anyhow::bail!(
                "Failed to send keys to '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        // Then, send Enter as a separate command if requested
        // This separation ensures reliable prompt delivery to Claude Code
        // Using C-m (Ctrl+M) which is the control sequence for carriage return
        if enter {
            let enter_args = args!["send-keys", "-t", target, "C-m"];
            let enter_result = self.executor.execute(enter_args)?;

            if !enter_result.success {
                anyhow::bail!(
                    "Failed to send Enter to '{}': {}",
                    target,
                    enter_result.stderr.trim()
                );
            }
        }

        Ok(())
    }

    /// Get the title of a tmux pane.
    ///
    /// # Arguments
    /// * `target` - Target pane (session:window.pane format)
    ///
    /// # Returns
    /// The pane title if available, None if not found or on error.
    pub fn get_pane_title(&self, target: &str) -> Result<Option<String>> {
        let result =
            self.executor
                .execute(args!["list-panes", "-t", target, "-F", "#{pane_title}"])?;

        if !result.success {
            return Ok(None);
        }

        let title = result.stdout.lines().next().map(|s| s.to_string());
        Ok(title.filter(|s| !s.is_empty()))
    }

    /// Capture the content of a tmux pane.
    ///
    /// # Arguments
    /// * `target` - Target pane (session:window.pane format)
    /// * `lines` - Number of lines to capture from the end
    ///
    /// # Returns
    /// The captured pane content as a string.
    pub fn capture_pane(&self, target: &str, lines: usize) -> Result<String> {
        let start_line = format!("-{lines}");
        // -e flag preserves ANSI escape sequences (colors)
        let result = self.executor.execute(args![
            "capture-pane",
            "-t",
            target,
            "-p",
            "-e",
            "-S",
            &start_line,
        ])?;

        if !result.success {
            anyhow::bail!(
                "Failed to capture pane '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        Ok(result.stdout)
    }

    /// Capture content from all panes in a session.
    ///
    /// Returns a combined string with each pane's content separated by a header.
    pub fn capture_all_panes(&self, session: &str, lines_per_pane: usize) -> Result<String> {
        let panes = self.list_panes(session)?;

        if panes.is_empty() {
            return Ok(String::new());
        }

        let mut combined = String::new();
        for (idx, pane) in panes.iter().enumerate() {
            let pane_target = format!("{session}.{}", pane.index);
            if let Ok(content) = self.capture_pane(&pane_target, lines_per_pane) {
                if idx > 0 {
                    combined.push_str("\n─────────────────────────────────────────\n");
                }
                combined.push_str(&format!("── Pane {} ──\n", idx + 1));
                combined.push_str(&content);
            }
        }

        Ok(combined)
    }

    /// Select a layout for the current window.
    pub fn select_layout(&self, target: &str, layout: &str) -> Result<()> {
        let result = self
            .executor
            .execute(args!["select-layout", "-t", target, layout])?;

        if !result.success {
            anyhow::bail!(
                "Failed to select layout '{}' for '{}': {}",
                layout,
                target,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    // ========================================================================
    // Cockpit Layout Strategy
    // ========================================================================

    /// Apply a Cockpit layout to a window based on the number of tasks.
    ///
    /// This creates panes and arranges them according to the specified layout strategy.
    pub fn apply_cockpit_layout(
        &self,
        session_name: &str,
        window_name: &str,
        layout: CockpitLayout,
        directories: &[&str],
    ) -> Result<()> {
        let target = format!("{session_name}:{window_name}");

        match layout {
            CockpitLayout::Single => {
                // No splits needed, just ensure we're in the right directory
                if let Some(dir) = directories.first() {
                    self.send_keys(&target, &format!("cd {dir}"), true)?;
                }
            }
            CockpitLayout::TwoVertical => {
                // Split vertically (side by side)
                if directories.len() >= 2 {
                    self.split_window_vertical(&target, Some(directories[1]), None)?;
                }
            }
            CockpitLayout::TwoHorizontal => {
                // Split horizontally (stacked)
                if directories.len() >= 2 {
                    self.split_window_horizontal(&target, Some(directories[1]), None)?;
                }
            }
            CockpitLayout::MainLeft => {
                // Main pane on left, smaller panes stacked on right
                if directories.len() >= 2 {
                    self.split_window_vertical(&target, Some(directories[1]), None)?;
                    if directories.len() >= 3 {
                        let right_pane = format!("{target}.1");
                        self.split_window_horizontal(&right_pane, Some(directories[2]), None)?;
                    }
                }
                self.select_layout(&target, "main-vertical")?;
            }
            CockpitLayout::MainTop => {
                // Main pane on top, smaller panes below
                if directories.len() >= 2 {
                    self.split_window_horizontal(&target, Some(directories[1]), None)?;
                    if directories.len() >= 3 {
                        let bottom_pane = format!("{target}.1");
                        self.split_window_vertical(&bottom_pane, Some(directories[2]), None)?;
                    }
                }
                self.select_layout(&target, "main-horizontal")?;
            }
            CockpitLayout::Grid => {
                // 2x2 grid layout
                if directories.len() >= 2 {
                    self.split_window_vertical(&target, Some(directories[1]), None)?;
                }
                if directories.len() >= 3 {
                    self.split_window_horizontal(&format!("{target}.0"), Some(directories[2]), None)?;
                }
                if directories.len() >= 4 {
                    self.split_window_horizontal(&format!("{target}.1"), Some(directories[3]), None)?;
                }
                self.select_layout(&target, "tiled")?;
            }
        }

        Ok(())
    }

    /// Automatically select a Cockpit layout based on the number of tasks.
    pub fn auto_cockpit_layout(task_count: usize) -> CockpitLayout {
        match task_count {
            0 | 1 => CockpitLayout::Single,
            2 => CockpitLayout::TwoVertical,
            3 => CockpitLayout::MainLeft,
            _ => CockpitLayout::Grid,
        }
    }

    // ========================================================================
    // Dashboard Session Operations
    // ========================================================================

    /// Create a dashboard session for a project.
    ///
    /// The dashboard session shows all worktree sessions in a tiled layout.
    /// Each pane attaches to a worktree session using nested tmux.
    ///
    /// # Arguments
    /// * `project_name` - Name of the project
    /// * `worktree_sessions` - List of (session_id, worktree_path) for each worktree
    ///
    /// # Returns
    /// `Ok(true)` if the dashboard session was created, `Ok(false)` if it already existed.
    pub fn create_dashboard_session(
        &self,
        project_name: &str,
        worktree_sessions: &[(String, String)],
    ) -> Result<bool> {
        let dashboard_session = format!("{project_name}__dashboard");

        // Check if dashboard already exists
        if self.has_session(&dashboard_session)? {
            return Ok(false);
        }

        // Create the dashboard session (detached)
        // Use the first worktree's path as the starting directory
        let start_dir = worktree_sessions.first().map(|(_, path)| path.as_str());
        self.new_session(&dashboard_session, start_dir, true)?;

        // Set aggressive-resize for proper nested tmux behavior
        let _ = self.set_option(&dashboard_session, "aggressive-resize", "on");

        // Create panes for each worktree
        // Use session name only as target (tmux selects active window automatically)
        // This avoids issues with base-index settings (window could be 0 or 1)
        let target = &dashboard_session;

        for (idx, (session_id, _path)) in worktree_sessions.iter().enumerate() {
            let attach_cmd = format!(
                "unset TMUX; tmux attach -t {session_id} 2>/dev/null || echo 'Session not found: {session_id}'"
            );

            if idx == 0 {
                // First pane: use send_keys since the session was just created with a shell
                self.send_keys(target, &attach_cmd, true)?;
            } else {
                // Additional panes: split with command directly
                // This runs the attach command immediately in the new pane
                self.split_window_horizontal(target, None, Some(&attach_cmd))?;
            }
        }

        // Apply tiled layout for even distribution
        self.select_layout(target, "tiled")?;

        Ok(true)
    }

    /// Get the dashboard session name for a project.
    /// Uses double underscore to avoid tmux's session:window delimiter.
    pub fn dashboard_session_name(project_name: &str) -> String {
        format!("{project_name}__dashboard")
    }

    /// Add a pane to an existing dashboard session for a new worktree.
    ///
    /// This is called when a new worktree is created and the dashboard
    /// already exists. It adds a new pane and re-tiles the layout.
    ///
    /// # Arguments
    /// * `project_name` - Name of the project
    /// * `session_id` - Session ID for the new worktree
    ///
    /// # Returns
    /// `Ok(true)` if the pane was added, `Ok(false)` if dashboard doesn't exist.
    pub fn add_pane_to_dashboard(&self, project_name: &str, session_id: &str) -> Result<bool> {
        let dashboard_session = Self::dashboard_session_name(project_name);

        // Check if dashboard exists
        if !self.has_session(&dashboard_session)? {
            return Ok(false);
        }

        // Use session name only as target (avoids base-index issues)
        let target = &dashboard_session;

        // Build attach command
        let attach_cmd = format!(
            "unset TMUX; tmux attach -t {session_id} 2>/dev/null || echo 'Session not found: {session_id}'"
        );

        // Split window with command to attach to worktree session directly
        self.split_window_horizontal(target, None, Some(&attach_cmd))?;

        // Re-apply tiled layout for even distribution
        self.select_layout(target, "tiled")?;

        Ok(true)
    }

    /// Synchronize the dashboard session with the current worktrees.
    ///
    /// This recreates the dashboard session to reflect the current state
    /// of worktrees. Used after worktree deletion to remove stale panes.
    ///
    /// # Arguments
    /// * `project_name` - Name of the project
    /// * `worktree_sessions` - Current list of (session_id, worktree_path)
    ///
    /// # Returns
    /// `Ok(true)` if the dashboard was synced, `Ok(false)` if dashboard didn't exist.
    pub fn sync_dashboard(
        &self,
        project_name: &str,
        worktree_sessions: &[(String, String)],
    ) -> Result<bool> {
        let dashboard_session = Self::dashboard_session_name(project_name);

        // Check if dashboard exists
        if !self.has_session(&dashboard_session)? {
            return Ok(false);
        }

        // Kill the old dashboard and recreate it
        self.kill_session(&dashboard_session)?;
        self.create_dashboard_session(project_name, worktree_sessions)?;

        Ok(true)
    }

    /// List panes in a target (session:window).
    pub fn list_panes(&self, target: &str) -> Result<Vec<TmuxPane>> {
        let result = self.executor.execute(args![
            "list-panes",
            "-t",
            target,
            "-F",
            "#{pane_index}:#{pane_active}:#{pane_current_path}",
        ])?;

        if !result.success {
            anyhow::bail!(
                "Failed to list panes in '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        let panes = result
            .stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    Some(TmuxPane {
                        index: parts[0].parse().unwrap_or(0),
                        active: parts[1] == "1",
                        current_path: parts[2..].join(":"), // Handle paths with colons
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(panes)
    }

    /// Kill a specific pane.
    pub fn kill_pane(&self, target: &str) -> Result<()> {
        let result = self.executor.execute(args!["kill-pane", "-t", target])?;

        if !result.success {
            anyhow::bail!(
                "Failed to kill pane '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Respawn a pane with a new command.
    ///
    /// This kills the current process in the pane and starts the specified command.
    /// More reliable than send_keys for initial pane setup.
    pub fn respawn_pane(&self, target: &str, command: &str) -> Result<()> {
        let result = self
            .executor
            .execute(args!["respawn-pane", "-k", "-t", target, command])?;

        if !result.success {
            anyhow::bail!(
                "Failed to respawn pane '{}': {}",
                target,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Ensure a dashboard session exists and all panes are properly attached.
    ///
    /// This method:
    /// - Creates a new dashboard if it doesn't exist
    /// - If it exists, refreshes all panes by re-sending attach commands
    /// - Adds missing panes if worktrees have been added
    ///
    /// # Arguments
    /// * `project_name` - Name of the project
    /// * `worktree_sessions` - List of (session_id, worktree_path) for each worktree
    pub fn ensure_dashboard_session(
        &self,
        project_name: &str,
        worktree_sessions: &[(String, String)],
    ) -> Result<()> {
        let dashboard_session = Self::dashboard_session_name(project_name);

        // If dashboard exists, kill it to recreate fresh
        // This ensures all panes are properly connected to their sessions
        if self.has_session(&dashboard_session)? {
            let _ = self.kill_session(&dashboard_session);
        }

        // Create fresh dashboard with all worktree panes
        self.create_dashboard_session(project_name, worktree_sessions)?;
        Ok(())
    }

    // ========================================================================
    // High-Level Operations
    // ========================================================================

    /// Create a project session with a window for a specific worktree/task.
    ///
    /// This is a convenience method that:
    /// 1. Ensures the session exists
    /// 2. Creates or switches to the window
    /// 3. Optionally sends a command to start
    pub fn create_project_window(
        &self,
        session_name: &str,
        window_name: &str,
        worktree_path: &str,
        initial_command: Option<&str>,
    ) -> Result<bool> {
        // Ensure session exists
        let session_created = self.ensure_session(session_name, None)?;

        // Check if window already exists
        if self.has_window(session_name, window_name)? {
            self.select_window(session_name, window_name)?;
            return Ok(false);
        }

        // Create new window
        self.new_window(session_name, window_name, Some(worktree_path))?;

        // Send initial command if provided
        if let Some(cmd) = initial_command {
            let target = format!("{session_name}:{window_name}");
            self.send_keys(&target, cmd, true)?;
        }

        Ok(session_created)
    }

    /// Load a tmux configuration file.
    pub fn source_file(&self, config_path: &str) -> Result<()> {
        let result = self.executor.execute(args!["source-file", config_path])?;

        if !result.success {
            anyhow::bail!(
                "Failed to source config '{}': {}",
                config_path,
                result.stderr.trim()
            );
        }

        Ok(())
    }

    /// Set a tmux option for a session.
    pub fn set_option(&self, session_name: &str, option: &str, value: &str) -> Result<()> {
        let result =
            self.executor
                .execute(args!["set-option", "-t", session_name, option, value,])?;

        // set-option can fail silently for some options, so we don't always error
        if !result.success && !result.stderr.is_empty() {
            anyhow::bail!(
                "Failed to set option '{}' = '{}': {}",
                option,
                value,
                result.stderr.trim()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_success(stdout: &str) -> TmuxCommandResult {
        TmuxCommandResult {
            success: true,
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    fn mock_failure(stderr: &str) -> TmuxCommandResult {
        TmuxCommandResult {
            success: false,
            stdout: String::new(),
            stderr: stderr.to_string(),
        }
    }

    fn to_strings(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_has_session_exists() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "test-session"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        assert!(orchestrator.has_session("test-session").unwrap());
    }

    #[test]
    fn test_has_session_not_exists() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "test-session"]))
            .returning(|_| Ok(mock_failure("session not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        assert!(!orchestrator.has_session("test-session").unwrap());
    }

    #[test]
    fn test_new_session_detached() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["new-session", "-d", "-s", "my-session"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator.new_session("my-session", None, true).unwrap();
    }

    #[test]
    fn test_new_session_with_directory() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "new-session",
                        "-d",
                        "-s",
                        "my-session",
                        "-c",
                        "/path/to/dir",
                    ])
            })
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .new_session("my-session", Some("/path/to/dir"), true)
            .unwrap();
    }

    #[test]
    fn test_ensure_session_creates_new() {
        let mut mock = MockTmuxExecutor::new();

        // First call: has-session returns false
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-session"]))
            .returning(|_| Ok(mock_failure("session not found")));

        // Second call: new-session
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["new-session", "-d", "-s", "my-session"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let created = orchestrator.ensure_session("my-session", None).unwrap();
        assert!(created);
    }

    #[test]
    fn test_ensure_session_already_exists() {
        let mut mock = MockTmuxExecutor::new();

        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-session"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let created = orchestrator.ensure_session("my-session", None).unwrap();
        assert!(!created);
    }

    #[test]
    fn test_list_windows() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "list-windows",
                        "-t",
                        "my-session",
                        "-F",
                        "#{window_index}:#{window_name}:#{window_active}",
                    ])
            })
            .returning(|_| Ok(mock_success("0:main:1\n1:issue-123:0\n2:issue-456:0\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let windows = orchestrator.list_windows("my-session").unwrap();

        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].name, "main");
        assert!(windows[0].active);
        assert_eq!(windows[1].name, "issue-123");
        assert!(!windows[1].active);
    }

    #[test]
    fn test_has_window() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .times(2)
            .returning(|_| Ok(mock_success("0:main:1\n1:issue-123:0\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        assert!(orchestrator.has_window("my-session", "issue-123").unwrap());
        assert!(!orchestrator.has_window("my-session", "issue-999").unwrap());
    }

    #[test]
    fn test_send_keys_with_enter_separated() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // First call: send the input text
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["send-keys", "-t", "my-session:main", "claude"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // Second call: send Enter separately (C-m = Ctrl+M)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["send-keys", "-t", "my-session:main", "C-m"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .send_keys("my-session:main", "claude", true)
            .unwrap();
    }

    #[test]
    fn test_auto_cockpit_layout() {
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::auto_cockpit_layout(0),
            CockpitLayout::Single
        );
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::auto_cockpit_layout(1),
            CockpitLayout::Single
        );
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::auto_cockpit_layout(2),
            CockpitLayout::TwoVertical
        );
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::auto_cockpit_layout(3),
            CockpitLayout::MainLeft
        );
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::auto_cockpit_layout(4),
            CockpitLayout::Grid
        );
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::auto_cockpit_layout(10),
            CockpitLayout::Grid
        );
    }

    #[test]
    fn test_split_window_vertical() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["split-window", "-h", "-t", "my-session:main"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .split_window_vertical("my-session:main", None, None)
            .unwrap();
    }

    #[test]
    fn test_split_window_horizontal() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args == to_strings(&["split-window", "-v", "-t", "my-session:main", "-c", "/path"])
            })
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .split_window_horizontal("my-session:main", Some("/path"), None)
            .unwrap();
    }

    #[test]
    fn test_new_window() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "new-window",
                        "-t",
                        "my-session",
                        "-n",
                        "issue-42",
                        "-c",
                        "/worktree/path",
                    ])
            })
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .new_window("my-session", "issue-42", Some("/worktree/path"))
            .unwrap();
    }

    #[test]
    fn test_select_window() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["select-window", "-t", "my-session:issue-42"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .select_window("my-session", "issue-42")
            .unwrap();
    }

    #[test]
    fn test_kill_session() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["kill-session", "-t", "my-session"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator.kill_session("my-session").unwrap();
    }

    #[test]
    fn test_kill_window() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["kill-window", "-t", "my-session:issue-42"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator.kill_window("my-session", "issue-42").unwrap();
    }

    #[test]
    fn test_select_layout() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args == to_strings(&["select-layout", "-t", "my-session:main", "main-vertical"])
            })
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .select_layout("my-session:main", "main-vertical")
            .unwrap();
    }

    #[test]
    fn test_source_file() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["source-file", "/path/to/config"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator.source_file("/path/to/config").unwrap();
    }

    #[test]
    fn test_set_option() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["set-option", "-t", "my-session", "mouse", "on"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .set_option("my-session", "mouse", "on")
            .unwrap();
    }

    #[test]
    fn test_capture_pane() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "capture-pane",
                        "-t",
                        "my-session:main",
                        "-p",
                        "-e",
                        "-S",
                        "-50",
                    ])
            })
            .returning(|_| {
                Ok(mock_success(
                    "Claude Code >\nI have analyzed the error.\nShall I fix it? [y/n]",
                ))
            });

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let output = orchestrator.capture_pane("my-session:main", 50).unwrap();
        assert_eq!(
            output,
            "Claude Code >\nI have analyzed the error.\nShall I fix it? [y/n]"
        );
    }

    #[test]
    fn test_capture_pane_failure() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_failure("pane not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let result = orchestrator.capture_pane("invalid-session:main", 50);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_attach_command_outside_tmux() {
        let args = TmuxOrchestrator::<RealTmuxExecutor>::build_attach_command("my-session", false);
        assert_eq!(args, vec!["attach", "-t", "my-session"]);
    }

    #[test]
    fn test_build_attach_command_inside_tmux() {
        let args = TmuxOrchestrator::<RealTmuxExecutor>::build_attach_command("my-session", true);
        assert_eq!(args, vec!["switch-client", "-t", "my-session"]);
    }

    #[test]
    fn test_get_pane_title() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args == to_strings(&["list-panes", "-t", "my-session:main", "-F", "#{pane_title}"])
            })
            .returning(|_| Ok(mock_success("⠋ Claude Code - Working\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let title = orchestrator.get_pane_title("my-session:main").unwrap();
        assert_eq!(title, Some("⠋ Claude Code - Working".to_string()));
    }

    #[test]
    fn test_get_pane_title_empty() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute().returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let title = orchestrator.get_pane_title("my-session:main").unwrap();
        assert_eq!(title, None);
    }

    #[test]
    fn test_get_pane_title_failure() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_failure("session not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let title = orchestrator.get_pane_title("invalid-session:main").unwrap();
        assert_eq!(title, None);
    }

    #[test]
    fn test_get_pane_title_multiple_lines_returns_first() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_success("First title\nSecond title\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let title = orchestrator.get_pane_title("my-session:main").unwrap();
        assert_eq!(title, Some("First title".to_string()));
    }

    #[test]
    fn test_get_pane_title_whitespace_only() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_success("   \n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let title = orchestrator.get_pane_title("my-session:main").unwrap();
        // "   " is not empty, so it should be returned
        assert_eq!(title, Some("   ".to_string()));
    }

    #[test]
    fn test_capture_pane_with_different_line_counts() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "capture-pane",
                        "-t",
                        "my-session:main",
                        "-p",
                        "-e",
                        "-S",
                        "-100",
                    ])
            })
            .returning(|_| Ok(mock_success("Line 1\nLine 2\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let output = orchestrator.capture_pane("my-session:main", 100).unwrap();
        assert_eq!(output, "Line 1\nLine 2\n");
    }

    #[test]
    fn test_create_project_window_new() {
        let mut mock = MockTmuxExecutor::new();

        // has_session returns false
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-session"]))
            .returning(|_| Ok(mock_failure("not found")));

        // new_session creates session
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["new-session", "-d", "-s", "my-session"]))
            .returning(|_| Ok(mock_success("")));

        // list_windows for has_window check
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "list-windows",
                        "-t",
                        "my-session",
                        "-F",
                        "#{window_index}:#{window_name}:#{window_active}",
                    ])
            })
            .returning(|_| Ok(mock_success("0:main:1\n")));

        // new_window
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "new-window",
                        "-t",
                        "my-session",
                        "-n",
                        "task-1",
                        "-c",
                        "/path/to/worktree",
                    ])
            })
            .returning(|_| Ok(mock_success("")));

        // send_keys for initial command (input text)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["send-keys", "-t", "my-session:task-1", "claude"]))
            .returning(|_| Ok(mock_success("")));

        // send_keys for Enter (sent separately as C-m)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["send-keys", "-t", "my-session:task-1", "C-m"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let created = orchestrator
            .create_project_window("my-session", "task-1", "/path/to/worktree", Some("claude"))
            .unwrap();
        assert!(created); // Session was created
    }

    #[test]
    fn test_create_project_window_exists() {
        let mut mock = MockTmuxExecutor::new();

        // has_session returns true
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-session"]))
            .returning(|_| Ok(mock_success("")));

        // list_windows shows window already exists
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "list-windows",
                        "-t",
                        "my-session",
                        "-F",
                        "#{window_index}:#{window_name}:#{window_active}",
                    ])
            })
            .returning(|_| Ok(mock_success("0:main:1\n1:task-1:0\n")));

        // select_window
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["select-window", "-t", "my-session:task-1"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let created = orchestrator
            .create_project_window("my-session", "task-1", "/path/to/worktree", Some("claude"))
            .unwrap();
        assert!(!created); // Session already existed, window existed
    }

    #[test]
    fn test_new_session_failure() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_failure("duplicate session")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let result = orchestrator.new_session("existing-session", None, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_keys_without_enter() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["send-keys", "-t", "my-session:main", "q"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .send_keys("my-session:main", "q", false)
            .unwrap();
    }

    #[test]
    fn test_new_window_without_directory() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args == to_strings(&["new-window", "-t", "my-session", "-n", "window-name"])
            })
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .new_window("my-session", "window-name", None)
            .unwrap();
    }

    #[test]
    fn test_list_panes() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "list-panes",
                        "-t",
                        "my-session:0",
                        "-F",
                        "#{pane_index}:#{pane_active}:#{pane_current_path}",
                    ])
            })
            .returning(|_| Ok(mock_success("0:1:/home/user\n1:0:/home/user/project\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let panes = orchestrator.list_panes("my-session:0").unwrap();

        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].index, 0);
        assert!(panes[0].active);
        assert_eq!(panes[0].current_path, "/home/user");
        assert_eq!(panes[1].index, 1);
        assert!(!panes[1].active);
        assert_eq!(panes[1].current_path, "/home/user/project");
    }

    #[test]
    fn test_list_panes_handles_path_with_colon() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_success("0:1:/path:with:colons\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let panes = orchestrator.list_panes("my-session:0").unwrap();

        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].current_path, "/path:with:colons");
    }

    #[test]
    fn test_kill_pane() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["kill-pane", "-t", "my-session:0.1"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator.kill_pane("my-session:0.1").unwrap();
    }

    #[test]
    fn test_kill_pane_failure() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_failure("pane not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let result = orchestrator.kill_pane("invalid-pane");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_pane_to_dashboard_no_existing_dashboard() {
        let mut mock = MockTmuxExecutor::new();

        // has_session returns false - dashboard doesn't exist
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .returning(|_| Ok(mock_failure("session not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let added = orchestrator
            .add_pane_to_dashboard("my-project", "my-project__feature")
            .unwrap();
        assert!(!added);
    }

    #[test]
    fn test_add_pane_to_dashboard_success() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session returns true
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // split_window_horizontal with command
        mock.expect_execute()
            .withf(|args| {
                args[0] == "split-window"
                    && args[1] == "-v"
                    && args[2] == "-t"
                    && args[3] == "my-project__dashboard"
                    && args[4].contains("tmux attach -t my-project__feature")
            })
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // select_layout
        mock.expect_execute()
            .withf(|args| {
                *args == to_strings(&["select-layout", "-t", "my-project__dashboard", "tiled"])
            })
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let added = orchestrator
            .add_pane_to_dashboard("my-project", "my-project__feature")
            .unwrap();
        assert!(added);
    }

    #[test]
    fn test_sync_dashboard_no_existing_dashboard() {
        let mut mock = MockTmuxExecutor::new();

        // has_session returns false - dashboard doesn't exist
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .returning(|_| Ok(mock_failure("session not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let synced = orchestrator
            .sync_dashboard(
                "my-project",
                &[("my-project__main".to_string(), "/path/main".to_string())],
            )
            .unwrap();
        assert!(!synced);
    }

    #[test]
    fn test_dashboard_session_name() {
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::dashboard_session_name("my-project"),
            "my-project__dashboard"
        );
        assert_eq!(
            TmuxOrchestrator::<RealTmuxExecutor>::dashboard_session_name("project-with-dashes"),
            "project-with-dashes__dashboard"
        );
    }

    #[test]
    fn test_list_panes_failure() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_failure("session not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let result = orchestrator.list_panes("invalid-session:0");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_panes_empty() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute().returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let panes = orchestrator.list_panes("my-session:0").unwrap();
        assert!(panes.is_empty());
    }

    #[test]
    fn test_add_pane_to_dashboard_split_fails() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session returns true
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // split_window_horizontal with command fails
        mock.expect_execute()
            .withf(|args| {
                args[0] == "split-window"
                    && args[1] == "-v"
                    && args[2] == "-t"
                    && args[3] == "my-project__dashboard"
                    && args[4].contains("tmux attach -t my-project__feature")
            })
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("no space for new pane")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let result = orchestrator.add_pane_to_dashboard("my-project", "my-project__feature");
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_dashboard_success() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session returns true (dashboard exists)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // kill_session
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["kill-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // has_session for create_dashboard_session (should return false after kill)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("not found")));

        // new_session
        mock.expect_execute()
            .withf(|args| {
                args[0] == "new-session" && args.contains(&"my-project__dashboard".to_string())
            })
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // set_option (aggressive-resize)
        mock.expect_execute()
            .withf(|args| args[0] == "set-option")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (input)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3].contains("main"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (enter)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3] == "C-m")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // select_layout
        mock.expect_execute()
            .withf(|args| args[0] == "select-layout")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let synced = orchestrator
            .sync_dashboard(
                "my-project",
                &[("my-project__main".to_string(), "/path/main".to_string())],
            )
            .unwrap();
        assert!(synced);
    }

    #[test]
    fn test_sync_dashboard_kill_fails() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session returns true
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // kill_session fails
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["kill-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("session not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let result = orchestrator.sync_dashboard(
            "my-project",
            &[("my-project__main".to_string(), "/path/main".to_string())],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_create_dashboard_session_already_exists() {
        let mut mock = MockTmuxExecutor::new();

        // has_session returns true (dashboard already exists)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let created = orchestrator
            .create_dashboard_session(
                "my-project",
                &[("my-project__main".to_string(), "/path/main".to_string())],
            )
            .unwrap();
        assert!(!created);
    }

    #[test]
    fn test_create_dashboard_session_multiple_worktrees() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session returns false (dashboard doesn't exist)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("not found")));

        // new_session
        mock.expect_execute()
            .withf(|args| args[0] == "new-session")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // set_option
        mock.expect_execute()
            .withf(|args| args[0] == "set-option")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (input)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3].contains("main"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (enter)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3] == "C-m")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // split_window for second pane with command
        mock.expect_execute()
            .withf(|args| args[0] == "split-window" && args[4].contains("feature"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // select_layout
        mock.expect_execute()
            .withf(|args| args[0] == "select-layout" && args[3] == "tiled")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let created = orchestrator
            .create_dashboard_session(
                "my-project",
                &[
                    ("my-project__main".to_string(), "/path/main".to_string()),
                    (
                        "my-project__feature".to_string(),
                        "/path/feature".to_string(),
                    ),
                ],
            )
            .unwrap();
        assert!(created);
    }

    #[test]
    fn test_list_panes_invalid_format_skipped() {
        let mut mock = MockTmuxExecutor::new();
        // Return some invalid lines mixed with valid ones
        mock.expect_execute()
            .returning(|_| Ok(mock_success("invalid\n0:1:/home\nincomplete:data\n1:0:/work\n")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let panes = orchestrator.list_panes("my-session:0").unwrap();

        // Should only include valid entries
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].index, 0);
        assert_eq!(panes[1].index, 1);
    }

    // ========================================================================
    // TDD: respawn_pane tests
    // ========================================================================

    /// Test: respawn_pane runs a command in the specified pane.
    #[test]
    fn test_respawn_pane() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .withf(|args| {
                *args
                    == to_strings(&[
                        "respawn-pane",
                        "-k",
                        "-t",
                        "my-session:0.1",
                        "echo hello",
                    ])
            })
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .respawn_pane("my-session:0.1", "echo hello")
            .unwrap();
    }

    /// Test: respawn_pane failure returns error.
    #[test]
    fn test_respawn_pane_failure() {
        let mut mock = MockTmuxExecutor::new();
        mock.expect_execute()
            .returning(|_| Ok(mock_failure("pane not found")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        let result = orchestrator.respawn_pane("invalid-pane", "echo hello");
        assert!(result.is_err());
    }

    // ========================================================================
    // TDD: ensure_dashboard_session tests
    // ========================================================================

    /// Test: ensure_dashboard_session creates new dashboard when it doesn't exist.
    #[test]
    fn test_ensure_dashboard_session_creates_new() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session from ensure_dashboard_session returns false (no kill needed)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("not found")));

        // has_session from create_dashboard_session also returns false
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("not found")));

        // new_session
        mock.expect_execute()
            .withf(|args| args[0] == "new-session")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // set_option
        mock.expect_execute()
            .withf(|args| args[0] == "set-option")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (input)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3].contains("main"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (enter)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3] == "C-m")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // select_layout
        mock.expect_execute()
            .withf(|args| args[0] == "select-layout")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .ensure_dashboard_session(
                "my-project",
                &[("my-project__main".to_string(), "/path/main".to_string())],
            )
            .unwrap();
    }

    /// Test: ensure_dashboard_session recreates existing dashboard (kill + create).
    #[test]
    fn test_ensure_dashboard_session_refreshes_existing() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session returns true (dashboard already exists)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // kill_session to remove old dashboard
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["kill-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // has_session from create_dashboard_session returns false (just killed)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("not found")));

        // new_session
        mock.expect_execute()
            .withf(|args| args[0] == "new-session")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // set_option
        mock.expect_execute()
            .withf(|args| args[0] == "set-option")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (input)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3].contains("main"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (enter)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3] == "C-m")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // split_window for second pane with command
        mock.expect_execute()
            .withf(|args| args[0] == "split-window" && args[4].contains("feature"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // select_layout
        mock.expect_execute()
            .withf(|args| args[0] == "select-layout")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .ensure_dashboard_session(
                "my-project",
                &[
                    ("my-project__main".to_string(), "/path/main".to_string()),
                    (
                        "my-project__feature".to_string(),
                        "/path/feature".to_string(),
                    ),
                ],
            )
            .unwrap();
    }

    /// Test: ensure_dashboard_session handles worktrees increasing (recreates dashboard).
    #[test]
    fn test_ensure_dashboard_session_adds_missing_panes() {
        use mockall::Sequence;

        let mut mock = MockTmuxExecutor::new();
        let mut seq = Sequence::new();

        // has_session returns true (dashboard exists with fewer panes)
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // kill_session to remove old dashboard
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["kill-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // has_session from create_dashboard_session returns false
        mock.expect_execute()
            .withf(|args| *args == to_strings(&["has-session", "-t", "my-project__dashboard"]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_failure("not found")));

        // new_session
        mock.expect_execute()
            .withf(|args| args[0] == "new-session")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // set_option
        mock.expect_execute()
            .withf(|args| args[0] == "set-option")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (input)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3].contains("main"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // send_keys for first pane (enter)
        mock.expect_execute()
            .withf(|args| args[0] == "send-keys" && args[3] == "C-m")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // split_window for second pane with command
        mock.expect_execute()
            .withf(|args| args[0] == "split-window" && args[4].contains("feature"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        // select_layout to re-tile
        mock.expect_execute()
            .withf(|args| args[0] == "select-layout")
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(mock_success("")));

        let orchestrator = TmuxOrchestrator::with_executor(mock);
        orchestrator
            .ensure_dashboard_session(
                "my-project",
                &[
                    ("my-project__main".to_string(), "/path/main".to_string()),
                    (
                        "my-project__feature".to_string(),
                        "/path/feature".to_string(),
                    ),
                ],
            )
            .unwrap();
    }
}
