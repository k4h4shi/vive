#!/bin/bash

# vive installation script
# This script helps install vive and its dependencies

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Installation directory
INSTALL_DIR="/usr/local/bin"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VIVE_SCRIPT="${SCRIPT_DIR}/vive.sh"
VIVE_LINK="${INSTALL_DIR}/vive"

# Display vive banner
display_banner() {
    echo
    echo -e "${CYAN}${BOLD}"
    echo "     ██╗   ██╗██╗██╗   ██╗███████╗"
    echo "     ██║   ██║██║██║   ██║██╔════╝"
    echo "     ██║   ██║██║██║   ██║█████╗  "
    echo "     ╚██╗ ██╔╝██║╚██╗ ██╔╝██╔══╝  "
    echo "      ╚████╔╝ ██║ ╚████╔╝ ███████╗"
    echo "       ╚═══╝  ╚═╝  ╚═══╝  ╚══════╝"
    echo -e "${NC}"
    echo -e "${MAGENTA}${BOLD}    parallel AI fixer, alive in the shell${NC}"
    echo
    echo -e "${GREEN}=========================================${NC}"
    echo
}

# Display welcome message
display_welcome() {
    display_banner
    echo -e "${BOLD}Welcome to vive Installation Script!${NC}"
    echo
    echo "This script will:"
    echo "  • Check for required dependencies"
    echo "  • Install Claude CLI if needed"
    echo "  • Create a global 'vive' command"
    echo
    echo -e "${YELLOW}Press Enter to continue or Ctrl+C to cancel...${NC}"
    read -r
}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check dependencies
check_dependencies() {
    echo "Checking dependencies..."
    
    local missing_deps=()
    
    # Core dependencies
    if ! command_exists "bash"; then
        missing_deps+=("bash")
    fi
    
    if ! command_exists "tmux"; then
        missing_deps+=("tmux")
    fi
    
    if ! command_exists "expect"; then
        missing_deps+=("expect")
    fi
    
    if ! command_exists "git"; then
        missing_deps+=("git")
    fi
    
    if ! command_exists "gh"; then
        missing_deps+=("gh (GitHub CLI)")
    fi
    
    if ! command_exists "node"; then
        missing_deps+=("node")
    fi
    
    if ! command_exists "npm"; then
        missing_deps+=("npm")
    fi
    
    # Check for git worktree support
    if command_exists "git" && ! git worktree --help >/dev/null 2>&1; then
        missing_deps+=("git with worktree support")
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        echo -e "${RED}Missing dependencies:${NC}"
        for dep in "${missing_deps[@]}"; do
            echo "  - $dep"
        done
        echo
        echo "Please install missing dependencies before continuing."
        echo
        echo "On macOS with Homebrew:"
        echo "  brew install tmux expect gh node"
        echo
        return 1
    else
        echo -e "${GREEN}All core dependencies are installed!${NC}"
        return 0
    fi
}

# Function to check Claude CLI
check_claude_cli() {
    echo
    echo "Checking for Claude CLI..."
    
    if ! command_exists "claude"; then
        echo -e "${YELLOW}Claude CLI is not installed.${NC}"
        echo "Would you like to install it now? (y/n)"
        read -r response
        
        if [[ "$response" =~ ^[Yy]$ ]]; then
            echo "Installing Claude CLI..."
            if npm install -g @anthropic-ai/claude-cli; then
                echo -e "${GREEN}Claude CLI installed successfully!${NC}"
            else
                echo -e "${RED}Failed to install Claude CLI.${NC}"
                echo "Please install it manually: npm install -g @anthropic-ai/claude-cli"
                return 1
            fi
        else
            echo -e "${YELLOW}Warning: vive requires Claude CLI to function properly.${NC}"
            echo "Install it later with: npm install -g @anthropic-ai/claude-cli"
        fi
    else
        echo -e "${GREEN}Claude CLI is installed!${NC}"
    fi
}

# Function to install vive
install_vive() {
    echo
    echo "Installing vive..."
    
    # Make vive.sh executable
    chmod +x "${VIVE_SCRIPT}"
    
    # Create symlink
    if [ -L "${VIVE_LINK}" ]; then
        echo -e "${YELLOW}vive is already installed at ${VIVE_LINK}${NC}"
        echo "Would you like to reinstall? (y/n)"
        read -r response
        
        if [[ ! "$response" =~ ^[Yy]$ ]]; then
            echo "Installation cancelled."
            return 1
        fi
        
        sudo rm -f "${VIVE_LINK}"
    fi
    
    echo "Creating symlink..."
    if sudo ln -s "${VIVE_SCRIPT}" "${VIVE_LINK}"; then
        echo -e "${GREEN}vive installed successfully!${NC}"
        echo
        echo "You can now use 'vive' command from anywhere."
        echo "Try: vive --help"
    else
        echo -e "${RED}Failed to create symlink.${NC}"
        echo "You can add vive to your PATH instead:"
        echo "  export PATH=\"\$PATH:${SCRIPT_DIR}\""
        return 1
    fi
}

# Function to uninstall vive
uninstall_vive() {
    echo "Uninstalling vive..."
    
    if [ -L "${VIVE_LINK}" ]; then
        if sudo rm -f "${VIVE_LINK}"; then
            echo -e "${GREEN}vive uninstalled successfully!${NC}"
        else
            echo -e "${RED}Failed to remove symlink.${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}vive is not installed at ${VIVE_LINK}${NC}"
    fi
    
    echo
    echo "Cleanup complete. The vive directory at ${SCRIPT_DIR} was not removed."
    echo "You can delete it manually if desired."
}

# Main installation flow
main() {
    # Show welcome banner
    display_welcome
    
    # Check for uninstall flag
    if [ "$1" = "--uninstall" ] || [ "$1" = "-u" ]; then
        display_banner
        uninstall_vive
        exit $?
    fi
    
    # Regular installation
    if ! check_dependencies; then
        exit 1
    fi
    
    check_claude_cli
    
    install_vive
    
    # Show success banner
    if [ $? -eq 0 ]; then
        echo
        echo -e "${GREEN}${BOLD}✨ Installation Complete! ✨${NC}"
        echo
        echo -e "${CYAN}Start using vive with:${NC}"
        echo "  vive fix <issue-number>"
        echo "  vive batch <issue1> <issue2> <issue3>"
        echo "  vive sessions"
        echo
        echo -e "${MAGENTA}Happy parallel coding! 🚀${NC}"
        echo
    fi
}

# Run main function
main "$@" 