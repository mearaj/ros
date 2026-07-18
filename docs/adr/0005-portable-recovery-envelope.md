# ADR 0005: Owner-authorized portable recovery envelope

**Status:** Accepted  
**Date:** 18 July 2026  
**Approver:** Gotigin engineering (founder-accountable product default)

## Context

Same-installation backup uses the OS keystore SQLCipher key and therefore cannot
decrypt on a clean machine. Stage 3 acceptance requires verified restore into a
clean installation.

## Decision

Ship an **owner-authorized recovery envelope** alongside each portable backup:

1. Envelope format version `ros.recovery.v1`.
2. Contents (authenticated encryption): wrapped database key material, source
   installation identity, schema version, migration manifest checksums, backup
   SHA-256, created-at UTC, and creating actor id.
3. Wrapping secret: a 24–64 character Owner recovery passphrase, stretched with
   Argon2id (same parameters family as staff PIN hashing). The passphrase is
   never stored on disk; only a verifier salt/params travel with the envelope.
4. File layout: `*.rosbackup` (encrypted DB snapshot) + `*.rosrecovery` (envelope
   JSON with ciphertext fields). Both must be present for clean-install restore.
5. Clean-install restore: verify envelope → unwrap key → open snapshot → verify
   SQLCipher + schema contract → write a new installation database and store the
   unwrapped key in the new OS keystore. Never overwrite an existing live DB.
6. Offline: envelope creation and restore require an active Owner session on the
   source or target device respectively; no Gotigin Cloud dependency.

## Consequences

- Portable restore can be implemented and tested without cloud.
- Loss of both the OS keystore key and the Owner recovery passphrase is
  unrecoverable by design; support must not request key material.
- Implementation lives in `ros_storage` and is covered by automated tests.
