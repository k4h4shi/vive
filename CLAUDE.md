## Architecture Overview
- **Core**: Rust + Ratatui for TUI interface.
- **Agent**: Integrated with Claude Code via terminal interface.
- **Architecture**:
  - `src/main.rs`: Entry point.
  - `src/ui.rs`: UI rendering logic (Ratatui).
  - `src/state.rs`: Application state management.
  - `src/process.rs`: Subprocess management (Claude, Tmux).
  - `src/monitor.rs`: Output monitoring and parsing.
  - `src/tmux.rs`: Tmux control integration.

## Key Guidelines
- **Language**: Rust (Safe, idiomatic code).
- **Styling**: TUI based (Ratatui).
- **Testing**: TDD is preferred. Write tests before implementation.
- **Docs**: `docs/` contains requirements and specifications.

## Slash Commands
- **/plan**: Plan & Prioritize Tasks.

## Rule References
- **Skills**: `.claude/skills/`