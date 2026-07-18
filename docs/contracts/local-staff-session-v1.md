# Local staff session contract v1

## Scope

This contract covers the Community Edition's local-device staff-security
boundary. Owner-managed enrollment, role reassignment, PIN rotation, and
revocation are included; PIN recovery, passkeys, and Professional multi-device
identity are not yet complete.

## Credential rules

- Community provisioning creates one immutable `Owner` staff account and no
  default credential.
- The first owner must choose a six-to-twelve-digit numeric PIN in the app.
- Rust validates the input and stores only an Argon2id v1.3 verifier using
  64 MiB memory, three passes, and one lane in production builds. PIN text,
  salts, and verifier values never cross a Dart model, audit payload, log, or
  outbox envelope.
- Each wrong PIN attempt is an immutable local fact. Five failed attempts for
  the selected staff record within a rolling 15 minutes block further attempts
  until the window passes.
- Only the active Owner session can enroll a Manager, Cashier, or Kitchen
  account, change a non-owner role, rotate a PIN, or revoke a non-owner
  account. Enrollment creates the immutable account, active-status fact, first
  credential, audit fact, and sync envelope in one transaction. A role change
  is a reasoned new role fact; the owner role is reserved. A PIN rotation is a
  new credential fact; revocation is a new status fact and immediately
  invalidates a session.

## Session rules

- A successful unlock appends an immutable `unlocked` session event, audited
  with the staff identity, role, device, and correlation id. Its expiry is 15
  minutes after the local database clock's event time.
- Locking appends an immutable `locked` event. The latest event for the device
  controls whether a session is active; stale or revoked identities are not
  accepted.
- Flutter may choose a displayable staff ID for the unlock form, but it never
  supplies a role, actor ID, session record, or authorization decision to any
  mutation. Rust resolves the active session for every bridge mutation.

## Authorization matrix

| Operation group | Owner | Manager | Cashier | Kitchen |
| --- | --- | --- | --- | --- |
| Menu/catalog, inventory, expenses, refunds, cash drawer, cancellations | Yes | Yes | No | No |
| Counter sale, held order, send to kitchen | Yes | Yes | Yes | No |
| Kitchen ticket progression | Yes | Yes | No | Yes |

## Append-only and migration rules

- Staff accounts, role events, credentials, status events, session events, and
  PIN attempts are `STRICT` encrypted SQLite tables with update/delete
  rejection triggers. The latest role event is the effective role for a later
  local session, while earlier role attribution remains retained.
- Migration `0016_local_staff_security.sql` maps the pre-existing local owner
  identity into the owner staff account without inventing a credential.
- Existing installations therefore stop at the owner-PIN setup screen after
  migration; a migration never creates a predictable PIN or silently operates
  under an unauthenticated role.
