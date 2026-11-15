#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-${ROOT_DIR}/dist}"
mkdir -p "${OUT_DIR}"
OUT_DIR="$(cd "${OUT_DIR}" && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n1)"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"

if [ -z "${VERSION}" ]; then
    echo "Cannot read workspace version from Cargo.toml" >&2
    exit 1
fi

cd "${ROOT_DIR}"
if [ "${SKIP_BUILD:-0}" != "1" ]; then
    cargo build --release -p mskey-linux
fi

BIN="${TARGET_DIR}/release/mskey-linux"
[ -x "${BIN}" ] || { echo "Missing ${BIN}" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

render_xml() {
    "${BIN}" --print-xml \
        | sed -e "s#<exec>.* --ibus</exec>#<exec>$1 --ibus</exec>#" \
              -e "s#<setup>.* --setup</setup>#<setup>$1 --setup</setup>#"
}

render_desktop() {
    sed "s#/usr/local/bin/mskey-linux#$1#g" "${ROOT_DIR}/data/mskey.desktop"
}

DEB_ROOT="${WORK}/deb"
install -Dm755 "${BIN}" "${DEB_ROOT}/usr/bin/mskey-linux"
render_xml /usr/bin/mskey-linux | install -Dm644 /dev/stdin "${DEB_ROOT}/usr/share/ibus/component/mskey.xml"
render_desktop /usr/bin/mskey-linux | install -Dm644 /dev/stdin "${DEB_ROOT}/usr/share/applications/mskey.desktop"
{ cat "${ROOT_DIR}/LICENSE"; echo; cat "${ROOT_DIR}/THIRD-PARTY-NOTICES.md"; } | install -Dm644 /dev/stdin "${DEB_ROOT}/usr/share/doc/mskey/copyright"
INSTALLED_KB="$(du -sk "${DEB_ROOT}/usr" | cut -f1)"
mkdir -p "${DEB_ROOT}/DEBIAN"
cat > "${DEB_ROOT}/DEBIAN/control" <<EOF
Package: mskey
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: Bui Dang Minh <buidangminh23@gmail.com>
Installed-Size: ${INSTALLED_KB}
Depends: ibus, libgtk-3-0t64 | libgtk-3-0, libglib2.0-0t64 | libglib2.0-0
Recommends: xdotool, wtype
Homepage: https://github.com/buidangminh23/ms-key
Description: Vietnamese Unicode input method for IBus
 MS Key is a lightweight Vietnamese input method engine for IBus with
 Telex, VNI, VIQR and custom keymaps, spelling-aware English word restore,
 macros and a GTK settings panel.
EOF
cat > "${DEB_ROOT}/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v ibus >/dev/null 2>&1; then
    ibus write-cache --system >/dev/null 2>&1 || true
fi
exit 0
EOF
chmod 755 "${DEB_ROOT}/DEBIAN/postinst"
DEB="${OUT_DIR}/mskey_${VERSION}_${ARCH}.deb"
dpkg-deb --root-owner-group --build "${DEB_ROOT}" "${DEB}" >/dev/null

TAR_NAME="MSKey-${VERSION}-linux-${ARCH}"
TAR_ROOT="${WORK}/${TAR_NAME}"
install -Dm755 "${BIN}" "${TAR_ROOT}/mskey-linux"
install -Dm644 "${ROOT_DIR}/data/mskey.desktop" "${TAR_ROOT}/mskey.desktop"
install -Dm644 "${ROOT_DIR}/LICENSE" "${TAR_ROOT}/LICENSE"
install -Dm644 "${ROOT_DIR}/THIRD-PARTY-NOTICES.md" "${TAR_ROOT}/THIRD-PARTY-NOTICES.md"
cat > "${TAR_ROOT}/install.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
IBUS_COMPONENT_DIR="${IBUS_COMPONENT_DIR:-/usr/share/ibus/component}"
APP_DIR="${APP_DIR:-/usr/share/applications}"
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi
${SUDO} install -Dm755 "${HERE}/mskey-linux" "${BIN_DIR}/mskey-linux"
"${BIN_DIR}/mskey-linux" --print-xml | ${SUDO} install -Dm644 /dev/stdin "${IBUS_COMPONENT_DIR}/mskey.xml"
sed "s#/usr/local/bin/mskey-linux#${BIN_DIR}/mskey-linux#g" "${HERE}/mskey.desktop" | ${SUDO} install -Dm644 /dev/stdin "${APP_DIR}/mskey.desktop"
echo "Installed ${BIN_DIR}/mskey-linux"
echo "Run 'ibus restart', then add 'MS Key (Tiếng Việt)' in Settings > Keyboard > Input Sources."
EOF
cat > "${TAR_ROOT}/uninstall.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
PREFIX="${PREFIX:-/usr/local}"
IBUS_COMPONENT_DIR="${IBUS_COMPONENT_DIR:-/usr/share/ibus/component}"
APP_DIR="${APP_DIR:-/usr/share/applications}"
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi
${SUDO} rm -f "${PREFIX}/bin/mskey-linux" "${IBUS_COMPONENT_DIR}/mskey.xml" "${APP_DIR}/mskey.desktop"
rm -f "${XDG_CONFIG_HOME:-${HOME}/.config}/autostart/mskey.desktop"
echo "Removed MS Key. Run 'ibus restart' to unload it."
EOF
chmod 755 "${TAR_ROOT}/install.sh" "${TAR_ROOT}/uninstall.sh"
TARBALL="${OUT_DIR}/${TAR_NAME}.tar.gz"
tar -C "${WORK}" --owner=0 --group=0 -czf "${TARBALL}" "${TAR_NAME}"

echo "${DEB}"
echo "${TARBALL}"
