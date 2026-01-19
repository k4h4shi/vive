# Functional Requirements

## 1. Project Management
- [ ] **Configurable Root**: Users can set a root directory (e.g., `~/src`) to recursively scan for Git repositories.
- [ ] **Manual Add/Remove**: Ability to manually add or ignore specific repositories.
- [ ] **Project List**: Display a scrollable list of all managed projects.

## 2. Task (Worktree) Management
- [ ] **Auto-Discovery**: Automatically detect existing git worktrees as "Tasks".
- [ ] **Create Task**: UI to create a new worktree (branch name + base commit) directly from Vive.
- [ ] **Delete Task**: UI to remove a worktree and delete the branch (with confirmation).

## 3. Agent Status Monitoring
- [ ] **Status Detection**: Real-time identification of Claude's state:
    - 🟢 **Working**: Processing a prompt.
    - 🟡 **Input Needed**: Waiting for user confirmation or next prompt.
    - 🔴 **Stopped/Error**: Process terminated unexpectedly.
    - ⚪ **Idle**: Session open but no active command.
- [ ] **Visual Indicators**: Clear icons/colors in the TUI next to each task.

## 4. Orchestration & Navigation
- [ ] **Open Project**: Switch terminal to a Tmux session dedicated to the project.
- [ ] **Auto-Layout**: When opening a project, automatically create panes for active tasks.
- [ ] **Quick Switch**: Global hotkey or UI element to jump between project sessions instantly.
- [ ] **Mouse Support**: Clickable project list and tabs.

## 5. Configuration
- [ ] **Config File**: `~/.vive/config.toml`
    - `projects_root`: string
    - `ignored_dirs`: list<string>
    - `tmux_prefix`: string (optional override)
