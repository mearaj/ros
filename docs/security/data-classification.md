# Community data classification baseline

**Status:** implementation-aligned baseline, 17 July 2026

This document classifies the local Community Edition data that the current
product stores. It is not a legal retention schedule; that requires the
founder’s jurisdictional and commercial decision before cloud retention,
marketing, analytics, customer-data export, or a statutory export feature is
enabled.

| Class | Examples | Current control | Retention boundary |
| --- | --- | --- | --- |
| Restricted credentials | Owner/staff PIN input, Argon2id verifier, local database key | PIN text never persists; verifiers and database keys never cross Flutter; keys remain in OS secure storage | No supported export or recovery of key/PIN material |
| Restricted financial integrity data | Orders, invoices, payment allocations, refunds, expenses, cash-drawer events | SQLCipher storage, append-only tables, role checks, audit and sync envelopes | Retained locally; no supported destructive UI |
| Owner-selected financial aggregate export | UTC-day/tender payment, refund, and expense aggregates plus summary | Active Owner is enforced in Rust; integrity is verified; output is bounded; native save dialog selects the destination | The chosen external file is outside SQLCipher and follows the Owner's destination policy |
| Confidential personal data | Optional customer name, phone, email, marketing consent | SQLCipher storage; only current active profile reaches the counter; audit payloads omit contact values | Owner/manager can append an anonymized profile; formal legal retention/export remains undecided |
| Confidential operational data | Staff display names/roles, product catalogue, stock movements, kitchen tickets | SQLCipher storage and least-privilege local sessions | Retained locally; staff and catalogue changes preserve history |
| Internal integrity metadata | Audit hashes, migration checksums, outbox IDs, device sequence | Rust storage only; audit timeline excludes IDs, hashes, and payloads | Retained for verification and eventual Professional sync |
| Public product content | Bundled menu-image asset keys and their licence/provenance record | Packaged assets; provenance documented separately | May be redistributed only within the recorded asset licence terms |

## Handling rules

1. Flutter receives only the minimum projection required for its screen; it
   does not receive database keys, PIN verifiers, or raw audit payloads.
2. New logs, telemetry, cloud endpoints, exports, and support tooling must not
   include Restricted data and must minimize Confidential data. Local
   diagnostics follow
   [local-diagnostics-v1.md](../contracts/local-diagnostics-v1.md): allow-listed
   event codes only, and any cloud share requires explicit Owner consent.
3. A feature that introduces customer export, cloud retention, marketing, or
   analytics must be reviewed against an approved retention/legal policy
   before release.
4. Anonymization is an append-only operational control. It does not make an
   unsupported claim about paper records, copied screenshots, or separately
   retained backups.
5. The narrow financial aggregate export is documented in
   [financial-export-v1.md](../contracts/financial-export-v1.md). It does not
   authorize a customer, credential, audit-payload, or statutory-data export.
