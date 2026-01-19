---
name: worktree-manager
description: Create and manage git worktrees for parallel Claude Code sessions. Use when the user asks to start parallel work, isolate tasks by branch, or set up a new worktree environment.
---

# Worktree Manager (Vive)

## Goal
Set up an isolated work environment using git worktree so multiple Claude Code sessions can run in parallel without interfering.

## Defaults (important)
- Worktree location: `.worktrees/<safe-branch-name>` under the repo root.
- Always ensure `.worktrees/` is ignored by git.
- Never delete directories with rm -rf. Use `git worktree remove`.

## Procedure (must follow in order)
1) Preflight
   - Run: `git status --porcelain`
   - If uncommitted changes exist, ask user to commit/stash or create worktree from clean state.
   - Run: `git fetch --all --prune`

2) Decide branch + base
   - Inputs: branch name (required), base ref (optional; default `origin/main`)
   - If branch exists: create worktree from existing.
   - If new: create new branch from base.

3) Ensure ignore rules
   - Ensure `.worktrees/` is in `.gitignore`.

4) Create the worktree
   - `mkdir -p .worktrees`
   - `git worktree add .worktrees/<safe-branch-name> -b <branch> <base>` (omit -b if existing)

5) Initialize the dev environment (CRITICAL for parallel execution)
   
   **A. Check Rust Toolchain**
   - Run: `cd .worktrees/<safe-branch-name> && cargo check`
   - This ensures dependencies are fetched and the environment is ready for compiling.

6) Output next steps
   - Print:
     - Worktree path
     - Command to start session: `cd .worktrees/<safe-branch-name> && claude`

## Safety constraints
- Do not remove worktrees unless explicitly asked.
- Keep changes scoped to the requested worktree.
