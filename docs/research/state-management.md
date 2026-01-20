# State Management & Hysteresis

Raw process monitoring is noisy. An agent might briefly stop consuming CPU while waiting for a network response, or terminal output might pause. Direct mapping causes UI flickering.

## Reference Implementation
- [tmuxcc/src/monitor/task.rs](https://github.com/nyanko3141592/tmuxcc/blob/master/src/monitor/task.rs)

## 1. Hysteresis (Smoothing)

To prevent status flickering (e.g., Working -> Idle -> Working in 100ms), implement a hysteresis mechanism.

**Logic:**
If the status changes from **Working** to **Idle**, do not update immediately. Wait for a "cooldown" period (e.g., 2000ms). If it becomes Working again within that time, treat it as continuously Working.

### Implementation Reference

```rust
struct MonitorTask {
    // Track when each agent was last seen as "active"
    last_active: HashMap<String, Instant>,
    hysteresis_ms: u64, // 2000ms
}

impl MonitorTask {
    fn update_status(&mut self, target: &str, raw_status: AgentStatus) -> AgentStatus {
        let now = Instant::now();
        
        match raw_status {
            AgentStatus::Working | AgentStatus::Waiting => {
                self.last_active.insert(target.to_string(), now);
                raw_status
            },
            AgentStatus::Idle => {
                if let Some(last) = self.last_active.get(target) {
                    if now.duration_since(*last).as_millis() < self.hysteresis_ms as u128 {
                        // Still in cool-down, report as Working
                        return AgentStatus::Working;
                    }
                }
                AgentStatus::Idle
            }
            _ => raw_status
        }
    }
}
```

## 2. Spinner Detection in Window Title

Claude Code updates the window title (or tmux pane title) with spinners (`⠐⠇⠋⠙⠸`) even when stdout is not updating.

**Strategy:**
1. Fetch pane title via `tmux list-panes -F "#{pane_title}"`.
2. Check if title contains spinner characters.
3. If yes, force status to **Working**, overriding **Idle** from process check.

```rust
fn title_has_spinner(title: &str) -> bool {
    title.chars().any(|c| matches!(c, '⠿' | '⠇' | '⠋' | '⠙' | '⠸' | '⠴' | '⠦'))
}
```

## 3. Data Flow

```mermaid
graph TD
    A[Process Check (CPU/State)] --> D[Raw Status]
    B[Output Parsing (Regex)] --> D
    C[Title Check (Spinner)] --> D
    
    D --> E[Hysteresis Filter]
    E --> F[Final UI Status]
```
