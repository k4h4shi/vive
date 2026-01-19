# vive

**vive** — parallel AI agent manager, alive in the shell.

A simple CLI to manage multiple Claude agents in tmux sessions, each isolated in its own git worktree.

## Why vive?

| Problem | Solution |
| ------- | -------- |
| Running AI tasks one by one is slow | **Parallel agents** handle multiple issues at once |
| Losing context when switching between tasks | **tmux sessions** let you detach/attach anytime |
| Branch conflicts and accidents | **git worktrees** create isolated environments |

## Installation

```bash
git clone https://github.com/k4h4shi/vive.git
cd vive
./install.sh
```

Or manually:

```bash
sudo ln -s $(pwd)/vive /usr/local/bin/vive
```

## Quick Start

```bash
# Navigate to your project
cd ~/projects/my-app

# Start working on issue #123
vive start 123

# (Claude starts and runs /fix 123)
# Press Ctrl+b d to detach and let it run in background

# Check what's running
vive list

# Jump back in
vive attach 123

# Clean up when done
vive cleanup 123
```

## Commands

| Command | Description |
| ------- | ----------- |
| `vive start <issue> [desc]` | Start a new session for an issue |
| `vive attach <issue>` | Attach to an existing session |
| `vive list` | List active sessions |
| `vive cleanup <issue\|all>` | Clean up session(s) and worktree(s) |
| `vive help` | Show help |

## Workflow

1. **Start**: `vive start 123` creates a worktree and tmux session, then launches Claude with `/fix 123`
2. **Monitor**: Watch the AI work, or detach with `Ctrl+b d` to let it run in the background
3. **Switch**: Use `vive attach` to jump between different issues
4. **Cleanup**: Use `vive cleanup` to remove sessions and worktrees

## Tmux Shortcuts

| Shortcut | Action |
| -------- | ------ |
| `Ctrl+b d` | Detach from session (keeps running) |
| `Ctrl+b [` | Scroll mode (arrows to navigate, `q` to exit) |
| `Ctrl+b c` | Create new window |
| `Ctrl+b n` | Next window |
| `Ctrl+b p` | Previous window |

## Requirements

- bash
- tmux
- git (with worktree support)
- claude CLI

## License

MIT
