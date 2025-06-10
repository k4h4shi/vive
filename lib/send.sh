#!/usr/bin/env bash
# vive send command functionality

send_message_to_vive_session() {
    if [ "$#" -lt 2 ]; then
        echo -e "${RED}Usage: vive send <TargetIdentifier> <message...>${NC}"
        echo -e "${YELLOW}Example: vive send 42 Please create a pull request.${NC}"
        echo -e "${YELLOW}Example: vive send main Please create a pull request.${NC}"
        return 1
    fi

    local target_identifier="$1"
    shift
    local message_to_send="$*"
    local target_session_name=""
    
    # Determine session name
    if [[ "$target_identifier" =~ ^[0-9]+$ ]]; then
        # Issue number
        target_session_name="${PROJECT_NAME}-issue-${target_identifier}"
    elif [ "$target_identifier" = "main" ]; then
        # Main branch session
        target_session_name=$(get_main_session_name)
        if [ $? -ne 0 ] || [ -z "$target_session_name" ]; then
            echo -e "${RED}Error: No main branch session found${NC}"
            echo -e "${YELLOW}Available sessions:${NC}"
            tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-(issue|main)-" || echo "No active sessions"
            return 1
        fi
    else
        # Direct session name
        target_session_name="$target_identifier"
    fi

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