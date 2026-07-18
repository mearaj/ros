# Release verification gate

This checklist is the evidence required for a publishable Restaurant Operating
System release. A debug build or passing unit suite is necessary but never
sufficient by itself.

CI verifies checksum-pinned Flutter Debug builds on both Linux and Windows x64.
The Windows job is a compile/test compatibility gate for the declared desktop
target; it is not evidence of a publishable Windows Release artifact, signing,
secure-store smoke test, or controlled production SQLCipher linkage.

## Automated local gate

Run from the repository root:

```bash
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cd apps/ros
flutter pub get --enforce-lockfile
flutter analyze
flutter test
flutter test integration_test
flutter build linux --debug
```

The current suite covers encrypted local storage, migration source checksums
recomputed against the reviewed manifest before database mutation, schema
contracts, audit chains, immutable invoices/payments/refunds, snapshot-bound
draft settlement, kitchen state transitions and cancellation acknowledgement,
safe backup snapshots, immutable inventory movements and tracked-stock sale
enforcement, product-bound modifier pricing and immutable snapshots, sync
acknowledgement idempotency, image bounds, local Argon2id PIN/session throttling
and authenticated role boundaries, and core counter UI behavior.

The bundled SQLCipher development test path runs storage tests serially via
repository test configuration after concurrent native tests showed an
intermittent crash. This is a development-test containment measure, not
evidence that production concurrent-load testing has been completed; the
reviewed production SQLCipher artifact must pass its own concurrency smoke
suite before release.

## Dependency evidence gate

After dependency resolution and automated verification, return to the
repository root and create a new baseline evidence bundle for the release
candidate:

```bash
./scripts/generate-dependency-evidence.sh \
  target/release-evidence/dependencies-rc1
```

Verify its `SHA256SUMS` and retain it with the exact source revision. This is a
deterministic inventory and lockfile snapshot, not a publication SBOM or
open-source notice. The artifact-specific SBOM, license/notices review,
vulnerability disposition, and signing requirements are defined in the
[dependency evidence runbook](dependency-evidence-and-sbom.md).

## Functional acceptance gate

On a clean desktop user session, record evidence for:

1. Create Community setup, configure an Owner PIN, lock and unlock the local
   session, then create a category, product, and menu image.
2. Complete a cash, card, and UPI sale offline; restart and confirm invoice
   history and report totals.
3. Create a held order with an optional kitchen instruction, then send it to
   Kitchen Display. Confirm the Kitchen Display sees that instruction but no
   payment data or cancellation rationale. Reopen the Counter after a refresh
   or restart and confirm it remains identified as **sent**, cannot be edited
   or sent a second time, and can settle only with its saved fulfillment,
   product/quantity, and kitchen-instruction snapshot. As an Owner or Manager, send a second order,
   enter a reasoned cancellation, and verify Kitchen receives a prominent
   stop-work notice without the counter's free-text reason; normal ticket
   progression must be unavailable. Acknowledge that notice as a Kitchen
   session and confirm it leaves the active queue while retained history/audit
   evidence remains. Finally, reopen a third sent order as an Owner or
   Manager: confirm the original ticket receives its own cancellation notice,
   the counter receives a new editable revision, and sending that revision
   creates a distinct replacement kitchen ticket.
4. Record a reasoned full refund and confirm the original invoice remains
   visible while net reporting changes.
5. Create a verified backup and record its checksum. Do not claim a portable
   restore test until the approved recovery-envelope policy exists.
6. Record opening stock, a purchase, waste, and a reasoned adjustment; complete
   a tracked-item sale and confirm its derived balance. Attempt an overdraw and
   confirm that no additional invoice is recorded.
7. Open a cash drawer, record cash activity and a cash expense, then close it
   with a counted amount; verify the expected cash and variance are retained.
8. Add an optional customer with marketing consent left unchecked, attach the
   customer to an offline sale, correct the profile with a reason, then
   anonymize it as an Owner/Manager. Confirm the prior invoice remains while
   the customer can no longer be selected for a new sale.
9. Record a split cash/UPI sale whose allocations equal the displayed total,
   then issue a partial refund from Reports (amount less than remaining
   refundable balance, with a reason). Confirm the invoice remains immutable, the
   payment-method totals and cash drawer reflect allocations, and an unequal
   tender entry is rejected without creating an invoice.
10. Mark an active menu item sold out with a reason. Confirm it stays visible
    to menu managers but disappears from POS, that a direct sale attempt is
    rejected, and that a reasoned resume restores it without losing its image,
    price, or history.
11. Add two optional, non-negative modifier choices to a product, including one
    with a price addition. At POS, add the same product once with a modifier
    and once without it; confirm they remain distinct cart lines, complete the
    sale, and verify the Rust-calculated total. Send a modified draft to
    Kitchen and verify it shows modifier names without prices. Archive one
    modifier with a reason; confirm it cannot be selected for a new order while
    the earlier KDS/receipt snapshot remains unchanged. Retain the automated
    storage-test evidence that direct update/delete attempts remain
    trigger-rejected.
12. As Owner, change an active non-owner staff member's role with a reason.
    Lock and unlock as that staff member; confirm the newly effective role is
    enforced in the workspace and by Rust while the prior account and role
    history remain retained. Also confirm the locked workspace exposes no
    restaurant records before a staff member authenticates.
13. Open a recent finalized invoice, confirm its reprint retains the original
    line names, quantities, prices, selected modifier names, payment
    allocations, and any refund total, then copy the receipt text. Confirm a
    later menu price or modifier-availability change does not alter the
    reprint. Do not represent this as printer or PDF support.
14. For a tracked menu item, set a reasoned low-stock threshold above its
    current derived balance. Confirm the stock ledger flags it without changing
    any movement or balance; record a purchase and confirm the flag clears.
    Then clear the threshold with a reason, confirm the alert setting is gone,
    and set the same threshold again. Confirm all movements and history remain.
15. As the Owner, open the verified audit-history view from Reports. Confirm
    recent actions appear with sequence and time but no audit payload, device
    identifier, staff identifier, or credential data. Lock or switch to a
    non-owner session and confirm the view is unavailable.
16. In Reports, confirm **Top items** reflects finalized historical line
    snapshots after a menu item is renamed or repriced. Confirm its gross label
    remains explicit rather than allocating an invoice-level refund to a line.
17. Record an operating expense and confirm the Reports **Expenses** figure
    increases by exactly that immutable amount. Treat it as an operating total,
    not a tax, profit, or statutory-accounting calculation.
18. As the Owner, choose **Export verified financial CSV** in Reports. Confirm
    the native save dialog requires an explicit destination, cancellation
    creates no file, and a saved CSV contains only UTC-day/tender aggregates
    and its final summary—no customer data, product names, invoice IDs,
    credentials, or audit payloads. Switch to Manager and confirm the command
    is unavailable and denied by Rust storage.
19. In Reports, pick a UTC accounting day and confirm totals, recent invoices,
    and top items are scoped to that day. Confirm **Discounts** and **Tax
    collected** match finalized invoice snapshots for the day.
20. As Owner/Manager, apply a percentage or fixed discount at POS, complete the
    sale, and confirm Reports and the immutable receipt show the same discount
    and tax totals Rust computed.
21. Open the tax catalog, create a rate, set a product tax treatment, archive
    the rate, and confirm archived rates cannot be newly applied while prior
    invoice snapshots remain unchanged.
22. As Owner/Manager, void a non-refunded invoice with a reason. Confirm the
    original invoice remains, Reports exclude it from the UTC day totals, and a
    second void or a refund after void is rejected.
23. As Owner, create a verified backup, run **Verify local backup** on the same
    file name, then restore beside live data. Confirm
    `restaurant-os.restored.db` appears, the live database is unchanged, and
    portable/clean-install restore remains out of scope.
24. As Owner/Manager, close the selected UTC accounting day with a reason.
    Confirm the close snapshot is retained, a second close for that day is
    rejected, and reopen is not offered.
25. Confirm invoice, expense, audit, and sync timestamps display in the branch
    time zone (for example Asia/Kolkata → IST) while accounting-day selection
    remains UTC.
26. As Owner, open **Local diagnostics** from Reports. Confirm recent allow-listed
    event codes appear after a sale, refund or void, backup verify/restore, and
    day close. Export a pack with the save dialog; cancel writes nothing. Share
    without `ROS_DIAGNOSTICS_SHARE_URL` fail-closes while still allowing export.
    Clear removes only local diagnostic events.

## Publication blockers requiring accountable evidence

- Reviewed production SQLCipher 4.17.x artifacts and provenance for every
  published platform.
- Signed-desktop key-store create/reopen/missing-key smoke tests.
- Review evidence that every embedded local migration source matches its
  reviewed SHA-256 manifest value, including the fail-closed mismatch test.
- Code-signing identities, release distribution account, signed artifact
  checksums, and an artifact-specific validated SBOM plus reviewed open-source
  notices. The baseline dependency-evidence bundle alone does not satisfy this
  gate.
- Android release signing must use a controlled `android/key.properties`
  configuration; the Gradle project rejects missing or incomplete settings and
  never substitutes the debug signing key. Android remains an unsupported
  local-database release target until its secure-store and real-device gates
  are accepted.
- Selected and physically tested receipt/printer hardware where printing is
  advertised.
- Cloud provider account, tenant configuration, credentials, data policy, and
  deployed authentication/sync tests before Professional is sold.
- Legal review before any statutory, PCI, or accessibility-certification claim.

These items are intentionally tracked in the founder intervention log rather
than bypassed in code.

## Stage 6 evidence index

| Gate | Evidence location |
|------|-------------------|
| Founder cloud / recovery / approval / printer / commercial defaults | [founder-intervention-log.md](founder-intervention-log.md), ADRs 0003, 0005–0009 |
| Production SQLCipher artifact policy | [sqlcipher-artifact-manifest.md](sqlcipher-artifact-manifest.md), `third_party/sqlcipher/` |
| Dependency inventory baseline | `./scripts/generate-dependency-evidence.sh` |
| Artifact SBOM placeholder gate | `./scripts/generate-sbom-placeholder.sh` |
| Owner/cashier/kitchen guides | [owner-guides.md](owner-guides.md) |
| Accessibility smoke checklist | [accessibility-acceptance.md](accessibility-acceptance.md) |
| Professional sync client | `crates/ros_sync_client` |
| Owner dashboard | `apps/owner-dashboard/` |
| Staging infra skeleton | `infra/` |
| API env template | `services/api/.env.example` |
| OpenTelemetry / ops notes | `services/api/README.md` |
