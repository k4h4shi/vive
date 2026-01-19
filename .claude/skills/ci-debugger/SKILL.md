---
name: ci-debugger
description: Debugs and fixes CI/CD pipeline failures. Use when the user mentions "CI failed", "build error", "fix the build", or when a PR check fails.
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# CI Debugger

This skill helps investigate and fix CI failures in GitHub Actions for the Vive project (Rust).

## Instructions

1.  **Check Status**:

    - Run `gh run list --limit 1 --json databaseId,status,conclusion,headBranch` to see the latest run.
    - If successful, inform the user.

2.  **Analyze Logs**:

    - If failed (`conclusion: failure`), fetch the failure logs:
      ```bash
      gh run view <RUN_ID> --log-failed
      ```
    - Read the logs to identify the root cause (e.g., compilation error, test failure, clippy lint, formatting issue).

3.  **Local Reproduction & Fix**:

    - **Attempt to reproduce** the error locally using the appropriate Rust toolchain command:
      - **Compilation**: `cargo check` or `cargo build`
      - **Tests**: `cargo test`
      - **Lints**: `cargo clippy -- -D warnings`
      - **Formatting**: `cargo fmt -- --check`
    - **Fix the code** based on the error.
    - **Verify** the fix locally.

4.  **Push Fix**:
    - Once verified, commit and push the changes:
      ```bash
      git add .
      git commit -m "fix(ci): Resolve CI failure (<error-summary>)"
      git push origin HEAD
      ```
