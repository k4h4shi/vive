---
description: Fetch all issues, prioritize them, and identify the critical path. Usage: /plan
---

# Plan & Prioritize Tasks

Use this command to analyze the current project state, prioritize open issues, and determine the critical path for development.

## 1. Fetch Issues

First, fetch all open issues from GitHub to understand the current workload.

```bash
gh issue list --limit 100 --state open --json number,title,labels,updatedAt,assignees
```

## 2. Analyze & Prioritize

Based on the fetched issues and the project goals (refer to `docs/` if needed), analyze the tasks.

### Output Format

Output the analysis in the following markdown format:

```markdown
# Project Plan Analysis

## 1. Critical Path
- **Goal**: [Current Milestone/Goal]
- **Blocking**: [List of blocking issues]
- **Path**: [Issue A] -> [Issue B] -> [Goal]

## 2. Priority List
| Priority | Issue | Title | Reasoning |
| :--- | :--- | :--- | :--- |
| P0 | #123 | Title | [Reason for high priority] |
| P1 | #124 | Title | [Reason] |
| P2 | #125 | Title | [Reason] |

## 3. Recommendations
- **Next Action**: [What to do next]
- **Missing Tasks**: [Any necessary tasks not yet tracked as issues]
```

## 3. Action

Ask the user if they want to:
1. Create new issues for missing tasks (using `gh issue create`).
2. Start working on the highest priority issue (using `/fix <issue_number>`).
