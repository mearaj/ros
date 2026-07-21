#!/usr/bin/env bash
# Build a signed Linux x86_64 AppImage and publish it to ros-website/public/downloads/.
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
Usage: ./scripts/build-linux-release.sh

Builds Flutter Linux release, packages ros-linux-x86_64.AppImage, GnuPG-signs it,
and copies binary + .sig into ros-website/public/downloads/.

Prerequisites:
  - Flutter Linux desktop enabled
  - ./scripts/generate-release-signing-keys.sh
  - Reviewed production SQLCipher artifact for x86_64-unknown-linux-gnu
    (docs/runbooks/sqlcipher-artifact-manifest.md)
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
require_cmd install
require_cmd curl

cleanup() {
  if [[ -n "${GNUPGHOME:-}" && -d "${GNUPGHOME}" ]]; then
    rm -rf "${GNUPGHOME}"
  fi
  if [[ -n "${APPDIR:-}" && -d "${APPDIR}" ]]; then
    rm -rf "${APPDIR}"
  fi
}
trap cleanup EXIT

prepare_sqlcipher_for_release "x86_64-unknown-linux-gnu"
import_release_gpg_home
ensure_downloads_dir

# Prefer BFD over rust-lld when linking static SQLCipher into the cdylib.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=bfd"

APP_DIR="$(ros_app_dir)"
OUT_DIR="$(downloads_dir)"
BINARY_NAME="ros-linux-x86_64.AppImage"
SIG_NAME="${BINARY_NAME}.sig"
VERSION="$(read_pubspec_version)"

cd "${APP_DIR}"
flutter pub get --enforce-lockfile
flutter build linux --release

BUNDLE="${APP_DIR}/build/linux/x64/release/bundle"
if [[ ! -x "${BUNDLE}/ros" ]]; then
  echo "Expected Linux release binary at ${BUNDLE}/ros" >&2
  exit 1
fi

APPDIR="$(mktemp -d "${TMPDIR:-/tmp}/ros-appdir-XXXXXX")"
mkdir -p "${APPDIR}/usr/share/applications" "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

# Flutter Linux looks for lib/libapp.so and data/ next to the executable.
# Keep the release bundle layout intact inside the AppImage.
cp -a "${BUNDLE}/ros" "${APPDIR}/ros"
cp -a "${BUNDLE}/lib" "${APPDIR}/lib"
cp -a "${BUNDLE}/data" "${APPDIR}/data"

# Ship production SQLCipher shared library next to Flutter native libs.
# SQLCipher's SONAME is typically libsqlite3.so.0. GTK/tinysparql also load
# libsqlite3.so.0; without our copy first on LD_LIBRARY_PATH, open binds to
# system SQLite while sqlite3_key still comes from libsqlcipher.so → key fails.
SQLCIPHER_DIR="${ROOT}/third_party/sqlcipher/linux-x86_64"
if [[ -d "${SQLCIPHER_DIR}" ]]; then
  shopt -s nullglob
  for so in "${SQLCIPHER_DIR}"/libsqlcipher.so* "${SQLCIPHER_DIR}"/libsqlite3.so*; do
    cp -a "${so}" "${APPDIR}/lib/"
  done
  shopt -u nullglob
  if [[ -e "${APPDIR}/lib/libsqlcipher.so" && ! -e "${APPDIR}/lib/libsqlite3.so.0" ]]; then
    ln -sfn libsqlcipher.so "${APPDIR}/lib/libsqlite3.so.0"
  fi
  if [[ -e "${APPDIR}/lib/libsqlcipher.so" && ! -e "${APPDIR}/lib/libsqlite3.so" ]]; then
    ln -sfn libsqlcipher.so "${APPDIR}/lib/libsqlite3.so"
  fi
fi

cat >"${APPDIR}/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
# Prepend only — never append after system paths for sqlite resolution.
export LD_LIBRARY_PATH="${HERE}/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
cd "${HERE}" || exit 1
exec "${HERE}/ros" "$@"
EOF
chmod 755 "${APPDIR}/AppRun"

cat >"${APPDIR}/ros.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Restaurant Operating System
Comment=Gotigin Restaurant Operating System ${VERSION}
Exec=ros
Icon=ros
Categories=Office;Finance;
Terminal=false
EOF
cp "${APPDIR}/ros.desktop" "${APPDIR}/usr/share/applications/ros.desktop"

ICON_SRC="${APP_DIR}/assets/brand"
if [[ -f "${ROOT}/ros-website/public/logo.png" ]]; then
  cp "${ROOT}/ros-website/public/logo.png" "${APPDIR}/ros.png"
  cp "${ROOT}/ros-website/public/logo.png" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/ros.png"
elif [[ -f "${ICON_SRC}/logo.png" ]]; then
  cp "${ICON_SRC}/logo.png" "${APPDIR}/ros.png"
  cp "${ICON_SRC}/logo.png" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/ros.png"
else
  # AppImage still validates without a custom icon file in some tool versions;
  # keep a tiny placeholder desktop Icon= name only.
  :
fi

TOOLS_DIR="${ROOT}/tools/appimagetool"
mkdir -p "${TOOLS_DIR}"
APPIMAGE_TOOL="${TOOLS_DIR}/appimagetool-x86_64.AppImage"
if [[ ! -x "${APPIMAGE_TOOL}" ]]; then
  echo "Downloading appimagetool..."
  curl -fsSL -o "${APPIMAGE_TOOL}" \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
  chmod +x "${APPIMAGE_TOOL}"
fi

STAGING_APPIMAGE="${OUT_DIR}/${BINARY_NAME}.building"
rm -f "${STAGING_APPIMAGE}" "${OUT_DIR}/${BINARY_NAME}" "${OUT_DIR}/${SIG_NAME}"

# appimagetool may need FUSE; fall back to extract-and-run if necessary.
if ! ARCH=x86_64 "${APPIMAGE_TOOL}" "${APPDIR}" "${STAGING_APPIMAGE}"; then
  echo "appimagetool direct run failed; trying extracted mode..."
  EXTRACT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ros-ait-XXXXXX")"
  cd "${EXTRACT_DIR}"
  "${APPIMAGE_TOOL}" --appimage-extract >/dev/null
  ARCH=x86_64 "${EXTRACT_DIR}/squashfs-root/AppRun" "${APPDIR}" "${STAGING_APPIMAGE}"
  cd "${APP_DIR}"
  rm -rf "${EXTRACT_DIR}"
fi

mv "${STAGING_APPIMAGE}" "${OUT_DIR}/${BINARY_NAME}"
chmod 755 "${OUT_DIR}/${BINARY_NAME}"
detach_sign_file "${OUT_DIR}/${BINARY_NAME}" "${OUT_DIR}/${SIG_NAME}"

echo
echo "Published:"
echo "  ${OUT_DIR}/${BINARY_NAME}"
echo "  ${OUT_DIR}/${SIG_NAME}"
echo "  sha256=$(sha256_file "${OUT_DIR}/${BINARY_NAME}")"
