#!/usr/bin/env bash
# Generate production release signing identities for ROS (Android, Windows PFX
# helper material is Windows-only — this script creates Android + GnuPG keys).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/release-common.sh
source "${ROOT}/scripts/lib/release-common.sh"

FORCE=0
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1 ;;
    -h|--help)
      cat <<'EOF'
Usage: ./scripts/generate-release-signing-keys.sh [--force]

Creates production signing material under secrets/signing/ (gitignored):
  - Android release keystore + apps/ros/android/key.properties
  - GnuPG release key + public key under ros-website/public/keys/

Windows Authenticode PFX generation runs on Windows via:
  .\scripts\generate-release-signing-keys.ps1
EOF
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

require_cmd keytool
require_cmd gpg
require_cmd openssl

SIGNING_ROOT="${ROOT}/secrets/signing"
ANDROID_DIR="${SIGNING_ROOT}/android"
GPG_DIR="${SIGNING_ROOT}/gpg"
PUB_KEY_DIR="${ROOT}/ros-website/public/keys"
PUB_KEY_PATH="${PUB_KEY_DIR}/gotigin-ros-release.pub.asc"

mkdir -p "${ANDROID_DIR}" "${GPG_DIR}" "${PUB_KEY_DIR}"

ANDROID_STORE="${ANDROID_DIR}/ros-release.p12"
ANDROID_PROPS="${ANDROID_DIR}/key.properties"
ANDROID_APP_PROPS="${ROOT}/apps/ros/android/key.properties"
GPG_SEC="${GPG_DIR}/gotigin-ros-release.sec"
GPG_PUB="${GPG_DIR}/gotigin-ros-release.pub"
GPG_PASS_FILE="${GPG_DIR}/passphrase"

if [[ "${FORCE}" -eq 0 ]]; then
  for path in "${ANDROID_STORE}" "${GPG_SEC}"; do
    if [[ -e "${path}" ]]; then
      echo "Refusing to overwrite existing ${path}. Pass --force to rotate." >&2
      exit 1
    fi
  done
else
  # keytool opens an existing PKCS#12 with -storepass; a new random password
  # against the old file fails with "keystore password was incorrect".
  rm -f \
    "${ANDROID_STORE}" \
    "${ANDROID_DIR}/ros-release.jks" \
    "${ANDROID_PROPS}" \
    "${ANDROID_APP_PROPS}" \
    "${GPG_SEC}" \
    "${GPG_PUB}" \
    "${GPG_PASS_FILE}"
fi

# --- Android release keystore -------------------------------------------------
STORE_PASSWORD="$(openssl rand -base64 32 | tr -d '\n' | tr '/+' '_A')"
KEY_PASSWORD="${STORE_PASSWORD}"
KEY_ALIAS="gotigin-restaurant-os"

keytool -genkeypair \
  -keystore "${ANDROID_STORE}" \
  -storetype PKCS12 \
  -storepass "${STORE_PASSWORD}" \
  -keypass "${KEY_PASSWORD}" \
  -alias "${KEY_ALIAS}" \
  -keyalg RSA \
  -keysize 4096 \
  -validity 10950 \
  -dname "CN=Gotigin Restaurant Operating System, OU=Release Engineering, O=Gotigin, L=Remote, ST=NA, C=IN"

# Paths inside key.properties are resolved relative to apps/ros/android/.
cat >"${ANDROID_PROPS}" <<EOF
storeFile=../../../secrets/signing/android/ros-release.p12
storePassword=${STORE_PASSWORD}
keyAlias=${KEY_ALIAS}
keyPassword=${KEY_PASSWORD}
EOF
chmod 600 "${ANDROID_PROPS}" "${ANDROID_STORE}"
cp "${ANDROID_PROPS}" "${ANDROID_APP_PROPS}"
chmod 600 "${ANDROID_APP_PROPS}"
# Remove legacy JKS if rotating from an older generator run.
rm -f "${ANDROID_DIR}/ros-release.jks"

# --- GnuPG release signing key ------------------------------------------------
# Empty passphrase file => unprotected secret key suitable for automated local
# release packaging. Protect the secrets/signing tree with filesystem ACLs.
: >"${GPG_PASS_FILE}"
chmod 600 "${GPG_PASS_FILE}"

GNUPGHOME="$(mktemp -d "${TMPDIR:-/tmp}/ros-gpg-XXXXXX")"
export GNUPGHOME
chmod 700 "${GNUPGHOME}"
cleanup_gpg() {
  rm -rf "${GNUPGHOME}"
}
trap cleanup_gpg EXIT

BATCH_FILE="${GNUPGHOME}/batch"
cat >"${BATCH_FILE}" <<EOF
%echo Generating Gotigin ROS release key
Key-Type: RSA
Key-Length: 4096
Key-Usage: sign
Name-Real: Gotigin ROS Release
Name-Email: ros-release@gotigin.com
Expire-Date: 0
%no-protection
%commit
%echo done
EOF

gpg --batch --generate-key "${BATCH_FILE}"
FPR="$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr:/ {print $10; exit}')"
if [[ -z "${FPR}" ]]; then
  echo "Failed to read generated GnuPG fingerprint." >&2
  exit 1
fi

gpg --batch --export-secret-keys --armor "${FPR}" >"${GPG_SEC}"
gpg --batch --export --armor "${FPR}" >"${GPG_PUB}"
chmod 600 "${GPG_SEC}"
chmod 644 "${GPG_PUB}"
cp "${GPG_PUB}" "${PUB_KEY_PATH}"
chmod 644 "${PUB_KEY_PATH}"

echo
echo "Android keystore : ${ANDROID_STORE}"
echo "Android props    : ${ANDROID_APP_PROPS}"
echo "GnuPG secret     : ${GPG_SEC}"
echo "GnuPG public     : ${PUB_KEY_PATH}"
echo
echo "Next on Windows 11 VM: .\\scripts\\generate-release-signing-keys.ps1"
echo "Then build with scripts/build-{linux,android}-release.sh / build-windows-release.ps1"
