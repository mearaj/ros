#!/usr/bin/env bash
# Completely clear the unpublished local ROS development installation on this
# desktop (Linux, macOS, or Windows). Stops a running app if needed, clears
# every development restaurant-profile database and secure-store credential,
# and removes the application-support directory so the next run is a true
# first-launch.

set -euo pipefail

readonly application_id='com.gotigin.ros'
# Pre-rename application-support directory that may still hold an old Owner DB.
readonly legacy_application_id='com.gotigin.restaurant_os'
readonly development_database_file_name='restaurant-os.development.db'
readonly repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: scripts/uninstall-local-ros.sh

Irreversibly remove the local ROS development installation on this machine
(Linux, macOS, or Windows desktop). No confirmation flags are required.

This stops a running `ros` process if needed, clears every development
restaurant-profile database and matching secure-store credential, removes the
application-support directory (including the pre-rename legacy path), and
removes the local Flutter debug bundle.

  -h, --help    Show this help
EOF
}

fail() {
  printf 'ROS local uninstall did not complete: %s\n' "$*" >&2
  exit 1
}

while (($# > 0)); do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1 (this script takes no uninstall flags; see --help)"
      ;;
  esac
done

host_system() {
  case "$(uname -s)" in
    Linux) printf 'Linux\n' ;;
    Darwin) printf 'Darwin\n' ;;
    MINGW* | MSYS* | CYGWIN*) printf 'Windows\n' ;;
    *) fail "local uninstall is not supported on $(uname -s)." ;;
  esac
}

desktop_support_directory_for() {
  local app_id="$1"
  case "$(host_system)" in
    Linux)
      printf '%s/%s\n' "${XDG_DATA_HOME:-"$HOME/.local/share"}" "$app_id"
      ;;
    Darwin)
      printf '%s/Library/Application Support/%s\n' "$HOME" "$app_id"
      ;;
    Windows)
      [[ -n "${APPDATA:-}" ]] || fail 'APPDATA is unavailable.'
      if [[ "$app_id" == "$application_id" ]]; then
        printf '%s/Gotigin Software & Hardware Private Limited/Restaurant Operating System\n' "$APPDATA"
      else
        printf '%s/%s\n' "$APPDATA" "$app_id"
      fi
      ;;
  esac
}

desktop_build_path() {
  case "$(host_system)" in
    Linux) printf '%s/apps/ros/build/linux/x64/debug/bundle\n' "$repository_root" ;;
    Darwin) printf '%s/apps/ros/build/macos/Build/Products/Debug/ros.app\n' "$repository_root" ;;
    Windows) printf '%s/apps/ros/build/windows/x64/runner/Debug\n' "$repository_root" ;;
  esac
}

app_is_running() {
  if [[ "$(host_system)" == 'Windows' ]]; then
    tasklist /FI 'IMAGENAME eq ros.exe' /NH 2>/dev/null | tr '[:upper:]' '[:lower:]' | grep -q 'ros\.exe'
    return
  fi
  if ! command -v pgrep >/dev/null 2>&1; then
    return 1
  fi
  pgrep -x ros >/dev/null 2>&1
}

kill_running_app() {
  if [[ "$(host_system)" == 'Windows' ]]; then
    taskkill /IM ros.exe /F >/dev/null 2>&1 || true
    return
  fi
  if command -v pkill >/dev/null 2>&1; then
    pkill -x ros >/dev/null 2>&1 || true
  else
    pgrep -x ros 2>/dev/null | xargs -r kill >/dev/null 2>&1 || true
  fi
  # Give SQLCipher / keyring handles a moment to release before lock acquisition.
  sleep 1
}

remove_path() {
  local path="$1"
  [[ -e "$path" || -L "$path" ]] || return 0
  [[ -n "$path" && "$path" != '/' && "$path" != "$HOME" ]] ||
    fail "refusing to remove unsafe path: $path"
  printf 'Removing %s\n' "$path"
  rm -rf -- "$path"
}

describe_support_tree() {
  local dir="$1"
  [[ -d "$dir" ]] || {
    printf '  (absent)\n'
    return 0
  }
  printf '  database: %s\n' "$dir/$development_database_file_name"
  [[ -f "$dir/restaurant-profiles.json" ]] &&
    printf '  profiles registry: %s\n' "$dir/restaurant-profiles.json"
  [[ -d "$dir/profiles" ]] &&
    printf '  additional profile databases under: %s\n' "$dir/profiles"
  [[ -d "$dir/portable-backups" ]] &&
    printf '  portable kits under: %s\n' "$dir/portable-backups"
  [[ -d "$dir/diagnostics" ]] &&
    printf '  diagnostics under: %s\n' "$dir/diagnostics"
}

clear_support_directory() {
  local selected_support_dir="$1"
  local label="$2"

  printf '\n%s\n' "$label"
  printf 'Support directory: %s\n' "$selected_support_dir"
  describe_support_tree "$selected_support_dir"

  if [[ -d "$selected_support_dir" ]]; then
    cargo run \
      --locked \
      --manifest-path "$repository_root/apps/ros/rust/Cargo.toml" \
      --bin ros_local_uninstall \
      -- \
      --support-dir "$selected_support_dir" \
      --confirm-local-data-loss
  else
    printf 'Support directory already absent; skipping credential helper.\n'
  fi

  remove_path "$selected_support_dir"
}

primary_support_dir="$(desktop_support_directory_for "$application_id")"
legacy_support_dir="$(desktop_support_directory_for "$legacy_application_id")"

if app_is_running; then
  printf 'ROS is running; stopping it before uninstalling.\n'
  kill_running_app
fi

clear_support_directory "$primary_support_dir" 'Primary development install'
if [[ "$legacy_support_dir" != "$primary_support_dir" ]]; then
  clear_support_directory "$legacy_support_dir" 'Legacy development install (pre-rename)'
fi

remove_path "$(desktop_build_path)"

printf 'Local development uninstall complete. The next development run starts with a new empty workspace (no Owner PIN).\n'
