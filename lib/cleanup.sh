#!/usr/bin/env bash
# vive cleanup functionality

# Function to clean up worktrees and tmux sessions
run_cleanup() {
    local target_issue="$1"
    
    if [ -n "$target_issue" ]; then
        echo -e "${BLUE}Starting cleanup process for issue #$target_issue...${NC}"
    else
        echo -e "${BLUE}Starting cleanup process...${NC}"
    fi

    local worktree_base_dir="$REPO_ROOT/.vive/issues"
    local current_branch
    current_branch=$(git rev-parse --abbrev-ref HEAD)

    # 1. Clean up worktrees
    echo -e "${YELLOW}Cleaning up worktrees...${NC}"
    if [ -d "$worktree_base_dir" ]; then
        find "$worktree_base_dir" -mindepth 1 -maxdepth 1 -type d | while read -r worktree_path; do
            local worktree_name=$(basename "$worktree_path")
            local issue_number=${worktree_name} # Assuming worktree_name is just the issue number
            
            # Skip if target_issue is specified and this is not the target
            if [ -n "$target_issue" ] && [ "$issue_number" != "$target_issue" ]; then
                continue
            fi
            
            # For session name, prepend with project name and 'issue-'
            local session_name="${PROJECT_NAME}-issue-${issue_number}"

            # Check if it's the current worktree's issue to avoid self-deletion if running from within a vive worktree
            if [[ "$current_branch" == *"/$issue_number" ]]; then # Check if current branch is related to this issue worktree
                echo -e "${YELLOW}Skipping active worktree for current branch: $worktree_path (branch: $current_branch)${NC}"
                continue
            fi

            # Check if a tmux session exists for this worktree
            if tmux has-session -t "$session_name" 2>/dev/null; then
                echo -e "${YELLOW}Skipping worktree with active tmux session: $worktree_path (session: $session_name)${NC}"
            else
                echo -e "${BLUE}Removing worktree: $worktree_path${NC}"
                git worktree remove --force "$worktree_path" || echo -e "${RED}Failed to remove worktree $worktree_path, it might be locked or already removed.${NC}"
                # Also remove the branch if it exists and follows the pattern ${PROJECT_NAME}-issue-*
                local branch_name="${PROJECT_NAME}-issue-${issue_number}"
                if git show-ref --quiet "refs/heads/$branch_name"; then
                    echo -e "${BLUE}Removing associated branch: $branch_name${NC}"
                    git branch -D "$branch_name" || echo -e "${RED}Failed to remove branch $branch_name${NC}"
                fi
            fi
        done
    else
        echo -e "${GREEN}Worktree base directory $worktree_base_dir not found. No worktrees to clean.${NC}"
    fi

    # 2. Clean up tmux sessions that don't have a corresponding worktree directory
    echo -e "${YELLOW}Cleaning up stale tmux sessions...${NC}"
    tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" | while read -r session_name; do # Use $PROJECT_NAME
        local issue_number=${session_name#${PROJECT_NAME}-issue-} # Use $PROJECT_NAME
        
        # Skip if target_issue is specified and this is not the target
        if [ -n "$target_issue" ] && [ "$issue_number" != "$target_issue" ]; then
            continue
        fi
        
        local worktree_dir_check="$worktree_base_dir/$issue_number"
        if [ ! -d "$worktree_dir_check" ]; then
            echo -e "${BLUE}Killing stale tmux session '$session_name' as its worktree directory '$worktree_dir_check' is missing.${NC}"
            tmux kill-session -t "$session_name"
        fi
    done

    # 3. Clean up old log files and completion markers for non-existent sessions
    echo -e "${YELLOW}Cleaning up old log files and completion markers...${NC}"
    find /tmp -maxdepth 1 -type f \( -name "claude_session_${PROJECT_NAME}-issue-*.log" -o -name "claude_completed_${PROJECT_NAME}-issue-*" \) | while read -r file_path; do # Use $PROJECT_NAME
        local base_name=$(basename "$file_path")
        local session_part
        if [[ "$base_name" == claude_session_* ]]; then
            session_part=${base_name#claude_session_}
            session_part=${session_part%_*.log}
        elif [[ "$base_name" == claude_completed_* ]]; then
            session_part=${base_name#claude_completed_}
        fi
        
        if [ -n "$session_part" ]; then # Ensure session_part is not empty
             # $session_part should be like ${PROJECT_NAME}-issue-NUMBER
            local issue_number_from_file=${session_part#${PROJECT_NAME}-issue-}
            
            # Skip if target_issue is specified and this is not the target
            if [ -n "$target_issue" ] && [ "$issue_number_from_file" != "$target_issue" ]; then
                continue
            fi
            
            if ! tmux has-session -t "$session_part" 2>/dev/null; then
                # Further check if the worktree also doesn't exist before removing logs
                local worktree_dir_for_log_check="$worktree_base_dir/$issue_number_from_file"
                if [ ! -d "$worktree_dir_for_log_check" ]; then
                    echo -e "${BLUE}Removing stale file for non-existent session/worktree '$session_part': $file_path${NC}"
                    rm -f "$file_path"
                else
                    echo -e "${YELLOW}Keeping file for existing worktree (though session is dead): $file_path (worktree: $worktree_dir_for_log_check)${NC}"
                fi
            fi
        fi
    done

    # 4. Clean up stray expect processes (more robust check)
    echo -e "${YELLOW}Cleaning up stray expect processes...${NC}"
    # Looking for processes running watchdog.exp and having a VIVE_SESSION_NAME like ${PROJECT_NAME}-issue-*
    # This is more robust than just grepping for session name in ps output.
    pgrep -af "expect .*watchdog.exp" | while read -r pid process_cmd; do
        # Extract VIVE_SESSION_NAME from the process's environment
        # This requires appropriate permissions (e.g., running as the same user)
        # Using `strings` on /proc/[pid]/environ is a common Linux method.
        # For macOS, `ps eww -p [pid]` can show environment, but parsing is harder.
        # Let's try a simpler grep on the command line first, as VIVE_SESSION_NAME is an argument to expect.
        if echo "$process_cmd" | grep -qE "${PROJECT_NAME}-issue-[0-9]+"; then # Use $PROJECT_NAME
            local session_name_in_cmd=$(echo "$process_cmd" | grep -oE "${PROJECT_NAME}-issue-[0-9]+") # Use $PROJECT_NAME
            local issue_number_from_cmd=${session_name_in_cmd#${PROJECT_NAME}-issue-}
            
            # Skip if target_issue is specified and this is not the target
            if [ -n "$target_issue" ] && [ "$issue_number_from_cmd" != "$target_issue" ]; then
                continue
            fi
            
            if [ -n "$session_name_in_cmd" ] && ! tmux has-session -t "$session_name_in_cmd" 2>/dev/null; then
                echo -e "${BLUE}Killing stray expect process (PID: $pid) for dead tmux session '$session_name_in_cmd'${NC}"
                kill "$pid" 2>/dev/null || true 
                sleep 0.1
                kill -9 "$pid" 2>/dev/null || true
            fi
        fi 
    done

    echo -e "${GREEN}Cleanup process completed.${NC}"
} 