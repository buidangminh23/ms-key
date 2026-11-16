#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
APP_DIR="${DIST_DIR}/MSKey.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n1)"

cd "${ROOT_DIR}"
cargo build --release -p mskey-ffi

cd "${ROOT_DIR}/macos/MSKeyMac"
swift build -c release

BIN_PATH="$(swift build -c release --show-bin-path)/MSKeyMac"

rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cp "${BIN_PATH}" "${APP_DIR}/Contents/MacOS/MSKey"
cp "${ROOT_DIR}/macos/MSKeyMac/Info.plist" "${APP_DIR}/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "${VERSION}" "${APP_DIR}/Contents/Info.plist"
cp "${ROOT_DIR}/assets/branding/mskey.icns" "${APP_DIR}/Contents/Resources/mskey.icns"
cp "${ROOT_DIR}/assets/branding/mskey-mark.png" "${APP_DIR}/Contents/Resources/mskey-mark.png"
cp "${ROOT_DIR}/scripts/macos-local-sign.sh" "${APP_DIR}/Contents/Resources/macos-local-sign.sh"
chmod 755 "${APP_DIR}/Contents/Resources/macos-local-sign.sh"
chmod +x "${APP_DIR}/Contents/MacOS/MSKey"

if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "${APP_DIR}" >/dev/null 2>&1 || true
fi

echo "Built ${APP_DIR}"
