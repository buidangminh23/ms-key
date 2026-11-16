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

download_file() {
    if [ -t 2 ] && [ "${TERM:-dumb}" != dumb ] && [ "${MSKEY_REDUCE_MOTION:-0}" != 1 ]; then
        curl -fL --progress-bar "$1" -o "$2"
    else
        curl -fsSL "$1" -o "$2"
    fi
}

run_stage() (
    LABEL="$1"
    shift
    if [ ! -t 2 ] || [ "${TERM:-dumb}" = dumb ] || [ "${MSKEY_REDUCE_MOTION:-0}" = 1 ]; then
        echo "MS Key: ${LABEL}"
        "$@"
        exit $?
    fi
    "$@" &
    STAGE_PID=$!
    trap 'kill "${STAGE_PID}" 2>/dev/null || true; printf "\r\033[K" >&2' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    FRAMES='|/-\'
    INDEX=0
    while kill -0 "${STAGE_PID}" 2>/dev/null; do
        printf '\rMS Key %s %s' "${FRAMES:INDEX:1}" "${LABEL}" >&2
        INDEX=$(( (INDEX + 1) % 4 ))
        sleep 0.1
    done
    STATUS=0
    wait "${STAGE_PID}" || STATUS=$?
    trap - EXIT INT TERM
    printf '\r\033[K' >&2
    if [ "${STATUS}" -eq 0 ]; then printf 'MS Key: %s — done\n' "${LABEL}" >&2; fi
    exit "${STATUS}"
)

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
STAGED=""
cleanup() {
    rm -rf "${WORK}"
    if [ -n "${STAGED}" ]; then rm -rf "${STAGED}"; fi
}
trap cleanup EXIT

echo "Downloading checksums..."
download_file "${BASE_URL}/SHA256SUMS" "${WORK}/SHA256SUMS"
tr -d '\r' < "${WORK}/SHA256SUMS" > "${WORK}/SHA256SUMS.lf"
LINE="$(grep -E '  MSKey-[0-9][^ ]*-macos-universal\.zip$' "${WORK}/SHA256SUMS.lf" | head -n1 || true)"
[ -n "${LINE}" ] || fail "no macOS build listed in SHA256SUMS"
EXPECTED="${LINE%% *}"
ASSET="${LINE##* }"

echo "Downloading ${ASSET}..."
download_file "${BASE_URL}/${ASSET}" "${WORK}/${ASSET}"
ACTUAL="$(shasum -a 256 "${WORK}/${ASSET}" | cut -d' ' -f1)"
[ "${ACTUAL}" = "${EXPECTED}" ] || fail "checksum mismatch for ${ASSET}"

run_stage "Unpacking MS Key" ditto -x -k "${WORK}/${ASSET}" "${WORK}/unpacked"
NEW_APP="${WORK}/unpacked/${APP_NAME}"
[ -d "${NEW_APP}" ] || fail "${APP_NAME} not found in ${ASSET}"
GOT_ID="$(defaults read "${NEW_APP}/Contents/Info" CFBundleIdentifier 2>/dev/null || true)"
[ "${GOT_ID}" = "${BUNDLE_ID}" ] || fail "unexpected bundle identifier '${GOT_ID}'"
VERSION="$(defaults read "${NEW_APP}/Contents/Info" CFBundleShortVersionString 2>/dev/null || echo unknown)"
run_stage "Verifying app signature" codesign --verify --deep --strict "${NEW_APP}" || fail "code signature check failed"

find "${NEW_APP}" -exec xattr -d com.apple.quarantine {} \; 2>/dev/null || true

LOCAL_SIGNED=0
SIGNER="${NEW_APP}/Contents/Resources/macos-local-sign.sh"
if [ -f "${SIGNER}" ] && run_stage "Signing with this Mac's MS Key certificate" bash "${SIGNER}" "${NEW_APP}"; then
    LOCAL_SIGNED=1
else
    echo "Note: kept the release signature; Accessibility may need to be allowed again after this update."
fi

STAGED="${DEST_DIR}/.${APP_NAME}.new.$$"
BACKUP="${DEST_DIR}/.${APP_NAME}.old.$$"
rm -rf "${STAGED}"
run_stage "Installing MS Key" ditto "${NEW_APP}" "${STAGED}"
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
if [ "${LOCAL_SIGNED}" = "1" ]; then
    echo "MS Key is signed with a certificate kept on this Mac, so the Accessibility permission stays across future updates."
    echo "If typing does not work after this first switch, remove the old MS Key entry in System Settings > Privacy & Security > Accessibility and allow it once more."
else
    echo "On first launch, allow MS Key in System Settings > Privacy & Security > Accessibility."
    echo "After each update, remove the old MS Key entry there and allow it again."
fi

if [ "${LAUNCH}" = "1" ]; then
    open "${DEST_APP}"
fi
