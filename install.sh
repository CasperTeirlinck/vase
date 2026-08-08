#!/usr/bin/env bash
# Install the latest vase.app from GitHub releases into /Applications.
#   curl -fsSL https://raw.githubusercontent.com/CasperTeirlinck/vase/main/install.sh | bash
set -euo pipefail

REPO="CasperTeirlinck/vase"
DEST="/Applications"

[ "$(uname)" = "Darwin" ] || {
  echo "vase only ships a macOS build." >&2
  exit 1
}

echo "Finding the latest vase release..."
url=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" |
  grep -o '"browser_download_url": *"[^"]*-macos\.zip"' | head -1 | cut -d'"' -f4)
[ -n "$url" ] || {
  echo "No macOS .app asset found in the latest release." >&2
  exit 1
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "Downloading ${url##*/}..."
curl -fsSL "$url" -o "$tmp/vase.zip"
ditto -x -k "$tmp/vase.zip" "$tmp"

SUDO=""
[ -w "$DEST" ] || SUDO="sudo"
[ -n "$SUDO" ] && echo "Installing to ${DEST} (needs your password)..."
$SUDO rm -rf "${DEST}/vase.app"
$SUDO mv "$tmp/vase.app" "${DEST}/vase.app"
# Downloaded apps are quarantined; vase is unsigned, so clear it to avoid the "unidentified developer" block.
$SUDO xattr -dr com.apple.quarantine "${DEST}/vase.app" 2>/dev/null || true

echo "Installed ${DEST}/vase.app"
echo "Launch it, then grant Accessibility and Input Monitoring in System Settings."
