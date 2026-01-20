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

static SPINNER_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[⏺⠿⠇⠋⠙⠸⠴⠦⠧⠖⠏▶►]").unwrap());

static SUBAGENT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)[⏺⠿⠇⠋⠙⠸⠴⠦⠧⠖⠏]\s*(?:Task|Agent)\s*(?:\([^)]*subagent_type\s*[:=]\s*["']?(\w[\w-]*)["']?\)|(\w+))"#).unwrap()
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

    // Check for approval prompts first (highest priority)
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

    // Check for spinner/working state in recent output
    let tail_for_working = get_tail_lines(content, WORKING_DETECTION_LINES);
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
}
