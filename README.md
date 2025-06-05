<div align="center">
    <picture>
      <img src="./LOGO.png" height="128" style="border-radius: 50%;">
    </picture>

# vive

vive — parallel AI fixer, **alive in the shell**

> A mac-native CLI that spins up multiple Claude-powered coding agents in parallel, each isolated in its own `git worktree` + `tmux` pane, with `expect`-driven auto-approval.

[![GitHub](https://img.shields.io/badge/GitHub-k4h4shi%2Fvive-blue)](https://github.com/k4h4shi/vive)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

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

### 🚀 Installation

```bash
# Clone the repository
git clone https://github.com/k4h4shi/vive.git
cd vive

# Option 1: Use the installation script (recommended)
./install.sh

# Option 2: Manual installation
chmod +x vive.sh
sudo ln -s $(pwd)/vive.sh /usr/local/bin/vive

# Option 3: Add to PATH
export PATH="$PATH:$(pwd)"
```

**Uninstall:**

```bash
./install.sh --uninstall
```

---

### 🛠️ Key Features

- **Parallel Agents** – `vive batch 41 42 43` solves 3 Issues simultaneously
- **Multi-Project Support** – Works with any Git repository, not just specific projects
- **Attach / Detach** – `vive attach 42` jump into any session anytime, `Ctrl-b d` to background
- **Auto-approval** – Automatically ⏎ for familiar "Proceed?" prompts, warning sound + stop for unknown questions
- **Safe Git** – Full automation from worktree creation → work → PR creation → cleanup
- **Dynamic Project Detection** – Automatically detects repository root and project name

---

### 📖 Quick Start

```bash
# Navigate to any Git repository
cd /path/to/your/project

# Run vive commands
vive fix 42            # Single Issue
vive batch 41 42 43    # Multiple Issues in parallel

# Watch or jump in
vive sessions          # List active sessions
vive attach 42         # Watch in real-time
```

### 🌍 Multi-Project Usage

vive automatically detects the current Git repository and adapts its behavior:

```bash
# In project "myapp"
cd ~/projects/myapp
vive fix 123
# Creates: myapp-issue-123 worktree and tmux session

# In project "website"
cd ~/projects/website
vive fix 456
# Creates: website-issue-456 worktree and tmux session
```

---

### 📚 Command Reference

| Command                  | Description                               | Example               |
| ------------------------ | ----------------------------------------- | --------------------- |
| `vive fix <issue>`       | Fix a single issue                        | `vive fix 42`         |
| `vive batch <issues...>` | Fix multiple issues in parallel           | `vive batch 41 42 43` |
| `vive sessions`          | List all active vive sessions             | `vive sessions`       |
| `vive attach <issue>`    | Attach to a running session               | `vive attach 42`      |
| `vive logs <issue>`      | Show logs for a specific issue            | `vive logs 42`        |
| `vive cleanup`           | Clean up completed sessions and worktrees | `vive cleanup`        |
| `vive cleanup all`       | Force cleanup all sessions and worktrees  | `vive cleanup all`    |

**Inside tmux sessions:**

- `Ctrl-b d` – Detach from session (runs in background)
- `Ctrl-b [` – Enter scroll mode (use arrow keys to navigate)
- `q` – Exit scroll mode

---

### 📦 Dependencies

vive requires the following tools to be installed:

**🔧 Core Requirements:**

- **bash** – Shell scripting environment
- **tmux** – Terminal multiplexer for session management and parallel panes
- **expect** – Automation tool for handling interactive prompts and auto-approval
- **git** (with worktree support) – Version control with isolated working trees
- **GitHub CLI (gh)** – For automated PR creation and repository operations
- **claude** – Claude AI command-line interface

**📦 Package Management:**

- **npm** – Package manager for dependency installation
- **node** – JavaScript runtime (required by npm)

**🛠️ System Utilities:**

- **rsync** – File synchronization for Claude config
- **ps/kill** – Process management for cleanup operations
- **grep/awk/sed** – Text processing for log analysis
- **watch** – Real-time monitoring (optional, for log following)

**macOS Installation Example:**

```bash
# Install core dependencies via Homebrew
brew install tmux expect gh node

# Install claude CLI
npm install -g @anthropic-ai/claude-cli
```

---

### 🔧 Internal Architecture

```
vive/
├── vive.sh           # Main CLI entry point
├── lib/
│   ├── utils.sh      # Common utilities and pwd-based repo detection
│   ├── git.sh        # Git operations and worktree management
│   ├── issue.sh      # Issue processing and management
│   ├── session.sh    # Tmux session management
│   ├── cleanup.sh    # Cleanup operations
│   └── batch.sh      # Batch processing for multiple issues
├── watchdog.exp      # Expect script (approval/error detection)
├── README.md         # This file
└── LICENSE           # MIT License
```

**🔑 Key Components:**

- **Repository Detection**: Uses `git rev-parse --show-toplevel` to find repo root
- **Project Name**: Extracted from Git remote URL for unique worktree/session names
- **tmux pane**: Launches Claude for each session in isolated environment
- **expect script**: Watches stdout, auto-approves known prompts, alerts on unknown
- **Flag system**: Uses `/tmp/vive_*` files for inter-process communication

---

### 🎯 Use Cases

1. **Fix Multiple Bugs in Parallel**

   ```bash
   vive batch 101 102 103 104 105
   # Fix 5 bugs simultaneously, each in its own worktree
   ```

2. **Long-Running Refactoring**

   ```bash
   vive fix 200
   vive attach 200  # Check progress
   # Ctrl-b d        # Detach and let it continue
   ```

3. **Cleanup After Work**
   ```bash
   vive sessions    # Check what's running
   vive cleanup     # Clean up completed work
   ```

---

### 📋 Roadmap

- [x] Extract to external repository
- [x] Add multi-project support with dynamic repository detection
- [ ] Add voice notifications with macOS `say` command
- [ ] Add agent system for enhanced automation
- [ ] Create Homebrew formula for easier installation
- [ ] Add support for other AI coding assistants
- [ ] Add configuration file support (.viverc)

---

### 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

### 📄 License

MIT License - See [LICENSE](LICENSE) file for details

---

### 🙏 Acknowledgments

- Built for developers who value their time
- Inspired by the need to parallelize AI-assisted development
- Special thanks to the Claude API for making this possible

---
