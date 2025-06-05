#!/usr/bin/env bash
# vive CLI

set -e

# Load library files
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/lib"

# Load dependencies in order (important: load dependent ones later)
source "$SCRIPT_DIR/lib/utils.sh"      # Common utilities (colors, REPO_ROOT, basic functions)
source "$SCRIPT_DIR/lib/git.sh"        # Git operations
source "$SCRIPT_DIR/lib/session.sh"    # tmux session management
source "$SCRIPT_DIR/lib/issue.sh"      # Issue processing
source "$SCRIPT_DIR/lib/cleanup.sh"    # Cleanup operations
source "$SCRIPT_DIR/lib/batch.sh"      # Batch processing

# Main process
main() {
    if [ $# -eq 0 ]; then
        # No arguments: show help
        usage
    elif [ "$1" = "help" ] || [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
        # Show help
        usage
    elif [ "$1" = "batch" ]; then
        # Batch mode (multiple issues in parallel)
        shift  # Remove "batch"
        process_batch "$@"
    elif [ "$1" = "sessions" ]; then
        # tmux session list
        check_requirements
        show_tmux_sessions
    elif [ "$1" = "attach" ]; then
        # Attach to tmux session
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify issue number${NC}"
            echo "Example: $cmd attach 42"
            exit 1
        fi
        check_requirements
        attach_tmux_session "$2"
    elif [ "$1" = "logs" ]; then
        # Show tmux session logs
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify issue number${NC}"
            echo "Example: $cmd logs 42"
            echo "Example (real-time): $cmd logs 42 --follow"
            exit 1
        fi
        
        local issue_identifier="$2"
        local follow_mode="false"
        
        # Parse options
        shift 2  # Remove "logs" and issue_identifier
        while [[ $# -gt 0 ]]; do
            case $1 in
                -f|--follow)
                    follow_mode="true"
                    shift
                    ;;
                *)
                    echo -e "${RED}Error: Unknown option '$1'${NC}"
                    echo "Available options: --follow (-f)"
                    exit 1
                    ;;
            esac
        done
        
        check_requirements
        show_tmux_logs "$issue_identifier" "$follow_mode"
    elif [ "$1" = "cleanup" ]; then
        # Worktree cleanup
        shift  # Remove "cleanup"
        run_cleanup "$@"
    elif [ "$1" = "fix" ]; then
        # Issue resolution mode
        local issue_number="$2"
        local use_async="true"
        local keep_worktree="false"
        
        # Parse options
        shift 2  # Remove "fix" and issue_number
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
                    echo -e "${RED}Error: Unknown option '$1'${NC}"
                    exit 1
                    ;;
            esac
        done
        
        run_issue_mode "$issue_number" "$use_async" "$keep_worktree"
    elif [ "$1" = "issue" ]; then
        # Issue creation mode
        if [ $# -eq 1 ]; then
            # No arguments: interactive mode
            create_issue
        else
            # With arguments: non-interactive mode
            shift  # Remove "issue"
            parse_create_issue_args "$@"
        fi
    elif [ "$1" = "expect-pause" ]; then
        # Pause expect process
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify issue number or session name${NC}"
            echo "Example: $cmd expect-pause 42"
            exit 1
        fi
        control_expect_process "$2" "pause"
    elif [ "$1" = "expect-resume" ]; then
        # Resume expect process
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify issue number or session name${NC}"
            echo "Example: $cmd expect-resume 42"
            exit 1
        fi
        control_expect_process "$2" "resume"
    elif [ "$1" = "expect-stop" ]; then
        # Stop expect process
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify issue number or session name${NC}"
            echo "Example: $cmd expect-stop 42"
            exit 1
        fi
        control_expect_process "$2" "stop"
    elif [ "$1" = "expect-reattach" ]; then
        # Reattach expect process
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify issue number or session name${NC}"
            echo "Example: $cmd expect-reattach 42"
            exit 1
        fi
        reattach_expect_process "$2"
    elif [ "$1" = "watchdog" ]; then
        # Watchdog recovery (in case it's missing)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify issue number${NC}"
            echo "Example: $cmd watchdog 42"
            exit 1
        fi
        
        local issue_number="$2"
        watch_session "$issue_number"
    else
        # Unknown command
        echo -e "${RED}Error: Unknown command '$1'${NC}"
        echo ""
        usage
        exit 1
    fi
}

# Execute script
main "$@" 