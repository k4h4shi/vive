# Vive

**The AI Development Orchestrator.**

> ⚠️ **Status: Under Active Development (v2)**
> 
> We are reimagining Vive as a robust TUI application written in Rust.
> The legacy shell-script version is deprecated.

Vive is a terminal-based tool designed to orchestrate multi-project, multi-agent development workflows. It manages Git Worktrees, Tmux sessions, and Claude Code agents to provide a seamless "Cockpit" for parallel development.

## Vision

Manage 10+ AI agents across 5+ repositories without losing your mind.

- **Dashboard**: See everything at once. Projects, tasks, and agent statuses.
- **Orchestration**: One click to set up a Tmux environment perfectly laid out for the project.
- **Monitoring**: Know exactly when an agent needs your input (🟢 Working vs 🟡 Waiting).

## Documentation

- [Concept](docs/concept.md)
- [Architecture](docs/architecture.md)
- [Functional Requirements](docs/requirements.md)

## Tech Stack

- **Rust**
- **Ratatui** (TUI)
- **Tmux** (Backend)

## Roadmap

Check `docs/requirements.md` for the feature list.
