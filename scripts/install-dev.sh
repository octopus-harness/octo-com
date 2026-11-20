#!/usr/bin/env bash
set -euo pipefail

# Determine repository root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${DEST_DIR:-/usr/local/bin}"

echo "==> Building octo-com release binaries..."
cd "$REPO_ROOT"
cargo build --release

COM_BIN="$REPO_ROOT/target/release/com"
OCTO_COM_BIN="$REPO_ROOT/target/release/octo-com"

if [[ ! -f "$COM_BIN" ]]; then
    echo "Error: Release binary not found at $COM_BIN" >&2
    exit 1
fi

echo "==> Ensuring target directory exists: $BIN_DIR"
if [[ ! -d "$BIN_DIR" ]]; then
    if [[ -w "$(dirname "$BIN_DIR")" ]]; then
        mkdir -p "$BIN_DIR"
    else
        echo "Elevating permissions with sudo to create $BIN_DIR..."
        sudo mkdir -p "$BIN_DIR"
    fi
fi

echo "==> Symlinking binaries to $BIN_DIR..."
if [[ -w "$BIN_DIR" ]]; then
    ln -sf "$COM_BIN" "$BIN_DIR/com"
    if [[ -f "$OCTO_COM_BIN" ]]; then
        ln -sf "$OCTO_COM_BIN" "$BIN_DIR/octo-com"
    fi
else
    echo "Elevating permissions with sudo to link binaries into $BIN_DIR..."
    sudo ln -sf "$COM_BIN" "$BIN_DIR/com"
    if [[ -f "$OCTO_COM_BIN" ]]; then
        sudo ln -sf "$OCTO_COM_BIN" "$BIN_DIR/octo-com"
    fi
fi

echo "==> Symlinks installed successfully:"
ls -l "$BIN_DIR/com"
if [[ -f "$OCTO_COM_BIN" ]]; then
    ls -l "$BIN_DIR/octo-com"
fi

echo ""
echo "You can now run 'com' or 'octo-com' from anywhere!"
