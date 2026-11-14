#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
APP_DIR="${DIST_DIR}/MSKey.app"

cd "${ROOT_DIR}"
cargo build --release -p mskey-ffi

cd "${ROOT_DIR}/macos/MSKeyMac"
swift build -c release

BIN_PATH="$(swift build -c release --show-bin-path)/MSKeyMac"

rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cp "${BIN_PATH}" "${APP_DIR}/Contents/MacOS/MSKey"
cp "${ROOT_DIR}/macos/MSKeyMac/Info.plist" "${APP_DIR}/Contents/Info.plist"
chmod +x "${APP_DIR}/Contents/MacOS/MSKey"

if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "${APP_DIR}" >/dev/null 2>&1 || true
fi

echo "Built ${APP_DIR}"
