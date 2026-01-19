#!/usr/bin/env bash
# vive installer
#
# Builds and installs the vive TUI binary.
# Usage:
#   ./install.sh              Install vive
#   ./install.sh --uninstall  Uninstall vive
#   ./install.sh --yes        Install without prompts (for CI/CD)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
QUIET=false

# Parse flags
for arg in "$@"; do
    case "$arg" in
        -y|--yes|--force)
            QUIET=true
            ;;
    esac
done

# Helper for conditional output
log() {
    if [[ "$QUIET" == false ]]; then
        echo "$@"
    fi
}

# Installation directory preference order:
# 1. ~/.local/bin (user-local, no sudo needed)
# 2. /usr/local/bin (system-wide, requires sudo)
determine_install_dir() {
    local local_bin="$HOME/.local/bin"
    local system_bin="/usr/local/bin"

    # Prefer ~/.local/bin if it exists or can be created
    if [[ -d "$local_bin" ]] || mkdir -p "$local_bin" 2>/dev/null; then
        echo "$local_bin"
        return
    fi

    # Fall back to /usr/local/bin
    echo "$system_bin"
}

INSTALL_DIR="$(determine_install_dir)"
INSTALL_PATH="$INSTALL_DIR/vive"

# Check if we need sudo for the install directory
needs_sudo() {
    [[ ! -w "$INSTALL_DIR" ]]
}

# Run command with sudo if necessary
maybe_sudo() {
    if needs_sudo; then
        sudo "$@"
    else
        "$@"
    fi
}

# Uninstall
if [[ "${1:-}" == "--uninstall" ]]; then
    echo "Uninstalling vive..."

    found=false

    # Check both possible locations (binary or symlink)
    for path in "$HOME/.local/bin/vive" "/usr/local/bin/vive"; do
        if [[ -e "$path" ]] || [[ -L "$path" ]]; then
            echo "Removing $path..."
            if [[ -w "$(dirname "$path")" ]]; then
                rm -f "$path"
            else
                sudo rm -f "$path"
            fi
            echo "  Removed."
            found=true
        fi
    done

    if [[ "$found" == false ]]; then
        echo "vive is not installed."
    else
        echo "Done."
    fi
    exit 0
fi

log "Installing vive..."
log ""

# Check for Rust toolchain
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust toolchain (cargo) is required but not installed." >&2
    echo "Install Rust with:" >&2
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
    exit 1
fi

# Check runtime dependencies
log "Checking dependencies..."
missing_deps=()
for cmd in tmux git; do
    if ! command -v "$cmd" &> /dev/null; then
        missing_deps+=("$cmd")
    fi
done

if [[ ${#missing_deps[@]} -gt 0 ]]; then
    echo "Error: Missing required dependencies: ${missing_deps[*]}" >&2
    exit 1
fi

log "  ✓ All dependencies found"
log ""

# Build the release binary
log "Building vive (release mode)..."
cd "$SCRIPT_DIR"
if [[ "$QUIET" == true ]]; then
    cargo build --release --quiet
else
    cargo build --release
fi

if [[ ! -f "$SCRIPT_DIR/target/release/vive" ]]; then
    echo "Error: Build failed - binary not found." >&2
    exit 1
fi

log "  ✓ Build successful"
log ""

# Ensure install directory exists
if [[ ! -d "$INSTALL_DIR" ]]; then
    log "Creating $INSTALL_DIR..."
    maybe_sudo mkdir -p "$INSTALL_DIR"
fi

# Remove existing installation (binary or symlink)
if [[ -e "$INSTALL_PATH" ]] || [[ -L "$INSTALL_PATH" ]]; then
    log "Removing existing installation at $INSTALL_PATH..."
    maybe_sudo rm -f "$INSTALL_PATH"
fi

# Also clean up old shell script symlink if it points to this project
if [[ -L "/usr/local/bin/vive" ]]; then
    old_target=$(readlink "/usr/local/bin/vive" 2>/dev/null || true)
    if [[ "$old_target" == *"$SCRIPT_DIR"* ]]; then
        log "Removing old shell script symlink..."
        sudo rm -f "/usr/local/bin/vive"
    fi
fi

# Install the binary
log "Installing binary to $INSTALL_PATH..."
maybe_sudo cp "$SCRIPT_DIR/target/release/vive" "$INSTALL_PATH"
maybe_sudo chmod +x "$INSTALL_PATH"

log ""
log "✓ vive installed successfully!"
log ""
log "Installation path: $INSTALL_PATH"
log ""

# Check if install directory is in PATH
if [[ "$QUIET" == false ]] && [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    log "⚠ Warning: $INSTALL_DIR is not in your PATH."
    log ""
    log "Add it to your shell configuration:"
    log "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
    log "  # or for zsh:"
    log "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
    log ""
fi

log "Usage:"
log "  vive              # Launch the TUI dashboard"
log ""
log "Environment variables:"
log "  VIVE_PROJECTS_ROOT  # Root directory for project discovery (default: ~/src)"
