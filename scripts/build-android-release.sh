#!/usr/bin/env bash
# Build a signed universal Android release APK (phones, tablets, other devices)
# and publish it to ros-website/public/downloads/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/release-common.sh
source "${ROOT}/scripts/lib/release-common.sh"

for arg in "$@"; do
  case "$arg" in
    --allow-bundled-sqlcipher)
      echo "Refused: --allow-bundled-sqlcipher is not a release path." >&2
      echo "Place reviewed SQLCipher artifacts under third_party/sqlcipher/." >&2
      echo "See docs/runbooks/sqlcipher-artifact-manifest.md and local-development.md." >&2
      exit 2
      ;;
    -h|--help)
      cat <<'EOF'
Usage: ./scripts/build-android-release.sh

Builds a release-signed universal APK for Android phones, tablets, and other
supported Android form factors, GnuPG-signs the APK for download verification,
and copies binary + .sig into ros-website/public/downloads/.

Prerequisites:
  - Android SDK / NDK via Flutter
  - ./scripts/generate-release-signing-keys.sh  (creates android/key.properties)
  - Reviewed production SQLCipher artifacts for Android ABIs
    (docs/runbooks/sqlcipher-artifact-manifest.md)

Note: packaging an APK does not by itself make Android a supported release
target. See docs/editions/community.md and docs/runbooks/release-verification.md.
EOF
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

require_cmd flutter
require_cmd gpg

cleanup() {
  if [[ -n "${GNUPGHOME:-}" && -d "${GNUPGHOME}" ]]; then
    rm -rf "${GNUPGHOME}"
  fi
}
trap cleanup EXIT

# Prefer arm64 as the required production artifact probe for Android packaging.
ANDROID_TRIPLE_PROBE="aarch64-linux-android"
prepare_sqlcipher_for_release "${ANDROID_TRIPLE_PROBE}"

ANDROID_PROPS="$(ros_app_dir)/android/key.properties"
if [[ ! -f "${ANDROID_PROPS}" ]]; then
  SIGNING_PROPS="$(signing_root)/android/key.properties"
  if [[ -f "${SIGNING_PROPS}" ]]; then
    cp "${SIGNING_PROPS}" "${ANDROID_PROPS}"
    chmod 600 "${ANDROID_PROPS}"
  else
    echo "Missing ${ANDROID_PROPS}. Run ./scripts/generate-release-signing-keys.sh" >&2
    exit 1
  fi
fi

import_release_gpg_home
ensure_downloads_dir

APP_DIR="$(ros_app_dir)"
OUT_DIR="$(downloads_dir)"
BINARY_NAME="ros-android.apk"
SIG_NAME="${BINARY_NAME}.sig"

cd "${APP_DIR}"
flutter pub get --enforce-lockfile
flutter build apk --release

APK_SRC="${APP_DIR}/build/app/outputs/flutter-apk/app-release.apk"
if [[ ! -f "${APK_SRC}" ]]; then
  echo "Expected release APK at ${APK_SRC}" >&2
  exit 1
fi

rm -f "${OUT_DIR}/${BINARY_NAME}" "${OUT_DIR}/${SIG_NAME}"
cp "${APK_SRC}" "${OUT_DIR}/${BINARY_NAME}"
chmod 644 "${OUT_DIR}/${BINARY_NAME}"
detach_sign_file "${OUT_DIR}/${BINARY_NAME}" "${OUT_DIR}/${SIG_NAME}"

echo
echo "Published:"
echo "  ${OUT_DIR}/${BINARY_NAME}"
echo "  ${OUT_DIR}/${SIG_NAME}"
echo "  sha256=$(sha256_file "${OUT_DIR}/${BINARY_NAME}")"
echo
echo "APK is Android-keystore signed (installable). .sig is GnuPG download verification."
