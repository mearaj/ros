# Restaurant Operating System

## Six Major Stages Production Release Plan

**Owner:** GOTIGIN SOFTWARE & HARDWARE PRIVATE LIMITED  
**Planning date:** 16 July 2026  
**Release objective:** Ship a dependable, local-first Restaurant Operating System with a complete Community Edition and a secure, working Professional foundation.  
**Product position:** A Gotigin company product; the hackathon is a delivery milestone, never the product’s identity or quality bar.  
**Status:** Implementation underway; the Stage 1 trusted local-data vertical
slice is complete, with product workflows continuing.

---

## 1. The promise we are making

Restaurant Operating System is a flagship product, not a disposable hackathon demo. The experience must feel fast, calm, elegant, and trustworthy at the busiest moment of restaurant service.

The product promise is:

> A restaurant must always be able to take an order, send it to the kitchen, create a bill, retrieve its records, and recover its data without depending on Gotigin Cloud.

The cloud makes a growing business easier to govern; it must never become a single point of failure for billing.

Six focused major stages can produce a production-ready **defined release**, but they cannot honestly make every imaginable restaurant workflow, hardware model, regulatory regime, or Enterprise integration complete. This plan therefore locks a high-quality vertical slice, applies production gates to it, and explicitly labels future work instead of pretending it is shipped.

At the planning baseline the workspace was greenfield. Stage 1 therefore
establishes the engineering foundation as well as the first product features.

---

## Delivery log

### 16 July 2026 — Stage 1 trusted local-data vertical slice

- Initialized the Git repository, Flutter/Dart client, Rust workspace, generated
  Flutter/Rust boundary, and Linux desktop build.
- Accepted Flutter/Dart for the adaptive client and Rust for trusted
  operational functionality; see ADR 0001.
- Added the first adaptive, local-first command-center interface. Its visible
  operational status is supplied by real Rust code through generated bindings.
- Established integer-money and append-only financial-mutation invariants with
  Rust tests.
- Added encrypted local SQLCipher storage, migration checksum recording,
  audit-event constraints, encrypted-file verification, HMAC integrity checks,
  and wrong-key failure tests.
- Added a verified local migration manifest: organizational and
  branch roots, archive-only categories/products, immutable local installation
  identity, and checksum validation on every database open. Before local
  migration code inspects or mutates a target database, it recomputes the
  SHA-256 checksum of every embedded migration source and fails closed if any
  source differs from its reviewed manifest value.
- Extended the verified manifest to local schema v4: branch-scoped invoice
  sequencing, orders and immutable order-line snapshots, finalized invoices,
  recorded payments, and future-sync outbox/acknowledgement records.
- Added schema v5 financial-integrity hardening: `REPLACE` cannot bypass
  immutable triggers, allocator state is bound to invoice progression,
  order/invoice/payment/outbox branch and currency relationships are checked,
  and opening a pre-existing partial table fails schema-contract verification.
- Added atomic, device-sequenced hash-chained audit events for catalog changes.
  Category and product records cannot be hard-deleted through the database;
  sensitive archival requires an actor, reason, and optimistic revision.
- Implemented Rust-owned desktop secure key storage and fail-closed database
  bootstrap. A missing key never causes an existing restaurant database to be
  replaced. Flutter never receives database-key material.
- Connected Community restaurant setup and persisted category creation to the
  Flutter command center through typed generated Rust APIs, then extended the
  same workflow to exact-price menu-item creation and a persisted sale-menu
  list. This is real local state, not seeded demo data.
- Completed the first genuine offline counter-sale vertical slice. The
  Flutter cart submits only product IDs, quantities, fulfillment, and payment
  method; Rust reloads trusted catalog prices and atomically records the
  finalized order, invoice, payment, device-hash-chained audit events, and
  future-sync outbox events before the UI shows an invoice receipt. Database
  triggers reject update/delete attempts for finalized orders, order lines,
  invoices, payments, and outbox records.
- At the Stage 1 milestone, the counter deliberately remained an immediate-sale
  workflow. Persistent drafts/tables, refunds, and KDS were delivered in the
  subsequent operating slices below, and product-bound optional modifiers were
  delivered in the later catalogue/POS slice. Taxes/discounts, hardware
  printing, and deployed cloud synchronization remain outside the current
  shipped local scope.
- Hardened connection setup: the app now requires SQLCipher's post-key
  encryption status, sets and reads back database-level foreign-key,
  defensive, and trusted-schema controls, and verifies WAL/full-synchronous
  durability, busy timeout, in-memory temporary storage, and secure deletion.
- Enforced the current platform scope in code. Android and iOS builds now stop
  before creating a local database until their native secure-store adapter is
  reviewed; the UI presents a safe storage-attention state instead of setup.
- Added the initial threat model, architecture decisions, root documentation,
  and CI verification workflow.
- Verified Rust tests/lints, Flutter analysis/tests, and a native Linux desktop
  build locally.

The development SQLCipher bundle is a reproducible Linux test foundation.
Production Windows artifacts must link and verify the reviewed SQLCipher 4.17.0
build before release; this remains a non-negotiable release gate.

The development SQLCipher bundle is now prohibited for release/profile Rust
builds and the feature graph rejects accidental development-plus-production
linkage. Cargokit now selects the production feature graph for profile/release
builds, and that graph is build-blocked until a controlled artifact manifest,
checksum/provenance verification, and controlled linker configuration exist.
It cannot silently fall back to a system SQLCipher library; therefore no
release artifact may be published yet.

The desktop secure-store adapter has automated contract coverage with a fake
store. Signed Windows, macOS, and Linux desktop sessions still need live
key-store write/read recovery smoke tests before release. Android and iOS
provisioning are not yet accepted release targets.

The Stage 1 local developer handoff is documented in
[docs/runbooks/local-development.md](docs/runbooks/local-development.md). It
defines separate Development and Release data/key-store namespaces, the exact
local verification sequence, and why Release packaging remains fail-closed
until its controlled native SQLCipher artifact and release evidence exist.

### Ongoing implementation — Stages 2–4 vertical slices

- Added durable held orders and restaurant tables. Saved revisions are
  append-only, settlement is exactly-once, and a reopened order retains its
  chosen table rather than using a hard-coded placeholder.
- Added an audited cancellation command for unchanged, unsent held orders.
  Cancellation requires a reason and preserves all prior revisions; it does
  not silently delete counter history.
- Added the separate sent-order cancellation/revision workflow. A kitchen-sent
  draft remains settlement-bound to its saved fulfillment and product/quantity
  snapshot, so it cannot be edited or re-sent as a different order. An
  Owner/Manager cancellation or reopen creates an immutable stop-work notice
  for the original kitchen ticket; reopening creates a new editable draft
  revision rather than rewriting the original. Kitchen sees the stop-work
  signal but never the counter's free-text reason, and its immutable
  acknowledgement removes the cancelled ticket from the active queue while
  retaining all history. See
  [docs/contracts/kitchen-cancellation-v1.md](docs/contracts/kitchen-cancellation-v1.md).
- Added a privacy-safe Kitchen Display flow. Kitchen tickets are snapshot
  based, expose no prices or payment data, move only through
  `new → preparing → ready → completed`, and cannot be deleted.
- Added bounded kitchen instructions as immutable draft/ticket snapshots.
  Counter can amend them only while an order is open; Kitchen receives the
  instruction but never cancellation rationales, prices, or payment data, and
  audit/sync metadata records only whether an instruction exists. See
  [docs/contracts/kitchen-instructions-v1.md](docs/contracts/kitchen-instructions-v1.md).
- Added owner-facing all-time local sales reporting from finalized invoices and
  payments, with cash/card/UPI subtotals and a gross top-items projection from
  immutable order-line snapshots. It intentionally excludes carts, drafts,
  and kitchen state; invoice-level refunds are not invented as item-level
  allocations. The same verified report now exposes the total of immutable
  operating-expense records without applying a tax or accounting policy.
- Added an append-only cash-drawer foundation. Owner/manager sessions can open
  one drawer per branch and close it with a counted amount; expected cash is
  derived from post-open cash sales, refunds, and cash expenses, while the
  resulting variance, audit event, and sync envelope are immutable. The
  owner-facing Reports workflow is implemented and reads open-drawer state
  from encrypted storage after restart.
- Added the first local staff-security boundary. Community now requires an
  explicit six-to-twelve-digit owner PIN (no default), stores only an Argon2id
  verifier, commits failed attempts for a five-failure/15-minute throttle, and
  derives a 15-minute active staff session inside Rust. The session, lock, and
  unlock facts are audit chained; catalog/financial controls require
  owner/manager, counter actions permit cashier, and KDS progression permits
  kitchen. Owner-managed staff enrollment, reasoned role reassignment, PIN
  rotation, and revocation are implemented; PIN recovery and manager approval
  workflows remain separate unfinished work.
- Added a privacy-safe Community customer slice. Customer contact data and
  consent are optional, profile corrections append a reasoned new revision,
  and owner/manager anonymization redacts the active contact profile without
  deleting immutable sale or audit history. A customer can be optionally
  attached to a sale only after Rust revalidates that the customer is active
  in the current branch. See `docs/contracts/customer-privacy-v1.md`.
- Added the storage-level correction rule for full or partial refunds: a
  refund is append-only, cannot exceed the original invoice, retains the
  original payment-method snapshot, reduces net reporting, emits audit/outbox
  facts, and never edits or deletes a recorded invoice/payment. The cashier
  owner full-refund interface is implemented; local staff role approval flows
  remain unfinished.
- Added a local integrity verification command that checks SQLCipher pages,
  schema contract, foreign keys, and every local device audit chain before a
  report is shown.
- Added a same-installation verified local backup action. It uses SQLite's
  online backup API rather than copying live WAL files, verifies SQLCipher and
  schema integrity on the result, reports a SHA-256 checksum, and refuses to
  overwrite an existing snapshot. Portable clean-install restore remains gated
  on the owner-authorized recovery-envelope decision.
- Added a narrow Owner-only financial CSV export. Rust verifies the active
  Owner session and local integrity inside one SQLite transaction, generates
  only bounded branch-level UTC/tender aggregates without customer or audit
  payload data, and leaves file choice to an explicit native save dialog. It
  is not a tax, statutory, PDF, printer, or portable-recovery feature; see
  [docs/contracts/financial-export-v1.md](docs/contracts/financial-export-v1.md).
- Defined and tested the local Professional sync envelope/acknowledgement
  contract. Pending operations remain immutable; identical acknowledgements
  are idempotent and conflicting acknowledgements are rejected. See
  [docs/runbooks/professional-sync-contract.md](docs/runbooks/professional-sync-contract.md).
- Added a feature-gated local Professional sync API path: bounded Ed25519
  device-token verification, claim/body scope binding, a tenant-context
  PostgreSQL transaction adapter, durable device-anchor serialization,
  branch-scoped device/actor authorization, immutable acceptance, and stable
  replay acknowledgements. Cloud migration 0004 supplies the acknowledgement
  and append-only grant/revocation facts. Sync remains disabled by default;
  production issuer/provisioning, final database roles, PostgreSQL integration
  proof, activation, reporting projections, and cloud deployment are not
  represented as complete.
- Added temporary menu availability controls. Owner/manager sold-out and
  resume commands are revisioned, reasoned, audit-chained, and syncable;
  sold-out items remain visible to managers but are excluded and rejected by
  the POS. Price corrections remain possible while sold out without changing
  that state. See `docs/contracts/catalog-availability-v1.md`.
- Added product-bound optional modifier choices. Owner/manager sessions create
  immutable names with non-negative price additions or reasonedly archive an
  option; update/delete triggers preserve their history. POS submits only a
  bounded set of option identities, Rust reloads the active catalogue and
  recalculates the trusted line price, and drafts, KDS tickets, invoices,
  receipts, audits, and sync envelopes retain their snapshots.
- Added receipt reprint from immutable local financial snapshots. Authorized
  counter staff can view one branch-scoped invoice, its historical line prices,
  split payment allocations, and retained refund total, then copy a local text
  receipt. This is deliberately not printer/PDF integration; see
  `docs/contracts/immutable-receipt-reprint-v1.md`.
- Added an owner-only verified audit-timeline view. It verifies local storage
  before loading and exposes only a bounded branch-scoped sequence, action,
  and timestamp—not audit payloads, hashes, device IDs, staff IDs, or
  credential data. See `docs/contracts/local-audit-timeline-v1.md`.
- Extended the Reports refund action to support controlled partial refunds.
  Owner/manager sessions load the remaining refundable balance from the
  immutable invoice detail, enter any positive amount up to that remainder
  with a required reason, and Rust continues to allocate across original
  tender methods without editing the invoice.
- Added an owner-only local Professional sync-queue view. It verifies
  integrity, reads the immutable outbox, and shows only sequence, event type,
  entity type, and created time—never payloads, hashes, or device/actor
  identifiers. Cloud upload remains a separate activation/client slice.
- Wired provider-neutral taxes and order discounts through local schema v28,
  `ros_core::pricing`, sale finalization, receipts, and manager/owner POS
  discount entry. Branch tax rates are archive-only named components; products
  choose `no_tax` / `exclusive` / `inclusive`. This is not a GST or e-invoice
  compliance claim. See `docs/contracts/pricing-adjustments-v1.md`.
- Community POS now previews the trusted payable total before split tender,
  accepts fixed or percentage manager/owner discounts, and Menu exposes branch
  tax-rate entry plus per-product tax treatment.
- Receipt reprint can save immutable receipt text to an explicit `.txt`
  destination; Reports surfaces retained refund totals beside net tender
  amounts; Menu can archive branch tax rates with a required reason.
- Reports are day-scoped to a UTC accounting day (default today) with a date
  picker; same-install backup restore writes `restaurant-os.restored.db`
  beside live data; owner/manager single-actor invoice voids mirror refunds
  (schema v29); Menu can archive empty categories and replace product images.
- Reports also surface day-scoped discount and tax totals, owner backup
  verification, branch-local timestamp display, and append-only UTC day close
  without reopen (schema v30).
- Local privacy-allowlisted diagnostics record technical event codes on device;
  Owners may export a redacted pack or voluntarily share it with Gotigin after
  explicit consent. Continuous telemetry and diagnostics intake hosting remain
  founder-gated ([local-diagnostics-v1.md](docs/contracts/local-diagnostics-v1.md)).

These are implemented slices, not a claim that the entire Stage 2–4 acceptance
scope (printer/PDF, corrections, advanced staff controls, portable
backup/restore, cloud authentication, tenancy, and deployed sync) is complete.

Inventory now has a committed ledger contract in
[docs/contracts/inventory-ledger-v1.md](docs/contracts/inventory-ledger-v1.md).
The encrypted schema, immutable opening/purchase/waste/adjustment commands,
owner/manager stock-control boundary, and atomic tracked-product sale deduction
are implemented and storage-tested. Community now includes a stock-ledger view
and reasoned movement entry, plus optional append-only low-stock threshold and
clear-policy events for tracked products. Suppliers, recipes, and forecasting
remain unfinished.

The release evidence required before publication is maintained in
[docs/runbooks/release-verification.md](docs/runbooks/release-verification.md).
A deterministic locked-dependency inventory generator is available as baseline
evidence, with its deliberately separate artifact-SBOM and open-source-notice
publication gates documented in
[docs/runbooks/dependency-evidence-and-sbom.md](docs/runbooks/dependency-evidence-and-sbom.md).

---

## 2. Release definition: what “production ready” means

The release is production ready only when all of the following are true for the included scope:

- A restaurant can complete normal service offline with durable local data.
- A restart, power interruption simulation, failed migration, and failed sync do not lose a completed order or invoice.
- Every security-sensitive monetary or operational mutation is traceable and cannot be silently deleted.
- A verified backup can be restored into a clean installation.
- Community data can be upgraded to Professional cloud sync without reinstalling or losing local operation.
- Professional cloud failure does not stop Community or Professional local POS workflows.
- Critical business flows have automated tests and a documented manual acceptance run.
- The release passes the security, privacy, operational, and UX gates in Section 15.
- The application is packaged, signed where platform credentials are available, observable, documented, and supportable.

Production ready does **not** mean “mathematically unbreakable,” “certified compliant in every jurisdiction,” or “all future Enterprise capabilities already built.” We will not make those claims without the required audit, legal review, hardware certification, and operational history.

---

## 3. Scope lock and edition behavior

### 3.1 Edition matrix for this release

| Capability | Community — Free Forever | Professional Evaluation | Professional Paid | Enterprise Paid |
|---|---|---|---|---|
| Subscription term | No expiry | 14 days | 1 year | 1 year |
| Single branch | Yes | Yes | Yes | Yes |
| Branch limit | 1 | Up to 5 | Up to 5 | Configured in entitlement |
| Local/offline POS | Yes | Yes | Yes | Yes |
| Menu, customers, tables, orders, invoices | Unlimited | Unlimited | Unlimited | Unlimited |
| Employees and local roles | Unlimited local staff | Yes | Yes | Yes |
| Inventory, purchases, expenses, reports, KDS | Yes | Yes | Yes | Yes |
| Cloud synchronization | No | Yes | Yes | Yes |
| Owner dashboard | No | Yes | Yes | Yes |
| Central/cross-branch reporting | No | Yes | Yes | Yes |
| Automatic cloud backups | No | Yes | Yes | Yes |
| Remote device/sync monitoring | No | Yes | Yes | Yes |
| API access | No | Yes | Yes | Contract/configuration dependent |
| Advanced roles | Local standard roles | Yes | Yes | Yes |
| SSO, on-premise, custom integrations | Not included | Not included | Not included | Contracted roadmap; not represented as shipped |

Professional Evaluation and Professional Paid use the same product capabilities. The difference is the entitlement: 14-day evaluation versus a one-year paid entitlement.

Enterprise v1 uses an annual, signed entitlement with a branch limit chosen during pricing. It must not falsely advertise unimplemented SSO, on-premise deployment, or custom integrations as available.

### 3.2 Entitlement safety contract

1. Community never expires and has no artificial limits on normal single-branch use.
2. A Professional Evaluation begins only after explicit activation and expires after 14 calendar days based on signed server time when available.
3. Professional and Enterprise entitlements are valid for one year and use a renewable, signed offline-verifiable license.
4. License expiry never deletes data, removes export access, or prevents a restaurant from opening its local data.
5. On trial or paid-term expiry, the application enters **Community Safe Mode**:
   - The owner selects one primary branch that remains fully read/write locally.
   - Other branch data remains visible and exportable, but becomes read-only until renewal.
   - Cloud-only controls stop; unsynced local events are retained safely for later renewal.
   - The user sees a clear, non-alarming explanation and renewal path.
6. Paid licenses have an explicit offline grace period set in the commercial policy; this plan proposes 30 days. A device clock change alone must not erase or lock data.
7. Manual, server-issued licenses are acceptable for the first release. Automated subscription billing is a separate integration and must not delay a safe license lifecycle.

---

## 4. Decisions required during the first 90 minutes

The following are product decisions, not coding details. The defaults below keep the six-stage progression moving unless the founder changes them before implementation begins.

| Decision | Provisional default | Why it matters |
|---|---|---|
| First supported platform | Windows 10/11 x64 desktop | Common restaurant hardware environment; enables a focused release and printer testing |
| Primary restaurant market | India, with generic tax configuration | Do not claim GST/e-invoicing compliance until its exact requirements are implemented and reviewed |
| Payment scope | Record cash, card, UPI, and split payments; no card data storage and no live gateway settlement | Avoids PCI scope while supporting normal counter operations |
| Printer scope | One named, tested thermal-printer model plus system print/PDF fallback | “All printers” is not testable in one focused release cycle |
| Cloud region/provider | Founder-selected managed provider and region | Determines privacy, credentials, backups, cost, and deployment |
| Professional identity | Email/password plus owner passkey option; staff PIN for rapid local POS unlock | Balances security and counter speed |
| Receipt format | Branded printable/PDF receipt with a controlled template | Makes billing deterministic before raw ESC/POS expansion |
| Currency and fiscal settings | Per-restaurant configurable, seeded with INR | Keeps the domain model internationally reusable |

No live payment processor, tax filing, or statutory e-invoice claim enters the release without its own compliance checklist and external validation.

---

## 5. Technology policy and recommended architecture

“Modern” means stable, well-maintained, secure, accessible, and easy to replace — not merely new. In Stage 1, pin the current stable compatible releases in lockfiles. Do not use beta, release-candidate, or abandoned packages simply because they are newer.

### 5.1 Recommended stack

| Layer | Recommended baseline | Reason |
|---|---|---|
| Client application | Flutter and Dart on desktop, tablet, and mobile | One native-compiled, adaptive UI codebase for restaurant devices without a browser-engine dependency |
| UI foundation | Dart sound null safety, Flutter Material 3 foundations, custom Gotigin design tokens, semantic widgets, form/schema validation | Fast, consistent, touch-friendly interface with accessibility built into the widget tree |
| Local domain/runtime | Rust core behind generated Flutter/Rust bindings | Rust owns database, filesystem, cryptographic, sync, and business-critical operations; Flutter remains focused on experience and presentation |
| Flutter/Rust boundary | flutter_rust_bridge generated bindings with contract tests | Typed asynchronous calls and generated Dart/Rust models instead of hand-written platform-channel glue |
| Local database | SQLCipher-backed SQLite 4.17.0 with WAL, foreign keys, busy timeout, transactional migrations, and OS-keystore-held keys | Durable offline-first storage with full database encryption and a stable long-term format |
| Cloud API | Rust service using a typed HTTP framework, OpenAPI contract, and SQLx-style checked SQL | Shares domain safety goals and avoids an untyped sync boundary |
| Cloud data | Managed PostgreSQL with tenant isolation and row-level-security defense in depth | Mature relational reporting, transactions, backups, and future scale |
| Object storage | S3-compatible encrypted object storage | Portable storage for encrypted backup artifacts and exports |
| Authentication | Standards-based OIDC session model, Argon2id password hashes, WebAuthn/passkeys for owners | Strong authentication without inventing cryptography |
| Observability | OpenTelemetry-compatible traces, metrics, and structured logs | Vendor-neutral operational visibility |
| Delivery | Git repository, protected main branch, CI, signed build artifacts, SBOM, and infrastructure as code | Repeatable and auditable releases |

For development and Linux verification, the workspace uses a bundled SQLCipher
test path. Release artifacts must link the reviewed, pinned SQLCipher 4.17.0
library for every supported platform, beginning with Windows x64.

The same business rules must live in a shared domain package or clearly mirrored, contract-tested domain layer. The UI must never become the only place that enforces a financial or permission rule.

### 5.2 Repository shape

    apps/
      ros/                     Flutter/Dart desktop, tablet, and mobile application
      owner-dashboard/         Professional web dashboard
    services/
      api/                     Cloud API, sync, entitlement, reporting
    crates/
      ros_core/                Rust business types, commands, validation, events
      ros_storage/             SQLCipher, migrations, audit storage, backup boundaries
      ros_bridge/              Versioned cross-boundary contracts
    database/
      local-migrations/
      cloud-migrations/
      seeds/
    infra/
      deployment/
      monitoring/
    docs/
      adr/
      runbooks/
      security/
    tests/
      fixtures/
      e2e/
      migration/

### 5.3 Architecture boundaries

    Restaurant device
      Flutter/Dart UI and local user/session
      -> typed Flutter/Rust bridge
      -> Rust domain commands and permission checks
      -> SQLite transaction: business record + audit event + sync outbox
      -> optional background sync client

    Gotigin Cloud
      API and entitlement verification
      -> PostgreSQL tenant data, immutable event log, reporting projections
      -> encrypted backup artifacts and device/sync health
      -> owner dashboard and documented API

The desktop application must be completely useful without the cloud. The cloud must accept retries safely and must never require a device to upload a raw SQLite database repeatedly as its normal sync mechanism.

---

## 6. Domain, integrity, and non-deletion model

### 6.1 Core identities

Every durable record receives a globally unique, sortable identifier such as UUIDv7. All money is stored as integer minor units, never floating point. All events record UTC time and display in the restaurant’s configured local time zone.

Core tenancy hierarchy:

    Organization
      -> Branch
        -> Device
        -> User membership and role
        -> Restaurant operational data

Core release entities:

- Organization, Branch, Device, User, Role, Permission, License Entitlement
- Product, Category, Modifier, Tax Rule, Table, Customer
- Order, Order Item, Kitchen Ticket, Invoice, Payment, Refund/Reversal
- Inventory Item, Stock Movement, Purchase, Purchase Item, Expense
- Shift, Cash Drawer Session, Day Close
- Audit Event, Sync Event, Backup Record, Migration Record

### 6.2 Mutation policy

Sensitive records are append-only in business meaning. A correction creates a new authorized event; it does not erase the original fact.

| Record type | Allowed action | Forbidden action |
|---|---|---|
| Final invoice | Void/correct through a linked adjustment or credit flow | Silent edit or hard delete |
| Payment/refund | Reversal/refund with reason and approver | Delete or overwrite amount/method |
| Sent kitchen ticket | Cancel/replace with reason | Erase the ticket |
| Stock movement | Counter-adjustment with reason | Delete movement |
| Purchase/expense | Void/correct with audit event | Delete finalized record |
| Cash drawer and day close | Authorized adjustment/reopen with evidence | Delete or alter history silently |
| User/role/permission | Versioned change event | Remove historical evidence |
| Audit, sync, license, backup records | Append only | User-initiated deletion |
| Draft order not sent to kitchen | Discard allowed | N/A |
| Product/table/category | Archive/deactivate, then preserve references | Hard delete while referenced |
| Customer personal data | Privacy-aware anonymization/delete workflow where legally required | Retain personal data without policy |

Every sensitive mutation captures actor, role, device, branch, timestamp, action type, reason, correlation ID, before/after summary, and required approver where applicable.

### 6.3 Tamper-evidence boundary

Community Edition prevents counter staff from silently deleting activity through the application. It cannot guarantee that an owner with unrestricted operating-system access will never alter a local file. That would be an untrue claim.

Professional strengthens the assurance by:

- generating a device-bound signing key held in the OS keystore;
- linking audit events with previous-event hashes;
- anchoring event sequence and hashes to the cloud on sync;
- alerting the owner to gaps, invalid signatures, clock anomalies, or failed sync;
- preserving server-side append-only audit history and backup history.

This is tamper-evident governance, not magical tamper-proofing.

---

## 7. Offline-first synchronization and upgrade design

### 7.1 Local write path

For every completed business action:

1. Validate the command and the user’s role.
2. Open one SQLite transaction.
3. Write the business projection.
4. Append its immutable audit event.
5. Add a sync-outbox event with operation ID and causal metadata.
6. Commit.
7. Tell the user “Saved locally” immediately.

The UI must not wait for the network to complete a sale.

### 7.2 Community-to-Professional activation

    Existing local SQLite database
      -> create verified encrypted backup snapshot
      -> owner explicitly enables Professional sync
      -> create organization, branch, and primary device identity
      -> upload baseline in resumable encrypted batches
      -> upload outbox events
      -> cloud validates, stores, and acknowledges
      -> ongoing incremental sync begins

The old local database remains usable throughout. A failed upgrade is recoverable from the verified pre-sync backup and does not invalidate the original local installation.

### 7.3 Sync protocol rules

- Use idempotency keys for every event; retrying cannot duplicate an invoice or payment.
- Batch events in an authenticated, versioned API contract.
- Persist server event and reporting projection atomically.
- Acknowledge only after durable server commit.
- Maintain a local outbox, acknowledgement cursor, retry backoff, and visible sync health.
- Never synchronize by overwriting one database file with another.
- Never resolve financial conflicts with last-write-wins.
- Use explicit conflict policy:
  - financial and stock events append or require manager resolution;
  - catalog edits use field-level version checks and an explicit conflict screen;
  - immutable invoice/payment facts never auto-merge;
  - branch data is isolated by branch ID, then aggregated for central reports.

### 7.4 SQLite operational rules

- Enable foreign keys, WAL mode, a sensible busy timeout, and integrity checks.
- Use SQLCipher from the first build; key every connection before SQL and verify cipher status.
- Use a random per-installation database key stored only in the operating-system secure store; Flutter never receives the key.
- Keep the database and its WAL/SHM companions in the same controlled application-data directory.
- Use SQLite’s backup API or equivalent verified snapshot process; never use a simple file copy while the database is live.
- Test power-loss and interrupted-migration recovery.
- Treat a SQLite database as local-device storage, never as a database file on a shared network drive.

---

## 8. Security and privacy baseline

### 8.1 Security architecture

The project uses OWASP ASVS 5.0.0 as the application-security verification baseline and NIST SSDF practices for secure development. The first release targets the relevant Level 2 controls pragmatically, documents deviations, and never marks an unchecked control as complete.

Mandatory controls:

- Threat model for POS staff, malicious cashier, compromised device, lost device, attacker on public network, cloud account compromise, and insider access.
- Argon2id password hashing; no reversible passwords.
- Owner passkeys/WebAuthn option; short-lived server sessions and rotation of refresh credentials.
- Staff PINs are rate-limited, salted/hashed, and scoped to the local device/session.
- Least-privilege roles, server-side authorization, default-deny access, and database tenant isolation.
- Parameterized queries, schema validation at every boundary, output encoding, and safe file handling.
- TLS for every cloud connection; certificate validation is never bypassed.
- Secrets only in managed secret stores or operating-system keychains; never in source code, logs, or configuration commits.
- SQLCipher-encrypted local data, encrypted backups, and transport encryption; envelope keys are rotated by policy.
- Signed release artifacts and update manifests; verify update origin before installation.
- Dependency lockfiles, SBOM, vulnerability scanning, secret scanning, and code review before release.
- Structured security events without payment PAN, passwords, access tokens, or unnecessary personal data.
- Rate limiting, abuse protection, audit alerts, and safe generic error messages at public API boundaries.

### 8.2 Authorization baseline

Initial standard roles:

- Owner: organization, branches, finance, users, settings, exports.
- Manager: controlled voids, discounts, stock adjustments, reports.
- Cashier: orders, payments, permitted customer actions.
- Kitchen: kitchen-display status only.

Actions such as voiding an invoice, refunding, reopening a day close, applying a high discount, or stock adjustment require a configured manager/owner permission, a reason, and an audit event. An approval cannot be represented by merely changing a boolean in the browser.

### 8.3 Privacy and retention

- Collect only data required for restaurant operation.
- Provide owner-controlled export.
- Classify customer data separately from finance/audit data.
- Write a retention/anonymization policy before marketing, analytics, or cross-tenant analysis is introduced.
- Do not use production restaurant data to train models or improve AI systems without explicit lawful authorization.

---

## 9. Database migration and recovery policy

Every local and cloud schema change is a versioned migration with a unique ID, checksum, author, timestamp, and test fixture.

Migration rules:

1. Back up before a local migration.
2. Run migrations transactionally whenever the database supports it.
3. Record success/failure in a migration journal.
4. Use expand -> backfill -> switch reads/writes -> contract, rather than destructive one-step rewrites.
5. Preserve compatibility between the current desktop release and the immediately preceding release during sync rollout.
6. Never run an irreversible destructive migration in this staged release.
7. Test a representative old database, a partially migrated database, and a recovery from an interrupted migration.
8. Verify post-migration schema version, foreign keys, and SQLite integrity.
9. For local embedded migrations, recompute each SQL source's SHA-256 checksum
   against the reviewed manifest before reading or mutating the target
   database; fail closed on any mismatch.
10. Restore a backup into a clean install as a release gate.

The release ships with a plain-language recovery runbook: where the backup is stored, how to verify it, how to restore it, and how to contact support without exposing secret data.

---

## 10. User-experience system

The interface should feel like a well-run restaurant: clear priorities, no drama, and no surprise.

### 10.1 UX principles

- Touch-first, keyboard-first, and mouse-friendly.
- Large, high-contrast targets; do not rely on color alone.
- Every money-changing action has visible confirmation and an undo/correction route where business rules permit it.
- Show local-save status separately from cloud-sync status.
- Prefer instant local transitions and optimistic UI only after durable local commit.
- Use clear human language: “Saved locally. Waiting to sync” is better than an opaque spinner.
- Make dangerous actions visually distinct, reason-required, and permission-aware.
- Preserve context after errors; never make a cashier re-enter an entire order because a print or network action failed.
- Meet WCAG 2.2 AA for the included desktop and web flows.

### 10.2 Core screens

Community:

1. First-run onboarding and restaurant settings
2. Sign in / staff PIN unlock
3. POS: dine-in, takeaway, menu search, cart, modifiers, discounts, split payment
4. Tables and order lifecycle
5. Kitchen Display System
6. Products, categories, tax rules, customers
7. Inventory, purchases, stock adjustments, expenses
8. Reports: sales, tax, payments, items, cash drawer, expenses, stock
9. Staff, permissions, audit timeline, backup/export/settings

Professional:

1. Organization and branch selector
2. Owner dashboard with cross-branch sales and operational health
3. Users, devices, roles, entitlement, and branch management
4. Sync health, backup status, alerts, and remote read-only monitoring
5. Documented API-key management and API reference

### 10.3 Experience acceptance targets

- A trained cashier can begin an order, add common items, record a payment, and create a receipt without navigating a settings screen.
- Offline state is explicit but does not block normal billing.
- A KDS user can advance an order without seeing financial data.
- A manager can explain every void, discount, refund, and stock correction from the audit timeline.
- The first successful local sale is possible from seeded demo data within minutes of installation.

---

## 11. Six major stages

Each stage ends with a working build, automated checks, a demo of that stage’s flows, and a written decision log. Do not defer testing to the final stage.

### Stage 1 — Foundation, trust boundaries, and first local workflow

**Objectives**

- Initialize Git, branch protection policy, monorepo, formatting, linting, type checks, test runner, CI, release artifact workflow, and issue/decision templates.
- Create architecture decision records for platform, cloud provider, identity, printer, tax/payment scope, encryption approach, and offline-license policy.
- Write the threat model, data classification, non-deletion policy, and initial security checklist.
- Establish Flutter design tokens, accessible adaptive shell, navigation, empty/error/loading states, and seeded demo restaurant.
- Implement Flutter/Rust bridge generation, local application bootstrap, SQLite connection policy, first migrations, encrypted-secret/keychain abstraction, local owner setup, and staff-role model.
- Create core entities and audit-event/outbox tables.
- Implement onboarding, menu/category setup, and a local product list as the first end-to-end persisted flow.

**Stage 1 acceptance**

- A fresh machine can build and launch the Flutter application on the supported desktop target from documented steps.
- A fresh local database migrates successfully; a seeded database starts successfully.
- A local product can be created, archived, restored from backup, and audited.
- CI runs format, lint, type, unit, dependency, and secret checks on every change.
- Threat model and architecture decisions are committed under docs.

### Stage 2 — Community Edition: fast, correct POS and billing

**Objectives**

- Build the POS flow for dine-in and takeaway: tables, menu/search, modifiers, cart, notes, taxes, discounts under permission, and order states.
- Create kitchen tickets and a minimal KDS flow.
- Record cash, card, UPI, and split payments as payment records without storing card data.
- Generate immutable invoices, printable/PDF receipts, invoice numbering, void/refund/correction pathways, and audit events.
- Add cash drawer session/open-close basics.
- Split tender is defined in `docs/contracts/split-payment-ledger-v1.md` and
  is implemented end-to-end: the POS collects bounded cash/card/UPI
  allocations, Rust verifies their exact trusted-invoice total, every
  allocation is immutable/audited/syncable, and refunds deterministically
  allocate across original methods. Migration 0018 also prevents method-level
  over-refunds, so cash-drawer and payment reporting remain correct.
- Test local transaction boundaries, duplicate-click prevention, permissions, invoice numbering, and power/restart recovery.

**Stage 2 acceptance**

- A cashier completes a full order-to-invoice journey offline.
- The invoice and payment survive a restart and appear in the audit timeline.
- A cashier cannot silently edit or delete a finalized invoice/payment.
- A manager correction is represented as a linked, reasoned event.
- KDS sees only the required kitchen ticket data.

### Stage 3 — Community Edition: restaurant operations and owner confidence

**Objectives**

- Complete inventory items, recipe/stock deduction scope agreed in Stage 1, purchases, stock movements, low-stock view, expenses, and approved adjustments.
- Complete local staff accounts/roles and manager approval flows.
- Build daily sales, payment, tax, item, expense, stock, and cash-drawer reports.
- Finish table/order lifecycle and KDS states.
- Add backup/export, restore wizard, data-integrity check, archive/deactivate master data, and customer privacy handling.
- Run Community accessibility, offline, backup/restore, and real-device printer/PDF acceptance scenarios.

**Stage 3 acceptance**

- Community Edition supports the complete stated single-branch operating loop: POS, KDS, inventory, purchases, expenses, reports, staff, and recovery.
- A verified backup restores into a clean installation and produces matching report totals.
- Sensitive operational history remains traceable after corrections.
- Community works without a cloud account or network connection.

### Stage 4 — Professional foundation: identity, cloud, upgrade, and sync

**Objectives**

- The provider-neutral Rust API now has explicit validated non-secret listener
  and deployment-environment configuration plus separate liveness/readiness
  probes. Its feature-gated local sync route verifies short-lived Ed25519
  device JWTs and uses a tenant-scoped PostgreSQL adapter, but production token
  issuance, credential provisioning, activation, and cloud deployment remain
  intentionally incomplete until the accountable cloud decisions are made.
- Cloud migrations 0002–0004 prevent cross-organization branch/device
  references, force RLS on tenant and authorization tables, make accepted
  events and authorization facts append-only, and retain canonical stable
  acknowledgements. A real PostgreSQL integration environment with the final
  non-owner API role is still required to prove cross-tenant denial,
  revocation ordering, replay safety, and rollback behavior.

- Provision production-like cloud environments: isolated development/staging/production configuration, PostgreSQL, encrypted object storage, secrets, TLS, and least-privilege service accounts.
- Implement cloud organization/branch/user/device model, owner authentication, role authorization, signed entitlements, 14-day evaluation, annual Professional, and annual Enterprise branch-limit configuration.
- Implement explicit Community-to-Professional activation: pre-sync backup, primary device registration, baseline upload, encrypted resumable transfer, and rollback/recovery behavior.
- Implement event/outbox sync, acknowledgements, idempotency, retry/backoff, server audit anchoring, and visible sync health.
- Build the first owner dashboard: organization, branch, device status, and basic consolidated sales.

**Stage 4 acceptance**

- A populated Community database upgrades to Professional without reinstalling or losing local access.
- A device can make invoices offline, reconnect, synchronize once, and safely retry synchronization without duplicate financial records.
- A different organization cannot access another organization’s data through API, UI, or direct tenant-scoped test.
- Trial expiry behavior enters Community Safe Mode without deleting or hiding data.

### Stage 5 — Professional and Enterprise controls

**Objectives**

- Add up to five Professional branches, centralized reporting, branch filter/switcher, remote health monitoring, and automatic encrypted cloud backup scheduling.
- Add advanced roles, multi-device staff access, device revocation, session management, and owner alerts for sync/device anomalies.
- Publish a versioned, authenticated API for products, customers, orders, invoices, reports, and sync status; generate its reference from the OpenAPI contract.
- Add Enterprise entitlement branch capacity and administrative configuration.
- Implement conflict screens and server validation for catalog conflicts; verify financial events only append or require resolution.
- Run network partition, duplicate-event, delayed-event, upgrade, key-loss, and revoked-device tests.

**Stage 5 acceptance**

- An owner can see branch-level and consolidated reporting for Professional.
- A revoked device loses cloud access without corrupting its existing local data.
- Automatic backup status is observable and a cloud backup can be restored into a separate verification environment.
- API authorization, tenant isolation, idempotency, and rate limits are covered by automated integration tests.

### Stage 6 — Hardening, release, recovery proof, and handoff

**Objectives**

- Freeze feature scope; fix only defects, security issues, accessibility defects, release blockers, and documentation gaps.
- Run full unit, integration, end-to-end, migration, backup/restore, offline/sync fault, and performance smoke suites.
- Run dependency/SBOM/secret/static analysis checks and a focused manual OWASP ASVS review.
- Test fresh installation, upgrade from Community, trial activation/expiry, paid expiry safe mode, and restore onto a clean machine.
- Package release artifacts, sign them where credentials exist, publish checksums and release notes, configure update channel, and deploy cloud production.
- Configure OpenTelemetry telemetry, error reporting, health dashboards, alert thresholds, data redaction, and on-call/support runbooks.
- Conduct founder acceptance demo using a realistic restaurant service script; record known limitations explicitly.

**Stage 6 acceptance**

- Every gate in Section 15 passes, or the release is held.
- A new user can install, onboard, operate offline, back up, restore, and upgrade using the written guides.
- A release candidate has a reproducible build provenance, checksums, SBOM, migration manifest, and rollback plan.
- Support receives installation, recovery, operational, and incident runbooks.

---

## 12. Test strategy

### Automated tests

- Domain unit tests for money, taxes, invoice numbering, permissions, void/reversal, inventory movement, entitlement state, and audit rules.
- SQLite integration tests for transactions, constraints, migration upgrades, WAL recovery, backups, and interrupted operations.
- Cloud integration tests for authentication, authorization, tenant isolation, RLS defense in depth, sync idempotency, event ordering, rate limits, backup metadata, and API contracts.
- End-to-end desktop tests for onboarding, POS, payment, invoice, KDS, inventory, report, backup/restore, Professional activation, and sync status.
- Contract tests so desktop and cloud cannot drift from the documented API.
- Accessibility tests plus keyboard-only and screen-reader smoke checks.
- Fault-injection tests: offline during sale, process crash during write, internet drop during sync, duplicate retries, stale device, expired entitlement, and failed migration.

### Manual acceptance script

Community offline path (current):

1. Install from a clean machine.
2. Configure a restaurant and staff (including branch time zone).
3. Create menu items, tax rates, and a table label for dine-in holds.
4. Take a dine-in order and send it to KDS.
5. Record split cash/UPI payment (payable after discount/tax) and export a receipt.
6. Void a mistaken invoice with owner/manager reason and inspect Reports/audit.
7. Record a purchase, stock adjustment, and expense.
8. Close the UTC accounting day (append-only; no reopen) and reconcile Reports.
9. Disconnect network and repeat a sale.
10. Create a verified backup, verify it, and restore beside live data on the
    same installation.

Still founder-gated / not claimed by this script:

11. Back up and restore to a clean install with an approved recovery envelope.
12. Dual-person approval for voids/refunds/reopen.
13. Activate Professional, sync a branch, inspect owner dashboard.
14. Revoke a device and verify no cross-organization data exposure.
15. Named thermal printer / signed release packaging.

---

## 13. Performance, availability, and observability targets

Targets must be measured on the agreed reference hardware and documented with the result.

| Area | Release target |
|---|---|
| Local order/cart interaction | Perceived instant response; investigate any repeated action above 150 ms on reference hardware |
| Local invoice commit | Durable local confirmation within 500 ms in normal reference conditions |
| Launch | Usable main view within 3 seconds on reference hardware after warm start |
| Offline availability | POS, KDS, reports, and local data management remain usable with no network |
| Cloud failure | No completed local sale is lost or blocked solely due to cloud unavailability |
| Sync | Visible queue/status and safe eventual retry; no duplicate commit after retry |
| Backup | Verified backup and clean-machine restoration demonstrated before release |
| Telemetry | Health, error, sync backlog, backup age, and API latency observable without storing sensitive payloads |

Use OpenTelemetry-compatible traces, metrics, and logs so operational data remains portable between vendors.

---

## 14. Operational readiness and documentation

The release must include:

- Product onboarding guide for restaurant owners.
- Quick cashier, kitchen, manager, and owner workflows.
- Installation and update guide for the supported platform.
- Backup, restore, export, and data-recovery guide.
- Professional activation, trial, renewal, expiry-safe-mode, and device-revocation guide.
- Printer/PDF support guide naming the tested hardware.
- API reference generated from the versioned OpenAPI contract.
- Architecture diagram, data model, ADRs, threat model, and non-deletion policy.
- Incident runbooks: device offline, cloud outage, sync backlog, failed backup, failed migration, lost device, compromised credentials, and suspected tampering.
- Known limitations and support escalation path.

---

## 15. Non-negotiable release gates

### Functional and data gates

- [ ] Community single-branch loop passes the manual acceptance script.
- [ ] Professional upgrade, synchronization, and owner dashboard pass.
- [ ] All critical state changes are durable local transactions.
- [ ] No finalized invoice, payment, stock movement, expense, or audit event can be silently deleted through supported interfaces.
- [ ] Backup/restore and migration recovery tests pass.
- [ ] Trial and annual-expiry safety behavior preserves data and primary-branch operation.

### Security gates

- [ ] Threat model and security checklist are reviewed.
- [ ] Authorization is verified server-side for every protected API action.
- [ ] Tenant-isolation tests pass; default-deny behavior is verified.
- [ ] No high/critical unresolved dependency or secret-scanning finding is accepted without written risk approval.
- [ ] Sensitive data is redacted from logs and telemetry.
- [ ] Encryption, key storage, session lifecycle, rate limits, and audit controls are tested.
- [ ] Release artifacts and update metadata are signed or the unsigned distribution is explicitly blocked from public production release.

### UX and accessibility gates

- [ ] POS works with touch, keyboard, and mouse.
- [ ] Critical flows pass contrast, focus, keyboard, and screen-reader smoke checks.
- [ ] Offline and sync states are understandable without technical knowledge.
- [ ] Error handling preserves the user’s work.
- [ ] The founder approves the realistic service-flow demo.

### Operational gates

- [ ] Staging and production secrets/configuration are separated.
- [ ] Health checks, alerts, dashboards, backups, and retention rules are configured.
- [ ] A rollback path and a recovery owner are documented.
- [ ] Support and incident runbooks are complete.

No deadline overrides a failed release gate. A feature can be deferred; an untested financial or security failure cannot be rationalized as “production ready.”

---

## 16. Major risks and deliberate mitigations

| Risk | Mitigation |
|---|---|
| Greenfield scope is broader than the six stages | Build a coherent vertical slice; freeze scope at each stage; defer unverified integrations rather than lowering quality |
| Printer/hardware variability | Support and test a named first hardware target plus PDF/system-print fallback |
| Data loss during upgrade or migration | Preflight backup, transactional migrations, sync outbox, recovery tests, and clean-machine restore |
| Counter fraud or accidental edits | Append-only financial events, approvals, reasons, audit trails, and Professional cloud anchoring |
| Network instability | Local-first commits, durable outbox, retries, idempotency, visible sync status |
| Cross-tenant cloud exposure | Organization/branch scoping in domain, API authorization, PostgreSQL RLS defense in depth, and negative tests |
| License expiry harms a restaurant | Community Safe Mode, no deletion, exports, primary-branch continuation, explicit grace policy |
| Regulatory overclaim | Do not advertise tax, payment, PCI, or statutory compliance beyond tested and reviewed scope |
| AI-generated code mistakes | Treat AI output as authored code: review, type-check, test, scan, and demonstrate every critical path |

---

## 17. How AI agents contribute without weakening quality

GPT-5.6 and other agents can compress implementation time by generating scaffolding, domain tests, UI variants, migration fixtures, API contracts, documentation, and fault-test cases. They do not replace accountable engineering.

For every generated change:

1. State the intended behavior and security/data constraints.
2. Review the diff against the domain and threat model.
3. Run the smallest relevant automated tests.
4. Exercise the user-facing path manually when it changes money, permissions, sync, backup, or migration behavior.
5. Preserve the decision and test result in the repository.

The proof point is not “AI wrote a lot of code quickly.” The proof point is that Gotigin shipped a fast, beautiful, auditable product with demonstrable operational discipline.

---

## 18. Post-release roadmap protection

The six-stage release deliberately leaves room for these properly engineered follow-ons:

- Native raw ESC/POS, cash-drawer, scanner, weighing-scale, and customer-display integrations.
- Full recipe/production inventory, supplier workflows, wastage, and forecasting.
- Delivery aggregator and payment-provider integrations.
- GST/e-invoicing or other jurisdiction-specific compliance modules.
- Advanced approval workflows, custom roles, anomaly detection, and fraud analytics.
- Enterprise SSO, on-premise deployment, custom APIs, SLAs, and organization analytics.
- Independent security assessment, penetration test, accessibility audit, disaster-recovery exercise, and load testing.

These are roadmap items, not excuses to weaken the core release. The architecture, contracts, audit model, and migration discipline in this plan keep them additive rather than rewriting the product later.

---

## 19. Authoritative engineering references

- OWASP Application Security Verification Standard 5.0.0: https://owasp.org/www-project-application-security-verification-standard/
- NIST Secure Software Development Framework: https://www.nist.gov/publications/secure-software-development-framework-ssdf-version-11-recommendations-mitigating-risk
- SQLite Write-Ahead Logging guidance: https://sqlite.org/wal.html
- PostgreSQL Row Security Policies: https://www.postgresql.org/docs/current/ddl-rowsecurity.html
- W3C Web Content Accessibility Guidelines 2.2: https://www.w3.org/TR/WCAG22/
- OpenTelemetry documentation: https://opentelemetry.io/docs/
