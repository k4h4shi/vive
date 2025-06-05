#!/usr/bin/env bash
# vive共通ユーティリティ

# カラー出力
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# REPO_ROOT計算：現在のディレクトリからGitリポジトリルートを検出
get_repo_root() {
    if ! command -v git &> /dev/null; then
        echo -e "${RED}エラー: gitがインストールされていません${NC}"
        exit 1
    fi
    
    if ! git rev-parse --is-inside-work-tree &> /dev/null; then
        echo -e "${RED}エラー: 現在のディレクトリはGitリポジトリではありません${NC}"
        echo "Gitリポジトリ内で vive を実行してください"
        exit 1
    fi
    
    REPO_ROOT=$(git rev-parse --show-toplevel)
    if [ -z "$REPO_ROOT" ]; then
        echo -e "${RED}エラー: Gitリポジトリルートが特定できません${NC}"
        exit 1
    fi
}

# 初期化時にREPO_ROOTを設定
get_repo_root

# プロジェクト名を取得（Gitリポジトリ名から）
get_project_name() {
    # リモートURLからリポジトリ名を取得
    local remote_url=$(git config --get remote.origin.url 2>/dev/null)
    if [ -n "$remote_url" ]; then
        # GitHubのURLからリポジトリ名を抽出
        # 例: git@github.com:user/repo.git -> repo
        # 例: https://github.com/user/repo.git -> repo
        PROJECT_NAME=$(echo "$remote_url" | sed -E 's/.*[\/:]([^\/]+)\.git$/\1/' | tr '[:upper:]' '[:lower:]')
    else
        # フォールバック: ディレクトリ名を使用
        PROJECT_NAME=$(basename "$REPO_ROOT" | tr '[:upper:]' '[:lower:]')
    fi
    
    if [ -z "$PROJECT_NAME" ]; then
        echo -e "${RED}エラー: プロジェクト名が特定できません${NC}"
        exit 1
    fi
}

# 初期化時にプロジェクト名を設定
get_project_name

# コマンド名（エイリアス対応）
cmd="vive"

# ヘルプ表示
show_help() {
    echo "vive - parallel AI fixer, alive in the shell"
    echo ""
    echo "コマンド:"
    echo "  $cmd fix <issue-number> [-s|--sync] [-k|--keep-worktree]  - Issue解決"
    echo "  $cmd batch <issue1,issue2,issue3>    - 複数Issue並行実行"
    echo "  $cmd issue [options]                 - Issue作成"
    echo "  $cmd sessions                        - セッション一覧"
    echo "  $cmd attach <issue-number>           - セッションにアタッチ"
    echo "  $cmd logs <issue-number> [-f|--follow]   - ログ表示（--followでリアルタイム監視）"
    echo "  $cmd watchdog <issue-number>         - watchdog復旧"
    echo "  $cmd cleanup [issue-number]          - Worktreeクリーンアップ"
    echo ""
    echo "例:"
    echo "  $cmd fix 42                          - Issue #42を非同期で解決"
    echo "  $cmd fix 42 -s                       - Issue #42を解決してアタッチ"
    echo "  $cmd fix 42 -k                       - 既存Worktreeを引き継いで実行"
    echo "  $cmd fix 42 -s -k                    - アタッチ + Worktree引き継ぎ"
    echo "  $cmd attach 42                       - Issue #42のセッションに接続"
    echo ""
    echo "Issue作成:"
    echo "  $cmd issue                           - 対話モード（非同期）"
    echo "  $cmd issue --title \"機能\" --auto-solve  - 作成後自動解決（非同期）"
    echo ""
    echo "オプション:"
    echo "  -s, --sync         tmux起動後にアタッチ（デフォルトは非同期で終了）"
    echo "  -k, --keep-worktree 既存Worktreeを削除せずに引き継ぐ"
    echo "  --auto-solve       Issue作成後に自動解決"
    echo ""
    echo "ヒント: デフォルトは非同期実行、Ctrl+B,Dでデタッチ、attachコマンドで再接続"
}

# tmuxチェック
check_tmux() {
    if ! command -v tmux &> /dev/null; then
        echo -e "${RED}エラー: tmuxがインストールされていません${NC}"
        echo "インストール方法:"
        echo "  macOS: brew install tmux"
        echo "  Ubuntu/Debian: sudo apt-get install tmux"
        exit 1
    fi
} 