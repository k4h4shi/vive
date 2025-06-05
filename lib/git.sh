#!/usr/bin/env bash
# vive git operations

# Check git status
check_git_status() {
    if [ -n "$(git status --porcelain)" ]; then
        echo
        echo -e "${YELLOW}Warning: You have uncommitted changes${NC}"
        echo
        git status --short
        echo
        
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
}

# Create and setup worktree
setup_worktree() {
    local issue_num="$1"
    local base_branch="$2"
    local worktree_base_dir="$REPO_ROOT/.vive/issues"
    local worktree_name="${PROJECT_NAME}-issue-${issue_num}"
    local worktree_dir="$worktree_base_dir/$issue_num"
    local branch_name="${PROJECT_NAME}-issue-${issue_num}"

    echo -e "${BLUE}Setting up worktree for issue #$issue_num...${NC}"
    echo -e "${BLUE}Base branch: $base_branch${NC}"
    echo -e "${BLUE}Worktree directory: $worktree_dir${NC}"
    echo -e "${BLUE}Branch name: $branch_name${NC}"

    # Ensure the base directory for worktrees exists
    mkdir -p "$worktree_base_dir"

    # Check if branch already exists
    if git show-ref --quiet "refs/heads/$branch_name"; then
        echo -e "${YELLOW}Branch '$branch_name' already exists.${NC}"
    else
        echo -e "${BLUE}Creating branch '$branch_name' from '$base_branch'...${NC}"
        git branch "$branch_name" "$base_branch"
        if [ $? -ne 0 ]; then 
            echo -e "${RED}Error creating branch '$branch_name' from '$base_branch'.${NC}"
            echo -e "${RED}Please ensure '$base_branch' exists and is up to date.${NC}"
            return 1
        fi
    fi

    # Check if worktree directory already exists
    if [ -d "$worktree_dir" ]; then
        echo -e "${YELLOW}Worktree directory '$worktree_dir' already exists.${NC}"
        # Optional: could add logic to check if it's a valid worktree and linked to the correct branch
        # For now, assume it's okay if it exists.
        echo -e "${GREEN}Worktree for issue #$issue_num already set up at $worktree_dir.${NC}"
        return 0
    fi

    echo -e "${BLUE}Creating worktree at '$worktree_dir' for branch '$branch_name'...${NC}"
    # Cleanup potential leftover from a failed previous attempt
    git worktree prune
    # Add the worktree
    if git worktree add -f "$worktree_dir" "$branch_name"; then
        echo -e "${GREEN}Worktree for issue #$issue_num created successfully at $worktree_dir${NC}"
        echo -e "${GREEN}Branch '$branch_name' is checked out in this worktree.${NC}"
        return 0
    else
        echo -e "${RED}Error creating worktree at '$worktree_dir' for branch '$branch_name'.${NC}"
        # Attempt to clean up a failed worktree add
        rm -rf "$worktree_dir"
        # It's also possible the branch was created but worktree add failed. 
        # Depending on policy, could remove branch here, or leave for manual cleanup.
        return 1
    fi
}

# Get the current worktree directory if inside a vive worktree
get_current_vive_worktree_dir() {
    local current_git_dir
    current_git_dir=$(git rev-parse --git-dir 2>/dev/null)
    if [[ "$current_git_dir" == *.git/worktrees/* ]]; then
        # Inside a worktree, .git is a file pointing to the main .git dir
        # readlink -f on .git file, then dirname to get the worktree root
        local git_file_path
        git_file_path=$(git rev-parse --git-path .git)
        if [ -f "$git_file_path" ]; then # It is indeed a file for worktrees
            # We want the directory containing this .git file
            dirname "$git_file_path"
            return 0
        fi
    elif [[ "$PWD" == "$REPO_ROOT/.vive/issues/"* ]]; then
         # Fallback: if PWD is inside the .vive/issues structure
         # This is less robust but can be a fallback.
         # Extract the <num> part and reconstruct
         local potential_issue_num=$(echo "$PWD" | sed -n "s|^$REPO_ROOT/.vive/issues/\([0-9]*\).*|\1|p")
         if [ -n "$potential_issue_num" ]; then
            echo "$REPO_ROOT/.vive/issues/$potential_issue_num"
            return 0
         fi 
    fi
    # Not in a vive worktree or failed to determine
    echo "$REPO_ROOT" # Default to REPO_ROOT if not in a specific worktree
    return 1
}

# Get the issue number from the current branch name if it matches the pattern
get_issue_num_from_branch() {
    local branch_name
    branch_name=$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --abbrev-ref HEAD 2>/dev/null)
    if [[ "$branch_name" =~ ^${PROJECT_NAME}-issue-([0-9]+)$ ]]; then # Use $PROJECT_NAME
        echo "${BASH_REMATCH[1]}"
    else
        echo ""
    fi
}

# Claude Code初期化チェック
check_claude_init() {
    local work_dir="$1"
    
    echo -e "${BLUE}Claude Code設定を同期中...${NC}"
    
    # .claudeディレクトリが存在しない場合は作成
    if [ ! -d "$work_dir/.claude" ]; then
        mkdir -p "$work_dir/.claude"
    fi
    
    # .claude設定ディレクトリ全体を同期
    if [ -d "$REPO_ROOT/.claude" ]; then
        rsync -av --delete "$REPO_ROOT/.claude/" "$work_dir/.claude/" > /dev/null 2>&1
        echo -e "${GREEN}Claude設定同期完了${NC}"
    fi
    
    # 設定内容を検証
    if [ -f "$work_dir/.claude/settings.json" ]; then
        echo -e "${GREEN}Claude Code設定確認完了${NC}"
    else
        echo -e "${RED}エラー: Claude Code設定ファイルが見つかりません${NC}"
        echo -e "${YELLOW}Claude Code設定を初期化します...${NC}"
        cd "$work_dir"
        claude code init
    fi
} 