#!/usr/bin/env python3
"""Completely clear an unpublished local Restaurant OS development install.

This tool is intentionally development-only. It never selects the production
database namespace, never reads a database key, and requires ``--yes`` before
it changes anything. Desktop credential deletion is delegated to the Rust
storage adapter so it matches the credential entry that ROS actually uses.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
from typing import Iterable


APPLICATION_ID = "com.gotigin.ros"
ANDROID_PACKAGE_ID = APPLICATION_ID
IOS_BUNDLE_ID = APPLICATION_ID
DEVELOPMENT_DATABASE_FILE_NAME = "restaurant-os.development.db"


def repository_root() -> Path:
    return Path(__file__).resolve().parent.parent


def desktop_support_directory() -> Path:
    system = platform.system()
    home = Path.home()
    if system == "Linux":
        return Path(os.environ.get("XDG_DATA_HOME", home / ".local" / "share")) / APPLICATION_ID
    if system == "Darwin":
        return home / "Library" / "Application Support" / APPLICATION_ID
    if system == "Windows":
        app_data = os.environ.get("APPDATA")
        if not app_data:
            raise RuntimeError("APPDATA is unavailable, so the Windows support directory cannot be identified.")
        return (
            Path(app_data)
            / "Gotigin Software & Hardware Private Limited"
            / "Restaurant Operating System"
        )
    raise RuntimeError(f"Desktop uninstall is not supported on {system}.")


def desktop_build_paths(root: Path) -> list[Path]:
    system = platform.system()
    if system == "Linux":
        return [root / "apps" / "ros" / "build" / "linux" / "x64" / "debug" / "bundle"]
    if system == "Darwin":
        return [root / "apps" / "ros" / "build" / "macos" / "Build" / "Products" / "Debug" / "ros.app"]
    if system == "Windows":
        return [root / "apps" / "ros" / "build" / "windows" / "x64" / "runner" / "Debug"]
    return []


def app_is_running() -> bool:
    system = platform.system()
    if system == "Windows":
        completed = subprocess.run(
            ["tasklist", "/FI", "IMAGENAME eq ros.exe", "/NH"],
            check=False,
            capture_output=True,
            text=True,
        )
        return "ros.exe" in completed.stdout.lower()
    if shutil.which("pgrep") is None:
        print("Warning: could not check for a running ROS process; close it before continuing.")
        return False
    completed = subprocess.run(
        ["pgrep", "-x", "ros"], check=False, capture_output=True, text=True
    )
    return completed.returncode == 0


def run(command: Iterable[str]) -> None:
    rendered = " ".join(str(part) for part in command)
    print(f"+ {rendered}")
    subprocess.run(list(command), check=True)


def remove_tree(path: Path) -> None:
    if not path.exists():
        return
    resolved = path.resolve()
    if resolved == Path(resolved.anchor) or resolved == Path.home().resolve():
        raise RuntimeError(f"Refusing to remove an unsafe path: {resolved}")
    print(f"Removing {resolved}")
    if resolved.is_dir():
        shutil.rmtree(resolved)
    else:
        resolved.unlink()


def uninstall_desktop(root: Path, args: argparse.Namespace) -> None:
    support_directory = Path(args.support_dir).expanduser() if args.support_dir else desktop_support_directory()
    database_path = support_directory / DEVELOPMENT_DATABASE_FILE_NAME
    print(f"Development support directory: {support_directory}")
    print(f"Development database: {database_path}")
    print("The matching development secure-store credential will also be deleted.")

    if args.dry_run:
        return
    if app_is_running():
        raise RuntimeError("ROS is still running. Close the desktop app before uninstalling it.")

    run(
        [
            "cargo",
            "run",
            "--locked",
            "--manifest-path",
            str(root / "apps" / "ros" / "rust" / "Cargo.toml"),
            "--bin",
            "ros_local_uninstall",
            "--",
            "--support-dir",
            str(support_directory),
            "--confirm-local-data-loss",
        ]
    )
    # The Rust helper has released its bootstrap-lock handle by now, so remove
    # the empty support directory and its non-sensitive lock file as well.
    remove_tree(support_directory)

    if not args.keep_build:
        for path in desktop_build_paths(root):
            remove_tree(path)
    if args.app_path:
        remove_tree(Path(args.app_path).expanduser())


def uninstall_android(args: argparse.Namespace) -> None:
    command = ["adb"]
    if args.device:
        command.extend(["-s", args.device])
    command.extend(["uninstall", ANDROID_PACKAGE_ID])
    print(f"Android package: {ANDROID_PACKAGE_ID}")
    if args.dry_run:
        return
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    output = f"{completed.stdout}\n{completed.stderr}".strip()
    if completed.returncode != 0 and "not installed" not in output.lower():
        raise RuntimeError(output or "adb uninstall failed")
    print(output or "Android package was already absent.")


def uninstall_ios_simulator(args: argparse.Namespace) -> None:
    device = args.device or "booted"
    print(f"iOS Simulator bundle: {IOS_BUNDLE_ID} on {device}")
    if args.erase_ios_simulator:
        print("Warning: erasing a simulator removes every app and all simulator data on that device.")
    if args.dry_run:
        return
    completed = subprocess.run(
        ["xcrun", "simctl", "uninstall", device, IOS_BUNDLE_ID],
        check=False,
        capture_output=True,
        text=True,
    )
    output = f"{completed.stdout}\n{completed.stderr}".strip()
    if completed.returncode != 0 and "not found" not in output.lower():
        raise RuntimeError(output or "simctl uninstall failed")
    print(output or "iOS Simulator app was already absent.")
    if args.erase_ios_simulator:
        run(["xcrun", "simctl", "erase", device])


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Irreversibly remove an unpublished local ROS development installation. "
            "Use --dry-run first; --yes is required for deletion."
        )
    )
    parser.add_argument(
        "--target",
        choices=("desktop", "android", "ios-simulator"),
        default="desktop",
        help="The local development target to remove (default: desktop host).",
    )
    parser.add_argument("--yes", action="store_true", help="Confirm irreversible local data loss.")
    parser.add_argument("--dry-run", action="store_true", help="Show the exact target without changing it.")
    parser.add_argument("--device", help="adb serial or iOS Simulator UDID; defaults to the active device.")
    parser.add_argument(
        "--support-dir",
        help="Override only the desktop application-support directory for a non-standard local run.",
    )
    parser.add_argument(
        "--keep-build",
        action="store_true",
        help="Keep the host's Flutter debug bundle after a desktop uninstall.",
    )
    parser.add_argument(
        "--app-path",
        help="Also remove this explicitly supplied local desktop app bundle or directory.",
    )
    parser.add_argument(
        "--erase-ios-simulator",
        action="store_true",
        help="After iOS Simulator uninstall, erase the whole simulator (all apps and data).",
    )
    arguments = parser.parse_args()
    if arguments.dry_run and arguments.yes:
        parser.error("Use either --dry-run or --yes, not both.")
    if not arguments.dry_run and not arguments.yes:
        parser.error("--yes is required for this destructive operation.")
    if arguments.erase_ios_simulator and arguments.target != "ios-simulator":
        parser.error("--erase-ios-simulator is only valid with --target ios-simulator.")
    if arguments.support_dir and arguments.target != "desktop":
        parser.error("--support-dir is only valid with --target desktop.")
    if arguments.app_path and arguments.target != "desktop":
        parser.error("--app-path is only valid with --target desktop.")
    if arguments.keep_build and arguments.target != "desktop":
        parser.error("--keep-build is only valid with --target desktop.")
    return arguments


def main() -> int:
    arguments = parse_arguments()
    root = repository_root()
    try:
        if arguments.target == "desktop":
            uninstall_desktop(root, arguments)
        elif arguments.target == "android":
            uninstall_android(arguments)
        else:
            uninstall_ios_simulator(arguments)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ROS local uninstall did not complete: {error}", file=sys.stderr)
        return 1

    if arguments.dry_run:
        print("Dry run complete. No local app, database, credential, or build artifact was changed.")
    else:
        print("Local development uninstall complete. The next development run starts with a new workspace.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
