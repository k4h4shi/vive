---
description: Cleanup a vive task. Usage: /cleanup [ISSUE_ID]
---

# Cleanup Task

Cleanup a development task (Worktree + Tmux Session) in Vive.

## Instructions

1.  **Identify the Issue ID**:
    - If not provided, ask the user or list active sessions.

2.  **Execute Cleanup**:
    - Use the `vive` CLI or direct Git/Tmux commands (if `vive` CLI cleanup is not yet implemented for external calls).
    - **Preferred**: Use the `vive` TUI interface (Press 'D' on the task).

    **Manual Fallback (if TUI is not accessible via command):**
    ```bash
    # 1. Kill Tmux Session
    tmux kill-session -t <PROJECT_NAME>:<ISSUE_ID> 2>/dev/null || true

    # 2. Remove Worktree
    git worktree remove .worktrees/<ISSUE_ID> --force

    # 3. Delete Branch
    git branch -D feature/<ISSUE_ID> 2>/dev/null || true
    ```

## Example

```bash
/cleanup 123
```
