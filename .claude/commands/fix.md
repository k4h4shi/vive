---
description: Fix a bug or implement a small feature in Vive (Rust). Usage: /fix <ISSUE_NUMBER> [DESCRIPTION]
---

# Fix / Implement Feature (Rust)

Implement a fix or feature for the Vive project (Rust/TUI).

## Instructions

1.  **Understand the Issue**:
    - Read the issue description using `gh issue view <ISSUE_NUMBER>`.
    - Read `docs/architecture.md` and `docs/spec-ja.md` to understand the context.

2.  **Create/Switch Worktree**:
    - Use the `worktree-manager` skill to set up an isolated environment.
    - Branch name: `fix/issue-<ISSUE_NUMBER>` or `feature/issue-<ISSUE_NUMBER>`.

3.  **Implement (TDD)**:
    - **Write Tests First**: Create a failing test in `tests/` or a unit test in `src/`.
    - **Implement**: Modify Rust code (`src/*.rs`) to pass the test.
    - **Verify**: Run `cargo test` and `cargo run` to verify the TUI behavior.

4.  **Review**:
    - Run `cargo clippy` and `cargo fmt`.
    - Check for regressions.

5.  **Submit**:
    - Push changes and create a PR.
    - `gh pr create --fill`

## Example

```bash
/fix 25 "Implement Tmux Orchestrator"
```
