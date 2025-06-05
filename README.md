<div align="center">
    <picture>
      <img src="./LOGO.png" height="128" style="border-radius: 50%;">
    </picture>

# vive

vive — parallel AI fixer, **alive in the shell**

> A mac-native CLI that spins up multiple Claude-powered coding agents in parallel, each isolated in its own `git worktree` + `tmux` pane, with `expect`-driven auto-approval.

</div>

---

### ✨ Why vive?

| Pain point                                                  | How vive helps                                               |
| ----------------------------------------------------------- | ------------------------------------------------------------ |
| ❌ Giving AI tasks serially one by one → long waiting times | **Parallel agents** handle multiple Issues at once           |
| ❌ Interactive prompts asking "Yes/No?" every time          | `expect` catches known prompts and **auto-approves**         |
| ❌ Forgetting progress and returning to terminal is tedious | `tmux` **sessions** allow attaching/detaching anytime        |
| ❌ Manual branch switching accidents                        | `git worktree` creates **completely isolated** working trees |

---

### Key Features

- **Parallel Agents** – `vive batch 41 42 43` solves 3 Issues simultaneously
- **Attach / Detach** – `vive attach 42` jump into any session anytime, `Ctrl-b d` to background
- **Auto-approval** – Automatically ⏎ for familiar "Proceed?" prompts, warning sound + stop for unknown questions
- **Safe Git** – Full automation from worktree creation → work → PR creation → cleanup

---

### Quick Start

```bash

# 2) run your first parallel fix
vive fix 42            # Single Issue
vive batch 41 42 43    # Multiple Issues in parallel

# 3) watch or jump in
vive sessions          # List active sessions
vive attach 42         # Watch in real-time
```

---

### Dependencies

vive requires the following tools to be installed:

**Core Requirements:**

- **bash** – Shell scripting environment
- **tmux** – Terminal multiplexer for session management and parallel panes
- **expect** – Automation tool for handling interactive prompts and auto-approval
- **git** (with worktree support) – Version control with isolated working trees
- **GitHub CLI (gh)** – For automated PR creation and repository operations
- **Claude Code CLI** – AI coding assistant (from Anthropic)

**Node.js ecosystem:**

- **npm** – Package manager for dependency installation
- **node** – JavaScript runtime (required by npm)

**System utilities:**

- **rsync** – File synchronization for Claude config
- **ps/kill** – Process management for cleanup operations
- **grep/awk/sed** – Text processing for log analysis
- **watch** – Real-time monitoring (optional, for log following)

---

### Internal Architecture

```
vive
├── vive.sh           # CLI wrapper
├── lib/
│   ├── utils.sh      # common utilities and helpers
│   ├── git.sh        # git operations and worktree management
│   ├── issue.sh      # issue processing and management
│   ├── session.sh    # tmux session management
│   ├── cleanup.sh    # cleanup operations
│   └── batch.sh      # batch processing for multiple issues
├── watchdog.exp      # expect script (approval/error detection)
└── formula/          # Homebrew tap .rb files
```

- **tmux pane** launches Claude Code for each session
- **expect** watches stdout, auto-⏎ for known prompts, stop + warning for unknown ones
- **/tmp/vive\_\* flags** communicate completion/errors to parent process

---

### Roadmap

1. Extract to external repository
2. Add voice notifications with macOS `say` command
3. Add agent system for enhanced automation

---

### License

MIT

---
