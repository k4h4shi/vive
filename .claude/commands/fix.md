---
description: Implement a feature or fix a bug following TDD in a dedicated worktree. Usage: /fix [ISSUE_NUMBER]
---

# Implement Issue (Agent-Assisted TDD Workflow)

Follow this workflow to implement a feature or fix a bug using a dedicated worktree and specialized agents.

## 1. Preparation (Worktree Setup)

1.  **Understand the Requirement**: Read the issue and referenced docs (`docs/architecture.md`, `docs/spec-ja.md`, etc.).
2.  **Create Worktree**: Use the `worktree-manager` skill to create an isolated environment.
    - Branch: `feature/issue-<NUMBER>` (or `fix/issue-<NUMBER>`)
    - Base: `origin/main`

3.  **Switch Context**:
    - Change directory to the new worktree: `cd .worktrees/<branch-name>`
    - **IMPORTANT**: All subsequent commands must be run INSIDE this worktree directory.

## 2. Planning & Design (Self-Correction)

**Before coding**, ask yourself: "Do I have a clear plan?"

- If the implementation is complex or touches multiple layers:
  - **Invoke the `planner` agent**.
  - Ask it to generate an implementation plan based on `docs/` and architectural rules.
  - Review the plan.

## 3. Implementation Flow (TDD)

Follow **Nested TDD** cycle (as defined in `tdd-assistant` skill).

### Step 0: Create Test List
### Step 1-4: Red -> Green -> Refactor

- **Note**: Use the `tdd-assistant` skill to guide this process.
- **Build Errors**: If you encounter build or type errors:
  - **Use the `ci-debugger` skill**.
  - Let it fix the compilation issues with minimal changes.

## 4. Final Verification & Documentation

When implementation is complete, **BEFORE** attempting to finish:

1.  **MANDATORY: Update Documentation**:
    - **Use the `doc-updater` skill**. This is NOT optional.
    - Check `docs/architecture.md`, `docs/requirements.md`, etc.
    - Ensure `docs/` exactly matches your code changes.

2.  **Run Pre-commit Verification**:
    - Run `cargo fmt` and `cargo clippy`.
    - Run all tests: `cargo test`.
    - `git commit -m "..."`

3.  **AI Quality Gate**:
    - Now you can safely finish. The system will check:
        1. Did the commit succeed?
        2. Is the documentation updated?

## 5. Push & PR

```bash
git push origin HEAD
gh pr create --title "feat: Title" --body "Closes #<ISSUE_NUMBER>"
```
