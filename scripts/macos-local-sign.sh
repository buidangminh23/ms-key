#!/usr/bin/env bash
set -euo pipefail

APP="${1:-}"
[ -n "${APP}" ] && [ -d "${APP}" ] || { echo "usage: macos-local-sign.sh /path/to/MSKey.app" >&2; exit 2; }

IDENTITY_NAME="MS Key Local Signing"
SIGN_DIR="${MSKEY_SIGNING_DIR:-${HOME}/Library/Application Support/MSKey/signing}"
KEYCHAIN="${SIGN_DIR}/mskey-signing.keychain-db"
PASS_FILE="${SIGN_DIR}/keychain-pass"

OPENSSL="/usr/bin/openssl"
[ -x "${OPENSSL}" ] || OPENSSL="$(command -v openssl || true)"
[ -n "${OPENSSL}" ] || { echo "macos-local-sign: openssl not found" >&2; exit 3; }

for tool in security codesign; do
    command -v "${tool}" >/dev/null 2>&1 || { echo "macos-local-sign: ${tool} not found" >&2; exit 3; }
done

mkdir -p "${SIGN_DIR}"
chmod 700 "${SIGN_DIR}"

if [ ! -f "${KEYCHAIN}" ] || [ ! -s "${PASS_FILE}" ]; then
    rm -f "${KEYCHAIN}" "${PASS_FILE}"
    WORK="$(mktemp -d)"
    trap 'rm -f "${WORK}"/*; rmdir "${WORK}" 2>/dev/null || true' EXIT
    ( umask 077 && "${OPENSSL}" rand -hex 32 > "${PASS_FILE}" )
    PASS="$(cat "${PASS_FILE}")"
    cat > "${WORK}/cert.cnf" <<'CNF'
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = MS Key Local Signing
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF
    "${OPENSSL}" req -x509 -newkey rsa:2048 -nodes -days 7300 \
        -config "${WORK}/cert.cnf" -keyout "${WORK}/key.pem" -out "${WORK}/cert.pem" >/dev/null 2>&1
    "${OPENSSL}" pkcs12 -export -inkey "${WORK}/key.pem" -in "${WORK}/cert.pem" \
        -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
        -name "${IDENTITY_NAME}" -out "${WORK}/identity.p12" -passout pass:mskey >/dev/null 2>&1
    security create-keychain -p "${PASS}" "${KEYCHAIN}"
    security set-keychain-settings "${KEYCHAIN}"
    security unlock-keychain -p "${PASS}" "${KEYCHAIN}"
    security import "${WORK}/identity.p12" -k "${KEYCHAIN}" -P mskey -T /usr/bin/codesign >/dev/null
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "${PASS}" "${KEYCHAIN}" >/dev/null
fi

PASS="$(cat "${PASS_FILE}")"
security unlock-keychain -p "${PASS}" "${KEYCHAIN}"
HASH="$(security find-certificate -c "${IDENTITY_NAME}" -Z "${KEYCHAIN}" | awk '/SHA-1 hash:/ { print $NF; exit }')"
[ -n "${HASH}" ] || { echo "macos-local-sign: signing certificate missing" >&2; exit 4; }

codesign --force --deep --keychain "${KEYCHAIN}" --sign "${HASH}" "${APP}"
codesign --verify --deep --strict "${APP}"
security lock-keychain "${KEYCHAIN}" >/dev/null 2>&1 || true
echo "Signed ${APP} with ${IDENTITY_NAME} (${HASH})"
