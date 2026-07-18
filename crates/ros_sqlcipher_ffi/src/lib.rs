//! The deliberately small unsafe boundary used to give SQLCipher raw key bytes.
//!
//! SQLCipher's `sqlite3_key` API accepts a byte buffer. Passing the database key
//! this way avoids materialising it as a hexadecimal SQL string in Rust memory.
//! No other crate may call SQLite's raw handle APIs.

#![deny(unsafe_code)]

#[cfg(all(
    feature = "development-bundled-sqlcipher",
    feature = "production-sqlcipher"
))]
compile_error!(
    "Choose exactly one SQLCipher linkage path: development-bundled-sqlcipher or production-sqlcipher."
);

#[cfg(not(any(
    feature = "development-bundled-sqlcipher",
    feature = "production-sqlcipher"
)))]
compile_error!("A SQLCipher linkage path is required for ros_sqlcipher_ffi.");

use std::convert::TryFrom;
use std::sync::{Mutex, OnceLock};

use rusqlite::{Connection, Error, Result, ffi};

// SQLCipher's per-connection keying is inexpensive and happens only while a
// database is opened. Serialising it also protects the bundled development
// build against native cipher initialisation races when multiple app isolates
// start at once.
static KEYING_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Configures a newly opened SQLCipher connection with raw key material.
///
/// Call this exactly once, immediately after opening a connection and before
/// executing any SQL. SQLCipher consumes the supplied bytes during the call;
/// the caller retains ownership and can zeroise its key when no longer needed.
pub fn apply_database_key(connection: &Connection, key: &[u8]) -> Result<()> {
    let key_length = i32::try_from(key.len()).map_err(|_| {
        Error::InvalidParameterName(
            "SQLCipher key length exceeds the SQLite C API limit".to_owned(),
        )
    })?;

    if key.is_empty() {
        return Err(Error::InvalidParameterName(
            "SQLCipher database key must not be empty".to_owned(),
        ));
    }

    let keying_lock = KEYING_LOCK.get_or_init(|| Mutex::new(()));
    let _keying_guard = keying_lock
        .lock()
        .unwrap_or_else(|poisoned_lock| poisoned_lock.into_inner());

    // SAFETY: `connection.handle()` is used only for this synchronous SQLCipher
    // keying call while `connection` remains borrowed. `key` is non-empty and
    // valid for `key_length` bytes for the full call. The SQLCipher API copies
    // the supplied key material; it does not retain this pointer.
    #[allow(unsafe_code)]
    let result_code =
        unsafe { ffi::sqlite3_key(connection.handle(), key.as_ptr().cast(), key_length) };

    if result_code == ffi::SQLITE_OK {
        Ok(())
    } else {
        Err(Error::SqliteFailure(ffi::Error::new(result_code), None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_keys_are_rejected_before_reaching_sqlcipher() {
        let connection = Connection::open_in_memory().expect("SQLite connection");

        assert!(apply_database_key(&connection, &[]).is_err());
    }
}
