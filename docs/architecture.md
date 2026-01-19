# Architecture

## Technology Stack

- **Language**: Rust
- **TUI Framework**: [Ratatui](https://github.com/ratatui-org/ratatui) (for rich, interactive terminal UI)
- **Backend Engine**: Tmux (via CLI or control mode)
- **Configuration**: TOML or YAML

## Component Design

### 1. Core (State Manager)
- Manages the in-memory state of projects and tasks.
- **Project Discovery**: Scans a configured root directory (e.g., `~/src/github`) for Git repositories.
- **Task Discovery**: Parses `git worktree list` for each project to identify active tasks.

### 2. Process Monitor (The "Pulse")
- A background worker or polling mechanism that checks the status of `claude` processes in each worktree.
- **Status Detection Logic**:
    - **Running**: PID exists, CPU active / recent stdout output.
    - **Waiting**: PID exists, process state is sleeping (S/S+), and stdout matches "input prompt" patterns (e.g., "> ", "Waiting for input").
    - **Done**: Process exited successfully.
    - **Error**: Process exited with error.

### 3. TUI Layer (The "Face")
- **Dashboard View**:
    - Sidebar: Project list with aggregate status indicators.
    - Main Area: Task details for the selected project.
- **Interaction**:
    - Keyboard navigation (vim-style j/k) and Mouse support.
    - "Open" action triggers the Tmux Orchestrator.

### 4. Tmux Orchestrator (The "Hands")
- Interacts with Tmux to create sessions, windows, and panes.
- **Cockpit Layout**: Automatically splits windows based on active tasks when a project is opened.
- **Session Management**: Ensures persistent sessions even when the TUI is closed.

## Data Flow

1.  **Init**: Load config -> Scan projects -> Scan worktrees.
2.  **Loop**:
    - Update process status for each active task.
    - Render TUI.
    - Handle User Input.
3.  **Action**:
    - User selects "Project A".
    - Orchestrator checks if Tmux session "vive-project-a" exists.
    - If not, create it with windows for each worktree.
    - Attach client to session.
