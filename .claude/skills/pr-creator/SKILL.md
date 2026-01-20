# PR Creator Skill

Creates a Pull Request with a standardized title format that includes the Issue number at the beginning.

## Usage

Use this skill when the user asks to "create a PR", "submit a PR", or "open a pull request".

## Workflow

1.  **Analyze Context**
    *   Get the current branch name: `git branch --show-current`
    *   Extract the Issue number from the branch name (e.g., `feature/issue-123` -> `123`, `fix/456-bug` -> `456`).
    *   If no Issue number is found in the branch name, ask the user for the Issue number.

2.  **Determine PR Title**
    *   Format: `[#IssueNum] Type: Title`
    *   Example: `[#55] feat: Issue Pickerの実装`
    *   Use the Issue title from GitHub if possible: `gh issue view {IssueNum} --json title --jq .title`
    *   If the Issue title is available, use it directly (prefixed with `[#IssueNum]`).
    *   If not, infer a title from the branch name or recent commits.

3.  **Prepare PR Body**
    *   Must include "Closes #{IssueNum}" to link the PR to the Issue.
    *   Include a "Summary" section describing the changes.
    *   Include a "Test Plan" section describing how to verify the changes.

4.  **Create PR**
    *   Push the branch if needed: `git push -u origin HEAD`
    *   Create the PR using `gh pr create`.

    ```bash
    gh pr create --title "[#{IssueNum}] {Title}" --body "$(cat <<'EOF'
    Closes #{IssueNum}

    ## Summary
    {Summary}

    ## Test Plan
    - [ ] {Test Step 1}
    - [ ] {Test Step 2}
    EOF
    )"
    ```

## Example

**Scenario**: Current branch is `feature/issue-55`. Issue #55 title is "Implement Issue Picker".

**Command**:
```bash
gh pr create --title "[#55] feat: Implement Issue Picker" --body "$(cat <<'EOF'
Closes #55

## Summary
Implemented the Issue Picker feature to allow selecting issues from a list when creating a new task.

## Test Plan
- [ ] Run `cargo run` and press `n`.
- [ ] Select "Pick from Issue" and verify the list is displayed.
EOF
)"
```
