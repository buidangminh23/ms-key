#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-${ROOT_DIR}/dist}"
mkdir -p "${OUT_DIR}"
OUT_DIR="$(cd "${OUT_DIR}" && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n1)"
BUILD_NUMBER="${BUILD_NUMBER:-1}"
RUST_TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

[ -n "${VERSION}" ] || { echo "Cannot read workspace version from Cargo.toml" >&2; exit 1; }

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

echo "${ZIP}"
echo "${OUT_DIR}/install-macos.sh"
