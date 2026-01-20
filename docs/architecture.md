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
- **Status Priority** (tmuxcc-inspired):
    - Waiting states (WaitingEdit, WaitingShell, WaitingOther) take priority over spinner detection.
    - This ensures "input waiting" is correctly displayed even when pane title has a spinner.
- **Hysteresis**: 500ms delay to prevent UI flickering during rapid status changes.
- **Preview Capture**: 200 lines captured to ensure command confirmation prompts are visible after long outputs.

### 3. TUI Layer (The "Face")
- **Dashboard View**:
    - Sidebar: Project list with aggregate status indicators.
    - Main Area: Task details for the selected project.
- **Focus Pane Management**:
    - **FocusPane Enum**: `Sidebar` (default) or `Preview` - determines which pane receives key events.
    - **Sidebar Focus**: j/k navigate the project/worktree list.
    - **Preview Focus**: j/k scroll the preview content, Ctrl-d/Ctrl-u for page scroll, g/G for top/bottom.
    - **Visual Feedback**: Active pane has yellow highlighted border; inactive pane has gray border.
- **Interaction**:
    - Keyboard navigation (vim-style j/k) and Mouse support.
    - Tab or h/l keys toggle focus between panes.
    - Mouse click on pane switches focus; mouse wheel scrolls the focused pane.
    - "Open" action triggers the Tmux Orchestrator.

### 4. Tmux Orchestrator (The "Hands")
- Interacts with Tmux to create sessions, windows, and panes.
- **Cockpit Layout**: Automatically splits windows based on active tasks when a project is opened.
- **Session Management**: Ensures persistent sessions even when the TUI is closed.

### 5. GitHub Integration (The "Bridge")
- Provides integration with GitHub via the `gh` CLI.
- **Issue Title Fetching**: Fetches Issue titles to display alongside worktree names (e.g., `#123 Fix login bug`).
- **Issue List Fetching**: Fetches open issues from the repository for the Issue Picker.
- **Caching**: Issue titles are cached to minimize API calls.
- **Branch Name Generation**: Auto-generates branch names from issues (e.g., `feature/issue-123`).

## Data Flow

```mermaid
graph TD
    subgraph Core
        D[Discovery Module] -->|Projects & Worktrees| S[AppState]
        M[Monitor Module] -->|Statuses (Map)| S
        GH[GitHub Module] -->|Issue Titles & Lists| S
    end

    subgraph TUI
        S -->|Render| V[View Layer]
        K[Input Event] -->|Handle| A[Action Handler]
    end

    subgraph Orchestration
        A -->|Create/Switch| T[Tmux Orchestrator]
        A -->|Send Keys| T
        A -->|Create Worktree| G[Git Wrapper]
        A -->|Fetch Issues| GH
    end

    V -->|Display| User
    User -->|Keyboard/Mouse| K
    T -->|Manage| Tmux[Tmux Process]
    G -->|Update| D
    GH -->|gh CLI| GitHub[GitHub API]
```

## State Management Strategy

1.  **Static Data (Projects/Worktrees)**:
    - Loaded once at startup via `Discovery Module`.
    - Reloaded explicitly when user requests "Refresh" or performs "Create Task".

2.  **Dynamic Data (Agent Statuses)**:
    - Managed by `Monitor Module` running in a background async task.
    - Updates are sent to the main UI loop via a `tokio::sync::mpsc` channel.
    - `AppState` holds a `HashMap<SessionId, AgentStatus>` which is updated on every tick.

3.  **UI Rendering**:
    - The TUI renders based on a snapshot of `AppState`.
    - It joins the Static Data (Project Tree) with Dynamic Data (Status Map) using `SessionId` as the key.

4.  **Favorites Persistence**:
    - Favorites are stored in `~/.vive/favorites.toml`.
    - Loaded at startup, saved immediately when toggled.
    - **Robustness**: If loading fails (e.g., corrupted file), a `favorites_load_failed` flag is set.
    - When this flag is set, saving is skipped to prevent overwriting potentially valid data.
    - This prevents data loss in edge cases where the file cannot be read but may still contain valid favorites.

### 6. MCP Server (Model Context Protocol)

The MCP server exposes Vive's internal state to external tools (like Claude Code) via the Model Context Protocol.

- **Transport**: Stdio (standard input/output)
- **Mode**: Standalone server mode (`vive --mcp-server`)
- **Implementation**: Uses the `rmcp` crate (official Rust MCP SDK)

#### Resources

| URI | Description |
|-----|-------------|
| `vive://projects` | All projects and worktrees discovered by Vive |
| `vive://status` | Agent statuses for all sessions (project:branch) |
| `vive://logs/{session_id}` | Pane preview content for a specific session |

#### Architecture

```mermaid
graph LR
    subgraph Vive MCP Server
        SS[SharedState<br/>Arc&lt;RwLock&lt;ViveStateSnapshot&gt;&gt;]
        MH[MCP Handler<br/>ViveMcpServer]
    end

    subgraph External
        CC[Claude Code<br/>or other MCP client]
    end

    CC -->|stdio| MH
    MH -->|read| SS
```

#### State Snapshot

The MCP server uses a `ViveStateSnapshot` struct that captures:
- **Projects**: Name, path, and worktrees for each discovered project
- **Statuses**: Agent status (Working, Idle, Waiting, etc.) for each session
- **Pane Previews**: Terminal output content for each session

#### Usage

To run Vive as an MCP server:

```bash
vive --mcp-server
```

Configure in Claude Desktop's `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "vive": {
      "command": "vive",
      "args": ["--mcp-server"]
    }
  }
}
```

## Configuration

Configuration is stored in `~/.vive/config.toml`.

### Terminal Launch Strategy

The `[terminal]` section controls how Vive launches tmux sessions:

```toml
[terminal]
# Launch strategy: "inline" (default) or "spawn"
# - inline: Replace current process with tmux attach (default)
# - spawn: Launch external terminal without suspending TUI
strategy = "spawn"

# Command to run for spawn strategy (e.g., "ghostty", "wezterm", "alacritty")
command = "ghostty"

# Arguments for the spawn command
# Use {session_id} as placeholder for the target session name
args = ["+e", "tmux attach -t {session_id}"]
```

**Strategies**:
- **inline** (default): Traditional behavior. The TUI is suspended and the current process is replaced with `tmux attach`. This is seamless but leaves the TUI.
- **spawn**: Launches a new terminal window without suspending the TUI. Vive remains open as a dashboard while the session runs in a separate window. Requires `command` to be configured.

**Use Cases**:
- Use `inline` for quick, focused work on a single task.
- Use `spawn` when monitoring multiple sessions or using Vive as a persistent dashboard.
