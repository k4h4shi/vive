# Rich UI Tree Visualization (Vive Optimized)

For Vive, we prioritize **project-based grouping** and **information density**. Unlike general-purpose monitoring tools, Vive users need to scan multiple parallel tasks quickly.

## Reference Implementation
- [tmuxcc/src/ui/components/agent_tree.rs](https://github.com/nyanko3141592/tmuxcc/blob/master/src/ui/components/agent_tree.rs)

## 1. Visual Hierarchy (Target)

**Design Goal**: Shallow tree, dense information.

```text
▼ mechanix (★)
  ├─ feature/ui-fix .... ⚙ Working (Fixing button styles...)
  └─ fix/bug-123 ....... ✎ Wait: Edit src/main.rs?

▼ vive
  └─ main .............. • Idle
```

### Key Differences from TmuxCC
1.  **Root is Project**: Grouping by Git repository, not Tmux session.
2.  **Inline Status**: Status icons and details are displayed on the same line as the Worktree name, reducing vertical scrolling.
3.  **No Deep Nesting**: Sub-agents (if any) are summarized in the detail text or hidden, rather than creating new tree branches.

## 2. Tree Construction Algorithm

We need a flatter rendering loop than the generic recursive tree.

### Logic

1.  **Sort Projects**:
    - **Primary Key**: Is Favorite? (Favorites go to top)
    - **Secondary Key**: Project Name (Alphabetical)
2.  Iterate `Projects`.
3.  Print Project Header (`▼ Name` + `★` indicator).
4.  Iterate `Worktrees` in Project.
5.  Determine Prefix: `├─` or `└─` based on whether it's the last worktree.
6.  **Layout the Line**:
    - `[Prefix]` `[Branch Name]` `[Padding]` `[Icon]` `[Status Text]`

### Sample Implementation (Rust)

```rust
fn render_sidebar(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut items = Vec::new();
    
    // Sorting Logic: Favorites First
    let mut sorted_projects: Vec<&Project> = state.projects.iter().collect();
    sorted_projects.sort_by(|a, b| {
        let a_fav = state.is_favorite(&a.name);
        let b_fav = state.is_favorite(&b.name);
        match (a_fav, b_fav) {
            (true, false) => std::cmp::Ordering::Less, // a comes first
            (false, true) => std::cmp::Ordering::Greater, // b comes first
            _ => a.name.cmp(&b.name), // Alphabetical fallback
        }
    });

    for project in sorted_projects {
        // Project Header
        let fav_icon = if state.is_favorite(&project.name) { "★ " } else { "" };
        items.push(ListItem::new(format!("▼ {}{}", fav_icon, project.name)));
        
        for (i, wt) in project.worktrees.iter().enumerate() {
            let is_last = i == project.worktrees.len() - 1;
            let prefix = if is_last { "└─" } else { "├─" };
            
            let status = state.get_status(&wt.tmux_target);
            let icon = status.icon(); // ⚙, ✎, etc.
            let detail = status.detail(); // "Fixing..." or "Edit src/main.rs"
            
            // Fixed width alignment for branch names
            let branch_display = format!("{:<20}", wt.branch);
            
            let line = Line::from(vec![
                Span::raw("  "),
                Span::raw(prefix),
                Span::styled(branch_display, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::raw(icon),
                Span::raw(" "),
                Span::styled(detail, Style::default().fg(Color::DarkGray)),
            ]);
            
            items.push(ListItem::new(line));
        }
    }
}
```

## 3. Indicators & Icons

Using shapes and symbols rather than just color for better visibility.

| Status | Icon | Color | Meaning |
| :--- | :--- | :--- | :--- |
| **Working** | `⚙` (Gear) | Yellow | Processing (high CPU or spinner) |
| **Wait (Edit)** | `✎` (Pencil) | Red (Bold) | Waiting for file edit approval |
| **Wait (Shell)** | `>` (Prompt) | Red | Waiting for shell command approval |
| **Wait (Other)** | `?` (Question) | Magenta | Waiting for general input/answer |
| **Idle** | `•` (Bullet) | DarkGray | Session active but idle |
| **Success** | `✓` (Check) | Green | Task completed |
| **Error** | `✖` (Cross) | Red | Process terminated/error |

## 4. Context Bar (Optional)

If space permits, a mini context bar can be added to the end of the line.

`... ✎ Wait (Edit)  [██░░]`
