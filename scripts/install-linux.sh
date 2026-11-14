#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
IBUS_COMPONENT_DIR="${IBUS_COMPONENT_DIR:-/usr/share/ibus/component}"
APP_DIR="${APP_DIR:-/usr/share/applications}"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to build mskey-linux" >&2
    exit 1
fi

if ! pkg-config --exists gtk+-3.0; then
    echo "gtk+-3.0 development headers not found (Ubuntu/Debian: sudo apt install libgtk-3-dev)" >&2
    exit 1
fi

cd "${ROOT_DIR}"
cargo build --release -p mskey-linux

BIN_SRC="${ROOT_DIR}/target/release/mskey-linux"
if [ -n "${CARGO_TARGET_DIR:-}" ] && [ -x "${CARGO_TARGET_DIR}/release/mskey-linux" ]; then
    BIN_SRC="${CARGO_TARGET_DIR}/release/mskey-linux"
fi

SUDO=""
if [ "$(id -u)" -ne 0 ] && [ ! -w "${BIN_DIR}" ]; then
    SUDO="sudo"
fi

${SUDO} install -Dm755 "${BIN_SRC}" "${BIN_DIR}/mskey-linux"
TMP_XML="$(mktemp)"
trap 'rm -f "${TMP_XML}"' EXIT
"${BIN_DIR}/mskey-linux" --print-xml > "${TMP_XML}"
${SUDO} install -Dm644 "${TMP_XML}" "${IBUS_COMPONENT_DIR}/mskey.xml"
${SUDO} install -Dm644 "${ROOT_DIR}/data/mskey.desktop" "${APP_DIR}/mskey.desktop"

echo "Installed ${BIN_DIR}/mskey-linux"
echo "Installed ${IBUS_COMPONENT_DIR}/mskey.xml"
echo "Installed ${APP_DIR}/mskey.desktop"

if command -v ibus >/dev/null 2>&1 && pgrep -x ibus-daemon >/dev/null 2>&1; then
    ibus write-cache >/dev/null 2>&1 || true
    ibus restart >/dev/null 2>&1 || true
    echo "Reloaded running IBus daemon."
fi
