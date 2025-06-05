#!/usr/bin/env bash
# vive issue operations

# Check if issue exists
check_issue_exists() {
    local issue_num=$1
    
    if ! command -v gh &> /dev/null; then
        echo -e "${RED}Error: GitHub CLI (gh) is not installed${NC}"
        exit 1
    fi
    
    if gh issue view "$issue_num" &> /dev/null; then
        return 0
    else
        echo -e "${RED}Error: Issue #${issue_num} not found${NC}"
        return 1
    fi
}

# Get issue information
get_issue_info() {
    local issue_num=$1
    
    # Retrieve issue information and export as global variables
    ISSUE_TITLE=$(gh issue view "$issue_num" --json title -q .title)
    ISSUE_BODY=$(gh issue view "$issue_num" --json body -q .body)
    
    echo -e "${GREEN}Issue information retrieved:${NC}"
    echo "Title: $ISSUE_TITLE"
    echo
}

# Issue解決モード
run_issue_mode() {
    local issue_number="$1"
    local use_async="$2"
    local keep_worktree="$3"
    
    if [ -z "$issue_number" ]; then
        echo -e "${RED}Error: Issue number must be specified${NC}"
        echo "Example: $cmd fix 42"
        exit 1
    fi
    
    echo -e "${GREEN}Starting Issue #${issue_number} resolution mode...${NC}"
    
    # Git status check
    check_git_status
    
    # Get issue information
    get_issue_info "$issue_number"
    
    # If in synchronous mode, confirm
    if [ "$use_async" != "true" ]; then
        echo -e "${YELLOW}Issue #${issue_number}: ${ISSUE_TITLE}${NC}"
        if [ "$keep_worktree" = "true" ]; then
            echo -e "${BLUE}This is a keep_worktree mode${NC}"
        fi
        echo -e "${YELLOW}Do you want to create a Worktree with Claude Code? (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}Issue resolution canceled${NC}"
            exit 0
        fi
    fi
    
    # Worktree setup
    local branch_name="issue/${issue_number}"
    local worktree_dir="$REPO_ROOT/../${PROJECT_NAME}-issue-${issue_number}"
    
    # Normalize path (use realpath if directory exists, otherwise manually normalize)
    if [ -d "$worktree_dir" ]; then
        worktree_dir="$(realpath "$worktree_dir")"
    else
        # Manually normalize path (resolve ../)
        worktree_dir="$(cd "$(dirname "$REPO_ROOT")" && pwd)/${PROJECT_NAME}-issue-${issue_number}"
    fi
    
    cd "$REPO_ROOT"
    
    # Worktree processing (branch based on keep_worktree option)
    if [ "$keep_worktree" = "true" ] && [ -d "$worktree_dir" ]; then
        echo -e "${BLUE}Continuing existing Worktree ${worktree_dir}...${NC}"
        
        # Remove problematic &1 file (for redirect error prevention)
        rm -f "$worktree_dir/&1" 2>/dev/null || true
        
        # If .git file does not exist, perform recovery processing
        if [ ! -f "$worktree_dir/.git" ]; then
            echo -e "${YELLOW}⚠️  .git file not found in Worktree. Performing recovery...${NC}"
            
            # Recreate worktree's gitdir path
            local git_worktree_path="$REPO_ROOT/.git/worktrees/${PROJECT_NAME}-issue-${issue_number}"
            if [ -d "$git_worktree_path" ]; then
                echo -e "${BLUE}.git file recovery in progress...${NC}"
                echo "gitdir: $git_worktree_path" > "$worktree_dir/.git"
                echo -e "${GREEN}✅ Worktree recovery completed${NC}"
            else
                echo -e "${RED}Error: Worktree metadata not found: $git_worktree_path${NC}"
                exit 1
            fi
        fi
        
        # If Worktree exists but is not in git worktree list, re-register
        if ! git worktree list | grep -q "$worktree_dir"; then
            echo -e "${YELLOW}Re-registration of Worktree is required...${NC}"
            
            # Check if existing branch exists
            if git show-ref --verify --quiet refs/heads/"$branch_name"; then
                echo -e "${YELLOW}Using existing branch ${branch_name}...${NC}"
                if ! git worktree add "$worktree_dir" "$branch_name" 2>/dev/null; then
                    echo -e "${RED}Failed to re-register Worktree${NC}"
                    echo -e "${YELLOW}Please manually check: $worktree_dir${NC}"
                    exit 1
                fi
            else
                echo -e "${YELLOW}Creating new branch ${branch_name} and associating Worktree...${NC}"
                if ! git worktree add "$worktree_dir" -b "$branch_name" 2>/dev/null; then
                    echo -e "${RED}Failed to create Worktree${NC}"
                    echo -e "${YELLOW}Please manually check: $worktree_dir${NC}"
                    exit 1
                fi
            fi
        fi
        
        # Confirm Worktree directory existence
        if [ ! -d "$worktree_dir" ]; then
            echo -e "${RED}Error: Worktree directory does not exist: $worktree_dir${NC}"
            exit 1
        fi
        
        # Git feature check (test if .git file is correctly functioning)
        cd "$worktree_dir"
        if ! git status >/dev/null 2>&1; then
            echo -e "${RED}Error: Worktree git status is incorrect${NC}"
            echo -e "${YELLOW}Please manually check: $worktree_dir${NC}"
            exit 1
        fi
        
        # Confirm Worktree status
        echo -e "${BLUE}Worktree status:${NC}"
        git status --short
        
        # Git status check (warn if there are uncommitted changes)
        if ! git diff --quiet || ! git diff --cached --quiet; then
            echo -e "${YELLOW}⚠️  There are uncommitted changes in Worktree${NC}"
            echo -e "${YELLOW}Changes will be preserved but please commit if necessary${NC}"
        fi
        
                 # Check for latest main branch update (if conflicts, leave to user)
         echo -e "${BLUE}Checking for updates from main branch...${NC}"
         git fetch origin main
         
         # Try merge (if conflicts, interrupt)
         if git merge-base --is-ancestor HEAD origin/main; then
             echo -e "${GREEN}Already contains latest main branch${NC}"
         else
             echo -e "${YELLOW}Merging updates from main branch...${NC}"
             if ! git merge origin/main --no-edit; then
                 echo -e "${RED}⚠️   Merge conflict occurred${NC}"
                 echo -e "${YELLOW}Please resolve conflicts before running Claude Code${NC}"
                 echo -e "${BLUE}After resolving conflicts, you can continue with the following command:${NC}"
                 echo "  cd $worktree_dir"
                 echo "  $cmd fix $issue_number -k"
                 exit 1
             fi
         fi
    else
        # Previous processing (delete and recreate Worktree)
        if [ -d "$worktree_dir" ]; then
            echo -e "${YELLOW}Deleting existing Worktree ${worktree_dir}...${NC}"
            git worktree remove --force "$worktree_dir" || true
        fi
        
        # Check if existing branch exists
        if git show-ref --verify --quiet refs/heads/"$branch_name"; then
            echo -e "${YELLOW}Deleting existing branch ${branch_name} and recreating...${NC}"
            git branch -D "$branch_name" || true
        fi
        
        # Create new branch and worktree
        echo -e "${BLUE}Creating new Worktree ${worktree_dir}...${NC}"
        git worktree add "$worktree_dir" -b "$branch_name"
    fi

    # Dependency installation (optimize based on keep_worktree option)
    cd "$worktree_dir"
    
    if [ "$keep_worktree" = "true" ] && [ -d "node_modules" ] && [ -f "package-lock.json" ]; then
        echo -e "${BLUE}Checking existing node_modules...${NC}"
        
        # Compare package-lock.json update time with node_modules update time
        if [ "package-lock.json" -nt "node_modules" ]; then
            echo -e "${YELLOW}package-lock.json updated, reinstalling dependencies...${NC}"
            npm ci --silent --no-audit --no-fund --prefer-offline
        else
            echo -e "${GREEN}Dependencies are up-to-date (skip installation)${NC}"
        fi
    else
        # Previous processing (new dependency installation)
        echo -e "${BLUE}Installing dependencies...${NC}"
        
        # Use npm cache for faster processing
        export NPM_CONFIG_CACHE="$HOME/.npm"
        
        # Use npm ci if package-lock.json exists (fast and reliable)
        if [ -f "package-lock.json" ]; then
            echo -e "${YELLOW}Installing dependencies using npm ci (cache usage)...${NC}"
            npm ci --silent --no-audit --no-fund --prefer-offline
        else
            echo -e "${YELLOW}Installing dependencies using npm install (cache usage)...${NC}"
            npm install --silent --no-audit --no-fund --prefer-offline
        fi
        
        echo -e "${GREEN}Dependency installation completed${NC}"
    fi

    # Claude Code initialization check (always run)
    check_claude_init "$worktree_dir"
    
    # Prompt creation (if keep_worktree mode, note it's a continuation job)
    local context_note=""
    if [ "$keep_worktree" = "true" ]; then
        context_note="

## Continuation Job Information
This is a continuation job from an existing Worktree.
- Please check existing changes and commit history
- Proceed appropriately based on previous job content
- Please check progress before continuing work"
    fi

    local prompt="Issue #${issue_number}: ${ISSUE_TITLE}

## Summary
$(echo "$ISSUE_BODY" | head -c 1000)$([ ${#ISSUE_BODY} -gt 1000 ] && echo "...")${context_note}

---
You are the AI pair developer for this Worktree.
Work directory: ${worktree_dir}

Steps:
1. Analyze Issue content and create implementation plan
2. Create appropriate tests (unit/E2E)
3. Implement/Refactor
4. Commit/Push/PR creation

Please create PR with \"#${issue_number}\" in PR title when finished."

    # Claude Code execution
    if [ "$use_async" = "true" ]; then
        # tmux mode
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "issue" "$issue_number" "false"
    else
        # tmux mode (synchronous/attach)
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "issue" "$issue_number" "true"
    fi
}

# Prompt mode
run_prompt_mode() {
    local prompt="$1"
    local use_async="$2"
    
    echo -e "${GREEN}Claude Code Prompt Execution Mode${NC}"
    echo -e "${BLUE}Prompt: $prompt${NC}"
    
    # If in synchronous mode, confirm
    if [ "$use_async" != "true" ]; then
        echo ""
        echo -e "${YELLOW}Do you want to create a Worktree with Claude Code? (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}Prompt execution canceled${NC}"
            exit 0
        fi
    fi
    
    # Git status check
    check_git_status
    
    # Worktree setup (for prompt)
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local branch_name="prompt/${timestamp}"
    local worktree_dir="$REPO_ROOT/../${PROJECT_NAME}-prompt-${timestamp}"
    
    cd "$REPO_ROOT"
    
    # Create new branch and worktree
    echo -e "${BLUE}Creating new Worktree ${worktree_dir}...${NC}"
    git worktree add "$worktree_dir" -b "$branch_name"

    # Dependency installation
    echo -e "${BLUE}Installing dependencies...${NC}"
    cd "$worktree_dir"
    
    # Use npm cache for faster processing
    export NPM_CONFIG_CACHE="$HOME/.npm"
    
    if [ -f "package-lock.json" ]; then
        echo -e "${YELLOW}Installing dependencies using npm ci (cache usage)...${NC}"
        npm ci --silent --no-audit --no-fund --prefer-offline
    else
        echo -e "${YELLOW}Installing dependencies using npm install (cache usage)...${NC}"
        npm install --silent --no-audit --no-fund --prefer-offline
    fi
    
    echo -e "${GREEN}Dependency installation completed${NC}"

    # Claude Code initialization check
    check_claude_init "$worktree_dir"
    
    # Claude Code execution
    if [ "$use_async" = "true" ]; then
        # tmux mode
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "prompt" "" "false"
    else
        # tmux mode (synchronous/attach)
        check_tmux
        run_claude_tmux "$prompt" "$worktree_dir" "prompt" "" "true"
    fi
}

# Issue creation mode (simple version)
create_issue() {
    local title="$1"
    local body="$2"
    local auto_solve="$3"
    local use_async="$4"
    
    echo -e "${GREEN}GitHub Issue Creation Mode${NC}"
    echo ""
    
    # GitHub CLI check
    if ! command -v gh &> /dev/null; then
        echo -e "${RED}Error: GitHub CLI (gh) is not installed${NC}"
        exit 1
    fi
    
    # Authentication check
    if ! gh auth status &> /dev/null; then
        echo -e "${RED}Error: GitHub CLI not authenticated${NC}"
        echo "Please run gh auth login"
        exit 1
    fi
    
    # If in non-interactive mode
    if [ -n "$title" ] && [ -n "$body" ]; then
        echo -e "${BLUE}Non-interactive mode for Issue creation${NC}"
        issue_title="$title"
        issue_body="$body"
    else
        # If in interactive mode
        echo -e "${BLUE}Interactive mode for Issue creation${NC}"
        
        # Title input
        echo -e "${BLUE}Please enter Issue title:${NC}"
        read -r issue_title
        
        if [ -z "$issue_title" ]; then
            echo -e "${RED}Error: Title not entered${NC}"
            exit 1
        fi
        
        # Body input
        echo -e "${BLUE}Please enter Issue body (empty line to end):${NC}"
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
    
    # Confirm
    echo ""
    echo -e "${YELLOW}=== Issue Creation Content Confirmation ===${NC}"
    echo -e "${BLUE}Title:${NC} $issue_title"
    echo -e "${BLUE}Body:${NC}"
    echo "$issue_body"
    echo ""
    
    if [ "$auto_solve" != "true" ]; then
        echo -e "${YELLOW}Do you want to create this Issue? (y/N):${NC}"
        read -r confirm
        
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo -e "${YELLOW}Issue creation canceled${NC}"
            exit 0
        fi
    fi
    
    # Issue creation
    echo -e "${BLUE}Creating Issue...${NC}"
    
    # Save body to temporary file
    temp_body_file="/tmp/issue_body_$(date +%s).md"
    echo "$issue_body" > "$temp_body_file"
    
    # GitHub CLI for Issue creation
    issue_url=$(gh issue create --title "$issue_title" --body-file "$temp_body_file")
    
    # Delete temporary file
    rm -f "$temp_body_file"
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Issue creation completed!${NC}"
        echo -e "${BLUE}URL: $issue_url${NC}"
        
        # Extract issue number
        issue_number=$(echo "$issue_url" | grep -o '[0-9]*$')
        echo -e "${BLUE}Issue number: #$issue_number${NC}"
        echo ""
        
        # Check if Claude Code should be used to solve the issue
        if [ "$auto_solve" = "true" ]; then
            echo -e "${GREEN}Starting automatic solution for Issue #$issue_number...${NC}"
            run_issue_mode "$issue_number" "$use_async"
        else
            echo -e "${YELLOW}Do you want to solve this Issue with Claude Code? (y/N):${NC}"
            read -r solve_confirm
            
            if [ "$solve_confirm" = "y" ] || [ "$solve_confirm" = "Y" ]; then
                echo -e "${GREEN}Starting solution for Issue #$issue_number...${NC}"
                run_issue_mode "$issue_number" "$use_async"
            fi
        fi
    else
        echo -e "${RED}❌ Issue creation failed${NC}"
        exit 1
    fi
}

# Command line argument parsing (simple version)
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
                echo -e "${RED}Error: Unknown option '$1'${NC}"
                echo "Usage: $cmd issue [--title \"Title\"] [--body \"Body\"] [--auto-solve] [-s|--sync]"
                exit 1
                ;;
        esac
    done
    
    create_issue "$title" "$body" "$auto_solve" "$use_async"
} 