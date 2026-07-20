# Initial Threat Model

**Status:** Stage 1 baseline  
**Last reviewed:** 17 July 2026

## Assets to protect

- Completed invoices, payments, refunds, discounts, day-close records, stock
  movements, purchases, and expenses.
- Audit history and the identity/reason behind security-sensitive actions.
- Restaurant, customer, staff, and device information.
- Database encryption keys, cloud credentials, licenses, session tokens, and
  backup artifacts.
- Availability of local billing and kitchen workflows.

## Trust boundaries

    Cashier/KDS user -> Flutter UI -> generated Rust bridge -> Rust domain/storage
    Restaurant device -> authenticated sync API -> Gotigin cloud tenant data
    Owner browser -> authenticated owner API -> central reporting/management

The Flutter UI is not trusted to authorize a sensitive operation on its own.
The cloud is not trusted as a requirement for a completed local sale.

## Primary threats and baseline controls

| Threat | Baseline control | Status |
|---|---|---|
| Cashier silently changes or removes a payment | Append-only financial events, Rust-owned active-staff roles, required reasons, audit trail; `REPLACE` and direct update/delete are trigger-rejected | Local invoice/payment/outbox protections, owner PIN, session expiry, basic staff lifecycle, and role-boundary tests verified; Owner PIN recovery and portable kit are implemented; passkeys remain pending |
| Cash drawer discrepancy or altered day-close count | Immutable drawer opening/closure facts, one open drawer per branch, derived expected cash, retained variance, audit/outbox records | Local drawer transaction, owner Reports workflow, variance calculation, and owner/manager authorization are implemented; configurable multi-person approval remains pending |
| Customer contact data retained or reused after a privacy request | Encrypted customer identities, append-only profile revisions, explicit consent, owner/manager anonymization, and branch-scoped sale-time eligibility checks | Community customer enrollment, correction, anonymization, and sale attachment are implemented; cloud retention and legal-policy decisions remain founder-owned |
| Lost or copied restaurant device | SQLCipher local database, OS secure key storage, signed builds, device revocation for Professional | Desktop keystore path implemented; mobile bootstrap is code-blocked; signed-device smoke tests pending |
| Internet outage during service | Local transaction + audit/outbox before UI success; background retry only | Local transaction/audit/outbox foundation complete; cloud retry and acknowledgement pending |
| Kitchen continues preparing a cancelled or changed order | Owner/manager-only reasoned cancellation or reopen creates an immutable stop-work notice; normal ticket progression is blocked; Kitchen must acknowledge before it leaves the active queue | Local sent-order cancellation/reopen and acknowledgement workflow is implemented; it is not a substitute for operational kitchen training or cloud delivery proof |
| Cross-restaurant cloud data exposure | Organization/branch scoping, server-side authorization, PostgreSQL RLS defense in depth | Forced-RLS migration foundation is implemented; authenticated API and deployed integration proof remain pending |
| Modified local database file | SQLCipher HMAC integrity check, device-bound audit-chain and cloud anchoring in Professional | Local HMAC check complete; anchoring pending |
| Credential theft or brute force | Argon2id PIN verifier, five-failure/15-minute local throttle, 15-minute Rust-owned device sessions, secure key store; Owner recovery passphrase (24–64) with the same throttle | Local staff enrollment/rotation/revocation, PIN/session foundation, Owner PIN recovery, and portable restore verified in storage; passkeys and managed cloud recovery remain pending |
| Malicious or vulnerable dependency | Lockfiles, CI checks, SBOM, review, scanning, signed releases | Lockfiles, CI, and deterministic baseline dependency evidence implemented; artifact-specific SBOM, notices review, vulnerability disposition, and signed release pending |
| Unsafe migration or upgrade | Verified backup, transactional migration, source checksum recomputation before target-database access, object-contract verification, recovery test | Local source/manifest mismatch fails closed before migration reads or mutates the target database; recovery and release evidence remain required |
| Improper personal-data retention | Data minimization, encrypted optional customer profiles, append-only anonymization, and [documented data classification](data-classification.md) | Local classification/anonymization foundation is implemented; export and legal retention policy remain founder-owned |

## Security invariants

1. A finalized invoice, payment, refund, stock movement, purchase, or expense
   must never be silently deleted through a supported interface.
2. A sensitive correction records its actor, role, device, time, reason, and
   predecessor relationship.
3. Every completed local sale is a durable local transaction before cloud sync.
4. Database keys never appear in Dart state, source control, logs, or crash
   reports.
5. A failed cloud service or expired entitlement never destroys data or blocks
   the selected Community Safe Mode branch from local operation.
6. A release cannot claim security/compliance certification without the
   applicable independent review.
7. If an existing local database cannot be matched to its secure key, the app
   must fail closed and preserve the database for recovery; it must never create
   a replacement key or silently reset the restaurant.
8. A build without an approved native secure-store adapter must fail before key
   generation or database creation; it must not treat an unsupported platform
   as a setup-ready restaurant.
9. A kitchen-sent draft can settle only against its saved fulfillment and
   product/quantity snapshot. Cancelling or reopening it must create an
   immutable kitchen stop-work fact, not silently change or erase its ticket.
10. Kitchen receives only a cancellation stop-work signal, never the counter's
    free-text rationale. Its acknowledgement remains immutable and is required
    before the cancelled ticket leaves the active queue.
11. Before a local embedded migration reads or mutates its target database, its
    SQL source must match the reviewed SHA-256 manifest; a mismatch fails
    closed.
