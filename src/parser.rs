//! Parser module for detecting Claude Code agent state from terminal output.
//!
//! This module provides robust regex-based parsing to detect:
//! - Approval prompts (file edit, create, shell command)
//! - Button UI (Yes/No selection)
//! - Subagent/spinner activity

use once_cell::sync::Lazy;
use regex::Regex;

/// Detected state of the Claude Code agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedStatus {
    /// Agent is idle or no recognizable state detected.
    Idle,
    /// Agent is working (spinner/subagent detected).
    Working { task: Option<String> },
    /// Agent is waiting for approval.
    WaitingApproval { approval_type: ApprovalType },
}

/// Type of approval being requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalType {
    /// File edit approval.
    FileEdit { path: Option<String> },
    /// File create approval.
    FileCreate { path: Option<String> },
    /// Shell command approval.
    ShellCommand { command: Option<String> },
    /// General yes/no prompt.
    General,
}

/// Subagent information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subagent {
    /// Type of subagent (e.g., "Explore", "Plan").
    pub subagent_type: String,
    /// Description or status of the subagent.
    pub description: Option<String>,
}

// Lazy-initialized regex patterns
static FILE_EDIT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(Edit|Write|Modify)\s+.*?\?|Do you want to (edit|write|modify)|Allow.*?edit")
        .unwrap()
});

static FILE_CREATE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Create\s+.*?\?|Do you want to create|Allow.*?create").unwrap());

static SHELL_COMMAND_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(Run|Execute)\s+(command|bash|shell)|Do you want to run|Allow.*?(command|bash)|run this command").unwrap()
});

static GENERAL_YESNO_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\[y/n\]|\[Y/n\]|\[yes/no\]|\(Y\)es\s*/\s*\(N\)o|Yes\s*/\s*No").unwrap()
});

static FILE_PATH_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)(?:file|path)[:\s]+([^\s\n]+)|([./][\w/.-]+\.\w+)").unwrap());

static COMMAND_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)(?:command|run)[:\s]+`([^`]+)`|```(?:bash|sh)?\n([^`]+)```").unwrap()
});

// Issue #65: Unified spinner character set used across all spinner-related patterns
const SPINNER_CHARS: &str = "⏺⠿⠇⠋⠙⠸⠴⠦⠧⠖⠏▶►✳✻✽✶✢";

// Legacy spinner pattern for basic spinner detection
static SPINNER_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!("[{SPINNER_CHARS}]")).unwrap());

// Subagent pattern uses subset of spinner chars (braille spinners only)
static SUBAGENT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)[⏺⠿⠇⠋⠙⠸⠴⠦⠧⠖⠏]\s*(?:Task|Agent)\s*(?:\([^)]*subagent_type\s*[:=]\s*["']?(\w[\w-]*)["']?\)|(\w+))"#).unwrap()
});

// Issue #65: Approval prompt patterns
static PROCEED_PROMPT_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Do you want to proceed\?").unwrap());

static NUMBERED_SELECTION_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*❯?\s*[123]\.\s*(Yes|No)").unwrap());

static ESC_TO_CANCEL_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Esc to cancel").unwrap());

// Issue #65: Completion pattern - past tense verb + time (e.g., "✻ Churned for 2m 20s")
static COMPLETION_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"[{SPINNER_CHARS}]\s+\w+(?:ed|éed)\s+for\s+\d+[msh]\s*\d*[msh]?")).unwrap()
});

// Issue #65: Active working pattern - verb ending with "…" or "..."
static ACTIVE_WORKING_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"[{SPINNER_CHARS}]\s+\w+(?:ing)?(?:…|\.{{3}})")).unwrap()
});

// Issue #65: Prompt detection patterns
static PROMPT_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^❯\s*(?:/\w+.*)?$").unwrap());

static UI_HINT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^❯\s+(?:Press up to edit|[123]\.\s*(?:Yes|No))").unwrap()
});

/// Number of lines from the end to check for approval prompts.
/// Issue #57: Limit approval detection to recent output to prevent
/// stale prompts from causing incorrect Waiting status.
const APPROVAL_DETECTION_LINES: usize = 30;

/// Number of lines from the end to check for spinner/working state.
const WORKING_DETECTION_LINES: usize = 20;

/// Extract the last N lines from content.
fn get_tail_lines(content: &str, n: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// Parse terminal content and return the detected status.
pub fn parse_status(content: &str) -> ParsedStatus {
    // Issue #57: Only check the last N lines for approval prompts
    // to prevent stale prompts from causing incorrect Waiting status
    let tail_for_approval = get_tail_lines(content, APPROVAL_DETECTION_LINES);

    // Issue #65: Check for "Do you want to proceed?" with numbered selection (highest priority)
    // This catches Bash command approval prompts like:
    //   Do you want to proceed?
    //   ❯ 1. Yes
    //     2. Yes, and don't ask again
    //     3. No
    if detect_proceed_prompt(&tail_for_approval) {
        return ParsedStatus::WaitingApproval {
            approval_type: ApprovalType::ShellCommand { command: None },
        };
    }

    // Check for approval prompts (file edit, create, shell command)
    if let Some(approval) = detect_approval(&tail_for_approval) {
        return ParsedStatus::WaitingApproval {
            approval_type: approval,
        };
    }

    // Check for button UI (already limited to last 10 lines internally)
    if detect_button_ui(content) {
        return ParsedStatus::WaitingApproval {
            approval_type: ApprovalType::General,
        };
    }

    // Issue #65: Check for active working pattern (verb + "…") FIRST
    // e.g., "✳ Percolating…", "✽ Forging…" indicates active work
    // This must come BEFORE prompt detection because active work takes precedence
    let tail_for_working = get_tail_lines(content, WORKING_DETECTION_LINES);
    if detect_active_working(&tail_for_working) {
        return ParsedStatus::Working {
            task: Some("Working".to_string()),
        };
    }

    // Issue #65: Check for completion pattern (past tense + time)
    // e.g., "✻ Churned for 2m 20s" indicates task is complete
    // Check this before idle prompt because completed pattern is more specific
    if detect_completion_pattern(&tail_for_approval) {
        return ParsedStatus::Idle;
    }

    // Issue #65: Check for idle prompt (❯ at end, not UI hint)
    // This comes AFTER active working and completion patterns because:
    // - Active work (verb + "…") should always be Working
    // - Completed sessions (past tense + time) should be Idle
    // - Only then, if there's a prompt, it's Idle
    let tail_for_prompt = get_tail_lines(content, 10);
    if detect_idle_prompt(&tail_for_prompt) {
        return ParsedStatus::Idle;
    }

    // Check for spinner/working state in recent output (legacy detection)
    if let Some(task) = detect_working(&tail_for_working) {
        return ParsedStatus::Working { task: Some(task) };
    }

    ParsedStatus::Idle
}

/// Parse terminal content and return detected subagents.
pub fn parse_subagents(content: &str) -> Vec<Subagent> {
    let mut subagents = Vec::new();

    for caps in SUBAGENT_PATTERN.captures_iter(content) {
        let subagent_type = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        subagents.push(Subagent {
            subagent_type,
            description: None,
        });
    }

    subagents
}

/// Detect approval prompts in the content.
fn detect_approval(content: &str) -> Option<ApprovalType> {
    // Check for file edit
    if FILE_EDIT_PATTERN.is_match(content) {
        let path = extract_file_path(content);
        return Some(ApprovalType::FileEdit { path });
    }

    // Check for file create
    if FILE_CREATE_PATTERN.is_match(content) {
        let path = extract_file_path(content);
        return Some(ApprovalType::FileCreate { path });
    }

    // Check for shell command
    if SHELL_COMMAND_PATTERN.is_match(content) {
        let command = extract_command(content);
        return Some(ApprovalType::ShellCommand { command });
    }

    // Check for general yes/no
    if GENERAL_YESNO_PATTERN.is_match(content) {
        return Some(ApprovalType::General);
    }

    None
}

/// Detect button UI (Yes/No on separate lines).
fn detect_button_ui(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().rev().take(10).collect();

    let mut has_yes = false;
    let mut has_no = false;
    let mut yes_line = 0;
    let mut no_line = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "Yes" || trimmed.starts_with("Yes,") {
            has_yes = true;
            yes_line = i;
        }
        if trimmed == "No" || trimmed.starts_with("No,") {
            has_no = true;
            no_line = i;
        }
    }

    // Both Yes and No found within 4 lines of each other
    has_yes && has_no && yes_line.abs_diff(no_line) <= 4
}

/// Detect working/spinner state.
fn detect_working(content: &str) -> Option<String> {
    if SPINNER_PATTERN.is_match(content) {
        // Try to extract task name from subagent pattern
        if let Some(caps) = SUBAGENT_PATTERN.captures(content) {
            let task = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str().to_string());
            return Some(task.unwrap_or_else(|| "Working".to_string()));
        }
        return Some("Working".to_string());
    }
    None
}

// =========================================================================
// Issue #65: New detection functions for improved accuracy
// =========================================================================

/// Issue #65: Detect "Do you want to proceed?" with numbered selection.
/// This catches Claude Code's Bash command approval prompts.
fn detect_proceed_prompt(content: &str) -> bool {
    // Must have "Do you want to proceed?" AND (numbered selection OR "Esc to cancel")
    // "Esc to cancel" appears in all Claude Code approval prompts
    PROCEED_PROMPT_PATTERN.is_match(content)
        && (NUMBERED_SELECTION_PATTERN.is_match(content) || ESC_TO_CANCEL_PATTERN.is_match(content))
}

/// Issue #65: Detect completion pattern (past tense + time).
/// e.g., "✻ Churned for 2m 20s", "✻ Sautéed for 4m 25s"
fn detect_completion_pattern(content: &str) -> bool {
    COMPLETION_PATTERN.is_match(content)
}

/// Issue #65: Detect active working pattern (verb + "…").
/// e.g., "✳ Percolating…", "✻ Collecting data…"
fn detect_active_working(content: &str) -> bool {
    ACTIVE_WORKING_PATTERN.is_match(content)
}

/// Issue #65: Detect idle prompt (❯ at end of content, not UI hint).
/// Returns true if there's an idle command prompt waiting for input.
fn detect_idle_prompt(content: &str) -> bool {
    // Check if there's a prompt pattern
    if !PROMPT_PATTERN.is_match(content) {
        return false;
    }

    // Make sure it's not a UI hint (e.g., "❯ Press up to edit", "❯ 1. Yes")
    if UI_HINT_PATTERN.is_match(content) {
        return false;
    }

    // Check that the prompt is at or near the end of content
    let lines: Vec<&str> = content.lines().collect();
    for line in lines.iter().rev().take(5) {
        let trimmed = line.trim();
        if trimmed.starts_with('❯') {
            // It's a prompt and not a UI hint
            return !UI_HINT_PATTERN.is_match(trimmed);
        }
        // Skip empty lines and separator lines
        if !trimmed.is_empty() && !trimmed.chars().all(|c| c == '─' || c == '-') {
            // Found non-empty, non-separator line that's not a prompt
            // Keep checking in case prompt is a few lines up
            continue;
        }
    }

    false
}

/// Extract file path from content.
fn extract_file_path(content: &str) -> Option<String> {
    FILE_PATH_PATTERN
        .captures(content)
        .and_then(|caps| caps.get(1).or_else(|| caps.get(2)))
        .map(|m| m.as_str().to_string())
}

/// Extract command from content.
fn extract_command(content: &str) -> Option<String> {
    COMMAND_PATTERN
        .captures(content)
        .and_then(|caps| caps.get(1).or_else(|| caps.get(2)))
        .map(|m| m.as_str().trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_idle() {
        let content = "Some random output\nNothing special here";
        assert_eq!(parse_status(content), ParsedStatus::Idle);
    }

    #[test]
    fn test_detect_file_edit_approval() {
        let content = "Do you want to edit this file?";
        match parse_status(content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::FileEdit { .. },
            } => {}
            other => panic!("Expected FileEdit approval, got {other:?}"),
        }
    }

    #[test]
    fn test_detect_file_create_approval() {
        let content = "Create new file src/test.rs?";
        match parse_status(content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::FileCreate { .. },
            } => {}
            other => panic!("Expected FileCreate approval, got {other:?}"),
        }
    }

    #[test]
    fn test_detect_shell_command_approval() {
        let content = "Do you want to run this command?\n```bash\ncargo test\n```";
        match parse_status(content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::ShellCommand { command },
            } => {
                assert_eq!(command, Some("cargo test".to_string()));
            }
            other => panic!("Expected ShellCommand approval, got {other:?}"),
        }
    }

    #[test]
    fn test_detect_general_yesno() {
        let content = "Continue? [y/n]";
        match parse_status(content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::General,
            } => {}
            other => panic!("Expected General approval, got {other:?}"),
        }
    }

    #[test]
    fn test_detect_button_ui() {
        let content = "Do you want to proceed?\n\nYes\nNo";
        match parse_status(content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::General,
            } => {}
            other => panic!("Expected General approval from button UI, got {other:?}"),
        }
    }

    #[test]
    fn test_detect_spinner_working() {
        let content = "⠋ Working on task...";
        match parse_status(content) {
            ParsedStatus::Working { .. } => {}
            other => panic!("Expected Working status, got {other:?}"),
        }
    }

    #[test]
    fn test_detect_subagent() {
        let content = "⠿ Task (subagent_type: Explore) searching files";
        let subagents = parse_subagents(content);
        assert!(!subagents.is_empty());
        assert_eq!(subagents[0].subagent_type, "Explore");
    }

    #[test]
    fn test_extract_file_path() {
        let content = "Edit file: src/main.rs";
        let path = extract_file_path(content);
        assert_eq!(path, Some("src/main.rs".to_string()));
    }

    #[test]
    fn test_extract_file_path_from_path() {
        let content = "Modifying ./src/lib.rs for changes";
        let path = extract_file_path(content);
        assert_eq!(path, Some("./src/lib.rs".to_string()));
    }

    #[test]
    fn test_extract_command() {
        let content = "Run command: `cargo build`";
        let cmd = extract_command(content);
        assert_eq!(cmd, Some("cargo build".to_string()));
    }

    #[test]
    fn test_button_ui_not_detected_when_far_apart() {
        let content = "Yes\n\n\n\n\n\n\n\n\n\nNo";
        // Lines are more than 4 apart
        assert!(!detect_button_ui(content));
    }

    // Additional tests for better coverage

    #[test]
    fn test_detect_file_edit_variations() {
        // Different phrasings for file edit
        let cases = [
            "Edit src/main.rs?",
            "Do you want to modify the file?",
            "Allow this edit?",
            "Write to file /tmp/test.txt?",
        ];
        for content in cases {
            match parse_status(content) {
                ParsedStatus::WaitingApproval {
                    approval_type: ApprovalType::FileEdit { .. },
                } => {}
                other => panic!("Expected FileEdit for '{content}', got {other:?}"),
            }
        }
    }

    #[test]
    fn test_detect_file_create_variations() {
        let cases = [
            "Create src/new_file.rs?",
            "Do you want to create this file?",
            "Allow file create?",
        ];
        for content in cases {
            match parse_status(content) {
                ParsedStatus::WaitingApproval {
                    approval_type: ApprovalType::FileCreate { .. },
                } => {}
                other => panic!("Expected FileCreate for '{content}', got {other:?}"),
            }
        }
    }

    #[test]
    fn test_detect_shell_command_variations() {
        let cases = [
            "Run command in bash?",
            "Execute shell command?",
            "Do you want to run this command?",
            "Allow bash execution?",
        ];
        for content in cases {
            match parse_status(content) {
                ParsedStatus::WaitingApproval {
                    approval_type: ApprovalType::ShellCommand { .. },
                } => {}
                other => panic!("Expected ShellCommand for '{content}', got {other:?}"),
            }
        }
    }

    #[test]
    fn test_detect_general_yesno_variations() {
        let cases = [
            "Continue? [Y/n]",
            "Proceed? [yes/no]",
            "(Y)es / (N)o",
            "Yes / No",
        ];
        for content in cases {
            match parse_status(content) {
                ParsedStatus::WaitingApproval {
                    approval_type: ApprovalType::General,
                } => {}
                other => panic!("Expected General for '{content}', got {other:?}"),
            }
        }
    }

    #[test]
    fn test_button_ui_with_comma_prefix() {
        let content = "Some question\n\nYes, proceed\nNo, cancel";
        assert!(detect_button_ui(content));
    }

    #[test]
    fn test_button_ui_only_yes_no_present() {
        // Only Yes present
        let content = "Question?\n\nYes";
        assert!(!detect_button_ui(content));

        // Only No present
        let content = "Question?\n\nNo";
        assert!(!detect_button_ui(content));
    }

    #[test]
    fn test_detect_all_spinner_characters() {
        let spinners = [
            '⏺', '⠿', '⠇', '⠋', '⠙', '⠸', '⠴', '⠦', '⠧', '⠖', '⠏', '▶', '►',
        ];
        for spinner in spinners {
            let content = format!("{spinner} Processing...");
            match parse_status(&content) {
                ParsedStatus::Working { .. } => {}
                other => panic!("Expected Working for spinner '{spinner}', got {other:?}"),
            }
        }
    }

    #[test]
    fn test_working_extracts_task_name() {
        let content = "⠿ Task (subagent_type: Plan) planning implementation";
        match parse_status(content) {
            ParsedStatus::Working { task } => {
                assert_eq!(task, Some("Plan".to_string()));
            }
            other => panic!("Expected Working with task, got {other:?}"),
        }
    }

    #[test]
    fn test_working_without_subagent_pattern() {
        let content = "⠋ Just a simple spinner";
        match parse_status(content) {
            ParsedStatus::Working { task } => {
                assert_eq!(task, Some("Working".to_string()));
            }
            other => panic!("Expected Working, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_subagents_empty() {
        let content = "Normal content without any subagent";
        let subagents = parse_subagents(content);
        assert!(subagents.is_empty());
    }

    #[test]
    fn test_parse_subagents_multiple() {
        let content = "⠿ Task (subagent_type: Explore) first\n⠋ Agent Plan doing stuff";
        let subagents = parse_subagents(content);
        assert_eq!(subagents.len(), 2);
    }

    #[test]
    fn test_extract_file_path_none() {
        let content = "Just some text without any relevant info";
        let path = extract_file_path(content);
        assert_eq!(path, None);
    }

    #[test]
    fn test_extract_file_path_absolute() {
        let content = "file: /home/user/project/src/main.rs";
        let path = extract_file_path(content);
        assert_eq!(path, Some("/home/user/project/src/main.rs".to_string()));
    }

    #[test]
    fn test_extract_command_from_code_block() {
        let content = "Running:\n```bash\nnpm install && npm test\n```";
        let cmd = extract_command(content);
        assert_eq!(cmd, Some("npm install && npm test".to_string()));
    }

    #[test]
    fn test_extract_command_none() {
        let content = "No command here";
        let cmd = extract_command(content);
        assert_eq!(cmd, None);
    }

    #[test]
    fn test_priority_approval_over_spinner() {
        // Content has both approval and spinner - approval should win
        let content = "⠋ Do you want to edit this file?";
        match parse_status(content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::FileEdit { .. },
            } => {}
            other => panic!("Expected FileEdit (approval has priority), got {other:?}"),
        }
    }

    #[test]
    fn test_priority_approval_over_button_ui() {
        // Content has both specific approval and button UI
        let content = "Do you want to run this command?\nYes\nNo";
        match parse_status(content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::ShellCommand { .. },
            } => {}
            other => {
                panic!("Expected ShellCommand (specific approval has priority), got {other:?}")
            }
        }
    }

    #[test]
    fn test_idle_with_non_matching_content() {
        let cases = [
            "",
            "   ",
            "Just some regular output",
            "claude code",
            "finished successfully",
        ];
        for content in cases {
            assert_eq!(
                parse_status(content),
                ParsedStatus::Idle,
                "Expected Idle for '{content}'"
            );
        }
    }

    // Issue #57: Old approval prompts in buffer should not trigger Waiting status
    #[test]
    fn test_old_approval_not_detected_when_far_from_end() {
        // Simulate: approval prompt at line 10, but 50 lines of new output after it
        let mut content = String::from("Do you want to edit this file?\n");
        for i in 0..50 {
            content.push_str(&format!(
                "Line {i} of new output after approval was handled\n"
            ));
        }
        // Old approval prompt is now far from the end, should be Idle
        assert_eq!(parse_status(&content), ParsedStatus::Idle);
    }

    #[test]
    fn test_recent_approval_still_detected() {
        // Approval prompt within the last few lines should still be detected
        let mut content = String::new();
        for i in 0..10 {
            content.push_str(&format!("Line {i} of output\n"));
        }
        content.push_str("Do you want to edit this file?\n");
        // Approval is at the end, should be detected
        match parse_status(&content) {
            ParsedStatus::WaitingApproval {
                approval_type: ApprovalType::FileEdit { .. },
            } => {}
            other => panic!("Expected FileEdit approval for recent prompt, got {other:?}"),
        }
    }

    #[test]
    fn test_old_shell_command_not_detected() {
        // Shell command approval at the beginning, lots of output after
        let mut content =
            String::from("Do you want to run this command?\n```bash\ncargo test\n```\n");
        for i in 0..50 {
            content.push_str(&format!("Test output line {i}\n"));
        }
        // Old shell command prompt should not be detected
        assert_eq!(parse_status(&content), ParsedStatus::Idle);
    }

    #[test]
    fn test_spinner_still_detected_with_old_approval() {
        // Old approval at the top, but spinner at the end = should be Working
        let mut content = String::from("Do you want to edit this file?\n");
        for i in 0..50 {
            content.push_str(&format!("Output line {i}\n"));
        }
        content.push_str("⠋ Now working on something new...\n");
        match parse_status(&content) {
            ParsedStatus::Working { .. } => {}
            other => panic!("Expected Working status with spinner, got {other:?}"),
        }
    }

    // =========================================================================
    // Issue #65: Improved Agent State Detection Tests
    // =========================================================================

    /// Issue #65: Completion patterns (past tense + time) should be Idle
    /// Covers: Churned, Sautéed, Crunched, and other past tense verbs
    #[test]
    fn test_issue65_completion_patterns() {
        let cases = [
            ("✻ Churned for 2m 20s\n❯", "Churned"),
            ("✻ Sautéed for 4m 25s\n❯ /cleanup", "Sautéed"),
            ("✻ Crunched for 2m 8s\n❯", "Crunched"),
            ("✳ Brewed for 1m 30s\n❯", "Brewed"),
        ];
        for (content, verb) in cases {
            assert_eq!(
                parse_status(content),
                ParsedStatus::Idle,
                "Completion pattern '{verb}' should be Idle"
            );
        }
    }

    /// Issue #65: Active working patterns (verb + "…") should be Working
    /// Covers: Percolating, Forging, and other active verbs
    #[test]
    fn test_issue65_active_working_patterns() {
        let cases = [
            ("✳ Percolating… (ctrl+c to interrupt)", "Percolating"),
            ("✽ Forging… (esc to interrupt)", "Forging"),
            ("✻ Thinking…", "Thinking"),
        ];
        for (content, verb) in cases {
            match parse_status(content) {
                ParsedStatus::Working { .. } => {}
                other => panic!("Active working pattern '{verb}' should be Working, got {other:?}"),
            }
        }
    }

    /// Issue #65: Prompt patterns (❯) should be Idle
    #[test]
    fn test_issue65_prompt_patterns() {
        let cases = [
            ("───────────────────────────\n❯\n───────────────────────────", "empty prompt"),
            ("───────────────────────────\n❯ /fix\n───────────────────────────", "command prompt"),
            ("❯ push して PR 作って", "Japanese command"),
        ];
        for (content, desc) in cases {
            assert_eq!(
                parse_status(content),
                ParsedStatus::Idle,
                "Prompt pattern '{desc}' should be Idle"
            );
        }
    }

    /// Issue #65: UI hints should NOT be treated as idle prompts
    #[test]
    fn test_issue65_ui_hints_not_idle() {
        let content = "✳ Percolating… (ctrl+c to interrupt)\n\n❯ Press up to edit queued messages";
        match parse_status(content) {
            ParsedStatus::Working { .. } => {}
            other => panic!("UI hint with active spinner should be Working, got {other:?}"),
        }
    }

    /// Issue #65: Approval patterns should be WaitingApproval
    #[test]
    fn test_issue65_approval_patterns() {
        let content = r#"
 Do you want to proceed?
 ❯ 1. Yes
   2. Yes, and don't ask again
   3. No

 Esc to cancel
"#;
        match parse_status(content) {
            ParsedStatus::WaitingApproval { .. } => {}
            other => panic!("Approval pattern should be WaitingApproval, got {other:?}"),
        }
    }

    // =========================================================================
    // Issue #65: Real Case Tests (from actual tmux captures)
    // =========================================================================

    /// Issue #65: Real case - issue-43 completed state
    #[test]
    fn test_issue65_real_case_issue43_completed() {
        let content = r#"
⏺ Implementation Complete
  This is a summary of what was done.

✻ Sautéed for 4m 25s
───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
❯ /cleanup                                                                                                   ↵ send
───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle)
"#;
        // This should be Idle because:
        // 1. "Sautéed for 4m 25s" is completion pattern (past tense)
        // 2. "❯ /cleanup" is a command prompt (not a UI hint)
        assert_eq!(
            parse_status(content),
            ParsedStatus::Idle,
            "Issue-43 completed state should be Idle"
        );
    }

    /// Issue #65: Real case - issue-50 prompt waiting
    #[test]
    fn test_issue65_real_case_issue50_prompt_waiting() {
        let content = r#"
⏺ Geminiによるコードレビューが完了しました。
  Review summary here...

❯
───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle)
"#;
        assert_eq!(
            parse_status(content),
            ParsedStatus::Idle,
            "Issue-50 prompt waiting should be Idle"
        );
    }

    /// Issue #65: Real case - issue-127 working
    #[test]
    fn test_issue65_real_case_issue127_working() {
        let content = r#"
✳ Percolating… (ctrl+c to interrupt · 1m 59s · ↑ 4.1k tokens · thought for 3s)

❯ Press up to edit queued messages
───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle)
"#;
        match parse_status(content) {
            ParsedStatus::Working { .. } => {}
            other => panic!("Issue-127 working state should be Working, got {other:?}"),
        }
    }

    /// Issue #65: Real case - Bash approval waiting
    #[test]
    fn test_issue65_real_case_bash_approval() {
        let content = r#"
⏺ Bash(npm test 2>&1 | tail -50) timeout: 3m 0s
  ⎿  Running…

───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
 Bash command

   npm test 2>&1 | tail -50
   Run unit tests

 Do you want to proceed?
 ❯ 1. Yes
   2. Yes, and don't ask again for npm test commands
   3. No

 Esc to cancel · Tab to add additional instructions
"#;
        match parse_status(content) {
            ParsedStatus::WaitingApproval { .. } => {}
            other => panic!("Bash approval should be WaitingApproval, got {other:?}"),
        }
    }

    /// Issue #65: Real case - git commit approval waiting (issue-140)
    #[test]
    fn test_issue65_real_case_git_commit_approval() {
        let content = r#"
⏺ Bash(git add -A && git commit -m "feat(schema): update Customer and Vehicle models per meeting notes…)
  ⎿  Running…

───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
 Bash command

   git add -A && git commit -m "feat(schema): update Customer and Vehicle models per meeting notes

   Issue #140: Update Prisma schema based on 2026-01-20 meeting notes.
   ...
   Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
   Commit all changes with detailed message

 Do you want to proceed?
 ❯ 1. Yes
   2. Yes, and don't ask again for similar commands in /path/to/project
   3. No

 Esc to cancel · Tab to add additional instructions
"#;
        match parse_status(content) {
            ParsedStatus::WaitingApproval { .. } => {}
            other => panic!("Git commit approval should be WaitingApproval, got {other:?}"),
        }
    }

    /// Issue #65: Real case - issue-140 working state (git push running)
    #[test]
    fn test_issue65_real_case_issue140_working() {
        let content = r#"
⏺ Bash(git push -u origin feature/issue-140 2>&1) timeout: 1m 0s
  ⎿  Running…

✽ Forging… (esc to interrupt · thinking)

───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
❯
───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle)
"#;
        // This should be Working because:
        // "✽ Forging…" is active working pattern (verb + "…")
        match parse_status(content) {
            ParsedStatus::Working { .. } => {}
            other => panic!("Issue-140 working state should be Working, got {other:?}"),
        }
    }

    /// Issue #65: Real case - issue-140 completed state (commit done, prompt waiting)
    #[test]
    fn test_issue65_real_case_issue140_completed() {
        let content = r#"
⏺ Bash(git add -A && git commit -m "feat(schema): update Customer and Vehicle models per meeting notes…)
  ⎿  [feature/issue-140 f4f4d70] feat(schema): update Customer and Vehicle models per meeting notes
      31 files changed, 868 insertions(+), 381 deletions(-)
      create mode 100644 web/prisma/migrations/20260121010000_update_customer_vehicle_schema/migration.sql

⏺ Issue #140 の実装が完了しました。

  完了した変更
  - Prisma スキーマの更新
  - マイグレーション
  - コード更新

✻ Sautéed for 6m 18s

───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
❯ push して PR 作って
───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle)
"#;
        // This should be Idle because:
        // 1. "Sautéed for 6m 18s" is completion pattern (past tense)
        // 2. "❯ push して PR 作って" is a command prompt (user input waiting)
        assert_eq!(
            parse_status(content),
            ParsedStatus::Idle,
            "Issue-140 completed state should be Idle"
        );
    }

    /// Issue #65: Real case - issue-140 gh pr view approval waiting
    #[test]
    fn test_issue65_real_case_issue140_gh_approval() {
        let content = r#"
✻ Crunched for 2m 57s

❯ ではPRを取得してください。

⏺ Bash(gh pr view 145 --json title,body,state,reviews,comments)
  ⎿  Running…

───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
 Bash command

   gh pr view 145 --json title,body,state,reviews,comments
   Get PR #145 details including reviews

 Do you want to proceed?
 ❯ 1. Yes
   2. Yes, and don't ask again for gh pr view commands in /path/to/project
   3. No

 Esc to cancel · Tab to add additional instructions
"#;
        // This should be WaitingApproval because:
        // "Do you want to proceed?" + numbered selection (❯ 1. Yes, 2. Yes..., 3. No)
        match parse_status(content) {
            ParsedStatus::WaitingApproval { .. } => {}
            other => panic!("gh pr view approval should be WaitingApproval, got {other:?}"),
        }
    }

    /// Issue #65: Real case - issue-140 completed after PR review (Crunched pattern)
    #[test]
    fn test_issue65_real_case_issue140_completed_crunched() {
        let content = r#"
⏺ Gemini がレビューを完了し、PR #145 にコメントを投稿しました。

  PR URL: https://github.com/k4h4shi/mechanix/pull/145

⏺ Ran 1 stop hook
  ⎿  Documentation has not been fully updated...
  ⎿  Stop hook error: Prompt hook condition was not met...

✻ Crunched for 2m 57s

───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
❯
───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle) · 1 file +0 -0
"#;
        // This should be Idle because:
        // 1. "Crunched for 2m 57s" is completion pattern (past tense)
        // 2. "❯" is an empty prompt (user input waiting)
        assert_eq!(
            parse_status(content),
            ParsedStatus::Idle,
            "Issue-140 completed (Crunched) state should be Idle"
        );
    }
}
