#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_APP=""
if [ "${1:-}" = "--dmg-from-app" ]; then
    [ $# -ge 2 ] || { echo "Usage: $0 --dmg-from-app <MSKey.app> [output-directory]" >&2; exit 2; }
    SOURCE_APP="$(cd "$2" && pwd)"
    shift 2
fi
OUT_DIR="${1:-${ROOT_DIR}/dist}"
mkdir -p "${OUT_DIR}"
OUT_DIR="$(cd "${OUT_DIR}" && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n1)"
if [ -n "${SOURCE_APP}" ]; then
    VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "${SOURCE_APP}/Contents/Info.plist")"
fi
BUILD_NUMBER="${BUILD_NUMBER:-1}"
RUST_TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

[ -n "${VERSION}" ] || { echo "Cannot read workspace version from Cargo.toml" >&2; exit 1; }

create_dmg() (
    SOURCE="$1"
    codesign --verify --deep --strict "${SOURCE}"
    [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "${SOURCE}/Contents/Info.plist")" = "org.mskey.mac" ]
    [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "${SOURCE}/Contents/Info.plist")" = "${VERSION}" ]
    SOURCE_ARCHS="$(lipo -archs "${SOURCE}/Contents/MacOS/MSKey")"
    case "${SOURCE_ARCHS}" in
        *arm64*x86_64*|*x86_64*arm64*) ;;
        *) echo "Expected a universal binary, got: ${SOURCE_ARCHS}" >&2; exit 1 ;;
    esac
    DMG_WORK="$(mktemp -d)"
    trap 'rm -rf "${DMG_WORK}"' EXIT
    mkdir -p "${DMG_WORK}/volume"
    ditto "${SOURCE}" "${DMG_WORK}/volume/MSKey.app"
    INSTALLER_PACKAGE="${ROOT_DIR}/macos/MSKeyInstaller"
    MACOSX_DEPLOYMENT_TARGET=13.0 swift build --package-path "${INSTALLER_PACKAGE}" -c release --arch arm64 --arch x86_64
    INSTALLER_BIN="$(swift build --package-path "${INSTALLER_PACKAGE}" -c release --arch arm64 --arch x86_64 --show-bin-path)/MSKeyInstaller"
    INSTALLER_APP="${DMG_WORK}/volume/Install MS Key.app"
    mkdir -p "${INSTALLER_APP}/Contents/MacOS" "${INSTALLER_APP}/Contents/Resources"
    cp "${INSTALLER_BIN}" "${INSTALLER_APP}/Contents/MacOS/MSKeyInstaller"
    cp "${INSTALLER_PACKAGE}/Info.plist" "${INSTALLER_APP}/Contents/Info.plist"
    plutil -replace CFBundleShortVersionString -string "${VERSION}" "${INSTALLER_APP}/Contents/Info.plist"
    plutil -replace CFBundleVersion -string "${BUILD_NUMBER}" "${INSTALLER_APP}/Contents/Info.plist"
    cp "${ROOT_DIR}/assets/branding/mskey.icns" "${INSTALLER_APP}/Contents/Resources/mskey.icns"
    cp "${ROOT_DIR}/assets/branding/mskey-mark.png" "${INSTALLER_APP}/Contents/Resources/mskey-mark.png"
    ditto "${SOURCE}" "${INSTALLER_APP}/Contents/Resources/MSKey.app"
    cp "${ROOT_DIR}/LICENSE" "${INSTALLER_APP}/Contents/Resources/LICENSE"
    codesign --force --sign - "${INSTALLER_APP}"
    codesign --verify --deep --strict "${INSTALLER_APP}"
    INSTALLER_ARCHS="$(lipo -archs "${INSTALLER_APP}/Contents/MacOS/MSKeyInstaller")"
    case "${INSTALLER_ARCHS}" in
        *arm64*x86_64*|*x86_64*arm64*) ;;
        *) echo "Expected a universal installer, got: ${INSTALLER_ARCHS}" >&2; exit 1 ;;
    esac
    ln -s /Applications "${DMG_WORK}/volume/Applications"
    cp "${ROOT_DIR}/assets/branding/mskey.icns" "${DMG_WORK}/volume/.VolumeIcon.icns"
    if command -v SetFile >/dev/null 2>&1; then
        SetFile -a C "${DMG_WORK}/volume"
    fi
    cat > "${DMG_WORK}/volume/Install.txt" <<'EOF'
MS Key for macOS 13 or later (Apple Silicon and Intel)

Recommended: install from Terminal instead of this disk image.

  curl -fsSL https://github.com/buidangminh23/ms-key/releases/latest/download/install-macos.sh | bash

To install from this disk image:

1. Quit MS Key if it is already running.
2. Open Install MS Key.app and click Cai dat (Install) — or drag MSKey.app to
   Applications using Finder — then eject this disk image.
3. Both apps are ad-hoc signed and not notarized, and a browser marks the
   download, so macOS refuses to open them. Clear the mark once in Terminal:

     xattr -dr com.apple.quarantine /Applications/MSKey.app

4. Open MS Key from Applications.
5. Allow MS Key in System Settings > Privacy & Security > Accessibility.

The installer keeps the previous app if copying fails or is cancelled.

After an update, remove the old MS Key Accessibility entry and allow it again.
EOF
    DMG="${OUT_DIR}/MSKey-${VERSION}-macos-universal.dmg"
    hdiutil create -volname "MS Key ${VERSION}" -srcfolder "${DMG_WORK}/volume" \
        -format UDZO -fs HFS+ "${DMG_WORK}/MSKey.dmg"
    hdiutil verify "${DMG_WORK}/MSKey.dmg"
    mv -f "${DMG_WORK}/MSKey.dmg" "${DMG}"
    echo "${DMG}"
)

if [ -n "${SOURCE_APP}" ]; then
    create_dmg "${SOURCE_APP}"
    exit 0
fi

cd "${ROOT_DIR}"
rustup target add "${RUST_TARGETS[@]}" >/dev/null
for target in "${RUST_TARGETS[@]}"; do
    cargo build --release -p mskey-ffi --target "${target}"
done

UNIVERSAL_LIB_DIR="${TARGET_DIR}/universal-apple-darwin/release"
mkdir -p "${UNIVERSAL_LIB_DIR}"
lipo -create \
    "${TARGET_DIR}/aarch64-apple-darwin/release/libmskey_ffi.a" \
    "${TARGET_DIR}/x86_64-apple-darwin/release/libmskey_ffi.a" \
    -output "${UNIVERSAL_LIB_DIR}/libmskey_ffi.a"

cd "${ROOT_DIR}/macos/MSKeyMac"
export MSKEY_FFI_LIB_DIR="${UNIVERSAL_LIB_DIR}"
swift build -c release --arch arm64 --arch x86_64
BIN_PATH="$(swift build -c release --arch arm64 --arch x86_64 --show-bin-path)/MSKeyMac"

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT
APP="${WORK}/MSKey.app"
mkdir -p "${APP}/Contents/MacOS" "${APP}/Contents/Resources"
cp "${BIN_PATH}" "${APP}/Contents/MacOS/MSKey"
chmod 755 "${APP}/Contents/MacOS/MSKey"
cp "${ROOT_DIR}/macos/MSKeyMac/Info.plist" "${APP}/Contents/Info.plist"
cp "${ROOT_DIR}/assets/branding/mskey.icns" "${APP}/Contents/Resources/mskey.icns"
cp "${ROOT_DIR}/assets/branding/mskey-mark.png" "${APP}/Contents/Resources/mskey-mark.png"
plutil -replace CFBundleShortVersionString -string "${VERSION}" "${APP}/Contents/Info.plist"
plutil -replace CFBundleVersion -string "${BUILD_NUMBER}" "${APP}/Contents/Info.plist"
cp "${ROOT_DIR}/LICENSE" "${APP}/Contents/Resources/LICENSE"
cp "${ROOT_DIR}/THIRD-PARTY-NOTICES.md" "${APP}/Contents/Resources/THIRD-PARTY-NOTICES.md"
codesign --force --deep --sign - "${APP}"
codesign --verify --deep --strict "${APP}"

ARCHS="$(lipo -archs "${APP}/Contents/MacOS/MSKey")"
case "${ARCHS}" in
    *arm64*x86_64*|*x86_64*arm64*) ;;
    *) echo "Expected a universal binary, got: ${ARCHS}" >&2; exit 1 ;;
esac

ZIP="${OUT_DIR}/MSKey-${VERSION}-macos-universal.zip"
rm -f "${ZIP}"
ditto -c -k --sequesterRsrc --keepParent "${APP}" "${ZIP}"
cp "${ROOT_DIR}/scripts/install-macos.sh" "${OUT_DIR}/install-macos.sh"
create_dmg "${APP}"

echo "${ZIP}"
echo "${OUT_DIR}/install-macos.sh"
