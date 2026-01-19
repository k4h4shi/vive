//! Project and worktree discovery module.
//!
//! This module provides functionality to:
//! - Scan directories recursively for Git repositories
//! - Parse `git worktree list` output to discover worktrees
//! - Define core data structures for projects and worktrees

// Allow dead code during development - these APIs will be used by UI/state modules
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

/// Represents a Git worktree (task) within a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    /// Absolute path to the worktree directory.
    pub path: PathBuf,
    /// The Git commit hash (HEAD).
    pub commit: String,
    /// The branch name, if any. None for detached HEAD.
    pub branch: Option<String>,
}

/// Represents a Git project (repository) that may contain multiple worktrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// The project name (derived from directory name).
    pub name: String,
    /// Absolute path to the main repository.
    pub path: PathBuf,
    /// List of worktrees associated with this project.
    pub worktrees: Vec<Worktree>,
}

impl Project {
    /// Creates a new Project with the given name and path.
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            worktrees: Vec::new(),
        }
    }

    /// Sets the worktrees for this project.
    pub fn with_worktrees(mut self, worktrees: Vec<Worktree>) -> Self {
        self.worktrees = worktrees;
        self
    }
}

impl Worktree {
    /// Creates a new Worktree with the given path, commit, and optional branch.
    pub fn new(
        path: impl Into<PathBuf>,
        commit: impl Into<String>,
        branch: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            commit: commit.into(),
            branch,
        }
    }

    /// Generates a unique Tmux session ID for this worktree.
    ///
    /// The format is `project_name:branch_name` (e.g., `mechanix:fix-bug-123`).
    /// Returns `None` if the worktree has no branch (detached HEAD).
    pub fn session_id(&self, project_name: &str) -> Option<String> {
        self.branch
            .as_ref()
            .map(|branch_name| format!("{project_name}:{branch_name}"))
    }
}

/// Parses the output of `git worktree list --porcelain`.
///
/// The porcelain format outputs one worktree per block, separated by blank lines:
/// ```text
/// worktree /path/to/main
/// HEAD abcd1234
/// branch refs/heads/main
///
/// worktree /path/to/feature
/// HEAD efgh5678
/// branch refs/heads/feature-branch
/// ```
///
/// Detached HEAD worktrees don't have a branch line.
pub fn parse_worktree_list(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_commit: Option<String> = None;
    let mut current_branch: Option<String> = None;

    for line in output.lines() {
        if line.is_empty() {
            // End of a worktree block
            if let (Some(path), Some(commit)) = (current_path.take(), current_commit.take()) {
                worktrees.push(Worktree::new(path, commit, current_branch.take()));
            }
            continue;
        }

        if let Some(path_str) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(path_str));
        } else if let Some(commit) = line.strip_prefix("HEAD ") {
            current_commit = Some(commit.to_string());
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            // Convert refs/heads/branch-name to just branch-name
            let branch_name = branch_ref.strip_prefix("refs/heads/").unwrap_or(branch_ref);
            current_branch = Some(branch_name.to_string());
        }
        // Ignore other lines like "bare", "detached", "locked", etc.
    }

    // Handle last worktree if output doesn't end with blank line
    if let (Some(path), Some(commit)) = (current_path, current_commit) {
        worktrees.push(Worktree::new(path, commit, current_branch));
    }

    worktrees
}

/// Discovers worktrees for a Git repository by running `git worktree list --porcelain`.
pub fn discover_worktrees(repo_path: &Path) -> Result<Vec<Worktree>> {
    let output = Command::new("git")
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git worktree list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree list failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_worktree_list(&stdout))
}

/// Checks if a directory is a Git repository.
fn is_git_repository(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Recursively scans a directory for Git repositories.
///
/// This function walks the directory tree starting from `root` and collects
/// all directories that contain a `.git` folder. It does not descend into
/// `.git` directories or into discovered repositories (to avoid nested repos).
///
/// The `ignored_dirs` parameter specifies directory names to skip during scanning.
pub fn scan_for_repositories(root: &Path, ignored_dirs: &[String]) -> Result<Vec<PathBuf>> {
    let mut repositories = Vec::new();
    scan_directory_recursive(root, &mut repositories, ignored_dirs)?;
    Ok(repositories)
}

fn scan_directory_recursive(
    dir: &Path,
    repositories: &mut Vec<PathBuf>,
    ignored_dirs: &[String],
) -> Result<()> {
    // Skip if not a directory or not readable
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()), // Skip unreadable directories
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden directories and ignored directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && (name.starts_with('.') || ignored_dirs.iter().any(|ignored| ignored == name))
        {
            continue;
        }

        if path.is_dir() {
            if is_git_repository(&path) {
                repositories.push(path);
                // Don't descend into git repositories to avoid nested repos
            } else {
                // Continue scanning subdirectories
                scan_directory_recursive(&path, repositories, ignored_dirs)?;
            }
        }
    }

    Ok(())
}

/// Discovers all projects and their worktrees starting from a root directory.
///
/// The `ignored_dirs` parameter specifies directory names to skip during scanning.
pub fn discover_projects(root: &Path, ignored_dirs: &[String]) -> Result<Vec<Project>> {
    let repo_paths = scan_for_repositories(root, ignored_dirs)?;

    let mut projects = Vec::new();
    for repo_path in repo_paths {
        let name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let worktrees = discover_worktrees(&repo_path).unwrap_or_default();

        projects.push(Project::new(name, repo_path).with_worktrees(worktrees));
    }

    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_new() {
        let wt = Worktree::new("/path/to/worktree", "abc123", Some("main".to_string()));
        assert_eq!(wt.path, PathBuf::from("/path/to/worktree"));
        assert_eq!(wt.commit, "abc123");
        assert_eq!(wt.branch, Some("main".to_string()));
    }

    #[test]
    fn test_worktree_detached_head() {
        let wt = Worktree::new("/path/to/worktree", "abc123", None);
        assert_eq!(wt.branch, None);
    }

    #[test]
    fn test_worktree_session_id() {
        let wt = Worktree::new(
            "/path/to/worktree",
            "abc123",
            Some("fix-bug-123".to_string()),
        );
        assert_eq!(
            wt.session_id("mechanix"),
            Some("mechanix:fix-bug-123".to_string())
        );
    }

    #[test]
    fn test_worktree_session_id_detached_head() {
        let wt = Worktree::new("/path/to/worktree", "abc123", None);
        assert_eq!(wt.session_id("mechanix"), None);
    }

    #[test]
    fn test_project_new() {
        let project = Project::new("my-project", "/path/to/project");
        assert_eq!(project.name, "my-project");
        assert_eq!(project.path, PathBuf::from("/path/to/project"));
        assert!(project.worktrees.is_empty());
    }

    #[test]
    fn test_project_with_worktrees() {
        let worktrees = vec![
            Worktree::new("/path/main", "abc", Some("main".to_string())),
            Worktree::new("/path/feature", "def", Some("feature".to_string())),
        ];
        let project = Project::new("my-project", "/path/to/project").with_worktrees(worktrees);
        assert_eq!(project.worktrees.len(), 2);
    }

    #[test]
    fn test_parse_worktree_list_single() {
        let output = r#"worktree /home/user/project
HEAD abcd1234567890
branch refs/heads/main
"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].path, PathBuf::from("/home/user/project"));
        assert_eq!(worktrees[0].commit, "abcd1234567890");
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
    }

    #[test]
    fn test_parse_worktree_list_multiple() {
        let output = r#"worktree /home/user/project
HEAD abcd1234
branch refs/heads/main

worktree /home/user/project/.worktrees/feature-1
HEAD efgh5678
branch refs/heads/feature-1

worktree /home/user/project/.worktrees/feature-2
HEAD ijkl9012
branch refs/heads/feature-2
"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 3);

        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(worktrees[1].branch, Some("feature-1".to_string()));
        assert_eq!(worktrees[2].branch, Some("feature-2".to_string()));
    }

    #[test]
    fn test_parse_worktree_list_detached_head() {
        let output = r#"worktree /home/user/project
HEAD abcd1234
detached
"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch, None);
    }

    #[test]
    fn test_parse_worktree_list_empty() {
        let output = "";
        let worktrees = parse_worktree_list(output);
        assert!(worktrees.is_empty());
    }

    #[test]
    fn test_parse_worktree_list_no_trailing_newline() {
        let output = r#"worktree /home/user/project
HEAD abcd1234
branch refs/heads/main"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
    }

    #[test]
    fn test_is_git_repository() {
        // This test uses a temp directory
        let temp_dir = std::env::temp_dir().join("vive_test_git_repo");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Initially not a git repo
        assert!(!is_git_repository(&temp_dir));

        // Create .git directory
        std::fs::create_dir(temp_dir.join(".git")).unwrap();
        assert!(is_git_repository(&temp_dir));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scan_for_repositories() {
        // Create a temp directory structure:
        // temp/
        //   repo1/
        //     .git/
        //   subdir/
        //     repo2/
        //       .git/
        //   not-a-repo/
        let temp_dir = std::env::temp_dir().join("vive_test_scan");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let repo1 = temp_dir.join("repo1");
        let repo2 = temp_dir.join("subdir").join("repo2");
        let not_repo = temp_dir.join("not-a-repo");

        std::fs::create_dir_all(repo1.join(".git")).unwrap();
        std::fs::create_dir_all(repo2.join(".git")).unwrap();
        std::fs::create_dir_all(&not_repo).unwrap();

        let repos = scan_for_repositories(&temp_dir, &[]).unwrap();

        assert_eq!(repos.len(), 2);
        assert!(repos.contains(&repo1));
        assert!(repos.contains(&repo2));
        assert!(!repos.contains(&not_repo));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scan_for_repositories_with_ignored_dirs() {
        // Create a temp directory structure:
        // temp/
        //   repo1/
        //     .git/
        //   ignored/
        //     repo2/
        //       .git/
        let temp_dir = std::env::temp_dir().join("vive_test_scan_ignored");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let repo1 = temp_dir.join("repo1");
        let repo2 = temp_dir.join("ignored").join("repo2");

        std::fs::create_dir_all(repo1.join(".git")).unwrap();
        std::fs::create_dir_all(repo2.join(".git")).unwrap();

        let ignored_dirs = vec!["ignored".to_string()];
        let repos = scan_for_repositories(&temp_dir, &ignored_dirs).unwrap();

        assert_eq!(repos.len(), 1);
        assert!(repos.contains(&repo1));
        assert!(!repos.contains(&repo2));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
