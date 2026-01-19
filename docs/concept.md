# Vive: The AI Development Orchestrator

## Vision

Vive is a terminal-based orchestration tool designed to manage multi-project, multi-agent development workflows. It scales the "Git Worktree + Tmux + Claude" pattern from a single project to a global development environment.

## Core Philosophy

- **Global Context**: Manage multiple projects simultaneously, not just one.
- **Visual Orchestration**: Provide a TUI dashboard to monitor the state of all AI agents across all projects.
- **Context Isolation**: Strictly enforce isolation using Git Worktrees and Tmux sessions.
- **Seamless Switching**: Enable instant context switching between projects and tasks without cognitive overhead.

## The Problem

Managing AI-driven development across multiple repositories is complex:
- **Visibility**: It's hard to know which agents are working, waiting for input, or finished.
- **Context Switching**: Jumping between projects requires resetting mental and terminal context.
- **Process Management**: Manually managing tmux sessions and worktrees for N projects is error-prone.

## The Solution

Vive acts as a control tower:
1.  **Dashboard**: A centralized view of all projects and their active tasks.
2.  **Monitor**: Real-time status tracking of Claude agents (Working vs. Waiting for Input).
3.  **Workspace**: Automated layout management using Tmux to present the relevant context immediately.
