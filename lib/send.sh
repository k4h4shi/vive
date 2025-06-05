#!/usr/bin/env bash
# vive send command functionality

send_message_to_vive_session() {
    if [ "$#" -lt 2 ]; then
        echo -e "${RED}Usage: vive send <TargetIssueNumber> <message...>${NC}"
        echo -e "${YELLOW}Example: vive send 42 Please create a pull request.${NC}"
        return 1
    fi

    local target_issue_num="$1"
    shift
    local message_to_send="$*"
    # PROJECT_NAME is expected to be defined in utils.sh and sourced globally
    local target_session_name="${PROJECT_NAME}-issue-${target_issue_num}"

    # Check if the target session exists
    if ! tmux has-session -t "$target_session_name" 2>/dev/null; then
        echo -e "${RED}Error: Target session '$target_session_name' not found.${NC}"
        echo -e "${YELLOW}Make sure the issue session is active. You can check with 'vive sessions'.${NC}"
        return 1
    fi

    echo -e "${BLUE}Sending message to session '$target_session_name': '$message_to_send'${NC}"
    # Send text first using literal mode, then send Enter separately (like watchdog.exp)
    tmux send-keys -t "${target_session_name}" -l "$message_to_send"
    tmux send-keys -t "${target_session_name}" C-m

    echo -e "${GREEN}Message sent to session '$target_session_name'.${NC}"
} 