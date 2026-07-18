# ADR 0002: SQLCipher-backed SQLite is the local operational database

**Status:** Accepted  
**Date:** 16 July 2026

## Context

Community Edition must operate entirely offline while preserving invoices,
payments, inventory, expenses, and audit history. Full database encryption is
required in addition to device-level encryption.

## Decision

Use SQLite through SQLCipher Community Edition 4.17.0 from the first product
release build. Rust is the sole database owner. Flutter/Dart receives typed
domain results through the generated Rust bridge and never receives a database
key.

Each installation creates a random 256-bit database key. The key is held only
in the operating system secure store: Windows Credential Manager on Windows,
Keychain on macOS, and Secret Service on supported Linux desktop sessions.
Device encryption such as BitLocker or FileVault remains recommended defense in
depth, not the only database protection. Android and iOS require dedicated
native secure-store adapters before database provisioning is enabled there.

The release build pins and links the SQLCipher library for each platform rather
than relying on a restaurant device's system SQLite. Windows x64 encrypted
database builds are a release gate.

## Required configuration and verification

- SQLCipher is keyed before any SQL statement.
- The connection verifies SQLCipher's post-key encryption status after opening.
- WAL, foreign keys, full synchronous durability for finalized financial
  writes, trusted-schema protection, defensive mode, and prepared statements
  are mandatory.
- Temporary storage is configured so plaintext temporary files do not leak to
  disk.
- Backups are encrypted and restored through verified application workflows.
- The application includes SQLCipher Community Edition attribution in Open
  Source Notices.

## Implementation status

The workspace currently validates the storage contract on Linux using the
bundled SQLCipher development source supplied by the Rust SQLite binding. It
proves encrypted headers, HMAC integrity checking, wrong-key failure,
transactional migrations, and audit constraints.

The database key is supplied as raw bytes through a deliberately tiny,
reviewed Rust FFI boundary to SQLCipher's `sqlite3_key` API. It is not formatted
into a SQL statement or passed to Flutter/Dart.

That development bundle is not the production artifact: it currently contains
SQLCipher 4.14.0 and is only a repeatable test path. The production Windows
pipeline must link the reviewed, pinned SQLCipher 4.17.0 library, validate
PRAGMA cipher_status, and exercise the same test suite.

The workspace rejects both simultaneous development/production linkage and any
release/profile build that retains the development bundle. Cargokit selects the
explicit production feature graph for profile/release builds; Debug selects the
isolated development graph. The production graph is currently build-blocked
until a controlled artifact manifest, checksum/provenance verifier, and
linker configuration are implemented, so it cannot select an arbitrary system
SQLCipher library. Signed-artifact smoke tests are also required before public
Release packaging exists. In production mode the runtime will reject a
SQLCipher version other than the approved 4.17.x line.

The desktop secure-store adapter is implemented in Rust. It generates key bytes
with the operating system CSPRNG, stores them through the native keyring API,
verifies the stored value before use, zeroizes the in-memory key wrapper, and
holds a bootstrap file lock while it creates or opens the database. It fails
closed when an existing database has no retrievable key, or a key exists without
its database; it has no plaintext-file, environment-variable, or Flutter-side
fallback. Headless CI uses only a fake store. Actual signed Windows, macOS, and
Linux desktop sessions must pass key-store write/read/reopen smoke tests before
public release; Android and iOS are explicitly blocked until their adapters
exist.

## Why not plain SQLite or SQLite SEE

Plain SQLite plus disk encryption alone does not protect exported or copied
database files once a volume is mounted. SQLite SEE is technically credible but
adds a proprietary license and a more costly custom build path without a clear
release advantage over SQLCipher for the first production release.

## References

- SQLCipher design: https://www.zetetic.net/sqlcipher/design/
- SQLCipher Community: https://www.zetetic.net/sqlcipher/community/
- SQLCipher 4.17.0 release: https://www.zetetic.net/blog/2026/07/08/sqlcipher-4-17-0-release/
- SQLCipher `cipher_status`: https://www.zetetic.net/sqlcipher/sqlcipher-api/
- SQLite WAL: https://sqlite.org/wal.html
