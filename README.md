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

## Installation

### Prerequisites

- **Rust** (with cargo) - [Install Rust](https://www.rust-lang.org/tools/install)
- **tmux** - Terminal multiplexer
- **git** - Version control

### Install from Source

```bash
git clone https://github.com/k4h4shi/vive.git
cd vive
./install.sh
```

This will:
1. Build the release binary using `cargo build --release`
2. Install to `~/.local/bin/vive` (or `/usr/local/bin/vive` if needed)

For CI/CD or scripted installations, use the `--yes` flag to suppress output:

```bash
./install.sh --yes
```

### Install with Cargo (for Rust developers)

```bash
git clone https://github.com/k4h4shi/vive.git
cd vive
cargo install --path .
```

### Uninstall

```bash
./install.sh --uninstall
```

## Usage

```bash
# Launch the TUI dashboard
vive
```

### Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `VIVE_PROJECTS_ROOT` | Root directory for project discovery | `~/src` |

## Tech Stack

- **Rust**
- **Ratatui** (TUI)
- **Tmux** (Backend)

## Roadmap

Check `docs/requirements.md` for the feature list.
