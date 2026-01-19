---
description: Clean up worktrees and sessions. Usage: /cleanup
---

# Cleanup Worktree (Vive)

## 1. List Active Sessions

```bash
./vive list
```

## 2. Cleanup a Session

To clean up a specific issue session (closes window and removes worktree):

```bash
./vive cleanup <ISSUE_ID>
```

Example:
```bash
./vive cleanup 123
```
