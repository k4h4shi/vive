#!/usr/bin/env bash
# vive バッチ処理関連

# バッチモード実行（複数Issue並行処理）
run_batch_mode() {
    local issue_list="$1"
    
    echo -e "${GREEN}バッチモード: 複数Issueの並行処理を開始します${NC}"
    echo ""
    
    # tmuxチェック
    check_tmux
    
    # Git状態チェック
    check_git_status
    
    # Issue番号をカンマで分割
    IFS=',' read -ra ISSUES <<< "$issue_list"
    local issue_count=${#ISSUES[@]}
    
    echo -e "${BLUE}処理対象Issue数: $issue_count${NC}"
    echo -e "${BLUE}Issues: ${ISSUES[*]}${NC}"
    echo ""
    
    # 各Issueの存在チェック
    echo -e "${YELLOW}Issue存在確認中...${NC}"
    local valid_issues=()
    for issue in "${ISSUES[@]}"; do
        issue=$(echo "$issue" | xargs)  # trim whitespace
        if [[ ! "$issue" =~ ^[0-9]+$ ]]; then
            echo -e "${RED}警告: '$issue' は有効なIssue番号ではありません（スキップ）${NC}"
            continue
        fi
        
        if gh issue view "$issue" &> /dev/null; then
            valid_issues+=("$issue")
            echo -e "${GREEN}✓ Issue #$issue${NC}"
        else
            echo -e "${RED}✗ Issue #$issue が見つかりません（スキップ）${NC}"
        fi
    done
    
    if [ ${#valid_issues[@]} -eq 0 ]; then
        echo -e "${RED}エラー: 有効なIssueがありません${NC}"
        exit 1
    fi
    
    echo ""
    echo -e "${YELLOW}${#valid_issues[@]}個のIssueを並行処理します。続行しますか？ (y/N):${NC}"
    read -r confirm
    
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        echo -e "${YELLOW}バッチ処理をキャンセルしました${NC}"
        exit 0
    fi
    
    # 各Issueを非同期モードで実行
    echo ""
    echo -e "${BLUE}並行処理を開始します...${NC}"
    local started_count=0
    
    for issue in "${valid_issues[@]}"; do
        echo ""
        echo -e "${GREEN}[$((started_count + 1))/${#valid_issues[@]}] Issue #$issue の処理を開始...${NC}"
        
        # 非同期モードで実行（サブシェルで独立実行）
        (
            run_issue_mode "$issue" "true"
        ) &
        
        # プロセス起動待機（連続起動による負荷を軽減）
        sleep 3
        
        started_count=$((started_count + 1))
    done
    
    echo ""
    echo -e "${GREEN}✅ ${started_count}個のIssue処理を開始しました${NC}"
    echo ""
    echo -e "${YELLOW}進捗確認コマンド:${NC}"
    echo "  セッション一覧: $cmd sessions"
    echo "  特定のセッションにアタッチ: $cmd attach <issue-number>"
} 