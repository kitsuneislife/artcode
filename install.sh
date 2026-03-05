#!/usr/bin/env bash
set -euo pipefail

# ── Artcode Installer ──────────────────────────────────────────────────────────
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/kitsuneislife/artcode/main/install.sh | bash
#
# Options (env vars):
#   ARTCODE_TAG      — version to install (default: latest release)
#   ARTCODE_INSTALL  — install dir       (default: /usr/local/bin)
# ──────────────────────────────────────────────────────────────────────────────

REPO="kitsuneislife/artcode"
INSTALL_DIR="${ARTCODE_INSTALL:-/usr/local/bin}"

# ── Detect OS / arch ──────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64) ASSET="art-linux-x86_64" ;;
      *) echo "Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      x86_64)  ASSET="art-macos-x86_64" ;;
      arm64)   ASSET="art-macos-arm64"  ;;
      *) echo "Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"
    echo "For Windows, download the .exe from https://github.com/$REPO/releases"
    exit 1
    ;;
esac

# ── Resolve version ───────────────────────────────────────────────────────────
if [ -z "${ARTCODE_TAG:-}" ]; then
  echo "Fetching latest release..."
  TAG="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
  if [ -z "$TAG" ]; then
    echo "Error: could not determine latest release tag. Set ARTCODE_TAG manually."
    exit 1
  fi
else
  TAG="$ARTCODE_TAG"
fi

echo "Installing Artcode $TAG ($ASSET)..."

# ── Download binary ───────────────────────────────────────────────────────────
URL="https://github.com/$REPO/releases/download/$TAG/$ASSET"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

curl -fsSL --progress-bar -o "$TMP" "$URL"
chmod +x "$TMP"

# ── Install ───────────────────────────────────────────────────────────────────
TARGET="$INSTALL_DIR/art"

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP" "$TARGET"
else
  echo "(sudo needed to install to $INSTALL_DIR)"
  sudo mv "$TMP" "$TARGET"
fi

echo ""
echo "✓ Artcode $TAG installed to $TARGET"
echo ""
echo "Teste rápido:"
echo "  art run examples/00_hello.art"
echo ""
echo "Docs: https://kitsuneislife.github.io/artcode"
