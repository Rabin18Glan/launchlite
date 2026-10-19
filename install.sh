#!/usr/bin/env bash
set -e

# LaunchLite One-Line Installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Rabin18Glan/launchlite/main/install.sh | bash
# Options via env:
#   PORT=8080 curl -fsSL ... | bash
#   INSTALL_DIR=/usr/local/bin curl -fsSL ... | bash

REPO="Rabin18Glan/launchlite"
BINARY_NAME="launchlite"

BOLD='\033[1m'
GREEN='\033[32m'
BLUE='\033[34m'
YELLOW='\033[33m'
RED='\033[31m'
NC='\033[0m'

echo -e "${BOLD}${BLUE}▲ LaunchLite Installer${NC}"
echo -e "Minimal, ultra-low-overhead pure-Rust PaaS\n"

# 1. Detect OS & Architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [ "$OS" != "linux" ]; then
    echo -e "${RED}Error: LaunchLite currently only supports Linux (detected: $OS).${NC}"
    exit 1
fi

case "$ARCH" in
    x86_64|amd64)
        TARGET="x86_64-unknown-linux-gnu"
        ;;
    aarch64|arm64)
        TARGET="aarch64-unknown-linux-gnu"
        ;;
    *)
        echo -e "${RED}Error: Unsupported architecture: $ARCH${NC}"
        exit 1
        ;;
esac

# 2. Determine target install directory
if [ -n "$INSTALL_DIR" ]; then
    DEST_DIR="$INSTALL_DIR"
elif [ "$(id -u)" -eq 0 ]; then
    DEST_DIR="/usr/local/bin"
elif [ -w "/usr/local/bin" ]; then
    DEST_DIR="/usr/local/bin"
else
    DEST_DIR="$HOME/.local/bin"
    mkdir -p "$DEST_DIR"
fi

DEST_PATH="$DEST_DIR/$BINARY_NAME"
TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

echo -e "Target system: ${BOLD}$OS ($ARCH)${NC}"
echo -e "Install location: ${BOLD}$DEST_PATH${NC}"

# 3. Download prebuilt binary from GitHub Releases or fallback to local cargo
DOWNLOADED=false
RELEASE_URL="https://github.com/$REPO/releases/latest/download/launchlite-$TARGET.tar.gz"

echo -e "Fetching pre-compiled binary from GitHub Releases..."
if curl -fsSL "$RELEASE_URL" -o "$TMP_DIR/launchlite.tar.gz" 2>/dev/null; then
    tar -xzf "$TMP_DIR/launchlite.tar.gz" -C "$TMP_DIR"
    if [ -f "$TMP_DIR/launchlite" ]; then
        DOWNLOADED=true
    fi
fi

if [ "$DOWNLOADED" = false ]; then
    # Direct binary download fallback
    DIRECT_BIN_URL="https://github.com/$REPO/releases/latest/download/launchlite-$TARGET"
    if curl -fsSL "$DIRECT_BIN_URL" -o "$TMP_DIR/launchlite" 2>/dev/null; then
        DOWNLOADED=true
    fi
fi

if [ "$DOWNLOADED" = false ]; then
    echo -e "${YELLOW}Notice: Pre-built release binary not found for $TARGET (releases may not be published yet).${NC}"
    if command -v cargo >/dev/null 2>&1; then
        echo -e "Cargo found on your system. Building LaunchLite from source..."
        git clone --depth 1 "https://github.com/$REPO.git" "$TMP_DIR/src"
        (cd "$TMP_DIR/src" && cargo build --release -p cli)
        cp "$TMP_DIR/src/target/release/$BINARY_NAME" "$TMP_DIR/launchlite"
        DOWNLOADED=true
    else
        echo -e "${RED}Error: Could not download pre-built binary and Cargo is not installed.${NC}"
        echo -e "Please install Rust/Cargo (https://rustup.rs) or download a release from:"
        echo -e "https://github.com/$REPO/releases"
        exit 1
    fi
fi

# 4. Copy to destination
chmod +x "$TMP_DIR/launchlite"

if [ -w "$DEST_DIR" ]; then
    cp "$TMP_DIR/launchlite" "$DEST_PATH"
else
    echo -e "Requesting sudo permissions to copy to $DEST_DIR..."
    sudo cp "$TMP_DIR/launchlite" "$DEST_PATH"
fi

# 5. Check PATH
if [[ ":$PATH:" != *":$DEST_DIR:"* ]]; then
    echo -e "${YELLOW}Warning: $DEST_DIR is not in your current PATH.${NC}"
    echo -e "Add it to your shell profile: ${BOLD}export PATH=\"\$PATH:$DEST_DIR\"${NC}"
fi

# 6. Success Output
DEFAULT_RUN_PORT="${PORT:-3000}"

echo -e "\n${GREEN}${BOLD}✓ LaunchLite successfully installed!${NC}"
echo -e "Binary: ${BOLD}$DEST_PATH${NC}\n"

echo -e "${BOLD}Quick Start:${NC}"
echo -e "  ${BLUE}# 1. Run in background on port $DEFAULT_RUN_PORT (one-liner):${NC}"
echo -e "  ${BOLD}launchlite start --port $DEFAULT_RUN_PORT${NC}\n"

echo -e "  ${BLUE}# 2. Check service status & RAM:${NC}"
echo -e "  ${BOLD}launchlite status${NC}\n"

echo -e "  ${BLUE}# 3. Stop background service:${NC}"
echo -e "  ${BOLD}launchlite stop${NC}\n"

echo -e "  ${BLUE}# 4. Or install as a permanent systemd service (auto-restart on reboot):${NC}"
echo -e "  ${BOLD}sudo launchlite service install --port $DEFAULT_RUN_PORT${NC}\n"

echo -e "Dashboard will be live at: ${BOLD}http://127.0.0.1:$DEFAULT_RUN_PORT${NC}\n"
