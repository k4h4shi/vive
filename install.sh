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

# Run command with sudo if necessary for package managers
maybe_sudo_pkg() {
    if [[ "$(id -u)" -ne 0 ]]; then
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

has_cmd() {
    command -v "$1" &> /dev/null
}

prompt_yes_no() {
    local prompt="$1"
    local default_yes="${2:-true}"

    if [[ "$QUIET" == true ]]; then
        return 0
    fi

    # If not running in a TTY, default to "no" unless explicitly forced.
    if [[ ! -t 0 ]]; then
        return 1
    fi

    local suffix="[Y/n]"
    if [[ "$default_yes" != true ]]; then
        suffix="[y/N]"
    fi

    while true; do
        read -r -p "$prompt $suffix " answer
        answer="${answer:-}"
        if [[ -z "$answer" ]]; then
            [[ "$default_yes" == true ]] && return 0 || return 1
        fi
        case "$answer" in
            y|Y|yes|YES) return 0 ;;
            n|N|no|NO) return 1 ;;
        esac
    done
}

detect_pkg_manager() {
    if has_cmd brew; then echo "brew"; return; fi
    if has_cmd apt-get; then echo "apt"; return; fi
    if has_cmd dnf; then echo "dnf"; return; fi
    if has_cmd yum; then echo "yum"; return; fi
    if has_cmd pacman; then echo "pacman"; return; fi
    if has_cmd apk; then echo "apk"; return; fi
    echo "unknown"
}

install_packages() {
    local pm
    pm="$(detect_pkg_manager)"
    case "$pm" in
        brew)
            brew install "$@"
            ;;
        apt)
            maybe_sudo_pkg apt-get update -y
            maybe_sudo_pkg apt-get install -y "$@"
            ;;
        dnf)
            maybe_sudo_pkg dnf install -y "$@"
            ;;
        yum)
            maybe_sudo_pkg yum install -y "$@"
            ;;
        pacman)
            maybe_sudo_pkg pacman -Sy --noconfirm "$@"
            ;;
        apk)
            maybe_sudo_pkg apk add --no-cache "$@"
            ;;
        *)
            return 1
            ;;
    esac
}

ensure_runtime_deps() {
    local deps=("$@")
    local missing=()
    for cmd in "${deps[@]}"; do
        if ! has_cmd "$cmd"; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -eq 0 ]]; then
        return 0
    fi

    echo "Missing required dependencies: ${missing[*]}" >&2

    if ! prompt_yes_no "Install missing dependencies now?" true; then
        echo "Please install them and re-run:" >&2
        echo "  ${missing[*]}" >&2
        exit 1
    fi

    log "Installing dependencies: ${missing[*]} ..."
    if ! install_packages "${missing[@]}"; then
        echo "Error: Could not auto-install dependencies (no supported package manager found)." >&2
        echo "Please install manually: ${missing[*]}" >&2
        exit 1
    fi
}

ensure_rust_toolchain() {
    # If cargo exists but rustup has no default toolchain, configure one.
    if has_cmd cargo; then
        if has_cmd rustup; then
            if ! rustup show active-toolchain &> /dev/null; then
                log "Configuring rustup default toolchain (stable)..."
                rustup toolchain install stable --profile minimal
                rustup default stable
            fi
        fi
        return 0
    fi

    # No cargo: install Rust via rustup (requires curl).
    ensure_runtime_deps curl

    if ! has_cmd rustup; then
        log "Installing Rust toolchain via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
    else
        log "Installing Rust stable toolchain via rustup..."
        rustup toolchain install stable --profile minimal
        rustup default stable
    fi

    # Ensure this shell sees cargo.
    if [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck disable=SC1090
        source "$HOME/.cargo/env"
    fi

    if ! has_cmd cargo; then
        echo "Error: Rust toolchain (cargo) is required but could not be installed." >&2
        exit 1
    fi
}

# Check runtime dependencies
log "Checking dependencies..."
ensure_runtime_deps tmux git
ensure_rust_toolchain

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
