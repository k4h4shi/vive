#!/usr/bin/env bash
# vive クリーンアップ関連

# Worktreeクリーンアップ
cleanup_worktrees() {
    local issue_number="$1"
    
    if [ -n "$issue_number" ]; then
        # 個別Issueのクリーンアップ
        echo -e "${YELLOW}Issue #${issue_number} のWorktreeとClaude Codeプロセスをクリーンアップします。${NC}"
        echo -e "${YELLOW}この操作は元に戻せません。続行しますか？ (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}クリーンアップをキャンセルしました${NC}"
            exit 0
        fi
        
        echo -e "${BLUE}Issue #${issue_number} のクリーンアップを開始します...${NC}"
        
        # 特定のイシューのtmuxセッションを終了
        echo -e "${YELLOW}tmuxセッションを終了中...${NC}"
        local session_name="issue-${issue_number}"
        if tmux has-session -t "$session_name" 2>/dev/null; then
            echo -e "${YELLOW}tmuxセッション終了: $session_name${NC}"
            tmux kill-session -t "$session_name" 2>/dev/null || true
        else
            echo -e "${GREEN}tmuxセッション $session_name は見つかりませんでした${NC}"
        fi
        
        # 特定のイシューのexpectプロセスを終了
        echo -e "${YELLOW}expectプロセスを終了中...${NC}"
        expect_pids=$(ps aux | grep -E "expect.*issue-${issue_number}" | grep -v grep | awk '{print $2}' || true)
        if [ -n "$expect_pids" ]; then
            echo -e "${YELLOW}expectプロセスを終了: $expect_pids${NC}"
            kill $expect_pids 2>/dev/null || true
            sleep 1
            kill -9 $expect_pids 2>/dev/null || true
        fi
        
        # 関連する一時ファイルをクリーンアップ
        echo -e "${YELLOW}一時ファイルをクリーンアップ中...${NC}"
        rm -f /tmp/claude_prompt_${issue_number}.txt 2>/dev/null || true
        rm -f /tmp/claude_session_issue-${issue_number}_*.log 2>/dev/null || true
        rm -f /tmp/claude_completed_issue-${issue_number} 2>/dev/null || true
        
        # 特定のイシューのClaude Codeプロセスを終了
        echo -e "${YELLOW}Claude Codeプロセスを終了中...${NC}"
        pkill -f "claude.*Issue.*${issue_number}" || true
        sleep 2
        
        cd "$REPO_ROOT"
        
        # 特定のworktreeを削除
        local worktree_dir="$REPO_ROOT/../${PROJECT_NAME}-issue-${issue_number}"
        if [ -d "$worktree_dir" ]; then
            echo -e "${YELLOW}Worktreeを削除中: $worktree_dir${NC}"
            git worktree remove --force "$worktree_dir" || true
        fi
        
        # issue/ブランチを削除
        local branch_name="issue/${issue_number}"
        if git show-ref --verify --quiet refs/heads/"$branch_name"; then
            echo -e "${YELLOW}ブランチを削除中: $branch_name${NC}"
            git branch -D "$branch_name" || true
        fi
        
        echo -e "${GREEN}Issue #${issue_number} のクリーンアップ完了${NC}"
    else
        # 全体のクリーンアップ（既存の処理）
        echo -e "${YELLOW}全てのWorktreeとClaude Codeプロセスをクリーンアップします。${NC}"
        echo -e "${YELLOW}この操作は元に戻せません。続行しますか？ (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}クリーンアップをキャンセルしました${NC}"
            exit 0
        fi
        
        echo -e "${BLUE}Worktreeクリーンアップを開始します...${NC}"
        
        # tmuxセッションを終了
        echo -e "${YELLOW}tmuxセッションを終了中...${NC}"
        for session in $(tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^issue-" || true); do
            echo -e "${YELLOW}tmuxセッション終了: $session${NC}"
            tmux kill-session -t "$session" 2>/dev/null || true
        done
        
        # expectプロセスを終了
        echo -e "${YELLOW}expectプロセスを終了中...${NC}"
        expect_pids=$(ps aux | grep -E "expect.*issue-" | grep -v grep | awk '{print $2}' || true)
        if [ -n "$expect_pids" ]; then
            echo -e "${YELLOW}expectプロセスを終了: $expect_pids${NC}"
            kill $expect_pids 2>/dev/null || true
            sleep 1
            # まだ残っている場合はKILL信号で強制終了
            kill -9 $expect_pids 2>/dev/null || true
        else
            echo -e "${GREEN}expectプロセスは見つかりませんでした${NC}"
        fi
        
        # 一時ファイルをクリーンアップ
        echo -e "${YELLOW}一時ファイルをクリーンアップ中...${NC}"
        rm -f /tmp/claude_prompt_*.txt 2>/dev/null || true
        rm -f /tmp/claude_expect_*.exp 2>/dev/null || true
        rm -f /tmp/claude_session_*.log 2>/dev/null || true
        rm -f /tmp/expect_resume_output_*.log 2>/dev/null || true
        rm -f /tmp/claude_completed_* 2>/dev/null || true
        
        # Claude Codeプロセスを終了
        echo -e "${YELLOW}Claude Codeプロセスを終了中...${NC}"
        pkill -f "claude.*Issue" || true
        sleep 2
        
        # 残っているClaude Codeプロセスを強制終了（現在のスクリプトは除外）
        current_pid=$$
        remaining_pids=$(ps aux | grep -i claude | grep -v grep | grep -v "claude-code.sh" | awk '{print $2}' | grep -v "^${current_pid}$" | xargs)
        if [ -n "$remaining_pids" ]; then
            echo -e "${YELLOW}残存プロセスを強制終了: $remaining_pids${NC}"
            kill $remaining_pids 2>/dev/null || true
            sleep 1
            # まだ残っている場合はKILL信号で強制終了
            kill -9 $remaining_pids 2>/dev/null || true
        fi
        
        cd "$REPO_ROOT"
        
        # 全てのworktreeを取得（メインディレクトリ以外）
        git worktree list --porcelain | grep -E "^worktree " | grep -v "^worktree $REPO_ROOT$" | while read -r line; do
            worktree_path=$(echo "$line" | sed 's/^worktree //')
            if [ -d "$worktree_path" ]; then
                echo -e "${YELLOW}Worktreeを削除中: $worktree_path${NC}"
                git worktree remove --force "$worktree_path" || true
            fi
        done
        
        # issue/ブランチを削除
        git branch | grep -E "^\s*issue/" | while read -r branch; do
            branch=$(echo "$branch" | xargs)  # trim whitespace
            echo -e "${YELLOW}ブランチを削除中: $branch${NC}"
            git branch -D "$branch" || true
        done
        
        # prompt/ブランチを削除
        git branch | grep -E "^\s*prompt/" | while read -r branch; do
            branch=$(echo "$branch" | xargs)  # trim whitespace
            echo -e "${YELLOW}ブランチを削除中: $branch${NC}"
            git branch -D "$branch" || true
        done
        
        # 最終確認（現在のスクリプトは除外）
        remaining_claude=$(ps aux | grep -i claude | grep -v grep | grep -v "claude-code.sh" | grep -v "^[[:space:]]*${current_pid}" | wc -l)
        if [ "$remaining_claude" -eq 0 ]; then
            echo -e "${GREEN}✅ 全てのClaude Codeプロセスが終了しました${NC}"
        else
            echo -e "${YELLOW}⚠️  まだClaude Codeプロセスが残っている可能性があります${NC}"
            ps aux | grep -i claude | grep -v grep | grep -v "claude-code.sh"
        fi
        
        echo -e "${GREEN}クリーンアップ完了${NC}"
    fi
} 