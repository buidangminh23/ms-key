#!/usr/bin/env bash
set -euo pipefail

REPO="buidangminh23/ms-key"
BUNDLE_ID="org.mskey.mac"
APP_NAME="MSKey.app"
BASE_URL="${MSKEY_BASE_URL:-https://github.com/${REPO}/releases/latest/download}"
DEST_DIR=""
LAUNCH=1

usage() {
    echo "Usage: install-macos.sh [--dir <folder>] [--no-launch]"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dir) DEST_DIR="${2:-}"; shift 2 ;;
        --no-launch) LAUNCH=0; shift ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done

fail() {
    echo "MS Key: $*" >&2
    exit 1
}

[ "$(uname -s)" = "Darwin" ] || fail "this installer only runs on macOS"

if [ -z "${DEST_DIR}" ]; then
    if [ -w /Applications ]; then DEST_DIR="/Applications"; else DEST_DIR="${HOME}/Applications"; fi
fi
mkdir -p "${DEST_DIR}"
DEST_DIR="$(cd "${DEST_DIR}" && pwd -P)"
[ -w "${DEST_DIR}" ] || fail "cannot write to ${DEST_DIR}"
DEST_APP="${DEST_DIR}/${APP_NAME}"

if pgrep -f "${DEST_APP}/Contents/MacOS/MSKey" >/dev/null 2>&1; then
    fail "MS Key is running from ${DEST_APP}. Quit it from the menu bar, then run the installer again."
fi

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

echo "Downloading checksums..."
curl -fsSL "${BASE_URL}/SHA256SUMS" -o "${WORK}/SHA256SUMS"
tr -d '\r' < "${WORK}/SHA256SUMS" > "${WORK}/SHA256SUMS.lf"
LINE="$(grep -E '  MSKey-[0-9][^ ]*-macos-universal\.zip$' "${WORK}/SHA256SUMS.lf" | head -n1 || true)"
[ -n "${LINE}" ] || fail "no macOS build listed in SHA256SUMS"
EXPECTED="${LINE%% *}"
ASSET="${LINE##* }"

echo "Downloading ${ASSET}..."
curl -fsSL "${BASE_URL}/${ASSET}" -o "${WORK}/${ASSET}"
ACTUAL="$(shasum -a 256 "${WORK}/${ASSET}" | cut -d' ' -f1)"
[ "${ACTUAL}" = "${EXPECTED}" ] || fail "checksum mismatch for ${ASSET}"

ditto -x -k "${WORK}/${ASSET}" "${WORK}/unpacked"
NEW_APP="${WORK}/unpacked/${APP_NAME}"
[ -d "${NEW_APP}" ] || fail "${APP_NAME} not found in ${ASSET}"
GOT_ID="$(defaults read "${NEW_APP}/Contents/Info" CFBundleIdentifier 2>/dev/null || true)"
[ "${GOT_ID}" = "${BUNDLE_ID}" ] || fail "unexpected bundle identifier '${GOT_ID}'"
VERSION="$(defaults read "${NEW_APP}/Contents/Info" CFBundleShortVersionString 2>/dev/null || echo unknown)"
codesign --verify --deep --strict "${NEW_APP}" || fail "code signature check failed"

find "${NEW_APP}" -exec xattr -d com.apple.quarantine {} \; 2>/dev/null || true

STAGED="${DEST_DIR}/.${APP_NAME}.new.$$"
BACKUP="${DEST_DIR}/.${APP_NAME}.old.$$"
rm -rf "${STAGED}"
ditto "${NEW_APP}" "${STAGED}"
if [ -e "${DEST_APP}" ]; then
    mv "${DEST_APP}" "${BACKUP}"
fi
if ! mv "${STAGED}" "${DEST_APP}"; then
    [ -e "${BACKUP}" ] && mv "${BACKUP}" "${DEST_APP}"
    rm -rf "${STAGED}"
    fail "could not move the new app into ${DEST_DIR}"
fi
rm -rf "${BACKUP}"

REMAINING="$(find "${DEST_APP}" -exec xattr -p com.apple.quarantine {} \; 2>/dev/null | wc -l | tr -d ' ')"
[ "${REMAINING}" = "0" ] || fail "quarantine flag still present on ${REMAINING} files"

echo "Installed MS Key ${VERSION} to ${DEST_APP}"
echo "On first launch, allow MS Key in System Settings > Privacy & Security > Accessibility."
echo "After each update, remove the old MS Key entry there and allow it again."

if [ "${LAUNCH}" = "1" ]; then
    open "${DEST_APP}"
fi
