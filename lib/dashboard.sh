#!/usr/bin/env bash
# vive dashboard functionality

create_vive_dashboard() {
    local dashboard_session_name="vive-dashboard"
    local vive_sessions_info=() # "issue_num" の配列

    # アクティブな vive-issue-* セッションから Issue 番号を抽出して配列に格納
    while IFS= read -r session_name; do
        if [[ "$session_name" =~ ^${PROJECT_NAME}-issue-([0-9]+)$ ]]; then
            local issue_num="${BASH_REMATCH[1]}"
            vive_sessions_info+=("$issue_num")
        fi
    done < <(tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" || true)

    if [ ${#vive_sessions_info[@]} -eq 0 ]; then
        echo -e "${YELLOW}No active 'vive-issue-*' sessions found to display in the dashboard.${NC}"
        return 1
    fi

    echo -e "${BLUE}Found ${#vive_sessions_info[@]} active vive-issue session(s). Creating dashboard...${NC}"

    # 既存のダッシュボードセッションがあれば削除
    if tmux has-session -t "$dashboard_session_name" 2>/dev/null; then
        echo -e "${YELLOW}Existing dashboard session '$dashboard_session_name' found. Killing it...${NC}"
        tmux kill-session -t "$dashboard_session_name"
    fi

    # 最初のセッションログで新しいダッシュボードセッションを開始
    local first_issue_num=${vive_sessions_info[0]}
    echo -e "${BLUE}Creating new session '$dashboard_session_name' and showing logs for issue #$first_issue_num...${NC}"
    # セッションの作業ディレクトリをリポジトリルートに設定
    tmux new-session -d -s "$dashboard_session_name" -n "Vive Logs" -c "$REPO_ROOT" "vive logs \"$first_issue_num\" -f"
    # ペインタイトルを設定 (tmux 1.9以降)
    tmux select-pane -t "$dashboard_session_name:0.0" -T "Issue #$first_issue_num"


    # 残りのセッションログを新しいペインで表示
    for i in $(seq 1 $((${#vive_sessions_info[@]} - 1))); do
        local current_issue_num=${vive_sessions_info[$i]}
        echo -e "${BLUE}Adding logs for issue #$current_issue_num to dashboard...${NC}"
        # 新しいペインを垂直分割で作成し、コマンドを実行
        tmux split-window -t "$dashboard_session_name:0" -v -c "$REPO_ROOT" "vive logs \"$current_issue_num\" -f"
        # 新しく作成されたペインにタイトルを設定
        tmux select-pane -t "$dashboard_session_name:0.+" -T "Issue #$current_issue_num"
        # 各ペイン追加後にレイアウトを調整 (tiledが一般的)
        tmux select-layout -t "$dashboard_session_name:0" tiled
    done

    # 最後に再度レイアウトを適用 (均等分割のため)
    tmux select-layout -t "$dashboard_session_name:0" tiled

    echo -e "${GREEN}Dashboard session '$dashboard_session_name' created successfully.${NC}"
    echo -e "${YELLOW}You can attach to it using: tmux attach-session -t $dashboard_session_name${NC}"

    read -p "Attach to the dashboard session '$dashboard_session_name' now? (y/N): " -n 1 -r
    echo # 改行
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        tmux attach-session -t "$dashboard_session_name"
    fi
} 