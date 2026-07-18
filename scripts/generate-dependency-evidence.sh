#!/usr/bin/env bash

set -euo pipefail

usage() {
  printf '%s\n' \
    'Usage: ./scripts/generate-dependency-evidence.sh OUTPUT_DIRECTORY' \
    '' \
    'Creates a new, non-overwriting Rust/Flutter dependency evidence bundle.'
}

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 64
fi

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd -- "${script_directory}/.." && pwd)"
output_argument="$1"

case "${output_argument}" in
  /*) output_directory="${output_argument}" ;;
  *) output_directory="${repository_root}/${output_argument}" ;;
esac

for required_command in cargo flutter jq sha256sum; do
  if ! command -v "${required_command}" >/dev/null 2>&1; then
    printf 'Required command is unavailable: %s\n' "${required_command}" >&2
    exit 69
  fi
done

required_inputs=(
  "${repository_root}/Cargo.lock"
  "${repository_root}/rust-toolchain.toml"
  "${repository_root}/apps/ros/pubspec.yaml"
  "${repository_root}/apps/ros/pubspec.lock"
  "${repository_root}/apps/ros/rust_builder/cargokit/build_tool/pubspec.yaml"
  "${repository_root}/apps/ros/rust_builder/cargokit/build_tool/pubspec.lock"
  "${repository_root}/apps/ros/android/gradle/wrapper/gradle-wrapper.properties"
)

for required_input in "${required_inputs[@]}"; do
  if [[ ! -f "${required_input}" ]]; then
    printf 'Required dependency input is unavailable: %s\n' "${required_input}" >&2
    exit 66
  fi
done

if [[ -e "${output_directory}" && ! -d "${output_directory}" ]]; then
  printf 'Output path exists and is not a directory: %s\n' "${output_directory}" >&2
  exit 73
fi

if [[ -d "${output_directory}" ]] &&
  [[ -n "$(find "${output_directory}" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  printf 'Refusing to overwrite non-empty output directory: %s\n' "${output_directory}" >&2
  exit 73
fi

temporary_directory="$(mktemp -d /tmp/ros-dependency-evidence.XXXXXX)"
cleanup() {
  rm -rf -- "${temporary_directory}"
}
trap cleanup EXIT

(
  cd -- "${repository_root}"
  cargo metadata --locked --offline --format-version 1
) >"${temporary_directory}/cargo-metadata.json"

jq -S '
  {
    schema: "gotigin-dependency-inventory-v1",
    scope: "packages resolved for the locked Cargo workspace",
    license_semantics: "declared_license is unreviewed package manifest metadata; null means no declaration was reported",
    packages: (
      [
        .packages[] |
        {
          ecosystem: "cargo",
          name,
          version,
          source: (.source // "workspace"),
          declared_license: (.license // null),
          declared_license_file_name: (
            if .license_file == null then
              null
            else
              (.license_file | gsub("\\\\"; "/") | split("/") | last)
            end
          )
        }
      ] | sort_by(.name, .version, .source)
    )
  }
' "${temporary_directory}/cargo-metadata.json" \
  >"${temporary_directory}/rust-packages.json"

(
  cd -- "${repository_root}/apps/ros"
  flutter pub deps --json
) >"${temporary_directory}/flutter-pub-deps.json"

jq -S '
  {
    schema: "gotigin-dependency-inventory-v1",
    scope: "packages resolved for the locked Flutter application",
    license_semantics: "flutter pub deps does not report package licenses; every declared_license value is intentionally null pending notice review",
    sdks: ((.sdks // []) | sort_by(.name, .version)),
    packages: (
      [
        .packages[] |
        {
          ecosystem: "pub",
          name,
          version,
          kind,
          source,
          declared_license: null,
          dependencies: ((.dependencies // []) | sort),
          direct_dependencies: ((.directDependencies // []) | sort),
          development_dependencies: ((.devDependencies // []) | sort)
        }
      ] | sort_by(.name, .version, .source, .kind)
    )
  }
' "${temporary_directory}/flutter-pub-deps.json" \
  >"${temporary_directory}/flutter-packages.json"

(
  cd -- \
    "${repository_root}/apps/ros/rust_builder/cargokit/build_tool"
  flutter pub deps --json
) >"${temporary_directory}/cargokit-pub-deps.json"

jq -S '
  {
    schema: "gotigin-dependency-inventory-v1",
    scope: "packages resolved for the locked Cargokit Dart build tool",
    license_semantics: "flutter pub deps does not report package licenses; every declared_license value is intentionally null pending notice review",
    sdks: ((.sdks // []) | sort_by(.name, .version)),
    packages: (
      [
        .packages[] |
        {
          ecosystem: "pub",
          name,
          version,
          kind,
          source,
          declared_license: null,
          dependencies: ((.dependencies // []) | sort),
          direct_dependencies: ((.directDependencies // []) | sort),
          development_dependencies: ((.devDependencies // []) | sort)
        }
      ] | sort_by(.name, .version, .source, .kind)
    )
  }
' "${temporary_directory}/cargokit-pub-deps.json" \
  >"${temporary_directory}/cargokit-build-tool-packages.json"

# The raw Cargo metadata contains machine-specific workspace and target paths.
# Only the normalized inventories belong in the deterministic evidence bundle.
rm -- "${temporary_directory}/cargo-metadata.json" \
  "${temporary_directory}/flutter-pub-deps.json" \
  "${temporary_directory}/cargokit-pub-deps.json"

cp -- "${repository_root}/Cargo.lock" \
  "${temporary_directory}/cargo.lock"
cp -- "${repository_root}/rust-toolchain.toml" \
  "${temporary_directory}/rust-toolchain.toml"
cp -- "${repository_root}/apps/ros/pubspec.yaml" \
  "${temporary_directory}/flutter-app.pubspec.yaml"
cp -- "${repository_root}/apps/ros/pubspec.lock" \
  "${temporary_directory}/flutter-app.pubspec.lock"
cp -- \
  "${repository_root}/apps/ros/rust_builder/cargokit/build_tool/pubspec.yaml" \
  "${temporary_directory}/cargokit-build-tool.pubspec.yaml"
cp -- \
  "${repository_root}/apps/ros/rust_builder/cargokit/build_tool/pubspec.lock" \
  "${temporary_directory}/cargokit-build-tool.pubspec.lock"
cp -- \
  "${repository_root}/apps/ros/android/gradle/wrapper/gradle-wrapper.properties" \
  "${temporary_directory}/android-gradle-wrapper.properties"

printf '%s\n' \
  'Restaurant Operating System dependency evidence' \
  '' \
  'This bundle is a deterministic inventory and lockfile snapshot.' \
  'It is not a CycloneDX/SPDX SBOM, an open-source notice, a vulnerability scan,' \
  'or legal approval. Null or declared license fields must not be interpreted as' \
  'a license conclusion. See docs/runbooks/dependency-evidence-and-sbom.md.' \
  >"${temporary_directory}/README.txt"

(
  cd -- "${temporary_directory}"
  find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%P\n' |
    LC_ALL=C sort |
    while IFS= read -r evidence_file; do
      sha256sum -- "${evidence_file}"
    done
) >"${temporary_directory}/SHA256SUMS"

mkdir -p -- "${output_directory}"
cp -a -- "${temporary_directory}/." "${output_directory}/"

printf 'Dependency evidence written to %s\n' "${output_directory}"
