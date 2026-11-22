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

HAS_FCITX5=0
FCITX5_STAGE="${WORK}/fcitx5-stage"
if command -v cmake >/dev/null 2>&1 && pkg-config --exists Fcitx5Core; then
    FFI_LIB="${TARGET_DIR}/release/libmskey_ffi.a"
    if [ ! -f "${FFI_LIB}" ] && [ "${SKIP_BUILD:-0}" != "1" ]; then
        cargo build --release -p mskey-ffi
    fi
    if [ ! -f "${FFI_LIB}" ] && [ -f "${TARGET_DIR}/release/libmskey_ffi.so" ]; then
        FFI_LIB="${TARGET_DIR}/release/libmskey_ffi.so"
    fi
    if [ -f "${FFI_LIB}" ]; then
        FCITX5_BUILD="${WORK}/fcitx5-build"
        cmake -S "${ROOT_DIR}/linux/fcitx5" -B "${FCITX5_BUILD}" \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=/usr \
            -DMSKEY_FFI_LIB="${FFI_LIB}"
        cmake --build "${FCITX5_BUILD}" -j"$(nproc 2>/dev/null || echo 2)"
        DESTDIR="${FCITX5_STAGE}" cmake --install "${FCITX5_BUILD}"
        HAS_FCITX5=1
    fi
fi

DEB_ROOT="${WORK}/deb"
install -Dm755 "${BIN}" "${DEB_ROOT}/usr/bin/mskey-linux"
render_xml /usr/bin/mskey-linux | install -Dm644 /dev/stdin "${DEB_ROOT}/usr/share/ibus/component/mskey.xml"
install -Dm755 "${ROOT_DIR}/linux/update/mskey-update" "${DEB_ROOT}/usr/lib/mskey/mskey-update"
install -Dm644 "${ROOT_DIR}/linux/update/org.mskey.update.policy" "${DEB_ROOT}/usr/share/polkit-1/actions/org.mskey.update.policy"
render_desktop /usr/bin/mskey-linux | install -Dm644 /dev/stdin "${DEB_ROOT}/usr/share/applications/mskey.desktop"
for size in 32 48 64 128 256 512; do
    install -Dm644 "${ROOT_DIR}/assets/branding/mskey-${size}.png" "${DEB_ROOT}/usr/share/icons/hicolor/${size}x${size}/apps/mskey.png"
done
if [ "${HAS_FCITX5}" = "1" ]; then
    cp -a "${FCITX5_STAGE}/." "${DEB_ROOT}/"
fi
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
Depends: ibus, curl, libgtk-3-0t64 | libgtk-3-0, libglib2.0-0t64 | libglib2.0-0
Recommends: xdotool, wtype, fcitx5, pkexec | policykit-1
Homepage: https://github.com/buidangminh23/ms-key
Description: Vietnamese Unicode input method for IBus and Fcitx5
 MS Key is a lightweight Vietnamese input method engine for IBus and Fcitx5 with
 Telex, VNI, VIQR and custom keymaps, spelling-aware English word restore,
 macros and a GTK settings panel.
EOF
cat > "${DEB_ROOT}/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v ibus >/dev/null 2>&1; then
    ibus write-cache --system >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
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
for size in 32 48 64 128 256 512; do
    install -Dm644 "${ROOT_DIR}/assets/branding/mskey-${size}.png" "${TAR_ROOT}/icons/mskey-${size}.png"
done
if [ "${HAS_FCITX5}" = "1" ]; then
    mkdir -p "${TAR_ROOT}/fcitx5"
    cp -a "${FCITX5_STAGE}/." "${TAR_ROOT}/fcitx5/"
fi
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
ICON_DIR="${ICON_DIR:-${PREFIX}/share/icons/hicolor}"
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi
${SUDO} install -Dm755 "${HERE}/mskey-linux" "${BIN_DIR}/mskey-linux"
"${BIN_DIR}/mskey-linux" --print-xml | ${SUDO} install -Dm644 /dev/stdin "${IBUS_COMPONENT_DIR}/mskey.xml"
sed "s#/usr/local/bin/mskey-linux#${BIN_DIR}/mskey-linux#g" "${HERE}/mskey.desktop" | ${SUDO} install -Dm644 /dev/stdin "${APP_DIR}/mskey.desktop"
for size in 32 48 64 128 256 512; do
    ${SUDO} install -Dm644 "${HERE}/icons/mskey-${size}.png" "${ICON_DIR}/${size}x${size}/apps/mskey.png"
done
if [ -d "${HERE}/fcitx5/usr" ] && command -v fcitx5 >/dev/null 2>&1; then
    ${SUDO} cp -a "${HERE}/fcitx5/usr/." /usr/
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    ${SUDO} gtk-update-icon-cache -q -t -f "${ICON_DIR}" || true
fi
echo "Installed ${BIN_DIR}/mskey-linux"
echo "Run 'ibus restart', then add 'MS Key (Tiếng Việt)' in Settings > Keyboard > Input Sources."
EOF
cat > "${TAR_ROOT}/uninstall.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
PREFIX="${PREFIX:-/usr/local}"
IBUS_COMPONENT_DIR="${IBUS_COMPONENT_DIR:-/usr/share/ibus/component}"
APP_DIR="${APP_DIR:-/usr/share/applications}"
ICON_DIR="${ICON_DIR:-${PREFIX}/share/icons/hicolor}"
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi
${SUDO} rm -f "${PREFIX}/bin/mskey-linux" "${IBUS_COMPONENT_DIR}/mskey.xml" "${APP_DIR}/mskey.desktop"
${SUDO} rm -f /usr/lib/*/fcitx5/mskey.so /usr/lib/fcitx5/mskey.so /usr/share/fcitx5/addon/mskey.conf /usr/share/fcitx5/inputmethod/mskey.conf
for size in 32 48 64 128 256 512; do
    ${SUDO} rm -f "${ICON_DIR}/${size}x${size}/apps/mskey.png"
done
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    ${SUDO} gtk-update-icon-cache -q -t -f "${ICON_DIR}" || true
fi
rm -f "${XDG_CONFIG_HOME:-${HOME}/.config}/autostart/mskey.desktop"
echo "Removed MS Key. Run 'ibus restart' to unload it."
EOF
chmod 755 "${TAR_ROOT}/install.sh" "${TAR_ROOT}/uninstall.sh"
TARBALL="${OUT_DIR}/${TAR_NAME}.tar.gz"
tar -C "${WORK}" --owner=0 --group=0 -czf "${TARBALL}" "${TAR_NAME}"

echo "${DEB}"
echo "${TARBALL}"
