#!/usr/bin/env bash
# vive dashboard functionality

create_vive_dashboard() {
    local dashboard_session_name="${PROJECT_NAME}-dashboard"
    local vive_sessions_info=() # "issue_num" の配列

    # アクティブな <PROJECT_NAME>-issue-* セッションから Issue 番号を抽出して配列に格納
    while IFS= read -r session_name; do
        if [[ "$session_name" =~ ^${PROJECT_NAME}-issue-([0-9]+)$ ]]; then
            local issue_num="${BASH_REMATCH[1]}"
            vive_sessions_info+=("$issue_num")
        fi
    done < <(tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" || true)

    if [ ${#vive_sessions_info[@]} -eq 0 ]; then
        echo -e "${YELLOW}No active '${PROJECT_NAME}-issue-*' sessions found to display in the dashboard.${NC}"
        return 1
    fi

    echo -e "${BLUE}Found ${#vive_sessions_info[@]} active ${PROJECT_NAME}-issue session(s). Creating dashboard...${NC}"

    if tmux has-session -t "$dashboard_session_name" 2>/dev/null; then
        echo -e "${YELLOW}Existing dashboard session '$dashboard_session_name' found. Killing it...${NC}"
        tmux kill-session -t "$dashboard_session_name"
    fi

    local first_issue_num=${vive_sessions_info[0]}
    echo -e "${BLUE}Creating new session '$dashboard_session_name' and showing logs for issue #$first_issue_num...${NC}"
    
    local standard_vive_tmux_conf="$LIB_DIR/default.tmux.conf"
    local tmux_command_prefix="tmux"
    if [ -f "$standard_vive_tmux_conf" ]; then
        echo -e "${BLUE}Loading vive standard tmux config: $standard_vive_tmux_conf${NC}"
        command tmux -f "$standard_vive_tmux_conf" new-session -d -s "$dashboard_session_name" -n "${PROJECT_NAME} Logs" -c "$REPO_ROOT" "vive logs \"$first_issue_num\" -f"
    else
        echo -e "${YELLOW}Vive standard tmux config not found at '$standard_vive_tmux_conf'. Using default tmux settings.${NC}"
        command tmux new-session -d -s "$dashboard_session_name" -n "${PROJECT_NAME} Logs" -c "$REPO_ROOT" "vive logs \"$first_issue_num\" -f"
    fi

    tmux select-pane -t "$dashboard_session_name:0.0" -T "Issue #$first_issue_num"

    # Only process additional issues if there are more than 1
    if [ ${#vive_sessions_info[@]} -gt 1 ]; then
        for i in $(seq 1 $((${#vive_sessions_info[@]} - 1))); do
            local current_issue_num=${vive_sessions_info[$i]}
            echo -e "${BLUE}Adding logs for issue #$current_issue_num to dashboard...${NC}"
            tmux split-window -t "$dashboard_session_name:0" -v -c "$REPO_ROOT" "vive logs \"$current_issue_num\" -f"
            tmux select-pane -t "$dashboard_session_name:0.+" -T "Issue #$current_issue_num"
            tmux select-layout -t "$dashboard_session_name:0" tiled
        done
    fi

    local user_input_pane_title="UserInput"
    echo -e "${BLUE}Adding a free command input pane titled '$user_input_pane_title'...${NC}"
    # Split the first pane specifically (0.0) to avoid creating extra panes
    tmux split-window -t "$dashboard_session_name:0.0" -h -p 25
    tmux select-pane -t "$dashboard_session_name:0.+" -T "$user_input_pane_title"
    tmux send-keys -t "$dashboard_session_name:0.+" "cd \"$REPO_ROOT\"" Enter C-l
    tmux send-keys -t "$dashboard_session_name:0.+" "echo 'This is the command input pane. Use '\''vive send <IssueNum> <message>'\'' here.'" Enter

    echo -e "${GREEN}Focused on '$user_input_pane_title' pane.${NC}"
    # Select the UserInput pane for user interaction
    tmux select-pane -t "$dashboard_session_name:0.+"
    tmux select-layout -t "$dashboard_session_name:0" tiled

    echo -e "${GREEN}Dashboard session '$dashboard_session_name' created successfully.${NC}"
    echo -e "${YELLOW}The '$user_input_pane_title' pane is focused for your commands.${NC}"
    echo -e "${YELLOW}To send a message: vive send <IssueNum> <message...>${NC}"

    echo -e "${BLUE}Attaching to dashboard session '$dashboard_session_name' automatically...${NC}"
    tmux attach-session -t "$dashboard_session_name"
} 