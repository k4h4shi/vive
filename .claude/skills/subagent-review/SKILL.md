---
name: subagent-review
description: Request a code review from the Gemini sub-agent for a Pull Request. Use when the user asks to review a PR, check the code, or mentions "review".
allowed-tools: Bash
---

# Sub-agent Review (Gemini)

This skill offloads the code review task to an external Gemini agent running via the `gemini` CLI.

## Instructions

When the user asks for a review (e.g., "Review PR #123", "Review this"), follow these steps:

1.  **Identify the PR Number**:

    - If the user provided a number, use it.
    - If not, try to find the current PR number using `gh pr view --json number -q .number`.

2.  **Invoke Gemini**:

    - Execute the `gemini` command with the review instruction.
    - Use the `--yolo` flag (if available/safe) or interactive mode to ensure it runs.
    - **Command**:
      ```bash
      gemini "/review <PR_NUMBER>" --yolo
      ```

3.  **Report**:
    - Inform the user that the review request has been sent to Gemini.
