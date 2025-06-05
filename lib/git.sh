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
    
    # Sync MCP config from main repo to worktree
    local repo_mcp_config_file
    repo_mcp_config_file=$(get_mcp_config_path) # Get full path from config

    local mcp_relative_path
    mcp_relative_path=$(get_config ".mcp.configPath" ".vive/mcp.json") # Get relative path for worktree structure

    if [ -f "$repo_mcp_config_file" ]; then
        local worktree_mcp_target_path="$worktree_dir/$mcp_relative_path"
        local worktree_mcp_target_dir
        worktree_mcp_target_dir=$(dirname "$worktree_mcp_target_path")

        mkdir -p "$worktree_mcp_target_dir"
        rsync -av "$repo_mcp_config_file" "$worktree_mcp_target_path" >/dev/null 2>&1
        echo -e "${GREEN}MCP configuration sync completed to $worktree_mcp_target_path${NC}"
    else
        echo -e "${YELLOW}MCP configuration file not found in main repository: $repo_mcp_config_file. Skipping sync.${NC}"
    fi

    # Check MCP configuration in worktree
    local worktree_mcp_check_path="$worktree_dir/$mcp_relative_path"
    if [ -f "$worktree_mcp_check_path" ]; then
        echo -e "${GREEN}MCP configuration check completed in worktree: $worktree_mcp_check_path${NC}"
    else
        echo -e "${RED}Error: MCP configuration file not found in worktree: $worktree_mcp_check_path${NC}"
        # Optionally, exit 1 if MCP config is critical for worktree operation
        # exit 1
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