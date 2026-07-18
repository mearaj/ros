# ADR 0004: Rust owns local database-key storage and bootstrap

**Status:** Accepted  
**Date:** 16 July 2026

## Context

Every Community installation needs a unique local SQLCipher key. The key must
not enter Dart state, application preferences, logs, a plaintext sidecar file,
or a cloud-sync request. A copied encrypted database should not be readable by
another operating-system user simply because they have the file.

## Decision

The Rust storage layer owns the entire database-key lifecycle.

- It creates a 256-bit key from the operating-system CSPRNG and holds it in an
  opaque, zeroizing Rust type. The type is not cloneable, serializable, or
  exposed through Flutter/Rust bridge contracts.
- Desktop builds use the Rust `keyring` v1 adapter: Windows Credential Manager,
  macOS Keychain, and Linux Secret Service. The keyring entry uses fixed,
  non-restaurant-specific metadata.
- A per-database bootstrap lock serializes key-store and initial database work.
  A newly generated key is stored and read back before SQLCipher creates the
  database.
- If a database exists and its key is missing, malformed, or inaccessible, the
  application fails closed with a recovery-required state. If a key exists but
  its database is absent, it also fails closed. There is no file, environment,
  or Flutter secure-storage fallback.
- Flutter requests domain operations only. It never opens SQLite, selects a
  key, receives key bytes, or decides whether a key may be replaced.

Android and iOS are not provisioned through this adapter. Their native
secure-store adapters and signed-device verification are prerequisites for
mobile release support. This restriction is enforced in the Rust bootstrap
entry point, so those targets receive a storage-attention state before a key or
database can be created.

## Consequences

The desktop product gains a narrow, auditable trust boundary and avoids
accidental plaintext key persistence. It also means an unavailable or locked
OS store is an operational error that must be resolved safely, rather than
silently bypassed. OS secure storage is not a backup: Professional recovery and
cross-device transfer require a separately designed, owner-authorized recovery
envelope.

Automated tests use an in-memory fake key store, because headless CI cannot
prove a real desktop session's secure-store behavior. Signed Windows, macOS,
and Linux builds must run create, reopen, and missing-key recovery smoke tests
before public release.
