#!/usr/bin/env bash
# vive tmuxセッション管理とwatchdog関連

# watchdog起動の共通処理
start_watchdog_process() {
    local session_name="$1"
    local delay_seconds="${2:-3}"  # デフォルト3秒待機
    local force_restart="${3:-false}"  # 強制再起動フラグ
    
    if [ -z "$session_name" ]; then
        echo -e "${RED}エラー: セッション名が指定されていません${NC}"
        return 1
    fi
    
    # expectスクリプトのパスを定義
    local expect_script="$REPO_ROOT/vive/watchdog.exp"
    local log_file="/tmp/claude_session_watchdog_${session_name}_$(date +%s).log"
    
    # expectスクリプトの存在確認
    if [ ! -f "$expect_script" ]; then
        echo -e "${RED}警告: expectスクリプトが見つかりません: $expect_script${NC}"
        return 1
    fi
    
    # tmuxセッションの存在確認
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}警告: tmuxセッション '$session_name' が見つかりません${NC}"
        return 1
    fi
    
    # 既存のwatchdogプロセスをチェックして処理
    local existing_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    if [ -n "$existing_pid" ]; then
        if [ "$force_restart" = "true" ]; then
            echo -e "${YELLOW}既存のwatchdogプロセス (PID: $existing_pid) を停止して新しいものを起動します...${NC}"
            kill "$existing_pid" 2>/dev/null || true
            sleep 1
            kill -9 "$existing_pid" 2>/dev/null || true
        else
            echo -e "${YELLOW}watchdogプロセスは既に動作中です (PID: $existing_pid)${NC}"
            echo -e "${BLUE}新しいwatchdogプロセスを追加で起動します...${NC}"
        fi
    fi
    
    # Claude Code起動を待つ
    if [ "$delay_seconds" -gt 0 ]; then
        echo -e "${YELLOW}Claude Code起動を${delay_seconds}秒待機してからwatchdogを起動します...${NC}"
        sleep "$delay_seconds"
    fi
    
    # watchdogプロセスを起動
    echo -e "${BLUE}watchdogプロセスを起動中...${NC}"
    nohup expect "$expect_script" attach "$session_name" "$log_file" > /dev/null 2>&1 &
    local watchdog_pid=$!
    
    # 起動確認
    sleep 1
    if ps -p $watchdog_pid > /dev/null 2>&1; then
        echo -e "${GREEN}✅ watchdogプロセスを開始しました (PID: $watchdog_pid)${NC}"
        return 0
    else
        echo -e "${RED}❌ watchdogプロセスの起動に失敗しました${NC}"
        echo -e "${YELLOW}ログファイル: $log_file${NC}"
        return 1
    fi
}

# セッション状態を取得する関数
get_session_status() {
    local session_name="$1"
    local log_file="/tmp/claude_session_${session_name}_*.log"
    
    # 完了フラグファイルの存在確認
    if [ -f "/tmp/claude_completed_$session_name" ]; then
        echo "待機中"
        return
    fi
    
    # expectログファイルから最終更新時刻を確認
    local latest_log=$(ls -t $log_file 2>/dev/null | head -1)
    if [ -n "$latest_log" ] && [ -f "$latest_log" ]; then
        # ログファイルの最終更新時刻を取得
        local last_modified=$(stat -f %m "$latest_log" 2>/dev/null || stat -c %Y "$latest_log" 2>/dev/null)
        local current_time=$(date +%s)
        local diff=$((current_time - last_modified))
        
        # 30秒以上更新がない場合は待機中
        if [ "$diff" -gt 30 ]; then
            echo "待機中"
        else
            echo "動作中"
        fi
    else
        # tmuxペインの内容から判断
        local output=$(tmux capture-pane -t "$session_name" -p 2>/dev/null | tail -10)
        if [ -z "$output" ]; then
            echo "待機中"
        else
            echo "動作中"
        fi
    fi
}

# tmuxセッション一覧表示（status機能統合版）
show_tmux_sessions() {
    echo -e "${GREEN}アクティブなClaude Code tmuxセッション:${NC}"
    echo ""
    
    local has_sessions=false
    
    # tmuxセッション一覧を取得（issue-*のみ）
    for session in $(tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^issue-" || true); do
        has_sessions=true
        local created=$(tmux list-sessions -F "#{session_name} #{session_created}" 2>/dev/null | grep "^$session " | awk '{print $2}')
        local created_date=$(date -r "$created" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "不明")
        
        # セッションの状態を取得
        local status=$(get_session_status "$session")
        
        echo -e "${BLUE}セッション: $session${NC}"
        echo "  作成日時: $created_date"
        
        # 状態に応じて色分け
        if [ "$status" = "待機中" ]; then
            echo -e "  状態: ${YELLOW}$status${NC}"
        else
            echo -e "  状態: ${GREEN}$status${NC}"
        fi
        
        # expectプロセスの有無
        local expect_running="なし"
        if ps aux | grep "expect.*$session" | grep -v grep > /dev/null 2>&1; then
            expect_running="実行中"
        fi
        echo "  expectプロセス: $expect_running"
        
        # Issue情報を表示
        if [[ "$session" =~ ^issue-([0-9]+)$ ]]; then
            local issue_num="${BASH_REMATCH[1]}"
            local issue_title=$(gh issue view "$issue_num" --json title -q .title 2>/dev/null || echo "取得失敗")
            echo "  Issue: #$issue_num - $issue_title"
            
            # Worktree情報
            local worktree_dir="$REPO_ROOT/../${PROJECT_NAME}-issue-${issue_num}"
            if [ -d "$worktree_dir" ]; then
                echo "  Worktree: $worktree_dir"
            fi
        fi
        
        echo ""
    done
    
    if [ "$has_sessions" = false ]; then
        echo -e "${YELLOW}アクティブなセッションはありません${NC}"
    else
        echo -e "${YELLOW}セッションにアタッチするには: $cmd attach <issue-number>${NC}"
    fi
}

# expectプロセスの制御
control_expect_process() {
    local session_identifier="$1"
    local action="$2"  # pause, resume, stop
    local session_name=""
    
    # セッション名の決定（数字ならissue-、それ以外はそのまま）
    if [[ "$session_identifier" =~ ^[0-9]+$ ]]; then
        session_name="issue-${session_identifier}"
    else
        session_name="$session_identifier"
    fi
    
    # expectプロセスのPIDを取得
    local expect_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    
    if [ -z "$expect_pid" ]; then
        echo -e "${YELLOW}expectプロセスが見つかりません（既に終了している可能性があります）${NC}"
        return 1
    fi
    
    case "$action" in
        "pause")
            echo -e "${YELLOW}expectプロセス (PID: $expect_pid) を一時停止します...${NC}"
            kill -STOP "$expect_pid"
            echo -e "${GREEN}✅ expectプロセスを一時停止しました${NC}"
            ;;
        "resume")
            echo -e "${YELLOW}expectプロセス (PID: $expect_pid) を再開します...${NC}"
            kill -CONT "$expect_pid"
            echo -e "${GREEN}✅ expectプロセスを再開しました${NC}"
            ;;
        "stop")
            echo -e "${YELLOW}expectプロセス (PID: $expect_pid) を停止します...${NC}"
            kill "$expect_pid" 2>/dev/null || true
            sleep 1
            # まだ生きていたら強制終了
            kill -9 "$expect_pid" 2>/dev/null || true
            echo -e "${GREEN}✅ expectプロセスを停止しました${NC}"
            ;;
        *)
            echo -e "${RED}エラー: 不明なアクション '$action'${NC}"
            return 1
            ;;
    esac
}

# tmuxセッションにアタッチ（改良版）
attach_tmux_session() {
    local session_identifier="$1"
    local session_name=""
    
    # セッション名の決定（数字ならissue-、それ以外はそのまま）
    if [[ "$session_identifier" =~ ^[0-9]+$ ]]; then
        session_name="issue-${session_identifier}"
    else
        session_name="$session_identifier"
    fi
    
    # セッション存在確認
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}エラー: セッション '$session_name' が見つかりません${NC}"
        echo ""
        echo -e "${YELLOW}利用可能なセッション:${NC}"
        tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^issue-" || echo "アクティブなセッションはありません"
        exit 1
    fi
    
    # expectプロセスの存在確認
    local expect_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    if [ -n "$expect_pid" ]; then
        echo -e "${BLUE}expectプロセス (PID: $expect_pid) が実行中です${NC}"
        echo -e "${GREEN}ユーザーアタッチ中は自動承認が一時的に無効化されます${NC}"
        echo -e "${YELLOW}作業開始通知も無効化されます${NC}"
    fi
    
    echo -e "${GREEN}セッション '$session_name' にアタッチします...${NC}"
    echo -e "${YELLOW}デタッチするには: Ctrl+B, D${NC}"
    sleep 1
    
    # tmuxセッションにアタッチ
    tmux attach-session -t "$session_name"
    
    echo -e "${GREEN}セッションから離脱しました${NC}"
}

# tmuxセッションのログ表示
show_tmux_logs() {
    local session_identifier="$1"
    local follow_mode="$2"
    local session_name=""
    
    # セッション名の決定（数字ならissue-、それ以外はそのまま）
    if [[ "$session_identifier" =~ ^[0-9]+$ ]]; then
        session_name="issue-${session_identifier}"
    else
        session_name="$session_identifier"
    fi
    
    # セッション存在確認
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}エラー: セッション '$session_name' が見つかりません${NC}"
        exit 1
    fi
    
    if [ "$follow_mode" = "true" ]; then
        echo -e "${GREEN}セッション '$session_name' をリアルタイム監視します${NC}"
        echo -e "${YELLOW}終了するには Ctrl+C を押してください${NC}"
        echo ""
        
        # watchコマンドでリアルタイム表示
        watch -n 1 "tmux capture-pane -t '$session_name:0.0' -p 2>/dev/null || echo 'セッション $session_name が見つかりません'"
    else
        echo -e "${GREEN}セッション '$session_name' のログ（最新50行）:${NC}"
        echo ""
        
        # tmuxペインの内容をキャプチャ
        tmux capture-pane -t "$session_name:0.0" -S -50 -E -1 -p
    fi
}

# Claude Code tmux実行
run_claude_tmux() {
    local prompt="$1"
    local worktree_dir="$2"
    local mode="$3"
    local issue_number="$4"
    local should_attach="$5"  # 新しい引数: 同期モードでアタッチするかどうか
    
    # セッション名の決定
    local session_name=""
    if [ -n "$issue_number" ] && [ "$issue_number" != "" ]; then
        session_name="issue-${issue_number}"
    else
        # プロンプトモード用のタイムスタンプ
        local timestamp=$(date +%Y%m%d_%H%M%S)
        session_name="prompt-${timestamp}"
    fi
    
    if [ "$should_attach" = "true" ]; then
        echo -e "${GREEN}tmuxセッション '$session_name' で同期実行を開始します...${NC}"
    else
        echo -e "${GREEN}tmuxセッション '$session_name' で非同期実行を開始します...${NC}"
    fi
    echo -e "${BLUE}作業ディレクトリ: $worktree_dir/native-app${NC}"
    
    # 既存のセッションがあれば削除
    if tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${YELLOW}既存のセッション '$session_name' を削除します...${NC}"
        tmux kill-session -t "$session_name"
    fi
    
    # プロンプトをファイルに保存
    local prompt_file="/tmp/claude_prompt_$(date +%s).txt"
    echo "$prompt" > "$prompt_file"
    
    # ログファイルのパスを定義
    local log_file="/tmp/claude_session_${session_name}_$(date +%s).log"
    
    # 作業ディレクトリの存在確認
    if [ ! -d "$worktree_dir/native-app" ]; then
        echo -e "${RED}エラー: 作業ディレクトリが存在しません: $worktree_dir/native-app${NC}"
        exit 1
    fi
    
    # MCPコンフィグの存在確認
    if [ ! -f "$REPO_ROOT/.cursor/mcp.json" ]; then
        echo -e "${RED}エラー: MCPコンフィグファイルが存在しません: $REPO_ROOT/.cursor/mcp.json${NC}"
        exit 1
    fi
    
    # tmuxセッションを作成（まだClaude Codeは起動しない）
    echo -e "${YELLOW}tmuxセッションを作成中...${NC}"
    
    tmux new-session -d -s "$session_name" -c "$worktree_dir/native-app" \
        "echo -e '${GREEN}Claude Code セッション準備中...${NC}'; \
         echo -e '${BLUE}セッション: $session_name${NC}'; \
         echo -e '${BLUE}作業ディレクトリ: \$(pwd)${NC}'; \
         echo ''; \
         echo -e '${YELLOW}Claude Codeを起動準備中...${NC}'; \
         echo ''; \
         echo 'Claude Code起動待機中...'; \
         exec bash"
    
    # セッション作成の確認
    sleep 1
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}❌ tmuxセッションの作成に失敗しました${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ tmuxセッション '$session_name' を作成しました${NC}"
    
    # expectスクリプトのパスを定義
    local expect_script="$REPO_ROOT/vive/watchdog.exp"
    
    # expectスクリプトの存在確認
    if [ ! -f "$expect_script" ]; then
        echo -e "${RED}エラー: expectスクリプトが見つかりません: $expect_script${NC}"
        exit 1
    fi
    
    # expectスクリプトをバックグラウンドで実行（startモード）
    echo -e "${YELLOW}Claude Codeを起動中（バックグラウンド）...${NC}"
    nohup expect "$expect_script" start "$prompt_file" "$REPO_ROOT/.cursor/mcp.json" "$session_name" "$log_file" > /dev/null 2>&1 &
    local expect_pid=$!
    
    echo -e "${GREEN}✅ Claude Code起動プロセスを開始しました (PID: $expect_pid)${NC}"
    echo ""
    
    # watchdogプロセスを起動（共通関数を使用）
    start_watchdog_process "$session_name" 3 true
    
    # 同期モードの場合はアタッチ
    if [ "$should_attach" = "true" ]; then
        echo -e "${YELLOW}数秒待ってからセッションにアタッチします...${NC}"
        sleep 3
        echo -e "${GREEN}セッション '$session_name' にアタッチします...${NC}"
        echo -e "${YELLOW}デタッチするには: Ctrl+B, D${NC}"
        sleep 1
        
        # tmuxセッションにアタッチ
        tmux attach-session -t "$session_name"
    else
        # 非同期モードの場合は状況報告のみ
        echo ""
        echo -e "${GREEN}✅ Claude Code を非同期で起動しました${NC}"
        echo ""
        echo -e "${YELLOW}操作方法:${NC}"
        echo "  セッション確認: $cmd sessions"
        echo "  アタッチ: $cmd attach $session_identifier"
        echo "  ログ表示: $cmd logs $session_identifier"
        echo ""
        echo -e "${BLUE}ヒント: 進捗はターミナル通知またはsayコマンドでお知らせします${NC}"
    fi
}

# expectプロセスの再アタッチ
reattach_expect_process() {
    local session_identifier="$1"
    local session_name=""
    
    # セッション名の決定（数字ならissue-、それ以外はそのまま）
    if [[ "$session_identifier" =~ ^[0-9]+$ ]]; then
        session_name="issue-${session_identifier}"
    else
        session_name="$session_identifier"
    fi
    
    # セッション存在確認
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}エラー: セッション '$session_name' が見つかりません${NC}"
        echo ""
        echo -e "${YELLOW}利用可能なセッション:${NC}"
        tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^issue-" || echo "アクティブなセッションはありません"
        return 1
    fi
    
    # 既存のexpectプロセスを確認
    local existing_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    if [ -n "$existing_pid" ]; then
        echo -e "${YELLOW}既存のexpectプロセス (PID: $existing_pid) が見つかりました${NC}"
        echo -e "${BLUE}既存のプロセスを停止してから新しいプロセスを起動しますか？ (Y/n):${NC}"
        read -r stop_existing
        
        if [ "$stop_existing" != "n" ] && [ "$stop_existing" != "N" ]; then
            echo -e "${YELLOW}既存のexpectプロセスを停止中...${NC}"
            kill "$existing_pid" 2>/dev/null || true
            sleep 1
            kill -9 "$existing_pid" 2>/dev/null || true
        else
            echo -e "${YELLOW}再アタッチをキャンセルしました${NC}"
            return 0
        fi
    fi
    
    # 再アタッチ用expectスクリプトのパスを定義
    local expect_script="$REPO_ROOT/vive/watchdog.exp"
    local log_file="/tmp/claude_session_reattach_${session_name}_$(date +%s).log"
    
    # expectスクリプトの存在確認
    if [ ! -f "$expect_script" ]; then
        echo -e "${RED}エラー: expectスクリプトが見つかりません: $expect_script${NC}"
        return 1
    fi
    
    # expectスクリプトをバックグラウンドで実行（attachモード）
    echo -e "${YELLOW}expectプロセスを再アタッチ中（バックグラウンド）...${NC}"
    nohup expect "$expect_script" attach "$session_name" "$log_file" > /dev/null 2>&1 &
    local new_pid=$!
    
    echo -e "${GREEN}✅ 新しいexpectプロセスを開始しました (PID: $new_pid)${NC}"
    echo ""
    echo -e "${YELLOW}詳細情報:${NC}"
    echo "  セッション: $session_name"
    echo "  expectログ: $log_file"
    echo ""
    echo -e "${BLUE}expectプロセスの管理:${NC}"
    echo "  一時停止: $cmd expect-pause $session_identifier"
    echo "  再開: $cmd expect-resume $session_identifier"
    echo "  停止: $cmd expect-stop $session_identifier"
    echo ""
    echo -e "${GREEN}expectプロセスが自動応答を再開しました${NC}"
}

# セッションをwatchdog状態にする（シンプルラッパー）
watch_session() {
    local issue_number="$1"
    
    if [ -z "$issue_number" ]; then
        echo -e "${RED}エラー: Issue番号を指定してください${NC}"
        echo "例: $cmd watchdog 42"
        return 1
    fi
    
    # セッション名の決定
    local session_name="issue-${issue_number}"
    
    echo -e "${GREEN}セッション '$session_name' のwatchdog復旧を開始します...${NC}"
    
    # tmuxチェック
    check_tmux
    
    # セッション存在確認
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}エラー: セッション '$session_name' が見つかりません${NC}"
        echo ""
        echo -e "${YELLOW}ヒント: まず以下のコマンドでIssueを開始してください:${NC}"
        echo "  $cmd fix $issue_number"
        return 1
    fi
    
    # 既存のexpectプロセスを確認して停止
    local existing_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    
    if [ -n "$existing_pid" ]; then
        echo -e "${YELLOW}既存のwatchdogプロセス (PID: $existing_pid) を停止します...${NC}"
        kill "$existing_pid" 2>/dev/null || true
        sleep 1
        kill -9 "$existing_pid" 2>/dev/null || true
        echo -e "${GREEN}✅ 既存のwatchdogプロセスを停止しました${NC}"
    fi
    
    # expectスクリプトのパスを定義
    local expect_script="$REPO_ROOT/vive/watchdog.exp"
    local log_file="/tmp/claude_session_watchdog_${session_name}_$(date +%s).log"
    
    # expectスクリプトの存在確認
    if [ ! -f "$expect_script" ]; then
        echo -e "${RED}エラー: expectスクリプトが見つかりません: $expect_script${NC}"
        return 1
    fi
    
    # expectスクリプトをバックグラウンドで実行（attachモード）
    echo -e "${BLUE}新しいwatchdogプロセスを起動中...${NC}"
    
    nohup expect "$expect_script" attach "$session_name" "$log_file" > /dev/null 2>&1 &
    local expect_pid=$!
    
    # 起動確認のため少し待機
    sleep 1
    
    # プロセスが正常に起動したか確認
    if ps -p $expect_pid > /dev/null 2>&1; then
        echo -e "${GREEN}✅ 新しいwatchdogプロセスを起動しました (PID: $expect_pid)${NC}"
        echo ""
        echo -e "${YELLOW}セッション操作:${NC}"
        echo "  アタッチ: $cmd attach $issue_number"
        echo "  ログ表示: $cmd logs $issue_number"
        echo ""
        echo -e "${BLUE}監視状態:${NC}"
        echo "  自動承認が有効です"
        echo "  ユーザーがアタッチ中は自動的に無効化されます"
    else
        echo -e "${RED}❌ watchdogプロセスの起動に失敗しました${NC}"
        echo -e "${YELLOW}ログを確認してください: $log_file${NC}"
        return 1
    fi
} 