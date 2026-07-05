#!/usr/bin/env bash
# Download Typst sidecar binary for the current platform/arch into src-tauri/binaries/
# Usage: ./scripts/download-typst.sh
set -euo pipefail

TYPST_VERSION="0.15.0"
BIN_DIR="$(dirname "$0")/../src-tauri/binaries"
mkdir -p "$BIN_DIR"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
  Darwin-arm64)  TARGET="aarch64-apple-darwin";    BIN_NAME="typst-aarch64-apple-darwin" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin";      BIN_NAME="typst-x86_64-apple-darwin" ;;
  Linux-x86_64)  TARGET="x86_64-unknown-linux-gnu"; BIN_NAME="typst-x86_64-unknown-linux-gnu" ;;
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
                 TARGET="x86_64-pc-windows-msvc";   BIN_NAME="typst-x86_64-pc-windows-msvc.exe" ;;
  *) echo "Unsupported platform: $OS-$ARCH"; exit 1 ;;
esac

OUT="$BIN_DIR/$BIN_NAME"
if [ -x "$OUT" ]; then
  echo "Already exists: $OUT (skip)"
  exit 0
fi

URL="https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-${TARGET}.tar.xz"
echo "Downloading Typst v${TYPST_VERSION} ($TARGET)..."
TMP=$(mktemp -d)
curl -sSL -o "$TMP/typst.tar.xz" "$URL"
tar xf "$TMP/typst.tar.xz" -C "$TMP"
cp "$TMP/typst-${TARGET}/typst" "$OUT"
chmod +x "$OUT"
rm -rf "$TMP"
echo "Done: $OUT"
