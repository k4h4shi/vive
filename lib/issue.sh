#!/usr/bin/env bash
# vive Issue処理関連

# Issue情報取得
get_issue_info() {
    local issue_number="$1"
    
    echo -e "${BLUE}Issue #${issue_number} の情報を取得中...${NC}"
    
    # GitHub CLIでIssue情報取得
    if ! command -v gh &> /dev/null; then
        echo -e "${RED}エラー: GitHub CLI (gh) がインストールされていません${NC}"
        exit 1
    fi
    
    # Issue存在チェック
    if ! gh issue view "$issue_number" &> /dev/null; then
        echo -e "${RED}エラー: Issue #${issue_number} が見つかりません${NC}"
        exit 1
    fi
    
    # Issue情報取得
    ISSUE_TITLE=$(gh issue view "$issue_number" --json title -q .title)
    ISSUE_BODY=$(gh issue view "$issue_number" --json body -q .body)
    ISSUE_LABELS=$(gh issue view "$issue_number" --json labels -q '.labels[].name' | tr '\n' ' ')
    
    echo -e "${GREEN}Issue情報取得完了:${NC}"
    echo "タイトル: $ISSUE_TITLE"
    echo "ラベル: $ISSUE_LABELS"
    echo ""
}

# Issue解決モード
run_issue_mode() {
    local issue_number="$1"
    local use_async="$2"
    local keep_worktree="$3"
    
    if [ -z "$issue_number" ]; then
        echo -e "${RED}エラー: Issue番号を指定してください${NC}"
        echo "例: $cmd fix 42"
        exit 1
    fi
    
    echo -e "${GREEN}Issue #${issue_number} 解決モードを開始します...${NC}"
    
    # Git状態チェック
    check_git_status
    
    # Issue情報取得
    get_issue_info "$issue_number"
    
    # 同期モードの場合は確認
    if [ "$use_async" != "true" ]; then
        echo -e "${YELLOW}Issue #${issue_number}: ${ISSUE_TITLE}${NC}"
        if [ "$keep_worktree" = "true" ]; then
            echo -e "${BLUE}既存のWorktreeを引き継ぐモードです${NC}"
        fi
        echo -e "${YELLOW}この内容でWorktreeを作成してClaude Codeを実行しますか？ (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}Issue解決をキャンセルしました${NC}"
            exit 0
        fi
    fi
    
    # Worktree設定
    local branch_name="issue/${issue_number}"
    local worktree_dir="$REPO_ROOT/../${PROJECT_NAME}-issue-${issue_number}"
    
    # パスを正規化（../を解決）
    # ディレクトリが存在する場合のみrealpathを使用、存在しない場合は手動で正規化
    if [ -d "$worktree_dir" ]; then
        worktree_dir="$(realpath "$worktree_dir")"
    else
        # 手動でパス正規化（../を解決）
        worktree_dir="$(cd "$(dirname "$REPO_ROOT")" && pwd)/${PROJECT_NAME}-issue-${issue_number}"
    fi
    
    cd "$REPO_ROOT"
    
    # Worktree処理（keep_worktreeオプションに応じて分岐）
    if [ "$keep_worktree" = "true" ] && [ -d "$worktree_dir" ]; then
        echo -e "${BLUE}既存のWorktree ${worktree_dir} を引き継ぎます...${NC}"
        
        # 問題のある&1ファイルを削除（リダイレクトエラーの対策）
        rm -f "$worktree_dir/&1" 2>/dev/null || true
        
        # .gitファイルが存在しない場合は復旧処理
        if [ ! -f "$worktree_dir/.git" ]; then
            echo -e "${YELLOW}⚠️  Worktreeの.gitファイルが見つかりません。復旧処理を実行します...${NC}"
            
            # worktreeのgitdirパスを再作成
            local git_worktree_path="$REPO_ROOT/.git/worktrees/${PROJECT_NAME}-issue-${issue_number}"
            if [ -d "$git_worktree_path" ]; then
                echo -e "${BLUE}.gitファイルを復旧中...${NC}"
                echo "gitdir: $git_worktree_path" > "$worktree_dir/.git"
                echo -e "${GREEN}✅ Worktreeを復旧しました${NC}"
            else
                echo -e "${RED}エラー: Worktreeのメタデータが見つかりません: $git_worktree_path${NC}"
                exit 1
            fi
        fi
        
        # Worktreeが存在するがgit worktreeリストにない場合は再登録
        if ! git worktree list | grep -q "$worktree_dir"; then
            echo -e "${YELLOW}Worktreeの再登録が必要です...${NC}"
            
            # 既存ブランチがあるかチェック
            if git show-ref --verify --quiet refs/heads/"$branch_name"; then
                echo -e "${YELLOW}既存のブランチ ${branch_name} を使用します${NC}"
                if ! git worktree add "$worktree_dir" "$branch_name" 2>/dev/null; then
                    echo -e "${RED}Worktreeの再登録に失敗しました${NC}"
                    echo -e "${YELLOW}手動で確認してください: $worktree_dir${NC}"
                    exit 1
                fi
            else
                echo -e "${YELLOW}新しいブランチ ${branch_name} を作成してWorktreeに関連付けます${NC}"
                if ! git worktree add "$worktree_dir" -b "$branch_name" 2>/dev/null; then
                    echo -e "${RED}Worktreeの作成に失敗しました${NC}"
                    echo -e "${YELLOW}手動で確認してください: $worktree_dir${NC}"
                    exit 1
                fi
            fi
        fi
        
        # Worktreeディレクトリの存在を再確認
        if [ ! -d "$worktree_dir" ]; then
            echo -e "${RED}エラー: Worktreeディレクトリが存在しません: $worktree_dir${NC}"
            exit 1
        fi
        
        # Gitの機能確認（.gitファイルが正しく機能しているかテスト）
        cd "$worktree_dir"
        if ! git status >/dev/null 2>&1; then
            echo -e "${RED}エラー: Worktreeのgit状態が不正です${NC}"
            echo -e "${YELLOW}手動で確認してください: $worktree_dir${NC}"
            exit 1
        fi
        
        # 既存Worktreeのステータス確認
        echo -e "${BLUE}Worktreeの状態:${NC}"
        git status --short
        
        # Gitの状態を確認（未コミットの変更がある場合は警告）
        if ! git diff --quiet || ! git diff --cached --quiet; then
            echo -e "${YELLOW}⚠️  Worktreeに未コミットの変更があります${NC}"
            echo -e "${YELLOW}変更は保持されますが、必要に応じて事前にコミットしてください${NC}"
        fi
        
        # 最新のmainブランチをマージ（競合がある場合はユーザーに委ねる）
        echo -e "${BLUE}mainブランチからの更新を確認中...${NC}"
        git fetch origin main
        
        # マージを試行（競合がある場合は中断）
        if git merge-base --is-ancestor HEAD origin/main; then
            echo -e "${GREEN}既に最新のmainブランチが含まれています${NC}"
        else
            echo -e "${YELLOW}mainブランチからの更新をマージ中...${NC}"
            if ! git merge origin/main --no-edit; then
                echo -e "${RED}⚠️  マージで競合が発生しました${NC}"
                echo -e "${YELLOW}競合を解決してからClaude Codeを実行してください${NC}"
                echo -e "${BLUE}競合解決後は以下のコマンドで続行できます:${NC}"
                echo "  cd $worktree_dir/native-app"
                echo "  $cmd fix $issue_number -k"
                exit 1
            fi
        fi
    else
        # 従来の処理（Worktreeを削除して再作成）
        if [ -d "$worktree_dir" ]; then
            echo -e "${YELLOW}既存のWorktree ${worktree_dir} を削除します...${NC}"
            git worktree remove --force "$worktree_dir" || true
        fi
        
        # 既存ブランチがあるかチェック
        if git show-ref --verify --quiet refs/heads/"$branch_name"; then
            echo -e "${YELLOW}既存のブランチ ${branch_name} を削除して再作成します...${NC}"
            git branch -D "$branch_name" || true
        fi
        
        # 新しいブランチとworktreeを作成
        echo -e "${BLUE}新しいWorktree ${worktree_dir} を作成します...${NC}"
        git worktree add "$worktree_dir" -b "$branch_name"
    fi

    # 依存関係のインストール（keep_worktreeオプションに応じて最適化）
    cd "$worktree_dir/native-app"
    
    if [ "$keep_worktree" = "true" ] && [ -d "node_modules" ] && [ -f "package-lock.json" ]; then
        echo -e "${BLUE}既存のnode_modulesを確認中...${NC}"
        
        # package-lock.jsonの更新時刻とnode_modulesの更新時刻を比較
        if [ "package-lock.json" -nt "node_modules" ]; then
            echo -e "${YELLOW}package-lock.jsonが更新されているため、依存関係を再インストールします...${NC}"
            npm ci --silent --no-audit --no-fund --prefer-offline
        else
            echo -e "${GREEN}依存関係は最新です（インストールをスキップ）${NC}"
        fi
    else
        # 従来の処理（依存関係を新規インストール）
        echo -e "${BLUE}依存関係をインストール中...${NC}"
        
        # npmキャッシュを活用して高速化
        export NPM_CONFIG_CACHE="$HOME/.npm"
        
        # package-lock.jsonが存在する場合はnpm ciを使用（高速・確実）
        if [ -f "package-lock.json" ]; then
            echo -e "${YELLOW}npm ci を使用して依存関係をインストール中（キャッシュ活用）...${NC}"
            npm ci --silent --no-audit --no-fund --prefer-offline
        else
            echo -e "${YELLOW}npm install を使用して依存関係をインストール中（キャッシュ活用）...${NC}"
            npm install --silent --no-audit --no-fund --prefer-offline
        fi
        
        echo -e "${GREEN}依存関係のインストール完了${NC}"
    fi

    # Claude Code初期化チェック（常に実行）
    check_claude_init "$worktree_dir"
    
    # プロンプト作成（keep_worktreeモードの場合は継続作業であることを明記）
    local context_note=""
    if [ "$keep_worktree" = "true" ]; then
        context_note="

## 継続作業について
これは既存のWorktreeを引き継いだ継続作業です。
- 既存の変更やコミット履歴を確認してください
- 前回の作業内容を踏まえて適切に継続してください
- 必要に応じて現在の進捗状況を確認してから作業を進めてください"
    fi

    local prompt="Issue #${issue_number}: ${ISSUE_TITLE}

## 概要
$(echo "$ISSUE_BODY" | head -c 1000)$([ ${#ISSUE_BODY} -gt 1000 ] && echo "...")${context_note}

---
あなたはこのWorktree専属のAIペアプロ開発者です。
作業ディレクトリ: ${worktree_dir}/native-app

手順:
1. Issue内容を分析し、実装プランを立案
2. 適切なテスト（ユニット/E2E）を作成
3. 実装・リファクタリング
4. コミット・プッシュ・PR作成

完了時にPRタイトルに「#${issue_number}」を含めてPR作成してください。"

    # Claude Code実行
    if [ "$use_async" = "true" ]; then
        # tmux モード
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "issue" "$issue_number" "false"
    else
        # tmux モード（同期・アタッチ）
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "issue" "$issue_number" "true"
    fi
}

# プロンプト指定モード
run_prompt_mode() {
    local prompt="$1"
    local use_async="$2"
    
    echo -e "${GREEN}Claude Code プロンプト実行モード${NC}"
    echo -e "${BLUE}プロンプト: $prompt${NC}"
    
    # 同期モードの場合は確認
    if [ "$use_async" != "true" ]; then
        echo ""
        echo -e "${YELLOW}この内容でWorktreeを作成してClaude Codeを実行しますか？ (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}プロンプト実行をキャンセルしました${NC}"
            exit 0
        fi
    fi
    
    # Git状態チェック
    check_git_status
    
    # Worktree設定（プロンプト用）
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local branch_name="prompt/${timestamp}"
    local worktree_dir="$REPO_ROOT/../${PROJECT_NAME}-prompt-${timestamp}"
    
    cd "$REPO_ROOT"
    
    # 新しいブランチとworktreeを作成
    echo -e "${BLUE}新しいWorktree ${worktree_dir} を作成します...${NC}"
    git worktree add "$worktree_dir" -b "$branch_name"

    # 依存関係のインストール
    echo -e "${BLUE}依存関係をインストール中...${NC}"
    cd "$worktree_dir/native-app"
    
    # npmキャッシュを活用して高速化
    export NPM_CONFIG_CACHE="$HOME/.npm"
    
    if [ -f "package-lock.json" ]; then
        echo -e "${YELLOW}npm ci を使用して依存関係をインストール中（キャッシュ活用）...${NC}"
        npm ci --silent --no-audit --no-fund --prefer-offline
    else
        echo -e "${YELLOW}npm install を使用して依存関係をインストール中（キャッシュ活用）...${NC}"
        npm install --silent --no-audit --no-fund --prefer-offline
    fi
    
    echo -e "${GREEN}依存関係のインストール完了${NC}"

    # Claude Code初期化チェック
    check_claude_init "$worktree_dir"
    
    # Claude Code実行
    if [ "$use_async" = "true" ]; then
        # tmux モード
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "prompt" "" "false"
    else
        # tmux モード（同期・アタッチ）
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "prompt" "" "true"
    fi
}

# Issue作成モード（シンプル版）
create_issue() {
    local title="$1"
    local body="$2"
    local auto_solve="$3"
    local use_async="$4"
    
    echo -e "${GREEN}GitHub Issue作成モード${NC}"
    echo ""
    
    # GitHub CLIの確認
    if ! command -v gh &> /dev/null; then
        echo -e "${RED}エラー: GitHub CLI (gh) がインストールされていません${NC}"
        exit 1
    fi
    
    # 認証確認
    if ! gh auth status &> /dev/null; then
        echo -e "${RED}エラー: GitHub CLIで認証されていません${NC}"
        echo "gh auth login を実行してください"
        exit 1
    fi
    
    # 非対話モードの場合
    if [ -n "$title" ] && [ -n "$body" ]; then
        echo -e "${BLUE}非対話モードでIssue作成${NC}"
        issue_title="$title"
        issue_body="$body"
    else
        # 対話モードの場合
        echo -e "${BLUE}対話モードでIssue作成${NC}"
        
        # タイトル入力
        echo -e "${BLUE}Issueタイトルを入力してください:${NC}"
        read -r issue_title
        
        if [ -z "$issue_title" ]; then
            echo -e "${RED}エラー: タイトルが入力されていません${NC}"
            exit 1
        fi
        
        # 本文入力
        echo -e "${BLUE}Issue本文を入力してください（空行で終了）:${NC}"
        issue_body=""
        while IFS= read -r line; do
            if [ -z "$line" ]; then
                break
            fi
            if [ -z "$issue_body" ]; then
                issue_body="$line"
            else
                issue_body="$issue_body"$'\n'"$line"
            fi
        done
    fi
    
    # 確認
    echo ""
    echo -e "${YELLOW}=== Issue作成内容確認 ===${NC}"
    echo -e "${BLUE}タイトル:${NC} $issue_title"
    echo -e "${BLUE}本文:${NC}"
    echo "$issue_body"
    echo ""
    
    if [ "$auto_solve" != "true" ]; then
        echo -e "${YELLOW}この内容でIssueを作成しますか？ (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}Issue作成をキャンセルしました${NC}"
            exit 0
        fi
    fi
    
    # Issue作成
    echo -e "${BLUE}Issueを作成中...${NC}"
    
    # 一時ファイルに本文を保存
    temp_body_file="/tmp/issue_body_$(date +%s).md"
    echo "$issue_body" > "$temp_body_file"
    
    # GitHub CLIでIssue作成
    issue_url=$(gh issue create --title "$issue_title" --body-file "$temp_body_file")
    
    # 一時ファイル削除
    rm -f "$temp_body_file"
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Issue作成完了！${NC}"
        echo -e "${BLUE}URL: $issue_url${NC}"
        
        # Issue番号を抽出
        issue_number=$(echo "$issue_url" | grep -o '[0-9]*$')
        echo -e "${BLUE}Issue番号: #$issue_number${NC}"
        echo ""
        
        # 続けてClaude Codeで解決するか確認
        if [ "$auto_solve" = "true" ]; then
            echo -e "${GREEN}自動解決モードでIssue #$issue_number の解決を開始します...${NC}"
            run_issue_mode "$issue_number" "$use_async"
        else
            echo -e "${YELLOW}このIssueをClaude Codeで解決しますか？ (y/N):${NC}"
            read -r solve_confirm
            
            if [ "$solve_confirm" = "y" ] || [ "$solve_confirm" = "Y" ]; then
                echo -e "${GREEN}Issue #$issue_number の解決を開始します...${NC}"
                run_issue_mode "$issue_number" "$use_async"
            fi
        fi
    else
        echo -e "${RED}❌ Issue作成に失敗しました${NC}"
        exit 1
    fi
}

# コマンドライン引数解析（シンプル版）
parse_create_issue_args() {
    local title=""
    local body=""
    local auto_solve="false"
    local use_async="true"
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --title)
                title="$2"
                shift 2
                ;;
            --body)
                body="$2"
                shift 2
                ;;
            --auto-solve)
                auto_solve="true"
                shift
                ;;
            -s|--sync)
                use_async="false"
                shift
                ;;
            *)
                echo -e "${RED}エラー: 不明なオプション '$1'${NC}"
                echo "使用方法: $cmd issue [--title \"タイトル\"] [--body \"本文\"] [--auto-solve] [-s|--sync]"
                exit 1
                ;;
        esac
    done
    
    create_issue "$title" "$body" "$auto_solve" "$use_async"
} 