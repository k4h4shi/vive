#!/usr/bin/env bash
# vive Git操作関連

# Git状態チェック
check_git_status() {
    cd "$REPO_ROOT"
    
    # 未コミットの変更があるかチェック
    if ! git diff --quiet || ! git diff --cached --quiet; then
        echo -e "${YELLOW}警告: 未コミットの変更があります${NC}"
        echo "現在の変更をコミットまたはstashしてから実行してください"
        echo ""
        git status --short
        exit 1
    fi
    
    # mainブランチに移動
    current_branch=$(git branch --show-current)
    if [ "$current_branch" != "main" ]; then
        echo -e "${BLUE}mainブランチに切り替えます...${NC}"
        git checkout main
        git pull origin main
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