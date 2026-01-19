//! Configuration module for vive.
//!
//! This module provides functionality to:
//! - Load configuration from `~/.vive/config.toml`
//! - Define default values for configuration options
//! - Support configuration for projects_root, ignored_dirs, and tmux_prefix

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Configuration directory name.
const CONFIG_DIR_NAME: &str = ".vive";
/// Configuration file name.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Default directories to ignore when scanning for projects.
const DEFAULT_IGNORED_DIRS: &[&str] = &[".git", "node_modules", ".worktrees", "target", "dist"];

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            projects_root: None,
            ignored_dirs: DEFAULT_IGNORED_DIRS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            tmux_prefix: None,
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
            fs::create_dir_all(&config_dir)
                .with_context(|| format!("Failed to create config directory: {}", config_dir.display()))?;
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
"#;
            fs::write(&config_path, default_content)
                .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
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
}

/// Get the user's home directory.
fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
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
        assert_eq!(config.effective_projects_root(), PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_config_file_path() {
        let path = Config::config_file_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains(".vive"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }
}
