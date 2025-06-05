#!/usr/bin/env bash
# vive exec command functionality

# Execute command in issue worktree
exec_in_worktree() {
    if [ "$#" -lt 2 ]; then
        echo -e "${RED}Usage: vive exec <IssueNumber> <command...>${NC}"
        echo -e "${YELLOW}Example: vive exec 42 npm install${NC}"
        echo -e "${YELLOW}Example: vive exec 42 git status${NC}"
        echo -e "${YELLOW}Example: vive exec 42 ls -la${NC}"
        return 1
    fi

    local issue_number="$1"
    shift
    local command_to_exec="$*"
    
    # Determine worktree directory path
    local worktree_dir="${REPO_ROOT}-issue-${issue_number}"
    
    # Check if worktree directory exists
    if [ ! -d "$worktree_dir" ]; then
        echo -e "${RED}Error: Worktree directory '$worktree_dir' not found${NC}"
        echo -e "${YELLOW}Make sure the issue worktree exists. You can create it with 'vive fix $issue_number'.${NC}"
        return 1
    fi

    echo -e "${BLUE}Executing in worktree '$worktree_dir': '$command_to_exec'${NC}"
    echo -e "${YELLOW}========================================${NC}"
    
    # Execute command in worktree directory
    (cd "$worktree_dir" && eval "$command_to_exec")
    local exit_code=$?
    
    echo -e "${YELLOW}========================================${NC}"
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}✅ Command completed successfully${NC}"
    else
        echo -e "${RED}❌ Command failed with exit code $exit_code${NC}"
    fi
    
    return $exit_code
}

# Interactive shell in worktree
shell_in_worktree() {
    if [ "$#" -lt 1 ]; then
        echo -e "${RED}Usage: vive shell <IssueNumber>${NC}"
        echo -e "${YELLOW}Example: vive shell 42${NC}"
        return 1
    fi

    local issue_number="$1"
    
    # Determine worktree directory path
    local worktree_dir="${REPO_ROOT}-issue-${issue_number}"
    
    # Check if worktree directory exists
    if [ ! -d "$worktree_dir" ]; then
        echo -e "${RED}Error: Worktree directory '$worktree_dir' not found${NC}"
        echo -e "${YELLOW}Make sure the issue worktree exists. You can create it with 'vive fix $issue_number'.${NC}"
        return 1
    fi

    echo -e "${GREEN}Starting interactive shell in worktree for issue #$issue_number${NC}"
    echo -e "${BLUE}Working directory: $worktree_dir${NC}"
    echo -e "${YELLOW}Type 'exit' to return to the original directory${NC}"
    echo ""
    
    # Start interactive shell in worktree directory
    (cd "$worktree_dir" && exec $SHELL)
} 