#!/usr/bin/env bash
# Provision pinned SQLCipher 4.17.0 under third_party/sqlcipher/.
# Linux: shared libsqlcipher.so (static TLS archives fail to link into the
# Flutter Rust cdylib). Windows: use provision-sqlcipher-artifacts.ps1.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SQLCIPHER_VERSION="4.17.0"
TARBALL_URL="https://github.com/sqlcipher/sqlcipher/archive/refs/tags/v${SQLCIPHER_VERSION}.tar.gz"
TARBALL_SHA256="79c0e164b9c059e7487bf8f29272f601cca5f3312cc267461f81e349962a5058"
PROVENANCE_URL="https://github.com/sqlcipher/sqlcipher/releases/tag/v${SQLCIPHER_VERSION}"

OUT_ROOT="${ROOT}/third_party/sqlcipher"
CACHE_DIR="${ROOT}/tools/sqlcipher-src"
TARBALL="${CACHE_DIR}/sqlcipher-${SQLCIPHER_VERSION}.tar.gz"
TARGET_TRIPLE="x86_64-unknown-linux-gnu"
ARTIFACT_DIR="${OUT_ROOT}/linux-x86_64"
ARTIFACT_LIB="${ARTIFACT_DIR}/libsqlcipher.so"

usage() {
  cat <<'EOF'
Usage: ./scripts/provision-sqlcipher-artifacts.sh [--force]

Downloads pinned SQLCipher 4.17.0, builds a shared OpenSSL-backed library for
Linux x86_64, installs it under third_party/sqlcipher/linux-x86_64/, and
rewrites MANIFEST.toml.

Windows: run .\scripts\provision-sqlcipher-artifacts.ps1 on an MSVC host.
EOF
}

FORCE=0
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This script builds the Linux artifact. On Windows use provision-sqlcipher-artifacts.ps1" >&2
  exit 1
fi

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Required command not found: $1" >&2
    exit 1
  }
}

require_cmd curl
require_cmd sha256sum
require_cmd tar
require_cmd make
require_cmd gcc
require_cmd pkg-config

if ! pkg-config --exists openssl; then
  echo "OpenSSL development files required (pkg-config openssl)." >&2
  exit 1
fi

mkdir -p "${CACHE_DIR}" "${ARTIFACT_DIR}/include"

if [[ -e "${ARTIFACT_LIB}" && "${FORCE}" -eq 0 ]]; then
  echo "Linux artifact already present: ${ARTIFACT_LIB}"
  echo "Pass --force to rebuild."
else
  if [[ ! -f "${TARBALL}" ]]; then
    echo "Downloading SQLCipher ${SQLCIPHER_VERSION}..."
    curl -fsSL -o "${TARBALL}.partial" "${TARBALL_URL}"
    mv "${TARBALL}.partial" "${TARBALL}"
  fi

  ACTUAL_SHA="$(sha256sum "${TARBALL}" | awk '{print $1}')"
  if [[ "${ACTUAL_SHA}" != "${TARBALL_SHA256}" ]]; then
    echo "Tarball checksum mismatch:" >&2
    echo "  expected ${TARBALL_SHA256}" >&2
    echo "  actual   ${ACTUAL_SHA}" >&2
    exit 1
  fi

  tar -xzf "${TARBALL}" -C "${CACHE_DIR}"
  SRC_DIR="${CACHE_DIR}/sqlcipher-${SQLCIPHER_VERSION}"
  if [[ ! -d "${SRC_DIR}" ]]; then
    echo "Expected extracted source at ${SRC_DIR}" >&2
    exit 1
  fi

  OPENSSL_CFLAGS="$(pkg-config --cflags openssl)"
  OPENSSL_LIBS="$(pkg-config --libs openssl)"

  cd "${SRC_DIR}"

  # SQLCipher 4.17.0 takes the address of a __thread variable (xoshiro_s), which
  # produces R_X86_64_DTPOFF32 failures with GCC -fPIC on current toolchains.
  # Use a process-wide RNG state instead; OpenSSL still backs the cipher.
  python3 - <<'PY'
from pathlib import Path
path = Path("src/sqlcipher.c")
text = path.read_text()
old = "static __thread volatile uint64_t xoshiro_s[4];"
new = "static volatile uint64_t xoshiro_s[4]; /* ROS: non-TLS for PIC link */"
if old not in text:
    raise SystemExit("expected __thread xoshiro_s declaration missing in src/sqlcipher.c")
path.write_text(text.replace(old, new, 1))
print("patched src/sqlcipher.c xoshiro_s TLS -> process-global")
PY

  ./configure \
    --enable-shared \
    --disable-static \
    --disable-tcl \
    --with-tempstore=yes \
    CFLAGS="-fPIC -DSQLITE_HAS_CODEC -DSQLCIPHER_CRYPTO_OPENSSL -DSQLITE_TEMP_STORE=2 -DSQLITE_EXTRA_INIT=sqlcipher_extra_init -DSQLITE_EXTRA_SHUTDOWN=sqlcipher_extra_shutdown -O2 ${OPENSSL_CFLAGS}" \
    LDFLAGS="${OPENSSL_LIBS}"

  make -j"$(nproc)" clean >/dev/null 2>&1 || true
  make -j"$(nproc)"

  BUILT_SO=""
  for candidate in \
    "${SRC_DIR}/.libs/libsqlite3.so" \
    "${SRC_DIR}/libsqlite3.so" \
    "${SRC_DIR}/.libs/libsqlcipher.so" \
    "${SRC_DIR}/libsqlcipher.so"
  do
    if [[ -e "${candidate}" ]]; then
      BUILT_SO="${candidate}"
      break
    fi
  done
  if [[ -z "${BUILT_SO}" ]]; then
    echo "Could not find built shared library under ${SRC_DIR}" >&2
    find "${SRC_DIR}" -name '*.so*' | head -40 >&2 || true
    exit 1
  fi

  rm -rf "${ARTIFACT_DIR}"
  mkdir -p "${ARTIFACT_DIR}/include"

  # Copy the shared object and any libtool versioned siblings from .libs/.
  SO_DIR="$(dirname "${BUILT_SO}")"
  shopt -s nullglob
  for f in "${SO_DIR}"/libsqlite3.so* "${SO_DIR}"/libsqlcipher.so*; do
    [[ -e "${f}" ]] || continue
    cp -a "${f}" "${ARTIFACT_DIR}/"
  done
  shopt -u nullglob

  if [[ ! -e "${ARTIFACT_DIR}/libsqlcipher.so" ]]; then
    if [[ -e "${ARTIFACT_DIR}/libsqlite3.so" ]]; then
      cp -a "${ARTIFACT_DIR}/libsqlite3.so" "${ARTIFACT_DIR}/libsqlcipher.so"
    else
      first_so="$(ls -1 "${ARTIFACT_DIR}"/libsqlite3.so.* 2>/dev/null | head -1 || true)"
      if [[ -n "${first_so}" ]]; then
        ln -sfn "$(basename "${first_so}")" "${ARTIFACT_DIR}/libsqlcipher.so"
      fi
    fi
  fi

  # AppImages / GTK resolve SONAME libsqlite3.so.0; ship that name so system
  # libsqlite3.so.0 cannot win the first open and split-bind with sqlite3_key.
  if [[ -e "${ARTIFACT_DIR}/libsqlcipher.so" && ! -e "${ARTIFACT_DIR}/libsqlite3.so.0" ]]; then
    ln -sfn libsqlcipher.so "${ARTIFACT_DIR}/libsqlite3.so.0"
  fi
  if [[ -e "${ARTIFACT_DIR}/libsqlcipher.so" && ! -e "${ARTIFACT_DIR}/libsqlite3.so" ]]; then
    ln -sfn libsqlcipher.so "${ARTIFACT_DIR}/libsqlite3.so"
  fi

  for hdr in sqlite3.h sqlite3ext.h; do
    if [[ -f "${SRC_DIR}/${hdr}" ]]; then
      cp -f "${SRC_DIR}/${hdr}" "${ARTIFACT_DIR}/include/${hdr}"
    fi
  done
  if [[ ! -f "${ARTIFACT_DIR}/include/sqlite3.h" ]]; then
    echo "sqlite3.h not found after build" >&2
    exit 1
  fi
  if [[ ! -e "${ARTIFACT_LIB}" ]]; then
    echo "Failed to install ${ARTIFACT_LIB}" >&2
    ls -la "${ARTIFACT_DIR}" >&2
    exit 1
  fi
fi

# Hash the real file behind the symlink when present.
REAL_LIB="$(readlink -f "${ARTIFACT_LIB}")"
LIB_SHA="$(sha256sum "${REAL_LIB}" | awk '{print $1}')"
REVIEWED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
LIB_BASENAME="$(basename "${REAL_LIB}")"

cat >"${ARTIFACT_DIR}/SHA256SUMS" <<EOF
${LIB_SHA}  ${LIB_BASENAME}
EOF

cat >"${OUT_ROOT}/MANIFEST.toml" <<EOF
schema_version = 1
sqlcipher_version = "${SQLCIPHER_VERSION}"
openssl_fips_note = "community OpenSSL via pkg-config; not FIPS-validated"
reviewed_by = "Gotigin engineering (pinned official SQLCipher ${SQLCIPHER_VERSION} self-build)"
reviewed_at_utc = "${REVIEWED_AT}"

[[artifact]]
target = "${TARGET_TRIPLE}"
path = "linux-x86_64/libsqlcipher.so"
sha256 = "${LIB_SHA}"
provenance_url = "${PROVENANCE_URL}"
EOF

WIN_LIB="${OUT_ROOT}/windows-x86_64/sqlcipher.lib"
if [[ -f "${WIN_LIB}" ]]; then
  WIN_SHA="$(sha256sum "${WIN_LIB}" | awk '{print $1}')"
  cat >>"${OUT_ROOT}/MANIFEST.toml" <<EOF

[[artifact]]
target = "x86_64-pc-windows-msvc"
path = "windows-x86_64/sqlcipher.lib"
sha256 = "${WIN_SHA}"
provenance_url = "${PROVENANCE_URL}"
EOF
fi

echo
echo "Provisioned Linux SQLCipher ${SQLCIPHER_VERSION}:"
echo "  ${ARTIFACT_LIB} -> ${REAL_LIB}"
echo "  sha256=${LIB_SHA}"
echo "  MANIFEST.toml updated"
echo
echo "Next: ./scripts/build-linux-release.sh"
