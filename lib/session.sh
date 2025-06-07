#!/usr/bin/env bash
# vive tmux session management and watchdog related

# Common watchdog startup process
start_watchdog_process() {
    local session_name="$1"
    local delay_seconds="${2:-3}"  # Default 3 seconds wait
    local force_restart="${3:-false}"  # Force restart flag
    
    if [ -z "$session_name" ]; then
        echo -e "${RED}Error: Session name not specified${NC}"
        return 1
    fi
    
    # Define expect script path
    local expect_script="$SCRIPT_DIR/watchdog.exp"
    local log_file="/tmp/claude_session_watchdog_${session_name}_$(date +%s).log"
    
    # Check if expect script exists
    if [ ! -f "$expect_script" ]; then
        echo -e "${RED}Warning: expect script not found: $expect_script${NC}"
        return 1
    fi
    
    # Check if tmux session exists
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}Warning: tmux session '$session_name' not found${NC}"
        return 1
    fi
    
    # Check and handle existing watchdog process
    local existing_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    if [ -n "$existing_pid" ]; then
        if [ "$force_restart" = "true" ]; then
            echo -e "${YELLOW}Stopping existing watchdog process (PID: $existing_pid) and starting new one...${NC}"
            kill "$existing_pid" 2>/dev/null || true
            sleep 1
            kill -9 "$existing_pid" 2>/dev/null || true
        else
            echo -e "${YELLOW}Watchdog process is already running (PID: $existing_pid)${NC}"
            echo -e "${BLUE}Starting additional watchdog process...${NC}"
        fi
    fi
    
    # Wait for Claude Code to start
    if [ "$delay_seconds" -gt 0 ]; then
        echo -e "${YELLOW}Waiting ${delay_seconds} seconds for Claude Code to start before launching watchdog...${NC}"
        sleep "$delay_seconds"
    fi
    
    # Start watchdog process
    echo -e "${BLUE}Starting watchdog process...${NC}"
    nohup expect "$expect_script" attach "$session_name" "$log_file" > /dev/null 2>&1 &
    local watchdog_pid=$!
    
    # Verify startup
    sleep 1
    if ps -p $watchdog_pid > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Watchdog process started (PID: $watchdog_pid)${NC}"
        return 0
    else
        echo -e "${RED}❌ Failed to start watchdog process${NC}"
        echo -e "${YELLOW}Log file: $log_file${NC}"
        return 1
    fi
}

# Get session status
get_session_status() {
    local session_name="$1"
    local log_file="/tmp/claude_session_${session_name}_*.log"
    
    # Check for completion flag file
    if [ -f "/tmp/claude_completed_$session_name" ]; then
        echo "idle"
        return
    fi
    
    # Check last update time from expect log file
    local latest_log=$(ls -t $log_file 2>/dev/null | head -1)
    if [ -n "$latest_log" ] && [ -f "$latest_log" ]; then
        # Get last modified time of log file
        local last_modified=$(stat -f %m "$latest_log" 2>/dev/null || stat -c %Y "$latest_log" 2>/dev/null)
        local current_time=$(date +%s)
        local diff=$((current_time - last_modified))
        
        # If no update for more than 30 seconds, consider idle
        if [ "$diff" -gt 30 ]; then
            echo "idle"
        else
            echo "running"
        fi
    else
        # Judge from tmux pane content
        local output=$(tmux capture-pane -t "$session_name" -p 2>/dev/null | tail -10)
        if [ -z "$output" ]; then
            echo "idle"
        else
            echo "running"
        fi
    fi
}

# Show tmux sessions list (with status integration)
show_tmux_sessions() {
    echo -e "${GREEN}Active Claude Code tmux sessions:${NC}"
    echo ""
    
    local has_sessions=false
    
    # Get tmux session list (issue-* only)
    for session in $(tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" || true); do
        has_sessions=true
        local created=$(tmux list-sessions -F "#{session_name} #{session_created}" 2>/dev/null | grep "^$session " | awk '{print $2}')
        local created_date=$(date -r "$created" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "Unknown")
        
        # Get session status
        local status=$(get_session_status "$session")
        
        echo -e "${BLUE}Session: $session${NC}"
        echo "  Created: $created_date"
        
        # Color-code based on status
        if [ "$status" = "idle" ]; then
            echo -e "  Status: ${YELLOW}$status${NC}"
        else
            echo -e "  Status: ${GREEN}$status${NC}"
        fi
        
        # Check expect process
        local expect_running="none"
        if ps aux | grep "expect.*$session" | grep -v grep > /dev/null 2>&1; then
            expect_running="running"
        fi
        echo "  Expect process: $expect_running"
        
        # Show Issue information
        if [[ "$session" =~ ^${PROJECT_NAME}-issue-([0-9]+)$ ]]; then
            local issue_num="${BASH_REMATCH[1]}"
            local issue_title=$(gh issue view "$issue_num" --json title -q .title 2>/dev/null || echo "Failed to retrieve")
            echo "  Issue: #$issue_num - $issue_title"
            
            # Worktree information
            local worktree_dir="$REPO_ROOT/.vive/issues/${issue_num}"
            if [ -d "$worktree_dir" ]; then
                echo "  Worktree: $worktree_dir"
            fi
        fi
        
        echo ""
    done
    
    if [ "$has_sessions" = false ]; then
        echo -e "${YELLOW}No active sessions${NC}"
    else
        echo -e "${YELLOW}To attach to a session: $cmd attach <issue-number>${NC}"
    fi
}

# Control expect process
control_expect_process() {
    local session_identifier="$1"
    local action="$2"  # pause, resume, stop
    local session_name=""
    
    # Determine session name (if number, add issue-, otherwise use as is)
    if [[ "$session_identifier" =~ ^[0-9]+$ ]]; then
        session_name="${PROJECT_NAME}-issue-${session_identifier}"
    else
        session_name="$session_identifier"
    fi
    
    # Get expect process PID
    local expect_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    
    if [ -z "$expect_pid" ]; then
        echo -e "${YELLOW}expect process not found (may have already terminated)${NC}"
        return 1
    fi
    
    case "$action" in
        "pause")
            echo -e "${YELLOW}Pausing expect process (PID: $expect_pid)...${NC}"
            kill -STOP "$expect_pid"
            echo -e "${GREEN}✅ Expect process paused${NC}"
            ;;
        "resume")
            echo -e "${YELLOW}Resuming expect process (PID: $expect_pid)...${NC}"
            kill -CONT "$expect_pid"
            echo -e "${GREEN}✅ Expect process resumed${NC}"
            ;;
        "stop")
            echo -e "${YELLOW}Stopping expect process (PID: $expect_pid)...${NC}"
            kill "$expect_pid" 2>/dev/null || true
            sleep 1
            # Force kill if still alive
            kill -9 "$expect_pid" 2>/dev/null || true
            echo -e "${GREEN}✅ Expect process stopped${NC}"
            ;;
        *)
            echo -e "${RED}Error: Unknown action '$action'${NC}"
            return 1
            ;;
    esac
}

# tmux session attach (improved version)
attach_tmux_session() {
    local session_identifier="$1"
    local session_name=""
    
    # Determine session name (if number, add issue-, otherwise use as is)
    if [[ "$session_identifier" =~ ^[0-9]+$ ]]; then
        session_name="${PROJECT_NAME}-issue-${session_identifier}"
    else
        session_name="$session_identifier"
    fi
    
    # Check session existence
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}Error: Session '$session_name' not found${NC}"
        echo ""
        echo -e "${YELLOW}Available sessions:${NC}"
        tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" || echo "No active sessions"
        exit 1
    fi
    
    # Check expect process
    local expect_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    if [ -n "$expect_pid" ]; then
        echo -e "${BLUE}expect process (PID: $expect_pid) is running${NC}"
        echo -e "${GREEN}User attach temporarily disables automatic approval${NC}"
        echo -e "${YELLOW}Starting notification temporarily disabled${NC}"
    fi
    
    echo -e "${GREEN}Attaching to session '$session_name'...${NC}"
    echo -e "${YELLOW}To detach: Ctrl+B, D${NC}"
    sleep 1
    
    # Attach to tmux session
    tmux attach-session -t "$session_name"
    
    echo -e "${GREEN}Detached from session${NC}"
}

# tmux session log display
show_tmux_logs() {
    local session_identifier="$1"
    local follow_mode="$2"
    local session_name=""
    
    # Determine session name (if number, add issue-, otherwise use as is)
    if [[ "$session_identifier" =~ ^[0-9]+$ ]]; then
        session_name="${PROJECT_NAME}-issue-${session_identifier}"
    else
        session_name="$session_identifier"
    fi
    
    # Check session existence
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}Error: Session '$session_name' not found${NC}"
        exit 1
    fi
    
    if [ "$follow_mode" = "true" ]; then
        echo -e "${GREEN}Watching session '$session_name' in real-time${NC}"
        echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
        echo ""
        
        # watch command for real-time display
        watch -n 1 "tmux capture-pane -t '$session_name:0.0' -p 2>/dev/null || echo 'Session $session_name not found'"
    else
        echo -e "${GREEN}Session '$session_name' log (latest 50 lines):${NC}"
        echo ""
        
        # Capture tmux pane content
        tmux capture-pane -t "$session_name:0.0" -S -50 -E -1 -p
    fi
}

# Claude Code tmux execution
run_claude_tmux() {
    local prompt="$1"
    local worktree_dir="$2"
    local mode="$3"
    local issue_number="$4"
    local should_attach="$5"  # New argument: Attach to synchronous mode
    
    # Determine session name
    local session_name=""
    if [ -n "$issue_number" ] && [ "$issue_number" != "" ]; then
        session_name="${PROJECT_NAME}-issue-${issue_number}"
    else
        # Timestamp for prompt mode
        local timestamp=$(date +%Y%m%d_%H%M%S)
        session_name="prompt-${timestamp}"
    fi
    
    if [ "$should_attach" = "true" ]; then
        echo -e "${GREEN}Starting synchronous execution in tmux session '$session_name'...${NC}"
    else
        echo -e "${GREEN}Starting asynchronous execution in tmux session '$session_name'...${NC}"
    fi
    echo -e "${BLUE}Working directory: $worktree_dir${NC}"
    
    # Remove existing session if exists
    if tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${YELLOW}Removing existing session '$session_name'...${NC}"
        tmux kill-session -t "$session_name"
        # Wait for session to fully terminate
        sleep 1
    fi
    
    # Define log file path
    local log_file="/tmp/claude_session_${session_name}_$(date +%s).log"
    
    # Check working directory existence
    if [ ! -d "$worktree_dir" ]; then
        echo -e "${RED}Error: Working directory does not exist: $worktree_dir${NC}"
        exit 1
    fi
    
    # Check MCP config existence
    local mcp_config_file
    mcp_config_file=$(get_mcp_config_path)
    if [ ! -f "$mcp_config_file" ]; then
        echo -e "${RED}Error: MCP config file not found: $mcp_config_file${NC}"
        exit 1
    fi
    
    # Create tmux session (Claude Code not yet started)
    echo -e "${YELLOW}Creating tmux session...${NC}"
    
    tmux new-session -d -s "$session_name" -c "$worktree_dir" \
        "echo -e '${GREEN}Claude Code session preparation...${NC}'; \
         echo -e '${BLUE}Session: $session_name${NC}'; \
         echo -e '${BLUE}Working directory: \$(pwd)${NC}'; \
         echo ''; \
         echo -e '${YELLOW}Preparing to start Claude Code with new shell-based approach...${NC}'; \
         echo ''; \
         exec bash"
    
    # Verify session creation
    sleep 1
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}❌ Failed to create tmux session${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ tmux session '$session_name' created${NC}"
    
    # Define expect script path
    local expect_script="$SCRIPT_DIR/watchdog.exp"
    
    # Start Claude Code and send prompt directly
    send_prompt_to_claude "$session_name" "$prompt" "$mcp_config_file"
    
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to send prompt to Claude Code${NC}"
        exit 1
    fi
    
    echo ""
    
    # Optional: Start watchdog process for auto-approval only (not for prompt delivery)
    echo -e "${BLUE}Starting watchdog for auto-approval (prompt delivery is now handled by shell)...${NC}"
    start_watchdog_process "$session_name" 3 true
    
    # If synchronous mode, attach
    if [ "$should_attach" = "true" ]; then
        echo -e "${YELLOW}Waiting a few seconds before attaching to session...${NC}"
        sleep 3
        echo -e "${GREEN}Attaching to session '$session_name'...${NC}"
        echo -e "${YELLOW}To detach: Ctrl+B, D${NC}"
        sleep 1
        
        # Attach to tmux session
        tmux attach-session -t "$session_name"
    else
        # If asynchronous mode, report status
        echo ""
        echo -e "${GREEN}✅ Started Claude Code asynchronously${NC}"
        echo ""
        echo -e "${YELLOW}Operation methods:${NC}"
        echo "  Session check: $cmd sessions"
        if [ -n "$issue_number" ] && [ "$issue_number" != "" ]; then
            echo "  Attach: $cmd attach $issue_number"
            echo "  Log display: $cmd logs $issue_number"
        else
            echo "  Attach: $cmd attach $session_name"
            echo "  Log display: $cmd logs $session_name"
        fi
        echo ""
        echo -e "${BLUE}Hint: Progress will be announced via terminal notification or say command${NC}"
    fi
}

# Reattach expect process
reattach_expect_process() {
    local session_identifier="$1"
    local session_name="${PROJECT_NAME}-issue-${session_identifier}"
    
    # Check session existence
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}Error: Session '$session_name' not found${NC}"
        echo ""
        echo -e "${YELLOW}Available sessions:${NC}"
        tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" || echo "No active sessions"
        return 1
    fi
    
    # Check existing expect process
    local existing_pid=$(ps aux | grep "expect.*$session_name" | grep -v grep | awk '{print $2}' | head -1)
    if [ -n "$existing_pid" ]; then
        echo -e "${YELLOW}Existing expect process (PID: $existing_pid) found${NC}"
        echo -e "${BLUE}Stop existing process before starting new one? (Y/n):${NC}"
        read -r stop_existing
        
        if [ "$stop_existing" != "n" ] && [ "$stop_existing" != "N" ]; then
            echo -e "${YELLOW}Stopping existing expect process...${NC}"
            kill "$existing_pid" 2>/dev/null || true
            sleep 1
            kill -9 "$existing_pid" 2>/dev/null || true
        else
            echo -e "${YELLOW}Reattach canceled${NC}"
            return 0
        fi
    fi
    
    # Define reattach expect script path
    local expect_script="$SCRIPT_DIR/watchdog.exp"
    local log_file="/tmp/claude_session_reattach_${session_name}_$(date +%s).log"
    
    # Check expect script existence
    if [ ! -f "$expect_script" ]; then
        echo -e "${RED}Error: expect script not found: $expect_script${NC}"
        return 1
    fi
    
    # Run expect script in background (attach mode)
    echo -e "${YELLOW}Reattaching expect process (background)...${NC}"
    nohup expect "$expect_script" attach "$session_name" "$log_file" > /dev/null 2>&1 &
    local new_pid=$!
    
    echo -e "${GREEN}✅ Started new expect process (PID: $new_pid)${NC}"
    echo ""
    echo -e "${YELLOW}Detailed information:${NC}"
    echo "  Session: $session_name"
    echo "  expect log: $log_file"
    echo ""
    echo -e "${BLUE}expect process management:${NC}"
    echo "  Pause: $cmd expect-pause $session_identifier"
    echo "  Resume: $cmd expect-resume $session_identifier"
    echo "  Stop: $cmd expect-stop $session_identifier"
    echo ""
    echo -e "${GREEN}expect process resumed automatic response${NC}"
}

# Send prompt directly to Claude Code via tmux (new shell-based approach)
send_prompt_to_claude() {
    local session_name="$1"
    local prompt="$2"
    local mcp_config_file="$3"
    local max_wait_time=60  # Maximum wait time in seconds
    local wait_interval=2   # Check interval in seconds
    
    echo -e "${BLUE}Starting Claude Code and sending prompt directly...${NC}"
    
    # Start Claude Code in the tmux session with Claude 4 Opus as default model
    tmux send-keys -t "$session_name" "ANTHROPIC_MODEL=claude-opus-4-20250514 claude --mcp-config '$mcp_config_file'" C-m
    
    # Wait for Claude Code to be ready (look for prompt indicator)
    local elapsed=0
    local claude_ready=false
    
    while [ $elapsed -lt $max_wait_time ]; do
        sleep $wait_interval
        elapsed=$((elapsed + wait_interval))
        
        # Capture tmux output to check if Claude is ready
        local output=$(tmux capture-pane -t "$session_name" -p 2>/dev/null || echo "")
        
        # Check for various ready indicators
        if [[ "$output" =~ (\?|\>|\$|Welcome|Ready) ]] && [[ ! "$output" =~ (Loading|Starting|Initializing) ]]; then
            claude_ready=true
            echo -e "${GREEN}Claude Code is ready (${elapsed}s)${NC}"
            break
        fi
        
        echo -e "${YELLOW}Waiting for Claude Code to be ready... (${elapsed}s/${max_wait_time}s)${NC}"
    done
    
    if [ "$claude_ready" = "false" ]; then
        echo -e "${YELLOW}Warning: Claude Code readiness timeout. Proceeding anyway...${NC}"
    fi
    
    # Additional short wait to ensure UI is stable
    sleep 2
    
    # Send the prompt directly
    echo -e "${BLUE}Sending prompt to Claude Code...${NC}"
    tmux send-keys -t "$session_name" -l "$prompt"
    tmux send-keys -t "$session_name" C-m
    
    echo -e "${GREEN}✅ Prompt sent successfully${NC}"
    return 0
}

# Put session in watchdog state (simple wrapper)
watch_session() {
    local issue_number="$1"
    local session_name="${PROJECT_NAME}-issue-${issue_number}"
    
    # Check session existence
    if ! tmux has-session -t "$session_name" 2>/dev/null; then
        echo -e "${RED}Error: Session '$session_name' not found${NC}"
        echo ""
        echo -e "${YELLOW}Available sessions:${NC}"
        tmux list-sessions -F "#{session_name}" 2>/dev/null | grep -E "^${PROJECT_NAME}-issue-" || echo "No active sessions"
        return 1
    fi
    
    echo -e "${BLUE}Reattaching watchdog expect process to session '$session_name'...${NC}"
    
    # Use the common watchdog startup function to reattach expect process
    start_watchdog_process "$session_name" 0 true
    
    echo -e "${GREEN}✅ Watchdog expect process reattached to session '$session_name'${NC}"
    echo ""
    echo -e "${YELLOW}Session management commands:${NC}"
    echo "  Status: $cmd sessions"
    echo "  Attach: $cmd attach $issue_number"
    echo "  Logs: $cmd logs $issue_number"
    echo "  Send message: $cmd send $issue_number <message>"
} 