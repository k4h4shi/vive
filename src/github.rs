//! GitHub integration module for fetching Issue titles.
//!
//! This module provides functionality to:
//! - Fetch Issue titles from GitHub using the `gh` CLI
//! - Cache fetched titles to avoid repeated API calls
//! - Handle errors gracefully when `gh` is unavailable or network fails

use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Result of fetching an Issue title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueTitleResult {
    /// Title was fetched successfully.
    Found(String),
    /// Issue was not found (404 or similar).
    NotFound,
    /// Fetch is still pending (async).
    Pending,
    /// An error occurred during fetch.
    Error(String),
}

/// Cache for Issue titles, keyed by (repo_path, issue_number).
#[derive(Debug, Default)]
pub struct IssueTitleCache {
    cache: HashMap<(String, u32), IssueTitleResult>,
}

impl IssueTitleCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a cached title if available.
    pub fn get(&self, repo_path: &str, issue_number: u32) -> Option<&IssueTitleResult> {
        self.cache.get(&(repo_path.to_string(), issue_number))
    }

    /// Store a title in the cache.
    pub fn set(&mut self, repo_path: String, issue_number: u32, result: IssueTitleResult) {
        self.cache.insert((repo_path, issue_number), result);
    }

    /// Check if a title is already cached.
    pub fn contains(&self, repo_path: &str, issue_number: u32) -> bool {
        self.cache
            .contains_key(&(repo_path.to_string(), issue_number))
    }

    /// Clear the cache.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// Shared cache wrapped in Arc<Mutex> for concurrent access.
pub type SharedIssueTitleCache = Arc<Mutex<IssueTitleCache>>;

/// Create a new shared cache.
pub fn new_shared_cache() -> SharedIssueTitleCache {
    Arc::new(Mutex::new(IssueTitleCache::new()))
}

/// Trait for fetching Issue titles, abstracted for testing.
pub trait IssueTitleFetcher: Send + Sync {
    /// Fetch the title for an Issue.
    fn fetch(&self, repo_path: &str, issue_number: u32) -> IssueTitleResult;
}

/// Real implementation that uses the `gh` CLI.
#[derive(Debug, Default, Clone)]
pub struct GhIssueFetcher;

impl IssueTitleFetcher for GhIssueFetcher {
    fn fetch(&self, repo_path: &str, issue_number: u32) -> IssueTitleResult {
        fetch_issue_title_sync(repo_path, issue_number)
    }
}

/// Fetch an Issue title using `gh issue view`.
///
/// This function runs synchronously and returns the title if successful.
/// It handles various error cases:
/// - `gh` CLI not installed -> Error
/// - Network failure -> Error
/// - Issue not found -> NotFound
pub fn fetch_issue_title_sync(repo_path: &str, issue_number: u32) -> IssueTitleResult {
    let output = Command::new("gh")
        .args([
            "issue",
            "view",
            &issue_number.to_string(),
            "--json",
            "title",
            "-q",
            ".title",
        ])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if title.is_empty() {
                    IssueTitleResult::NotFound
                } else {
                    IssueTitleResult::Found(title)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("Could not resolve") || stderr.contains("not found") {
                    IssueTitleResult::NotFound
                } else {
                    IssueTitleResult::Error(stderr.trim().to_string())
                }
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                IssueTitleResult::Error("gh CLI not installed".to_string())
            } else {
                IssueTitleResult::Error(e.to_string())
            }
        }
    }
}

/// Get or fetch an Issue title with caching.
///
/// This function checks the cache first, and if not found, fetches the title
/// and stores it in the cache.
pub fn get_or_fetch_issue_title<F: IssueTitleFetcher>(
    cache: &SharedIssueTitleCache,
    fetcher: &F,
    repo_path: &str,
    issue_number: u32,
) -> IssueTitleResult {
    // Check cache first
    {
        let cache_guard = cache.lock().unwrap();
        if let Some(result) = cache_guard.get(repo_path, issue_number) {
            return result.clone();
        }
    }

    // Fetch and cache
    let result = fetcher.fetch(repo_path, issue_number);
    {
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.set(repo_path.to_string(), issue_number, result.clone());
    }

    result
}

/// Format a display name for a worktree.
///
/// If an Issue title is available, returns a formatted string like:
/// `#123 Issue Title (branch-name)`
///
/// Otherwise, returns just the branch name.
pub fn format_worktree_display_name(
    branch_name: &str,
    issue_number: Option<u32>,
    issue_title: Option<&str>,
) -> String {
    match (issue_number, issue_title) {
        (Some(num), Some(title)) => {
            // Truncate title if too long
            let max_title_len = 30;
            let truncated_title = if title.chars().count() > max_title_len {
                let s: String = title.chars().take(max_title_len - 3).collect();
                format!("{s}...")
            } else {
                title.to_string()
            };
            format!("#{num} {truncated_title}")
        }
        (Some(num), None) => format!("#{num} ({branch_name})"),
        (None, _) => branch_name.to_string(),
    }
}

/// Represents a GitHub Issue from the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIssue {
    /// Issue number.
    pub number: u32,
    /// Issue title.
    pub title: String,
}

impl GitHubIssue {
    /// Create a new GitHubIssue.
    pub fn new(number: u32, title: impl Into<String>) -> Self {
        Self {
            number,
            title: title.into(),
        }
    }

    /// Generate a branch name from this issue.
    pub fn branch_name(&self) -> String {
        format!("issue-{}", self.number)
    }

    /// Generate a tmux window name from this issue.
    ///
    /// Format: `branch_name_sanitized_title`
    /// - Characters invalid in tmux window names (`.`, `:`) are removed or replaced.
    /// - The total length is capped at 100 characters (truncated with `...`).
    /// - If the title is empty, returns just the branch name.
    pub fn window_name(&self) -> String {
        let branch = self.branch_name();
        let title = self.title.trim();
        if title.is_empty() {
            return branch;
        }

        // Sanitize: remove characters problematic for tmux window names
        let sanitized: String = title
            .chars()
            .filter(|c| *c != '\n' && *c != '\r')
            .map(|c| match c {
                '.' => '_',
                ':' => ' ',
                _ => c,
            })
            .collect();

        // Normalize whitespace and trim
        let sanitized = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");

        let combined = format!("{branch}_{sanitized}");

        // Truncate if too long
        let max_len = 100;
        if combined.chars().count() > max_len {
            let truncated: String = combined.chars().take(max_len - 3).collect();
            format!("{truncated}...")
        } else {
            combined
        }
    }
}

/// Result of fetching the Issue list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueListResult {
    /// Issues were fetched successfully.
    Found(Vec<GitHubIssue>),
    /// No issues found.
    Empty,
    /// An error occurred during fetch.
    Error(String),
}

/// Trait for fetching Issue lists, abstracted for testing.
pub trait IssueListFetcher: Send + Sync {
    /// Fetch open issues from a repository.
    fn fetch_issues(&self, repo_path: &str) -> IssueListResult;
}

/// Real implementation that uses the `gh` CLI to fetch issue lists.
impl IssueListFetcher for GhIssueFetcher {
    fn fetch_issues(&self, repo_path: &str) -> IssueListResult {
        fetch_issue_list_sync(repo_path)
    }
}

/// Fetch open issues using `gh issue list`.
///
/// This function runs synchronously and returns a list of issues if successful.
pub fn fetch_issue_list_sync(repo_path: &str) -> IssueListResult {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--json",
            "number,title",
            "-q",
            ".[] | \"\\(.number)\\t\\(.title)\"",
        ])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let issues: Vec<GitHubIssue> = stdout
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.splitn(2, '\t').collect();
                        if parts.len() == 2 {
                            let number = parts[0].parse().ok()?;
                            let title = parts[1].to_string();
                            Some(GitHubIssue::new(number, title))
                        } else {
                            None
                        }
                    })
                    .collect();

                if issues.is_empty() {
                    IssueListResult::Empty
                } else {
                    IssueListResult::Found(issues)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                IssueListResult::Error(stderr.trim().to_string())
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                IssueListResult::Error("gh CLI not installed".to_string())
            } else {
                IssueListResult::Error(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_issue_new() {
        let issue = GitHubIssue::new(42, "Test Issue");
        assert_eq!(issue.number, 42);
        assert_eq!(issue.title, "Test Issue");
    }

    #[test]
    fn test_github_issue_branch_name() {
        let issue = GitHubIssue::new(123, "Add feature");
        assert_eq!(issue.branch_name(), "issue-123");
    }

    #[test]
    fn test_issue_title_cache_new() {
        let cache = IssueTitleCache::new();
        assert!(!cache.contains("/path", 123));
    }

    #[test]
    fn test_issue_title_cache_set_and_get() {
        let mut cache = IssueTitleCache::new();

        cache.set(
            "/repo".to_string(),
            42,
            IssueTitleResult::Found("Test Issue".to_string()),
        );

        assert!(cache.contains("/repo", 42));
        assert_eq!(
            cache.get("/repo", 42),
            Some(&IssueTitleResult::Found("Test Issue".to_string()))
        );
    }

    #[test]
    fn test_issue_title_cache_different_repos() {
        let mut cache = IssueTitleCache::new();

        cache.set(
            "/repo1".to_string(),
            1,
            IssueTitleResult::Found("Issue 1".to_string()),
        );
        cache.set(
            "/repo2".to_string(),
            1,
            IssueTitleResult::Found("Issue 2".to_string()),
        );

        assert_eq!(
            cache.get("/repo1", 1),
            Some(&IssueTitleResult::Found("Issue 1".to_string()))
        );
        assert_eq!(
            cache.get("/repo2", 1),
            Some(&IssueTitleResult::Found("Issue 2".to_string()))
        );
    }

    #[test]
    fn test_issue_title_cache_clear() {
        let mut cache = IssueTitleCache::new();

        cache.set(
            "/repo".to_string(),
            1,
            IssueTitleResult::Found("Test".to_string()),
        );
        assert!(cache.contains("/repo", 1));

        cache.clear();
        assert!(!cache.contains("/repo", 1));
    }

    #[test]
    fn test_format_worktree_display_name_with_title() {
        let result =
            format_worktree_display_name("issue-123", Some(123), Some("Add dark mode toggle"));
        assert_eq!(result, "#123 Add dark mode toggle");
    }

    #[test]
    fn test_format_worktree_display_name_with_long_title() {
        let long_title = "This is a very long issue title that exceeds the maximum length";
        let result = format_worktree_display_name("issue-456", Some(456), Some(long_title));
        assert!(result.contains("..."));
        assert!(result.starts_with("#456 "));
    }

    #[test]
    fn test_format_worktree_display_name_without_title() {
        let result = format_worktree_display_name("issue-789", Some(789), None);
        assert_eq!(result, "#789 (issue-789)");
    }

    #[test]
    fn test_format_worktree_display_name_no_issue() {
        let result = format_worktree_display_name("main", None, None);
        assert_eq!(result, "main");
    }

    // Mock fetcher for testing
    struct MockFetcher {
        result: IssueTitleResult,
    }

    impl IssueTitleFetcher for MockFetcher {
        fn fetch(&self, _repo_path: &str, _issue_number: u32) -> IssueTitleResult {
            self.result.clone()
        }
    }

    #[test]
    fn test_get_or_fetch_issue_title_caches_result() {
        let cache = new_shared_cache();
        let fetcher = MockFetcher {
            result: IssueTitleResult::Found("Cached Title".to_string()),
        };

        // First call should fetch
        let result1 = get_or_fetch_issue_title(&cache, &fetcher, "/repo", 1);
        assert_eq!(result1, IssueTitleResult::Found("Cached Title".to_string()));

        // Second call should use cache
        let result2 = get_or_fetch_issue_title(&cache, &fetcher, "/repo", 1);
        assert_eq!(result2, IssueTitleResult::Found("Cached Title".to_string()));
    }

    #[test]
    fn test_github_issue_window_name_basic() {
        let issue = GitHubIssue::new(123, "ログイン時のエラーハンドリングを改善");
        assert_eq!(
            issue.window_name(),
            "issue-123_ログイン時のエラーハンドリングを改善"
        );
    }

    #[test]
    fn test_github_issue_window_name_sanitizes_dot() {
        let issue = GitHubIssue::new(1, "fix v2.0.1 bug");
        let name = issue.window_name();
        assert!(!name.contains('.'));
        assert_eq!(name, "issue-1_fix v2_0_1 bug");
    }

    #[test]
    fn test_github_issue_window_name_sanitizes_colon() {
        let issue = GitHubIssue::new(2, "feat: add feature");
        let name = issue.window_name();
        assert!(!name.contains(':'));
        assert_eq!(name, "issue-2_feat add feature");
    }

    #[test]
    fn test_github_issue_window_name_truncates_long_title() {
        let long_title = "a".repeat(200);
        let issue = GitHubIssue::new(42, long_title);
        let name = issue.window_name();
        // Total window name should not exceed 100 characters
        assert!(name.chars().count() <= 100);
        assert!(name.ends_with("..."));
    }

    #[test]
    fn test_github_issue_window_name_empty_title() {
        let issue = GitHubIssue::new(99, "");
        assert_eq!(issue.window_name(), "issue-99");
    }

    #[test]
    fn test_get_or_fetch_issue_title_caches_not_found() {
        let cache = new_shared_cache();
        let fetcher = MockFetcher {
            result: IssueTitleResult::NotFound,
        };

        let result = get_or_fetch_issue_title(&cache, &fetcher, "/repo", 999);
        assert_eq!(result, IssueTitleResult::NotFound);

        // Should still be cached
        assert!(cache.lock().unwrap().contains("/repo", 999));
    }
}
