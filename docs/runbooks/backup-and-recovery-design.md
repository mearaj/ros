# Backup and recovery design

## Current release position

Local data is encrypted with SQLCipher, migrated transactionally, and checked
with SQLCipher page-integrity verification on open. The application now creates
a same-installation verified snapshot through SQLite's online backup API. It
never presents a raw live database-file copy as a safe backup while WAL is
active.

The snapshot is encrypted with the installation's existing OS-keystore-held
SQLCipher key, re-opened for integrity and schema-contract verification before
success is reported, and accompanied by a SHA-256 checksum result. It does not
overwrite an existing backup destination.

Only an active local Owner session can create this database copy. That rule is
enforced in Rust storage as well as in the Flutter presentation, so a cashier
or manager cannot bypass the UI to create a full encrypted restaurant snapshot.

Same-installation restore is available: the owner selects a verified backup
file name under `verified-backups`, Rust re-verifies integrity with the current
keystore key, and writes `restaurant-os.restored.db` beside the live database
without overwriting it. Reports also expose a dedicated **Verify local backup**
action that re-checks a named snapshot without writing restore output.

Portable clean-install restore is implemented per ADR 0005: an Owner creates a
verified backup plus a `ros.recovery.v1` envelope wrapping the SQLCipher key
with a recovery passphrase. `restore_portable_backup_to_clean_path` unwraps the
key, verifies integrity/schema, and writes a new destination without overwriting
an existing live database. Owner PIN recovery (ADR 0007) uses the same
passphrase verifier.

## Required implementation contract

1. Quiesce or use SQLite's online backup API from the Rust storage owner.
2. Verify the copied database with SQLCipher using the restored key material.
3. Record an encrypted manifest containing schema version, migration checksums,
   backup timestamp, source installation identity, and a SHA-256 checksum.
4. Restore only into a clean, explicitly chosen destination after verifying
   the manifest, database integrity, and migration compatibility.
5. Never overwrite an existing local restaurant database during restore.
6. Run a clean-machine restore and report-total comparison before release.

## Recovery rule

If the database and its OS-keystore key cannot be proven to belong together,
the app remains fail-closed and directs the operator to verified recovery.
Neither Flutter nor support tooling should request or log the SQLCipher key.
