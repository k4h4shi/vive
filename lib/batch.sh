#!/bin/bash
# vive バッチ処理関連

# Execute batch processing: parallel execution of multiple Issues

source "${LIB_DIR}/utils.sh"

process_batch() {
    echo -e "${GREEN}Batch mode: Starting parallel processing for multiple issues${NC}"
    
    # Accept multiple Issue numbers
    if [ $# -eq 0 ]; then
        echo "Usage: $0 batch <issue1> <issue2> <issue3> ..."
        echo "Example: $0 batch 41 42 43"
        return 1
    fi
    
    # Save Issue list
    local issues=("$@")
    local valid_issues=()
    local invalid_issues=()
    
    # Verify Issue existence
    echo
    echo "Checking issues..."
    echo -e "${YELLOW}Verifying issue existence...${NC}"
    for issue in "${issues[@]}"; do
        # Check if it's a number
        if ! [[ "$issue" =~ ^[0-9]+$ ]]; then
            echo -e "${RED}Warning: '$issue' is not a valid issue number (skipping)${NC}"
            invalid_issues+=("$issue")
            continue
        fi
        
        # Use check_issue from issue.sh
        if check_issue_exists "$issue"; then
            valid_issues+=("$issue")
        else
            invalid_issues+=("$issue")
        fi
    done
    
    # Error if no valid Issues
    if [ ${#valid_issues[@]} -eq 0 ]; then
        echo
        echo -e "${RED}Error: No valid issues found${NC}"
        return 1
    fi
    
    # Summary display
    echo
    echo "===== Processing Summary ====="
    echo -e "${GREEN}Valid issues (${#valid_issues[@]}):${NC} ${valid_issues[*]}"
    if [ ${#invalid_issues[@]} -gt 0 ]; then
        echo -e "${RED}Invalid issues (${#invalid_issues[@]}):${NC} ${invalid_issues[*]}"
    fi
    echo "=============================="
    echo
    
    # Parallel execution
    echo -e "${BLUE}Starting parallel processing...${NC}"
    echo
    
    local started_count=0
    for issue in "${valid_issues[@]}"; do
        echo -e "${GREEN}[$((started_count + 1))/${#valid_issues[@]}] Starting issue #$issue...${NC}"
        
        # Start asynchronously
        "$SCRIPT_DIR/vive.sh" fix "$issue" &
        
        # Brief wait between processes (to avoid resource contention)
        sleep 2
        
        ((started_count++))
    done
    
    echo
    echo "=============================="
    echo -e "${GREEN}✅ Started processing for ${started_count} issues${NC}"
    echo
    echo -e "${YELLOW}Progress check commands:${NC}"
    echo "  List sessions:        vive sessions"
    echo "  Attach to session:    vive attach <issue-number>"
    echo "  View logs:            vive logs <issue-number>"
    echo "  Follow logs:          vive logs <issue-number> -f"
    echo
    echo "All processes are running in the background."
    echo "=============================="
} 