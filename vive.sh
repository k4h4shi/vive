#!/usr/bin/env bash
# vive CLI - リファクタリング版

set -e

# ライブラリファイルの読み込み
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 依存関係の読み込み順序（重要：依存関係のあるものを後に読み込む）
source "$SCRIPT_DIR/lib/utils.sh"      # 共通ユーティリティ（カラー、REPO_ROOT、基本関数）
source "$SCRIPT_DIR/lib/git.sh"        # Git操作関連
source "$SCRIPT_DIR/lib/session.sh"    # tmuxセッション管理
source "$SCRIPT_DIR/lib/issue.sh"      # Issue処理関連
source "$SCRIPT_DIR/lib/cleanup.sh"    # クリーンアップ関連
source "$SCRIPT_DIR/lib/batch.sh"      # バッチ処理関連

# メイン処理
main() {
    if [ $# -eq 0 ]; then
        # 引数なし: ヘルプ表示
        show_help
    elif [ "$1" = "help" ] || [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
        # ヘルプ表示
        show_help
    elif [ "$1" = "batch" ]; then
        # バッチモード（複数Issue並行実行）
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号リストを指定してください${NC}"
            echo "例: $cmd batch 88,89,90"
            exit 1
        fi
        run_batch_mode "$2"
    elif [ "$1" = "sessions" ]; then
        # tmuxセッション一覧
        check_tmux
        show_tmux_sessions
    elif [ "$1" = "attach" ]; then
        # tmuxセッションにアタッチ
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号を指定してください${NC}"
            echo "例: $cmd attach 42"
            exit 1
        fi
        check_tmux
        attach_tmux_session "$2"
    elif [ "$1" = "logs" ]; then
        # tmuxセッションのログ表示
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号を指定してください${NC}"
            echo "例: $cmd logs 42"
            echo "例（リアルタイム）: $cmd logs 42 --follow"
            exit 1
        fi
        
        local issue_identifier="$2"
        local follow_mode="false"
        
        # オプション解析
        shift 2  # "logs" と issue_identifier を除去
        while [[ $# -gt 0 ]]; do
            case $1 in
                -f|--follow)
                    follow_mode="true"
                    shift
                    ;;
                *)
                    echo -e "${RED}エラー: 不明なオプション '$1'${NC}"
                    echo "使用可能なオプション: --follow (-f)"
                    exit 1
                    ;;
            esac
        done
        
        check_tmux
        show_tmux_logs "$issue_identifier" "$follow_mode"
    elif [ "$1" = "cleanup" ]; then
        # Worktreeクリーンアップ
        cleanup_worktrees "$2"
    elif [ "$1" = "fix" ]; then
        # Issue解決モード
        local issue_number="$2"
        local use_async="true"
        local keep_worktree="false"
        
        # オプション解析
        shift 2  # "fix" と issue_number を除去
        while [[ $# -gt 0 ]]; do
            case $1 in
                -s|--sync)
                    use_async="false"
                    shift
                    ;;
                -k|--keep-worktree)
                    keep_worktree="true"
                    shift
                    ;;
                *)
                    echo -e "${RED}エラー: 不明なオプション '$1'${NC}"
                    exit 1
                    ;;
            esac
        done
        
        run_issue_mode "$issue_number" "$use_async" "$keep_worktree"
    elif [ "$1" = "issue" ]; then
        # Issue作成モード
        if [ $# -eq 1 ]; then
            # 引数なし: 対話モード
            create_issue
        else
            # 引数あり: 非対話モード
            shift  # "issue" を除去
            parse_create_issue_args "$@"
        fi
    elif [ "$1" = "expect-pause" ]; then
        # expectプロセスを一時停止
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号またはセッション名を指定してください${NC}"
            echo "例: $cmd expect-pause 42"
            exit 1
        fi
        control_expect_process "$2" "pause"
    elif [ "$1" = "expect-resume" ]; then
        # expectプロセスを再開
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号またはセッション名を指定してください${NC}"
            echo "例: $cmd expect-resume 42"
            exit 1
        fi
        control_expect_process "$2" "resume"
    elif [ "$1" = "expect-stop" ]; then
        # expectプロセスを停止
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号またはセッション名を指定してください${NC}"
            echo "例: $cmd expect-stop 42"
            exit 1
        fi
        control_expect_process "$2" "stop"
    elif [ "$1" = "expect-reattach" ]; then
        # expectプロセスを再アタッチ
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号またはセッション名を指定してください${NC}"
            echo "例: $cmd expect-reattach 42"
            exit 1
        fi
        reattach_expect_process "$2"
    elif [ "$1" = "watchdog" ]; then
        # watchdog復旧（万が一いなくなった場合）
        if [ -z "$2" ]; then
            echo -e "${RED}エラー: Issue番号を指定してください${NC}"
            echo "例: $cmd watchdog 42"
            exit 1
        fi
        
        local issue_number="$2"
        watch_session "$issue_number"
    else
        # 不明なコマンド
        echo -e "${RED}エラー: 不明なコマンド '$1'${NC}"
        echo ""
        show_help
        exit 1
    fi
}

# スクリプト実行
main "$@" 