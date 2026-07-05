#!/usr/bin/env bash
# 下载当前平台/架构对应的 Typst sidecar 二进制到 src-tauri/binaries/
# 用法: ./scripts/download-typst.sh
set -euo pipefail

TYPST_VERSION="0.15.0"
BIN_DIR="$(dirname "$0")/../src-tauri/binaries"
mkdir -p "$BIN_DIR"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
  Darwin-arm64)  TARGET="aarch64-apple-darwin";   BIN_NAME="typst-aarch64-apple-darwin" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin";     BIN_NAME="typst-x86_64-apple-darwin" ;;
  Linux-x86_64)  TARGET="x86_64-unknown-linux-gnu"; BIN_NAME="typst-x86_64-unknown-linux-gnu" ;;
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
                 TARGET="x86_64-pc-windows-msvc";   BIN_NAME="typst-x86_64-pc-windows-msvc.exe" ;;
  *) echo "不支持的平台: $OS-$ARCH"; exit 1 ;;
esac

OUT="$BIN_DIR/$BIN_NAME"
if [ -x "$OUT" ]; then
  echo "已存在: $OUT（跳过）"
  exit 0
fi

URL="https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-${TARGET}.tar.xz"
echo "下载 Typst v${TYPST_VERSION} ($TARGET)..."
TMP=$(mktemp -d)
curl -sSL -o "$TMP/typst.tar.xz" "$URL"
tar xf "$TMP/typst.tar.xz" -C "$TMP"
cp "$TMP/typst-${TARGET}/typst" "$OUT"
chmod +x "$OUT"
rm -rf "$TMP"
echo "完成: $OUT"
