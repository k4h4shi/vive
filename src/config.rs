//! Configuration module for vive.
//!
//! This module provides functionality to:
//! - Load configuration from `~/.vive/config.toml`
//! - Define default values for configuration options
//! - Support configuration for projects_root, ignored_dirs, and tmux_prefix
//! - Load and save favorites to `~/.vive/favorites.toml`

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Configuration directory name.
const CONFIG_DIR_NAME: &str = ".vive";
/// Configuration file name.
const CONFIG_FILE_NAME: &str = "config.toml";
/// Favorites file name.
const FAVORITES_FILE_NAME: &str = "favorites.toml";

/// Default directories to ignore when scanning for projects.
const DEFAULT_IGNORED_DIRS: &[&str] = &[".git", "node_modules", ".worktrees", "target", "dist"];

/// Launch strategy for terminal sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchStrategy {
    /// Inline mode: replace current process with tmux attach (default).
    #[default]
    Inline,
    /// Spawn mode: launch external terminal without suspending TUI.
    Spawn,
}

/// Terminal configuration for session launching.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Launch strategy: "inline" (default) or "spawn".
    pub strategy: LaunchStrategy,

    /// Command to run for spawn strategy (e.g., "ghostty").
    pub command: Option<String>,

    /// Arguments for the spawn command.
    /// Use `{session_id}` as placeholder for the tmux target (session:window).
    #[serde(default)]
    pub args: Vec<String>,
}

/// Auto-kickstart configuration for task creation.
///
/// Supported placeholders for both `manual_command` and `issue_command`:
/// - `{issue_number}` - The GitHub issue number (issue_command only)
/// - `{session_id}` - The tmux target (e.g., `vive:issue-123`)
/// - `{branch_name}` - The git branch name (e.g., `issue-123`)
/// - `{project_name}` - The project name (e.g., `vive`)
/// - `{worktree_path}` - Absolute path to the worktree
///
/// Example configuration:
/// ```toml
/// [auto_kickstart]
/// # Start claude directly with the fix command
/// issue_command = "claude \"/fix {issue_number}\""
///
/// # Or use a custom wrapper script
/// # issue_command = "~/.local/bin/start-agent.sh {session_id} {issue_number}"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoKickstartConfig {
    /// Command to send for manual task creation.
    ///
    /// Supports placeholders: `{session_id}`, `{branch_name}`, `{project_name}`, `{worktree_path}`
    /// Default: empty (disabled)
    pub manual_command: String,

    /// Command to send for issue-based task creation.
    ///
    /// Supports placeholders: `{issue_number}`, `{session_id}`, `{branch_name}`,
    /// `{project_name}`, `{worktree_path}`
    /// Default: empty (disabled)
    pub issue_command: String,
}

impl AutoKickstartConfig {
    /// Build the issue command with issue number substituted.
    ///
    /// DEPRECATED: Use `build_issue_command_full` for full placeholder support.
    pub fn build_issue_command(&self, issue_number: u32) -> String {
        self.issue_command
            .replace("{issue_number}", &issue_number.to_string())
    }

    /// Build the issue command with all placeholders substituted.
    ///
    /// Supported placeholders:
    /// - `{issue_number}` - The issue number
    /// - `{session_id}` - The tmux target (e.g., "project:branch")
    /// - `{branch_name}` - The git branch name
    /// - `{project_name}` - The project name
    /// - `{worktree_path}` - The absolute path to the worktree
    pub fn build_issue_command_full(
        &self,
        issue_number: u32,
        session_id: &str,
        branch_name: &str,
        project_name: &str,
        worktree_path: &str,
    ) -> String {
        self.issue_command
            .replace("{issue_number}", &issue_number.to_string())
            .replace("{session_id}", session_id)
            .replace("{branch_name}", branch_name)
            .replace("{project_name}", project_name)
            .replace("{worktree_path}", worktree_path)
    }

    /// Build the manual command with all placeholders substituted.
    ///
    /// Supported placeholders:
    /// - `{session_id}` - The tmux target (e.g., "project:branch")
    /// - `{branch_name}` - The git branch name
    /// - `{project_name}` - The project name
    /// - `{worktree_path}` - The absolute path to the worktree
    pub fn build_manual_command_full(
        &self,
        session_id: &str,
        branch_name: &str,
        project_name: &str,
        worktree_path: &str,
    ) -> String {
        self.manual_command
            .replace("{session_id}", session_id)
            .replace("{branch_name}", branch_name)
            .replace("{project_name}", project_name)
            .replace("{worktree_path}", worktree_path)
    }
}

impl TerminalConfig {
    /// Check if the strategy is spawn mode.
    pub fn is_spawn(&self) -> bool {
        matches!(self.strategy, LaunchStrategy::Spawn)
    }

    /// Build the spawn command with session_id substituted.
    ///
    /// Returns `None` if no command is configured.
    pub fn build_spawn_command(&self, session_id: &str) -> Option<(String, Vec<String>)> {
        let command = self.command.as_ref()?;
        let args: Vec<String> = self
            .args
            .iter()
            .map(|arg| arg.replace("{session_id}", session_id))
            .collect();
        Some((command.clone(), args))
    }
}

/// Application configuration loaded from `~/.vive/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Directory to scan for projects.
    /// If not set, defaults to `~/src`.
    pub projects_root: Option<PathBuf>,

    /// Directories to exclude from scanning.
    /// Defaults to common directories like `.git`, `node_modules`, etc.
    pub ignored_dirs: Vec<String>,

    /// Optional tmux prefix key override (e.g., "C-a" for Ctrl+a).
    /// If not set, uses the default tmux prefix.
    pub tmux_prefix: Option<String>,

    /// Terminal configuration for session launching.
    /// DEPRECATED: Use `keybindings` instead for more flexible configuration.
    #[serde(default)]
    pub terminal: TerminalConfig,

    /// Custom keybindings for session actions.
    /// Keys are key names (e.g., "enter", "o", "n"), values are shell commands.
    /// Use `{session_id}` as placeholder for the tmux target (session:window).
    /// Use `{path}` as placeholder for the worktree path.
    ///
    /// Example:
    /// ```toml
    /// [keybindings]
    /// enter = "tmux switch-client -t {session_id}"
    /// o = "ghostty -e tmux attach -t {session_id}"
    /// n = "code {path}"
    /// ```
    #[serde(default)]
    pub keybindings: HashMap<String, String>,

    /// Auto-kickstart configuration for task creation.
    /// Configures commands sent to Claude after creating a new task.
    ///
    /// Example:
    /// ```toml
    /// [auto_kickstart]
    /// manual_command = "claude --print-architecture"
    /// issue_command = "/fix {issue_number}"
    /// ```
    #[serde(default)]
    pub auto_kickstart: AutoKickstartConfig,

    /// Base branch for worktree creation.
    /// If set, new worktrees will be created from this branch instead of the current HEAD.
    ///
    /// Example:
    /// ```toml
    /// base_branch = "develop"
    /// ```
    pub base_branch: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            projects_root: None,
            ignored_dirs: DEFAULT_IGNORED_DIRS.iter().map(|s| s.to_string()).collect(),
            tmux_prefix: None,
            terminal: TerminalConfig::default(),
            keybindings: HashMap::new(),
            auto_kickstart: AutoKickstartConfig::default(),
            base_branch: None,
        }
    }
}

impl Config {
    /// Returns the path to the configuration directory (`~/.vive`).
    pub fn config_dir() -> Option<PathBuf> {
        dirs_home().map(|home| home.join(CONFIG_DIR_NAME))
    }

    /// Returns the path to the configuration file (`~/.vive/config.toml`).
    pub fn config_file_path() -> Option<PathBuf> {
        Self::config_dir().map(|dir| dir.join(CONFIG_FILE_NAME))
    }

    /// Loads configuration from `~/.vive/config.toml`.
    ///
    /// If the file doesn't exist, returns default configuration.
    /// If the file exists but is invalid, returns an error.
    pub fn load() -> Result<Self> {
        let config_path = match Self::config_file_path() {
            Some(path) => path,
            None => return Ok(Self::default()),
        };

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

        Ok(config)
    }

    /// Initializes the configuration directory and creates a default config file.
    ///
    /// Returns the path to the created config file.
    #[allow(dead_code)]
    pub fn init() -> Result<PathBuf> {
        let config_dir = Self::config_dir().context("Could not determine home directory")?;
        let config_path = config_dir.join(CONFIG_FILE_NAME);

        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).with_context(|| {
                format!(
                    "Failed to create config directory: {}",
                    config_dir.display()
                )
            })?;
        }

        // Create default config file if it doesn't exist
        if !config_path.exists() {
            let default_content = r#"# vive configuration file
# See https://github.com/k4h4shi/vive for documentation

# Directory to scan for projects (defaults to ~/src if not set)
# projects_root = "/path/to/your/projects"

# Directories to exclude from scanning
ignored_dirs = [".git", "node_modules", ".worktrees", "target", "dist"]

# Optional tmux prefix key override (e.g., "C-a" for Ctrl+a)
# tmux_prefix = "C-a"

# Base branch for worktree creation (defaults to current HEAD if not set)
# If set, new worktrees will be created from this branch
# base_branch = "develop"

# Custom keybindings for opening sessions
# Use {session_id} as placeholder for the tmux target (session:window)
# Use {path} as placeholder for the worktree path
[keybindings]
# Default behavior (inline tmux switch) - uncomment to customize:
# enter = "tmux switch-client -t {session_id}"

# Open in new Ghostty window:
# o = "ghostty -e tmux attach -t {session_id}"

# Open in new terminal tab (example for iTerm2):
# o = "open -a iTerm && osascript -e 'tell application \"iTerm2\" to tell current window to create tab with default profile command \"tmux attach -t {session_id}\"'"

# Open in VS Code:
# n = "code {path}"

# Open in new tmux window:
# n = "tmux new-window -t {session_id}"

# DEPRECATED: [terminal] section - use [keybindings] instead for more flexibility
# The [terminal] section still works for backwards compatibility but will be
# removed in a future version.
# [terminal]
# strategy = "spawn"
# command = "ghostty"
# args = ["-e", "tmux attach -t {session_id}"]
"#;
            fs::write(&config_path, default_content).with_context(|| {
                format!("Failed to write config file: {}", config_path.display())
            })?;
        }

        Ok(config_path)
    }

    /// Returns the effective projects root directory.
    ///
    /// Uses `projects_root` from config if set, otherwise falls back to
    /// `VIVE_PROJECTS_ROOT` environment variable, then `~/src`.
    pub fn effective_projects_root(&self) -> PathBuf {
        if let Some(ref root) = self.projects_root {
            return root.clone();
        }

        if let Ok(env_root) = std::env::var("VIVE_PROJECTS_ROOT") {
            return PathBuf::from(env_root);
        }

        dirs_home()
            .map(|h| h.join("src"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Checks if a directory name should be ignored during scanning.
    #[allow(dead_code)]
    pub fn should_ignore(&self, dir_name: &str) -> bool {
        self.ignored_dirs.iter().any(|ignored| ignored == dir_name)
    }

    /// Check if custom keybindings are configured.
    pub fn has_keybindings(&self) -> bool {
        !self.keybindings.is_empty()
    }

    /// Build a command from keybinding with session_id substitution.
    ///
    /// Returns `None` if no keybinding is configured for the given key.
    pub fn build_keybinding_command(&self, key: &str, session_id: &str) -> Option<String> {
        self.keybindings
            .get(key)
            .map(|cmd| substitute_session_id(cmd, session_id))
    }

    /// Build a command from keybinding with all placeholder substitutions.
    ///
    /// Supports `{session_id}` and `{path}` placeholders.
    /// Returns `None` if no keybinding is configured for the given key.
    pub fn build_keybinding_command_with_placeholders(
        &self,
        key: &str,
        session_id: &str,
        path: Option<&str>,
    ) -> Option<String> {
        self.keybindings.get(key).map(|cmd| {
            let mut result = substitute_session_id(cmd, session_id);
            if let Some(p) = path {
                result = result.replace("{path}", p);
            }
            result
        })
    }
}

/// Get the user's home directory.
fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Substitute `{session_id}` placeholder in a command string.
pub fn substitute_session_id(command: &str, session_id: &str) -> String {
    command.replace("{session_id}", session_id)
}

/// Favorites data stored in `~/.vive/favorites.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Favorites {
    /// Set of favorite project names.
    #[serde(default)]
    pub projects: HashSet<String>,
}

impl Favorites {
    /// Returns the path to the favorites file (`~/.vive/favorites.toml`).
    pub fn favorites_file_path() -> Option<PathBuf> {
        Config::config_dir().map(|dir| dir.join(FAVORITES_FILE_NAME))
    }

    /// Loads favorites from `~/.vive/favorites.toml`.
    ///
    /// If the file doesn't exist, returns empty favorites.
    /// If the file exists but is invalid, returns an error.
    pub fn load() -> Result<Self> {
        let favorites_path = match Self::favorites_file_path() {
            Some(path) => path,
            None => return Ok(Self::default()),
        };

        if !favorites_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&favorites_path).with_context(|| {
            format!(
                "Failed to read favorites file: {}",
                favorites_path.display()
            )
        })?;

        let favorites: Favorites = toml::from_str(&content).with_context(|| {
            format!(
                "Failed to parse favorites file: {}",
                favorites_path.display()
            )
        })?;

        Ok(favorites)
    }

    /// Saves favorites to `~/.vive/favorites.toml`.
    pub fn save(&self) -> Result<()> {
        let favorites_path =
            Self::favorites_file_path().context("Could not determine favorites file path")?;

        // Ensure config directory exists
        if let Some(parent) = favorites_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize favorites")?;

        fs::write(&favorites_path, content).with_context(|| {
            format!(
                "Failed to write favorites file: {}",
                favorites_path.display()
            )
        })?;

        Ok(())
    }

    /// Check if a project is a favorite.
    pub fn is_favorite(&self, project_name: &str) -> bool {
        self.projects.contains(project_name)
    }

    /// Add a project to favorites.
    pub fn add(&mut self, project_name: &str) {
        self.projects.insert(project_name.to_string());
    }

    /// Remove a project from favorites.
    pub fn remove(&mut self, project_name: &str) {
        self.projects.remove(project_name);
    }

    /// Toggle favorite status.
    pub fn toggle(&mut self, project_name: &str) {
        if self.is_favorite(project_name) {
            self.remove(project_name);
        } else {
            self.add(project_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.projects_root.is_none());
        assert!(config.ignored_dirs.contains(&".git".to_string()));
        assert!(config.ignored_dirs.contains(&"node_modules".to_string()));
        assert!(config.tmux_prefix.is_none());
        // Terminal config should default to inline strategy
        assert_eq!(config.terminal.strategy, LaunchStrategy::Inline);
    }

    // ========================================================================
    // TDD: Terminal configuration tests
    // ========================================================================

    #[test]
    fn test_terminal_config_default() {
        let terminal_config = TerminalConfig::default();
        assert_eq!(terminal_config.strategy, LaunchStrategy::Inline);
        assert!(terminal_config.command.is_none());
        assert!(terminal_config.args.is_empty());
    }

    #[test]
    fn test_launch_strategy_default() {
        let strategy = LaunchStrategy::default();
        assert_eq!(strategy, LaunchStrategy::Inline);
    }

    #[test]
    fn test_parse_terminal_config_spawn_strategy() {
        let toml_content = r#"
[terminal]
strategy = "spawn"
command = "ghostty"
args = ["+e", "tmux attach -t {session_id}"]
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.terminal.strategy, LaunchStrategy::Spawn);
        assert_eq!(config.terminal.command, Some("ghostty".to_string()));
        assert_eq!(
            config.terminal.args,
            vec!["+e", "tmux attach -t {session_id}"]
        );
    }

    #[test]
    fn test_parse_terminal_config_inline_strategy() {
        let toml_content = r#"
[terminal]
strategy = "inline"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.terminal.strategy, LaunchStrategy::Inline);
        assert!(config.terminal.command.is_none());
    }

    #[test]
    fn test_terminal_config_build_spawn_command() {
        let terminal_config = TerminalConfig {
            strategy: LaunchStrategy::Spawn,
            command: Some("ghostty".to_string()),
            args: vec!["+e".to_string(), "tmux attach -t {session_id}".to_string()],
        };
        let (cmd, args) = terminal_config.build_spawn_command("my-session").unwrap();
        assert_eq!(cmd, "ghostty");
        assert_eq!(args, vec!["+e", "tmux attach -t my-session"]);
    }

    #[test]
    fn test_terminal_config_build_spawn_command_no_command() {
        let terminal_config = TerminalConfig {
            strategy: LaunchStrategy::Spawn,
            command: None,
            args: vec![],
        };
        let result = terminal_config.build_spawn_command("my-session");
        assert!(result.is_none());
    }

    #[test]
    fn test_terminal_config_is_spawn() {
        let inline_config = TerminalConfig {
            strategy: LaunchStrategy::Inline,
            ..Default::default()
        };
        assert!(!inline_config.is_spawn());

        let spawn_config = TerminalConfig {
            strategy: LaunchStrategy::Spawn,
            ..Default::default()
        };
        assert!(spawn_config.is_spawn());
    }

    #[test]
    fn test_should_ignore() {
        let config = Config::default();
        assert!(config.should_ignore(".git"));
        assert!(config.should_ignore("node_modules"));
        assert!(!config.should_ignore("src"));
    }

    #[test]
    fn test_parse_config() {
        let toml_content = r#"
projects_root = "/home/user/projects"
ignored_dirs = [".git", "vendor"]
tmux_prefix = "C-a"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(
            config.projects_root,
            Some(PathBuf::from("/home/user/projects"))
        );
        assert_eq!(config.ignored_dirs, vec![".git", "vendor"]);
        assert_eq!(config.tmux_prefix, Some("C-a".to_string()));
    }

    #[test]
    fn test_parse_partial_config() {
        let toml_content = r#"
projects_root = "/home/user/projects"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(
            config.projects_root,
            Some(PathBuf::from("/home/user/projects"))
        );
        // Should use defaults for unspecified fields
        assert!(!config.ignored_dirs.is_empty());
        assert!(config.tmux_prefix.is_none());
    }

    #[test]
    fn test_effective_projects_root_from_config() {
        let config = Config {
            projects_root: Some(PathBuf::from("/custom/path")),
            ..Default::default()
        };
        assert_eq!(
            config.effective_projects_root(),
            PathBuf::from("/custom/path")
        );
    }

    #[test]
    fn test_config_file_path() {
        let path = Config::config_file_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains(".vive"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn test_favorites_default() {
        let favorites = Favorites::default();
        assert!(favorites.projects.is_empty());
    }

    #[test]
    fn test_favorites_add_remove() {
        let mut favorites = Favorites::default();

        favorites.add("user/project-a");
        assert!(favorites.is_favorite("user/project-a"));
        assert!(!favorites.is_favorite("user/project-b"));

        favorites.add("user/project-b");
        assert!(favorites.is_favorite("user/project-a"));
        assert!(favorites.is_favorite("user/project-b"));

        favorites.remove("user/project-a");
        assert!(!favorites.is_favorite("user/project-a"));
        assert!(favorites.is_favorite("user/project-b"));
    }

    #[test]
    fn test_favorites_toggle() {
        let mut favorites = Favorites::default();

        favorites.toggle("user/project-a");
        assert!(favorites.is_favorite("user/project-a"));

        favorites.toggle("user/project-a");
        assert!(!favorites.is_favorite("user/project-a"));
    }

    #[test]
    fn test_favorites_parse() {
        let toml_content = r#"
projects = ["user/project-a", "user/project-b"]
"#;
        let favorites: Favorites = toml::from_str(toml_content).unwrap();
        assert!(favorites.is_favorite("user/project-a"));
        assert!(favorites.is_favorite("user/project-b"));
        assert!(!favorites.is_favorite("project-c"));
    }

    #[test]
    fn test_favorites_serialize() {
        let mut favorites = Favorites::default();
        favorites.add("user/project-a");
        favorites.add("user/project-b");

        let serialized = toml::to_string(&favorites).unwrap();
        assert!(serialized.contains("user/project-a"));
        assert!(serialized.contains("user/project-b"));
    }

    #[test]
    fn test_favorites_file_path() {
        let path = Favorites::favorites_file_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains(".vive"));
        assert!(path.to_string_lossy().ends_with("favorites.toml"));
    }

    // ========================================================================
    // TDD: Keybindings configuration tests (Issue #66)
    // ========================================================================

    #[test]
    fn test_keybindings_default_is_empty() {
        let config = Config::default();
        assert!(config.keybindings.is_empty());
    }

    #[test]
    fn test_parse_keybindings_from_toml() {
        let toml_content = r#"
[keybindings]
enter = "tmux switch-client -t {session_id}"
o = "~/.local/bin/ghostty-tab {session_id}"
n = "tmux new-window -t {session_id}"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.keybindings.len(), 3);
        assert_eq!(
            config.keybindings.get("enter"),
            Some(&"tmux switch-client -t {session_id}".to_string())
        );
        assert_eq!(
            config.keybindings.get("o"),
            Some(&"~/.local/bin/ghostty-tab {session_id}".to_string())
        );
        assert_eq!(
            config.keybindings.get("n"),
            Some(&"tmux new-window -t {session_id}".to_string())
        );
    }

    #[test]
    fn test_keybinding_substitute_session_id() {
        let command = "tmux switch-client -t {session_id}";
        let result = substitute_session_id(command, "myproject:feature");
        assert_eq!(result, "tmux switch-client -t myproject:feature");
    }

    #[test]
    fn test_keybinding_substitute_multiple_placeholders() {
        let command = "echo {session_id} && tmux attach -t {session_id}";
        let result = substitute_session_id(command, "my-session");
        assert_eq!(result, "echo my-session && tmux attach -t my-session");
    }

    #[test]
    fn test_mixed_config_keybindings_with_terminal() {
        let toml_content = r#"
[terminal]
strategy = "spawn"
command = "ghostty"

[keybindings]
enter = "custom-command {session_id}"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        // keybindings should be present
        assert!(config.keybindings.contains_key("enter"));
        // terminal config still parsed (for migration period)
        assert_eq!(config.terminal.command, Some("ghostty".to_string()));
    }

    #[test]
    fn test_build_keybinding_command() {
        use std::collections::HashMap;
        let mut keybindings = HashMap::new();
        keybindings.insert(
            "enter".to_string(),
            "tmux switch-client -t {session_id}".to_string(),
        );

        let config = Config {
            keybindings,
            ..Default::default()
        };

        let cmd = config.build_keybinding_command("enter", "my-session");
        assert_eq!(cmd, Some("tmux switch-client -t my-session".to_string()));
    }

    #[test]
    fn test_unknown_keybinding_returns_none() {
        let config = Config::default();
        let cmd = config.build_keybinding_command("x", "my-session");
        assert!(cmd.is_none());
    }

    #[test]
    fn test_has_keybindings() {
        let config = Config::default();
        assert!(!config.has_keybindings());

        let toml_content = r#"
[keybindings]
enter = "tmux switch-client -t {session_id}"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.has_keybindings());
    }

    #[test]
    fn test_keybinding_with_path_placeholder() {
        use std::collections::HashMap;
        let mut keybindings = HashMap::new();
        keybindings.insert(
            "o".to_string(),
            "code {path} && tmux attach -t {session_id}".to_string(),
        );

        let config = Config {
            keybindings,
            ..Default::default()
        };

        let cmd = config.build_keybinding_command_with_placeholders(
            "o",
            "my-session",
            Some("/home/user/project"),
        );
        assert_eq!(
            cmd,
            Some("code /home/user/project && tmux attach -t my-session".to_string())
        );
    }

    #[test]
    fn test_keybinding_without_path_placeholder() {
        use std::collections::HashMap;
        let mut keybindings = HashMap::new();
        keybindings.insert(
            "enter".to_string(),
            "tmux switch-client -t {session_id}".to_string(),
        );

        let config = Config {
            keybindings,
            ..Default::default()
        };

        let cmd = config.build_keybinding_command_with_placeholders("enter", "my-session", None);
        assert_eq!(cmd, Some("tmux switch-client -t my-session".to_string()));
    }

    #[test]
    fn test_auto_kickstart_default() {
        let config = AutoKickstartConfig::default();
        assert_eq!(config.manual_command, "");
        assert_eq!(config.issue_command, "");
    }

    #[test]
    fn test_auto_kickstart_build_issue_command() {
        let config = AutoKickstartConfig {
            manual_command: String::new(),
            issue_command: "/fix {issue_number}".to_string(),
        };
        assert_eq!(config.build_issue_command(42), "/fix 42");
        assert_eq!(config.build_issue_command(123), "/fix 123");
    }

    #[test]
    fn test_auto_kickstart_custom_command() {
        let config = AutoKickstartConfig {
            manual_command: "custom start".to_string(),
            issue_command: "gh issue view {issue_number}".to_string(),
        };
        assert_eq!(config.manual_command, "custom start");
        assert_eq!(config.build_issue_command(99), "gh issue view 99");
    }

    #[test]
    fn test_config_includes_auto_kickstart() {
        let config = Config::default();
        assert_eq!(config.auto_kickstart.manual_command, "");
    }

    // ========================================================================
    // TDD: Issue #76 - Configurable one-liner kickstart command
    // ========================================================================

    #[test]
    fn test_build_issue_command_full_all_placeholders() {
        let config = AutoKickstartConfig {
            manual_command: String::new(),
            issue_command: "claude \"/fix {issue_number}\" --session {session_id}".to_string(),
        };
        let result = config.build_issue_command_full(
            42,
            "myproject:issue-42",
            "issue-42",
            "myproject",
            "/home/user/src/myproject/.worktrees/issue-42",
        );
        assert_eq!(result, "claude \"/fix 42\" --session myproject:issue-42");
    }

    #[test]
    fn test_build_issue_command_full_with_worktree_path() {
        let config = AutoKickstartConfig {
            manual_command: String::new(),
            issue_command: "cd {worktree_path} && claude \"/fix {issue_number}\"".to_string(),
        };
        let result = config.build_issue_command_full(
            99,
            "proj:issue-99",
            "issue-99",
            "proj",
            "/path/to/worktree",
        );
        assert_eq!(result, "cd /path/to/worktree && claude \"/fix 99\"");
    }

    #[test]
    fn test_build_issue_command_full_with_project_and_branch() {
        let config = AutoKickstartConfig {
            manual_command: String::new(),
            issue_command: "echo {project_name}:{branch_name} && claude".to_string(),
        };
        let result = config.build_issue_command_full(
            1,
            "vive:feature-branch",
            "feature-branch",
            "vive",
            "/tmp/worktree",
        );
        assert_eq!(result, "echo vive:feature-branch && claude");
    }

    #[test]
    fn test_build_manual_command_full_with_placeholders() {
        let config = AutoKickstartConfig {
            manual_command: "claude --project {project_name} --cwd {worktree_path}".to_string(),
            issue_command: String::new(),
        };
        let result =
            config.build_manual_command_full("proj:branch", "branch", "proj", "/path/to/worktree");
        assert_eq!(result, "claude --project proj --cwd /path/to/worktree");
    }

    #[test]
    fn test_build_manual_command_full_with_session_id() {
        let config = AutoKickstartConfig {
            manual_command: "tmux send-keys -t {session_id} 'claude' Enter".to_string(),
            issue_command: String::new(),
        };
        let result = config.build_manual_command_full(
            "my-project:my-branch",
            "my-branch",
            "my-project",
            "/home/user/project",
        );
        assert_eq!(
            result,
            "tmux send-keys -t my-project:my-branch 'claude' Enter"
        );
    }

    #[test]
    fn test_build_issue_command_full_simple_command() {
        // Test the typical use case from issue #76
        let config = AutoKickstartConfig {
            manual_command: String::new(),
            issue_command: "claude \"/fix {issue_number}\"".to_string(),
        };
        let result = config.build_issue_command_full(
            76,
            "vive:issue-76",
            "issue-76",
            "vive",
            "/Users/user/src/vive/.worktrees/issue-76",
        );
        assert_eq!(result, "claude \"/fix 76\"");
    }

    #[test]
    fn test_build_issue_command_full_no_placeholders() {
        // Empty placeholders should just return the template as-is
        let config = AutoKickstartConfig {
            manual_command: String::new(),
            issue_command: "claude --interactive".to_string(),
        };
        let result = config.build_issue_command_full(42, "session", "branch", "project", "/path");
        assert_eq!(result, "claude --interactive");
    }

    // ========================================================================
    // TDD: Issue #88 - Configurable base branch for worktree creation
    // ========================================================================

    #[test]
    fn test_base_branch_default_is_none() {
        let config = Config::default();
        assert!(config.base_branch.is_none());
    }

    #[test]
    fn test_parse_base_branch_from_toml() {
        let toml_content = r#"
base_branch = "develop"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.base_branch, Some("develop".to_string()));
    }

    #[test]
    fn test_parse_base_branch_with_other_configs() {
        let toml_content = r#"
projects_root = "/home/user/projects"
base_branch = "main"
ignored_dirs = [".git"]
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.base_branch, Some("main".to_string()));
        assert_eq!(
            config.projects_root,
            Some(PathBuf::from("/home/user/projects"))
        );
    }
}
