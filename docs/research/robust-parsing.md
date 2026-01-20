# Robust Parsing Strategy

To reliably detect Claude Code's state and intentions from raw terminal output, simple string matching is insufficient. We need a robust regex-based approach.

## Reference Implementation
- [tmuxcc/src/parsers/claude_code.rs](https://github.com/nyanko3141592/tmuxcc/blob/master/src/parsers/claude_code.rs)

## 1. Approval Detection Patterns

The parser should identify *what* the agent is waiting for.

### Regex Patterns

```rust
// File Edit
Regex::new(r"(?i)(Edit|Write|Modify)\s+.*?\?|Do you want to (edit|write|modify)|Allow.*?edit")

// File Create
Regex::new(r"(?i)Create\s+.*?\?|Do you want to create|Allow.*?create")

// Shell Command
Regex::new(r"(?i)(Run|Execute)\s+(command|bash|shell)|Do you want to run|Allow.*?(command|bash)|run this command")

// General Yes/No
Regex::new(r"(?i)\[y/n\]|\[Y/n\]|\[yes/no\]|\(Y\)es\s*/\s*\(N\)o|Yes\s*/\s*No")
```

### Context Extraction

When a match is found, extract the target file or command for UI display.

```rust
// Extract file path
Regex::new(r"(?m)(?:file|path)[:\s]+([^\s\n]+)|([./][\w/.-]+\.\w+)")

// Extract command
Regex::new(r"(?m)(?:command|run)[:\s]+`([^`]+)`|```(?:bash|sh)?\n([^`]+)```")
```

## 2. Button UI Detection

Claude Code increasingly uses "button-like" interfaces where "Yes" and "No" appear on separate lines.

**Pattern Logic:**
1. Scan the last 10 lines of output.
2. Look for lines that exactly match "Yes", "No", or start with "Yes," (e.g. "Yes, and don't ask again").
3. If both "Yes" and "No" lines are found within close proximity (e.g. 4 lines), treat it as an approval prompt.

## 3. Subagent & Spinner Detection

Claude Code uses specific unicode characters for sub-tasks and loading states.

```rust
// Task Start
Regex::new(r#"(?m)[⏺⠿⠇⠋⠙⠸⠴⠦⠧⠖⠏]\s*Task\s*\([^)]*subagent_type\s*[:=]\s*["']?(\w[\w-]*)["']?"#)

// Running Indicator
Regex::new(r"(?m)^[^│]*[▶►⠿⠇⠋⠙⠸⠴⠦⠧⠖⠏]\s*(\w+)(?:\s*agent)?:?\s*(.*)$")
```

## Implementation Strategy

Create a `Parser` trait and a `ClaudeParser` struct.

```rust
pub trait AgentParser {
    /// Parse content and return high-level status
    fn parse_status(&self, content: &str) -> AgentStatus;
    
    /// Extract running sub-tasks
    fn parse_subagents(&self, content: &str) -> Vec<Subagent>;
}
```
