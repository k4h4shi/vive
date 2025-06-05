#!/usr/bin/env bash
# vive cleanup operations

# Worktree cleanup
cleanup_worktrees() {
    local issue_number="$1"
    
    if [ -n "$issue_number" ]; then
        # Individual issue cleanup
        echo -e "${YELLOW}This will cleanup worktree and Claude Code processes for Issue #${issue_number}.${NC}"
        echo -e "${YELLOW}This operation cannot be undone. Continue? (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}Cleanup cancelled${NC}"
            exit 0
        fi
        
        echo -e "${BLUE}Starting cleanup for Issue #${issue_number}...${NC}"
        
        # Terminate tmux session for specific issue
        echo -e "${YELLOW}Terminating tmux sessions...${NC}"
        local session_name="${PROJECT_NAME}-issue-${issue_number}"
        if tmux has-session -t "$session_name" 2>/dev/null; then
            echo -e "${YELLOW}Terminating tmux session: $session_name${NC}"
            tmux kill-session -t "$session_name" 2>/dev/null || true
        else
            echo -e "${GREEN}tmux session $session_name not found${NC}"
        fi
        
        # Terminate expect process for specific issue
        echo -e "${YELLOW}Terminating expect processes...${NC}"
        expect_pids=$(ps aux | grep -E "expect.*${PROJECT_NAME}-issue-${issue_number}" | grep -v grep | awk '{print $2}' || true)
        if [ -n "$expect_pids" ]; then
            echo -e "${YELLOW}Terminating expect processes: $expect_pids${NC}"
            kill $expect_pids 2>/dev/null || true
            sleep 1
            kill -9 $expect_pids 2>/dev/null || true
        fi
        
        # Clean up related temporary files
        echo -e "${YELLOW}Cleaning up temporary files...${NC}"
        rm -f /tmp/claude_prompt_${issue_number}.txt 2>/dev/null || true
        rm -f /tmp/claude_session_${PROJECT_NAME}-issue-${issue_number}_*.log 2>/dev/null || true
        rm -f /tmp/claude_completed_${PROJECT_NAME}-issue-${issue_number} 2>/dev/null || true
        
        # Terminate Claude Code process for specific issue
        echo -e "${YELLOW}Terminating Claude Code processes...${NC}"
        pkill -f "claude.*Issue.*${issue_number}" || true
        sleep 2
        
        cd "$REPO_ROOT"
        
        # Remove specific worktree
        local worktree_dir="$REPO_ROOT/../${PROJECT_NAME}-issue-${issue_number}"
        if [ -d "$worktree_dir" ]; then
            echo -e "${YELLOW}Removing worktree: $worktree_dir${NC}"
            git worktree remove --force "$worktree_dir" || true
        fi
        
        # Remove issue branch
        local branch_name="issue-${issue_number}"
        if git show-ref --verify --quiet refs/heads/"$branch_name"; then
            echo -e "${YELLOW}Removing branch: $branch_name${NC}"
            git branch -D "$branch_name" || true
        fi
        
        echo -e "${GREEN}Cleanup completed for Issue #${issue_number}${NC}"
    else
        # Full cleanup (existing process)
        echo -e "${YELLOW}This will cleanup all worktrees and Claude Code processes.${NC}"
        echo -e "${YELLOW}This operation cannot be undone. Continue? (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}Cleanup cancelled${NC}"
            exit 0
        fi
        
        echo -e "${BLUE}Starting worktree cleanup...${NC}"
        
        # Terminate tmux sessions
        echo -e "${YELLOW}Terminating tmux sessions...${NC}"
        for session in $(tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" || true); do
            echo -e "${YELLOW}Terminating tmux session: $session${NC}"
            tmux kill-session -t "$session" 2>/dev/null || true
        done
        
        # Terminate expect processes
        echo -e "${YELLOW}Terminating expect processes...${NC}"
        expect_pids=$(ps aux | grep -E "expect.*${PROJECT_NAME}-issue-" | grep -v grep | awk '{print $2}' || true)
        if [ -n "$expect_pids" ]; then
            echo -e "${YELLOW}Terminating expect processes: $expect_pids${NC}"
            kill $expect_pids 2>/dev/null || true
            sleep 1
            # Force kill with KILL signal if still remaining
            kill -9 $expect_pids 2>/dev/null || true
        else
            echo -e "${GREEN}No expect processes found${NC}"
        fi
        
        # Clean up temporary files
        echo -e "${YELLOW}Cleaning up temporary files...${NC}"
        rm -f /tmp/claude_prompt_*.txt 2>/dev/null || true
        rm -f /tmp/claude_expect_*.exp 2>/dev/null || true
        rm -f /tmp/claude_session_*.log 2>/dev/null || true
        rm -f /tmp/expect_resume_output_*.log 2>/dev/null || true
        rm -f /tmp/claude_completed_* 2>/dev/null || true
        
        # Terminate Claude Code processes
        echo -e "${YELLOW}Terminating Claude Code processes...${NC}"
        pkill -f "claude.*Issue" || true
        sleep 2
        
        # Force terminate remaining Claude Code processes (exclude current script)
        current_pid=$$
        remaining_pids=$(ps aux | grep -i claude | grep -v grep | grep -v "claude-code.sh" | awk '{print $2}' | grep -v "^${current_pid}$" | xargs)
        if [ -n "$remaining_pids" ]; then
            echo -e "${YELLOW}Force terminating remaining processes: $remaining_pids${NC}"
            kill $remaining_pids 2>/dev/null || true
            sleep 1
            # Force kill with KILL signal if still remaining
            kill -9 $remaining_pids 2>/dev/null || true
        fi
        
        cd "$REPO_ROOT"
        
        # Get all worktrees (except main directory)
        git worktree list --porcelain | grep -E "^worktree " | grep -v "^worktree $REPO_ROOT$" | while read -r line; do
            worktree_path=$(echo "$line" | sed 's/^worktree //')
            if [ -d "$worktree_path" ]; then
                echo -e "${YELLOW}Removing worktree: $worktree_path${NC}"
                git worktree remove --force "$worktree_path" || true
            fi
        done
        
        # Remove issue branches
        git branch | grep -E "^\s*issue-" | while read -r branch; do
            branch=$(echo "$branch" | xargs)  # trim whitespace
            echo -e "${YELLOW}Removing branch: $branch${NC}"
            git branch -D "$branch" || true
        done
        
        # Remove prompt branches
        git branch | grep -E "^\s*prompt-" | while read -r branch; do
            branch=$(echo "$branch" | xargs)  # trim whitespace
            echo -e "${YELLOW}Removing branch: $branch${NC}"
            git branch -D "$branch" || true
        done
        
        # Final check (exclude current script)
        remaining_claude=$(ps aux | grep -i claude | grep -v grep | grep -v "claude-code.sh" | grep -v "^[[:space:]]*${current_pid}" | wc -l)
        if [ "$remaining_claude" -eq 0 ]; then
            echo -e "${GREEN}✅ All Claude Code processes terminated${NC}"
        else
            echo -e "${YELLOW}⚠️  Some Claude Code processes may still be running${NC}"
            ps aux | grep -i claude | grep -v grep | grep -v "claude-code.sh"
        fi
        
        echo -e "${GREEN}Cleanup completed${NC}"
    fi
} 