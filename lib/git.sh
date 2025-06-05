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
    local issue_num=$1
    local branch_name="issue-${issue_num}"
    local worktree_name="${PROJECT_NAME}-issue-${issue_num}"
    local worktree_dir="$REPO_ROOT/../$worktree_name"
    
    # Delete existing worktree
    if [ -d "$worktree_dir" ]; then
        git worktree remove "$worktree_dir" --force 2>/dev/null || true
    fi
    
    # Create new worktree
    git worktree add "$worktree_dir" -b "$branch_name" || \
        git worktree add "$worktree_dir" "$branch_name"
    
    # Sync Claude config
    if [ -f "$REPO_ROOT/.cursor/mcp.json" ]; then
        mkdir -p "$worktree_dir/.cursor"
        rsync -av "$REPO_ROOT/.cursor/mcp.json" "$worktree_dir/.cursor/" >/dev/null 2>&1
        echo -e "${GREEN}Claude configuration sync completed${NC}"
    fi
    
    # Check Claude Code configuration
    if [ -f "$worktree_dir/.cursor/mcp.json" ]; then
        echo -e "${GREEN}Claude Code configuration check completed${NC}"
    else
        echo -e "${RED}Error: Claude Code configuration file not found${NC}"
        exit 1
    fi
    
    echo "$worktree_dir"
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