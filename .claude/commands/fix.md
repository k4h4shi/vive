---
description: Implement a feature or fix a bug in Vive. Usage: /fix [ISSUE_NUMBER]
---

# Implement Issue (Vive Workflow)

Follow this workflow to implement a feature for Vive.

## 1. Preparation

1.  **Create Worktree**:
    - Run: `vive start <ISSUE_NUMBER> "<DESCRIPTION>"`
    - This will create a worktree and start a tmux session.

2.  **Context**:
    - The new session will have Claude active.
    - `cd` to the worktree root is automatic.

## 2. Planning

- If the change is complex, invoke the `planner` agent.
- `agent run planner "How should we implement X?"`

## 3. Implementation

- Edit the `vive` script directly.
- **Verification**:
    - Since `vive` is a script, you can test it directly: `./vive <command>`
    - Check for syntax errors: `bash -n vive`
    - Check for style/bugs: `shellcheck vive` (if available)

## 4. Review & Documentation

1.  **Review**: Run `code-reviewer` to check your Bash script.
2.  **Docs**: Update `README.md` if command usage changes.
3.  **Install**: Update `install.sh` if installation logic changes.

## 5. Finish

1.  **Commit**: `git commit -m "feat: ..."`
2.  **Push**: `git push origin HEAD`
3.  **PR**: `gh pr create ...`
