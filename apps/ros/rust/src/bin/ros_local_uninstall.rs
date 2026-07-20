//! Development-only desktop companion for `scripts/uninstall-local-ros.sh`.
//!
//! The Bash launcher determines the platform's application-support path;
//! this binary deliberately uses the same Rust secure-store adapter as ROS to
//! clear every development restaurant profile database and credential together.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let mut support_directory = None;
    let mut confirmed = false;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--support-dir" => support_directory = arguments.next().map(PathBuf::from),
            "--confirm-local-data-loss" => confirmed = true,
            "--help" | "-h" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("Unknown argument: {argument}");
                print_usage();
                return ExitCode::from(2);
            }
        }
    }

    let Some(support_directory) = support_directory else {
        eprintln!("--support-dir is required.");
        print_usage();
        return ExitCode::from(2);
    };
    if !confirmed {
        eprintln!("--confirm-local-data-loss is required.");
        print_usage();
        return ExitCode::from(2);
    }

    match ros_storage::uninstall_development_local_install(&support_directory) {
        Ok(()) => {
            println!(
                "Development databases and secure-store credentials cleared under {}",
                support_directory.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Development uninstall did not complete: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "Usage: ros_local_uninstall --support-dir <path> --confirm-local-data-loss\n\
         Clears every ROS development restaurant-profile database and matching\n\
         secure-store credential under the support directory.\n\
         The ROS desktop app must be closed first (the shell script stops it)."
    );
}
