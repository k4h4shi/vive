#!/usr/bin/env bash
# vive configuration management

# Default configuration
DEFAULT_CONFIG='{
  "mcp": {
    "enabled": false,
    "configPath": ".vive/mcp.json"
  },
  "initialization": {
    "commands": [],
    "workingDirectory": ".",
    "skipOnKeepWorktree": true,
    "timeoutSeconds": 300
  },
  "prompts": {
    "template": "default",
    "customFields": {}
  }
}'

# Load project configuration
load_project_config() {
    local config_path="$REPO_ROOT/.vive/config.json"
    
    if [ -f "$config_path" ]; then
        echo -e "${BLUE}Loading project configuration: $config_path${NC}"
        PROJECT_CONFIG=$(cat "$config_path")
    else
        echo -e "${YELLOW}No project configuration found, using defaults${NC}"
        PROJECT_CONFIG="$DEFAULT_CONFIG"
    fi
}

# Get configuration value using jq
get_config() {
    local key="$1"
    local default="$2"
    
    if command -v jq &> /dev/null; then
        echo "$PROJECT_CONFIG" | jq -r "$key // \"$default\""
    else
        echo "$default"
    fi
}

# Check if MCP is enabled
is_mcp_enabled() {
    local enabled=$(get_config ".mcp.enabled" "false")
    [ "$enabled" = "true" ]
}

# Get MCP config path
get_mcp_config_path() {
    local mcp_path=$(get_config ".mcp.configPath" ".vive/mcp.json")
    echo "$REPO_ROOT/$mcp_path"
}

# Get initialization commands
get_initialization_commands() {
    if command -v jq &> /dev/null; then
        echo "$PROJECT_CONFIG" | jq -r '.initialization.commands[]' 2>/dev/null || true
    fi
}

# Run initialization commands
run_initialization() {
    local worktree_dir="$1"
    local keep_worktree="$2"
    
    # Check if we should skip initialization for keep_worktree mode
    local skip_on_keep=$(get_config ".initialization.skipOnKeepWorktree" "true")
    if [ "$keep_worktree" = "true" ] && [ "$skip_on_keep" = "true" ]; then
        echo -e "${YELLOW}Skipping initialization for keep_worktree mode${NC}"
        return 0
    fi
    
    local working_dir=$(get_config ".initialization.workingDirectory" ".")
    local timeout_seconds=$(get_config ".initialization.timeoutSeconds" "300")
    
    # Change to the specified working directory
    local init_dir="$worktree_dir/$working_dir"
    if [ ! -d "$init_dir" ]; then
        echo -e "${RED}Initialization working directory not found: $init_dir${NC}"
        return 1
    fi
    
    cd "$init_dir"
    
    local commands=($(get_initialization_commands))
    
    if [ ${#commands[@]} -eq 0 ]; then
        echo -e "${YELLOW}No initialization commands configured${NC}"
        return 0
    fi
    
    echo -e "${BLUE}Running initialization commands...${NC}"
    
    for cmd in "${commands[@]}"; do
        if [ "$cmd" = "null" ] || [ -z "$cmd" ]; then
            continue
        fi
        
        echo -e "${YELLOW}Executing: $cmd${NC}"
        
        # Run command with timeout
        if timeout "$timeout_seconds" bash -c "$cmd"; then
            echo -e "${GREEN}✅ Command completed: $cmd${NC}"
        else
            local exit_code=$?
            if [ $exit_code -eq 124 ]; then
                echo -e "${RED}❌ Command timed out after ${timeout_seconds}s: $cmd${NC}"
            else
                echo -e "${RED}❌ Command failed with exit code $exit_code: $cmd${NC}"
            fi
            return $exit_code
        fi
    done
    
    echo -e "${GREEN}All initialization commands completed successfully${NC}"
    return 0
}



# Create default vive configuration
create_default_config() {
    local config_dir="$REPO_ROOT/.vive"
    local config_file="$config_dir/config.json"
    
    if [ ! -d "$config_dir" ]; then
        mkdir -p "$config_dir"
    fi
    
    if [ ! -f "$config_file" ]; then
        echo -e "${BLUE}Creating default vive configuration...${NC}"
        echo "$DEFAULT_CONFIG" | jq '.' > "$config_file"
        echo -e "${GREEN}Created configuration file: $config_file${NC}"
        echo ""
        echo -e "${YELLOW}Please edit $config_file to configure initialization commands if needed.${NC}"
        echo -e "${BLUE}Example configurations:${NC}"
        echo -e "  Node.js: ${YELLOW}\"commands\": [\"npm install\"]${NC}"
        echo -e "  Python: ${YELLOW}\"commands\": [\"pip install -r requirements.txt\"]${NC}"
        echo -e "  Rust:   ${YELLOW}\"commands\": [\"cargo build\"]${NC}"
        echo -e "  Go:     ${YELLOW}\"commands\": [\"go mod download\"]${NC}"
        echo ""
    fi
    
    # Initialize Claude Code configuration
    init_claude_config
}

# Initialize Claude Code configuration files
init_claude_config() {
    local config_dir="$REPO_ROOT/.vive"
    local mcp_file="$config_dir/mcp.json"
    
    # Create MCP configuration if it doesn't exist
    if [ ! -f "$mcp_file" ]; then
        echo -e "${BLUE}Creating default MCP configuration...${NC}"
        cat > "$mcp_file" << 'EOF'
{
    "mcpServers": {}
}
EOF
        echo -e "${GREEN}Created MCP configuration file: $mcp_file${NC}"
        echo -e "${YELLOW}To add MCP servers, edit $mcp_file and set 'enabled: true' in .vive/config.json${NC}"
        echo ""
    fi
    
    # Create .gitignore entry for .vive if needed
    local gitignore_file="$REPO_ROOT/.gitignore"
    if [ -f "$gitignore_file" ]; then
        if ! grep -q "^\s*#*\s*\.vive/" "$gitignore_file" 2>/dev/null; then
            echo -e "${BLUE}Adding .vive/ to .gitignore...${NC}"
            echo "" >> "$gitignore_file"
            echo "# vive configuration (optional: commit if you want to share team settings)" >> "$gitignore_file"
            echo "# .vive/" >> "$gitignore_file"
            echo -e "${GREEN}Added .vive/ entry to .gitignore (commented out by default)${NC}"
        fi
    fi
}

# Initialize PROJECT_CONFIG
PROJECT_CONFIG="" 