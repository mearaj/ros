//! Development-only desktop companion for `scripts/uninstall-local-ros.py`.
//!
//! The Python launcher determines the platform's application-support path;
//! this binary deliberately uses the same Rust secure-store adapter as ROS to
//! clear the matching development credential and encrypted database together.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const DEVELOPMENT_DATABASE_FILE_NAME: &str = "restaurant-os.development.db";

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

    let database_path = support_directory.join(DEVELOPMENT_DATABASE_FILE_NAME);
    match ros_storage::uninstall_development_platform_database(&database_path) {
        Ok(()) => {
            println!("Development database and secure-store credential cleared.");
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
         Clears only the ROS development database and its matching secure-store credential.\n\
         The ROS desktop app must be closed first."
    );
}
