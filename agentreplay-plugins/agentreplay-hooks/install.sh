#!/bin/bash
# Agent Replay Hooks Plugin Installer
# Installs the agentreplay-hook CLI and sets up hook configurations

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Determine install directory (prefer user-writable locations like Homebrew)
get_install_dir() {
    # If user specified, use that
    if [ -n "$AGENTREPLAY_INSTALL_DIR" ]; then
        echo "$AGENTREPLAY_INSTALL_DIR"
        return
    fi
    
    # Priority order (like Homebrew - prefer user-writable locations)
    local candidates=(
        "$HOME/.local/bin"      # XDG standard user bin
        "$HOME/bin"             # Traditional user bin
        "/usr/local/bin"        # System-wide (may need sudo)
        "/opt/homebrew/bin"     # Homebrew on Apple Silicon
    )
    
    for dir in "${candidates[@]}"; do
        if [ -d "$dir" ] && [ -w "$dir" ]; then
            echo "$dir"
            return
        fi
    done
    
    # Create ~/.local/bin if nothing writable found
    mkdir -p "$HOME/.local/bin"
    echo "$HOME/.local/bin"
}

INSTALL_DIR="$(get_install_dir)"

echo "Agent Replay - Coding Agent Observability Plugin Installer"
echo "==========================================================="
echo ""

# Check for dependencies
echo "Checking dependencies..."

if ! command -v jq &> /dev/null; then
    echo "❌ jq is required but not installed."
    echo "   Install with: brew install jq (macOS) or apt install jq (Linux)"
    exit 1
fi

if ! command -v curl &> /dev/null; then
    echo "❌ curl is required but not installed."
    exit 1
fi

echo "✓ All dependencies found"
echo ""

# Install agentreplay-hook CLI
echo "Installing agentreplay-hook CLI to $INSTALL_DIR..."

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

if [ -w "$INSTALL_DIR" ]; then
    cp "$SCRIPT_DIR/agentreplay-hook" "$INSTALL_DIR/agentreplay-hook"
    chmod +x "$INSTALL_DIR/agentreplay-hook"
else
    echo "  (requires sudo)"
    sudo cp "$SCRIPT_DIR/agentreplay-hook" "$INSTALL_DIR/agentreplay-hook"
    sudo chmod +x "$INSTALL_DIR/agentreplay-hook"
fi

echo "✓ agentreplay-hook CLI installed"

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "⚠️  $INSTALL_DIR is not in your PATH"
    echo "   Add this to your ~/.zshrc or ~/.bashrc:"
    echo ""
    echo "   export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
fi

echo ""

# Setup Cursor hooks
setup_cursor() {
    echo "Setting up Cursor integration..."
    
    local cursor_hooks_dir="$HOME/.cursor/hooks"
    mkdir -p "$cursor_hooks_dir"
    
    # Copy all hook scripts
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-session-start.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-session-end.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-observation.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-tool.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-file-read.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-file-edit.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-shell.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-prompt.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-response.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-stop.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-summarize.sh" "$cursor_hooks_dir/"
    cp "$SCRIPT_DIR/hooks/cursor/agentreplay-context.sh" "$cursor_hooks_dir/"
    
    chmod +x "$cursor_hooks_dir/"*.sh
    
    # Create rules directory for context injection
    local cursor_rules_dir="$HOME/.cursor/rules"
    mkdir -p "$cursor_rules_dir"
    
    echo "✓ Cursor hooks installed to $cursor_hooks_dir"
    echo "✓ Context will be injected to $cursor_rules_dir/agentreplay-context.mdc"
    echo ""
    echo "To complete Cursor setup, add to your .cursor/hooks.json:"
    echo ""
    cat "$SCRIPT_DIR/hooks/cursor/hooks.json"
    echo ""
}

# Setup Claude Code hooks
setup_claude() {
    echo "Setting up Claude Code integration..."
    
    local claude_dir="$HOME/.claude"
    mkdir -p "$claude_dir"
    
    if [ -f "$claude_dir/settings.json" ]; then
        echo "⚠️  ~/.claude/settings.json already exists"
        echo "   Merge the hooks from: $SCRIPT_DIR/hooks/claude/settings.json"
    else
        cp "$SCRIPT_DIR/hooks/claude/settings.json" "$claude_dir/"
        echo "✓ Claude Code settings installed"
    fi
    echo ""
}

# Interactive setup
echo "Which coding agents do you want to set up?"
echo ""
echo "1) Cursor"
echo "2) Claude Code"
echo "3) Both"
echo "4) Skip (CLI only)"
echo ""
read -p "Enter choice [1-4]: " choice

case "$choice" in
    1)
        setup_cursor
        ;;
    2)
        setup_claude
        ;;
    3)
        setup_cursor
        setup_claude
        ;;
    4)
        echo "Skipping hook setup. You can set up hooks manually later."
        ;;
    *)
        echo "Invalid choice. Skipping hook setup."
        ;;
esac

echo ""
echo "Installation complete!"
echo ""
echo "Quick start:"
echo "  1. Start Agent Replay (./run-tauri.sh)"
echo "  2. Use your coding agent normally"
echo "  3. View sessions at: Coding Sessions in sidebar"
echo ""
echo "Configuration:"
echo "  AGENTREPLAY_URL=http://127.0.0.1:47100"
echo "  AGENTREPLAY_PROJECT_ID=1"
echo ""
echo "Test installation:"
echo "  agentreplay-hook status"
echo ""
