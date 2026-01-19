#!/usr/bin/env bash
# vive installer

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PATH="/usr/local/bin/vive"

if [[ "${1:-}" == "--uninstall" ]]; then
  echo "Uninstalling vive..."
  if [[ -L "$INSTALL_PATH" ]]; then
    sudo rm "$INSTALL_PATH"
    echo "vive uninstalled successfully."
  else
    echo "vive is not installed at $INSTALL_PATH"
  fi
  exit 0
fi

echo "Installing vive..."

# Check dependencies
for cmd in tmux git claude; do
  if ! command -v "$cmd" &> /dev/null; then
    echo "Error: $cmd is required but not installed."
    exit 1
  fi
done

# Create symlink (remove existing file or symlink first)
if [[ -e "$INSTALL_PATH" ]] || [[ -L "$INSTALL_PATH" ]]; then
  echo "Removing existing installation..."
  sudo rm -f "$INSTALL_PATH"
fi

sudo ln -s "${SCRIPT_DIR}/vive" "$INSTALL_PATH"

echo "vive installed successfully!"
echo ""
echo "Usage:"
echo "  vive start 123    # Start working on issue #123"
echo "  vive list         # List active sessions"
echo "  vive attach 123   # Attach to session"
echo "  vive cleanup 123  # Clean up session"
