# Local incident-response runbook

This runbook protects restaurant data first. It applies to the implemented
local Community workflow and records the additional evidence Professional
operations must collect once cloud sync is deployed. It is not a substitute
for a founder-approved support, retention, or notification policy.

## First response rules

1. Do not delete the database, its sidecar files, a backup, or the matching
   operating-system secure-store entry.
2. Do not ask a restaurant to send a database key, Owner PIN, credential
   verifier, raw database, audit payload, or customer-contact data through
   chat or email.
3. Record the application version, build mode, platform version, approximate
   local time, a screenshot of the safe user-facing status, and the actions
   immediately before the incident. Prefer an Owner-exported redacted
   diagnostics pack (see
   [local-diagnostics-v1.md](../contracts/local-diagnostics-v1.md)) over
   screenshots that may show customer or payment data. Redact customer and
   payment data from any remaining evidence.
4. If the counter can still open its encrypted local workspace, finish only
   normal local service. Do not retry a mutation repeatedly when the screen
   says it needs attention.
5. Escalate suspected tampering, lost devices, credential compromise, or
   missing keys to an accountable Gotigin security owner before attempting
   recovery.

## Local storage or key-store attention state

Symptoms include **Local storage needs attention**, failure to unlock, or an
existing database that cannot be opened.

1. Keep the app closed after recording the safe status.
2. Confirm the user is in the expected desktop operating-system account and,
   on Linux, an unlocked graphical Secret Service session.
3. Confirm available disk space and ordinary application-support directory
   access. Do not alter the encrypted file or key-store entry.
4. Restart only after the secure store is available. If it opens, run the
   owner integrity view before continuing with management work.
5. If the key remains unavailable, preserve both the database and its sidecar
   files for the approved recovery process. Creating a replacement key would
   make the existing encrypted data unrecoverable and is prohibited.

## Failed migration or application upgrade

1. Preserve the pre-upgrade installation and any verified backup unchanged.
2. Record the exact application/build version and the visible safe error.
3. Do not manually change SQLite `user_version`, migration checksums, schema,
   or triggers. The application verifies reviewed source checksums before it
   accesses a target database and must fail closed on a mismatch.
4. Retry only with the same reviewed artifact after confirming power, disk,
   and secure-store availability. A different build must first be reviewed
   against the recorded migration history.
5. Escalate with redacted version/status evidence and a copy of the verified
   backup checksum; never send keys or raw customer data.

## Failed or missing backup

1. Continue normal local service only if the live encrypted workspace opens
   normally; a backup failure must not cause a destructive reset.
2. Verify the chosen destination is writable and does not already contain a
   same-name snapshot. The current backup command refuses overwrite rather
   than replacing a prior recovery point.
3. Create a new verified backup to a controlled destination and record its
   SHA-256 result. Do not use a copied live WAL/database file as a backup.
4. A clean-install or cross-device restore remains blocked until the
   founder-approved recovery-envelope policy exists. Do not improvise by
   exporting the SQLCipher key.

## Suspected local tampering or inconsistent records

1. Stop management changes and preserve the device in its current state.
2. Use the owner integrity workflow if the application can open safely. It
   checks encrypted database pages, schema contract, foreign keys, and local
   audit chains before showing reports.
3. Treat a failed integrity/audit result as a security incident, not a normal
   data-editing task. Do not use a SQLite browser or manual SQL repair.
4. Record redacted error/status evidence, device custody, and the last known
   good verified-backup checksum. Escalate to the security owner.

## Lost device or suspected credential compromise

1. Remove the device from service and record its asset identity, last known
   user, and last successful local activity without copying restricted data.
2. Change affected Owner/staff PINs from a known-good device after the
   founder-approved identity proof procedure. Do not create a default or
   support bypass PIN.
3. For deployed Professional, revoke the device/session server-side and keep
   its prior local data preserved for evidence. Current local Community has no
   cloud revocation channel; physical custody and OS-account controls are the
   immediate containment boundary.
4. Follow the founder-approved notification and recovery policy before any
   portable restore or replacement-device provisioning.

## Professional sync outage or backlog

The current repository provides a validated sync-envelope foundation, not a
deployed Professional sync service. When deployment exists, its operator must
run a separately approved cloud runbook. Until then, the local rule is simple:
completed local invoices remain durable and queued facts are never manually
deleted or marked acknowledged.

When a deployed service reports a backlog, record the branch/device identity,
queue age/count, redacted status code, and the last durable acknowledgement.
Do not replay, reorder, or edit local audit envelopes by hand. The server must
validate the authenticated tenant/device, actor-bound hash chain, and
idempotency key in one transaction before returning an acknowledgement.

## Post-incident record

Document the incident timeline, impact, evidence locations, recovery action,
and owner of follow-up work. Add a regression test or release-gate update for
every software or runbook defect found. Credentials, database keys, raw audit
payloads, and unredacted customer data must never enter the incident record.
