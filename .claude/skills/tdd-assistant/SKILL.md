---
name: tdd-assistant
description: Guides the implementation using Nested TDD (Double Loop TDD) for Rust/TUI. Use when the user asks to implement a feature using TDD, mentions "TDD", or asks for implementation guidance.
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# TDD Assistant (Nested TDD for Rust)

This skill guides the implementation process using **Nested TDD (Double Loop TDD)**, adapted for Rust and TUI development.

## Instructions

When the user asks to implement a feature (e.g., "Implement process monitor", "Start TDD for #123"), follow this cycle:

### Phase 1: Preparation

1.  **Understand Requirements**: Read `docs/requirements.md`, `docs/architecture.md`, and the Issue description.
2.  **Worktree Context**: Ensure you are in the correct worktree (if applicable).

### Phase 2: Nested TDD Cycle

**Follow this loop strictly:**

1.  **Outer Loop: Red (Integration/Behavior)**

    - **Goal**: Define the expected behavior of a module or the system.
    - **Action**: Create or update an integration test in `tests/` or a high-level unit test that mocks dependencies.
    - **Example**: "Given a list of PIDs, the ProcessMonitor should return their statuses."
    - **Verify**: Run `cargo test` and confirm it **FAILS** (Red) or fails to compile due to missing types.

2.  **Inner Loop: Unit TDD**

    - **Goal**: Implement the internal logic needed to pass the outer loop test.
    - **Repeat** until the Outer test passes:
      a. **Red**: Write a failing unit test (`#[test]`) for a specific function/struct in `src/`.
      b. **Green**: Write the minimal Rust code to pass the unit test.
      c. **Refactor**: Clean up the code (use `clippy`, improve ownership/borrowing).

3.  **Outer Loop: Green**
    - **Goal**: Verify the feature works as a whole.
    - **Action**: Run the integration/high-level test again.
    - **Verify**: Confirm it **PASSES** (Green).

### Phase 3: Finalization

1.  **Refactor**: Review the codebase for duplication or design improvements.
2.  **Documentation**: Update `docs/` to reflect the new implementation.
3.  **Commit**: Commit the changes with a clear message.
