#!/usr/bin/env bash
# vive common utilities

# Color output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Get repository root directory
get_repo_root() {
    if ! command -v git &> /dev/null; then
        echo -e "${RED}Error: git is not installed${NC}"
        exit 1
    fi
    
    if ! git rev-parse --git-dir > /dev/null 2>&1; then
        echo -e "${RED}Error: Current directory is not a Git repository${NC}"
        exit 1
    fi
    
    local repo_root=$(git rev-parse --show-toplevel 2>/dev/null)
    if [ -z "$repo_root" ]; then
        echo -e "${RED}Error: Could not determine Git repository root${NC}"
        exit 1
    fi
    
    echo "$repo_root"
}

# Get project name from git remote URL
get_project_name() {
    local repo_root=$(get_repo_root)
    
    # Try to get from git remote
    local remote_url=$(git -C "$repo_root" remote get-url origin 2>/dev/null)
    
    if [ -n "$remote_url" ]; then
        # Extract project name from URL
        # Support both SSH and HTTPS formats
        # git@github.com:user/project.git -> project
        # https://github.com/user/project.git -> project
        local project_name=$(echo "$remote_url" | sed -E 's|.*[:/]([^/]+)/([^/]+)(\.git)?$|\2|' | sed 's/\.git$//')
        
        if [ -n "$project_name" ] && [ "$project_name" != "$remote_url" ]; then
            echo "$project_name"
            return
        fi
    fi
    
    # Fallback to directory name
    echo -e "${RED}Error: Could not determine project name${NC}"
    exit 1
}

# Initialize REPO_ROOT
REPO_ROOT=$(get_repo_root)

# Initialize PROJECT_NAME
PROJECT_NAME=$(get_project_name)

# Command name (for alias support)
cmd="vive"

usage() {
    cat << EOF
Usage: $0 [command] [options]

Commands:
  fix <issue>          Fix a single issue with AI assistance
  batch <issues...>    Process multiple issues in parallel  
  sessions             List all active vive sessions
  attach <issue>       Attach to a running session
  logs <issue>         Show logs for a specific issue
  cleanup              Clean up completed sessions and worktrees
  cleanup all          Force cleanup all sessions and worktrees
  dashboard            Show a dashboard with logs from all active vive-issue sessions

Options:
  -h, --help           Show this help message
  -s, --sync           Attach to tmux after startup (default: async exit)

Examples:
  $0 fix 42                      Fix issue #42
  $0 batch 41 42 43              Process issues #41, #42, #43 in parallel
  $0 attach 42                   Attach to session for issue #42
  $0 sessions                    List all active sessions
  $0 cleanup                     Clean up completed work

EOF
}

check_requirements() {
    if ! command -v tmux &> /dev/null; then
        echo -e "${RED}Error: tmux is not installed${NC}"
        exit 1
    fi
}

# tmux check
check_tmux() {
    if ! command -v tmux &> /dev/null; then
        echo -e "${RED}Error: tmux is not installed${NC}"
        echo "Installation:"
        echo "  macOS: brew install tmux"
        echo "  Ubuntu/Debian: sudo apt-get install tmux"
        exit 1
    fi
} 