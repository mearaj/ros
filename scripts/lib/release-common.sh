#!/usr/bin/env bash
# Shared helpers for ROS release packaging scripts.
# shellcheck shell=bash

release_root() {
  local here
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  printf '%s\n' "${here}"
}

require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Required command not found on PATH: ${cmd}" >&2
    exit 1
  fi
}

downloads_dir() {
  printf '%s\n' "$(release_root)/ros-website/public/downloads"
}

signing_root() {
  printf '%s\n' "$(release_root)/secrets/signing"
}

gpg_secret_key() {
  printf '%s\n' "$(signing_root)/gpg/gotigin-ros-release.sec"
}

gpg_public_key() {
  printf '%s\n' "$(signing_root)/gpg/gotigin-ros-release.pub"
}

ensure_downloads_dir() {
  mkdir -p "$(downloads_dir)"
}

ros_app_dir() {
  printf '%s\n' "$(release_root)/apps/ros"
}

read_pubspec_version() {
  local pubspec
  pubspec="$(ros_app_dir)/pubspec.yaml"
  awk '
    /^version:/ {
      ver=$2
      sub(/\+.*/, "", ver)
      print ver
      exit
    }
  ' "${pubspec}"
}

# Import the release secret into an isolated GNUPGHOME and leave GNUPGHOME set.
import_release_gpg_home() {
  local sec
  sec="$(gpg_secret_key)"
  if [[ ! -f "${sec}" ]]; then
    echo "Missing GnuPG release secret: ${sec}" >&2
    echo "Run ./scripts/generate-release-signing-keys.sh first." >&2
    exit 1
  fi
  require_cmd gpg
  export GNUPGHOME
  GNUPGHOME="$(mktemp -d "${TMPDIR:-/tmp}/ros-release-gpg-XXXXXX")"
  chmod 700 "${GNUPGHOME}"
  gpg --batch --import "${sec}" >/dev/null 2>&1
}

detach_sign_file() {
  local input="$1"
  local output="$2"
  if [[ -z "${GNUPGHOME:-}" ]]; then
    echo "detach_sign_file requires import_release_gpg_home first." >&2
    exit 1
  fi
  gpg --batch --yes --detach-sign --armor -o "${output}" "${input}"
}

sha256_file() {
  local path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${path}" | awk '{print $1}'
  else
    shasum -a 256 "${path}" | awk '{print $1}'
  fi
}

# Returns 0 when reviewed production SQLCipher artifacts for $1 exist.
has_production_sqlcipher_artifact() {
  local target_triple="$1"
  local manifest
  manifest="$(release_root)/third_party/sqlcipher/MANIFEST.toml"
  if [[ ! -f "${manifest}" ]]; then
    return 1
  fi
  if grep -q 'reviewed_by = ""' "${manifest}"; then
    return 1
  fi
  if ! grep -q "target = \"${target_triple}\"" "${manifest}"; then
    return 1
  fi
  return 0
}

# Export SQLITE3_* so libsqlite3-sys links the reviewed artifact, not pkg-config.
export_production_sqlcipher_env() {
  local target_triple="$1"
  local manifest root artifact_path search_dir include_dir
  root="$(release_root)"
  manifest="${root}/third_party/sqlcipher/MANIFEST.toml"
  artifact_path="$(
    awk -v target="${target_triple}" '
      $0 == "[[artifact]]" { in_art=1; t=""; p=""; next }
      in_art && $1 == "target" {
        gsub(/"/, "", $3); t=$3
      }
      in_art && $1 == "path" {
        gsub(/"/, "", $3); p=$3
      }
      in_art && t != "" && p != "" && t == target {
        print p; exit
      }
    ' "${manifest}"
  )"
  if [[ -z "${artifact_path}" ]]; then
    echo "Could not resolve artifact path for ${target_triple} in MANIFEST.toml" >&2
    exit 1
  fi
  search_dir="$(dirname "${root}/third_party/sqlcipher/${artifact_path}")"
  include_dir="${search_dir}/include"
  # libsqlite3-sys uses SQLCIPHER_* when the sqlcipher feature is enabled.
  export SQLCIPHER_LIB_DIR="${search_dir}"
  export SQLITE3_LIB_DIR="${search_dir}"
  if [[ "${artifact_path}" == *.so || "${artifact_path}" == *.dylib ]]; then
    unset SQLCIPHER_STATIC || true
  else
    export SQLCIPHER_STATIC=1
  fi
  if [[ -d "${include_dir}" ]]; then
    export SQLCIPHER_INCLUDE_DIR="${include_dir}"
    export SQLITE3_INCLUDE_DIR="${include_dir}"
  fi
  # Ensure the linker can find the shared object at runtime during the build.
  export LD_LIBRARY_PATH="${search_dir}:${LD_LIBRARY_PATH:-}"
  echo "SQLCIPHER_LIB_DIR=${SQLCIPHER_LIB_DIR}"
}

prepare_sqlcipher_for_release() {
  local target_triple="$1"
  if has_production_sqlcipher_artifact "${target_triple}"; then
    echo "Using production SQLCipher artifacts for ${target_triple}."
    export_production_sqlcipher_env "${target_triple}"
    return 0
  fi
  cat >&2 <<EOF
Production SQLCipher artifacts for ${target_triple} are not ready.

Place reviewed libraries under third_party/sqlcipher/ per
docs/runbooks/sqlcipher-artifact-manifest.md.

Do not work around this with the development SQLCipher feature, a system
library, or an unsigned ad-hoc library (docs/runbooks/local-development.md).
EOF
  exit 1
}
