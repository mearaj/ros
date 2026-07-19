# Community-first delivery contract

## Purpose and priority

This is the active delivery contract for ROS. It applies to every developer,
agent, review, and release decision until Community Edition has met its
acceptance gate.

**Priority order**:

1. Complete and stabilize Community Edition.
2. Verify Community on the declared mobile, tablet, and desktop targets.
3. Build and release Professional Evaluation (the free 14-day Professional
   tier) only after Community is accepted.
4. Build and release Professional Paid only after Professional Evaluation is
   accepted.
5. Enterprise remains contract-led work after those gates.

Professional and Enterprise source foundations may be maintained only where a
Community-safe change requires it. Do not start a Professional cloud,
entitlement, multi-branch, dashboard, billing, or Enterprise feature merely
because its code scaffold exists.

## Customer journey that must not regress

Every Community change must preserve this journey:

1. Install ROS and create a restaurant locally.
2. Create the Owner PIN immediately after setup; no default PIN exists.
3. On every cold app launch, show the PIN unlock screen before operational
   data is shown. A prior 15-minute session must not survive a process restart.
4. As Owner, create/reset/revoke staff credentials through **Team & PINs**.
   A non-owner cannot manage staff or reset the Owner PIN.
5. Create categories and menu items, then run the counter fully offline.
6. Optionally import the common Indian starter menu at any point. ROS adds only
   missing starter categories and items; imported items are disabled at zero
   price until the owner reviews each price and resumes each item deliberately.
7. Complete orders, kitchen work, refunds/voids, reports, backup/export, and
   recovery without a cloud account.
8. Lock, restart, reinstall, restore, or factory-reset according to the
   Community recovery contract without exposing protected data.

## Required UX rules

- Write for a restaurant owner or staff member, never for a developer.
- Every failed action must name the next useful action. Do not expose blanket
  messages such as “local changes could not be saved” when the app can identify
  a duplicate name, invalid input, missing Owner unlock, or setup requirement.
- Do not place essential Owner work behind an unlabeled icon. Staff management
  is a visible **Team & PINs** action for the active Owner.
- Visible status, diagnostic, and report text must be selectable and copyable.
  Editable fields retain normal input selection.
- Category visuals and menu-item visuals have different roles. The category
  image action must visibly offer all four choices: **app category artwork**,
  **verified Gotigin catalogue photo**, **restaurant-owned upload**, and
  **remove current image**. App category artwork is a distinct library and
  must never reuse the built-in dish-photo chooser.
- Restaurant-owned category uploads require a clear rights confirmation before
  saving. Gotigin catalogue selections retain the verified source checksum and
  licence record with the audited category revision; do not downgrade them to
  an unlabelled upload.
- Default templates are opt-in, explain exactly what they add, and never make
  unreviewed items sellable. Template data must be editable and safely
  disabled until reviewed.
- Prefer a short direct confirmation over a modal or administrative concept
  when the action is routine. Use destructive confirmations only for a real
  data-loss or irreversible boundary.

## Data, authorization, and migration invariants

These rules take precedence over UI convenience:

- Flutter presents workflows; Rust owns authorization, pricing, encrypted
  storage, migrations, audit events, and security decisions.
- Owner, Manager, Cashier, and Kitchen permissions are enforced in Rust for
  every mutation. A hidden or disabled button is never the only protection.
- Financial facts, staff credentials, staff status, audit records, and
  transaction history are append-only. Correct through a new fact; do not
  rewrite history.
- Removing a category image changes presentation only. It must not delete the
  category, its products, historic sales, or retained remote-image provenance.
- A category with no sold items may leave the active catalogue with its
  never-sold items. Any category containing sold items is retained for history.
- New storage behavior requires a versioned migration, a checksum entry, and
  migration/integrity tests. Never edit an already-shipped migration.
- Backups/restores must verify encryption, checksum, and schema, and must not
  silently overwrite a live database.
- Community must remain usable offline and without a Gotigin cloud account.

## Change protocol for agents and developers

Before changing Community behavior:

1. Read [Community Edition](community.md), the relevant contract under
   `docs/contracts/`, and the applicable ADR/runbook.
2. Identify the user journey and authorization/data invariants touched.
3. State the customer-visible outcome before implementing internal mechanics.
4. Preserve existing unrelated work in the shared worktree.

For every implementation change:

1. Add or update focused regression tests for the reported behavior.
2. Regenerate Flutter/Rust bindings when the Rust bridge API changes.
3. Run the proportionate Rust and Flutter checks. Any storage migration must
   include the storage suite; a Flutter workflow change must include analysis
   and widget tests.
4. Report the tested behavior, remaining platform evidence gaps, and any
   deliberate limitation honestly.

Do not convert an unclear error into a generic one merely to avoid showing
internal detail. Map known safe outcomes to plain language and retain only a
safe retry/recovery instruction for unexpected failures.

## Community acceptance gate

Community is ready to move to Professional Evaluation work only when all of
the following have evidence:

- The customer journey above passes on Linux and Windows, and passes on Android
  phone/tablet plus iOS/iPadOS and macOS when the required devices are
  available.
- Owner setup, cold-start PIN lock, staff lifecycle, local POS, KDS,
  inventory, reports, backup/restore, recovery, factory reset, and starter
  menu import have end-to-end acceptance evidence.
- Error states are understandable and actionable in the tested paths.
- Production SQLCipher, secure storage, signing, install/update/uninstall,
  accessibility, and release artifact gates are met for every platform claimed
  as supported.
- No open Community regression is being deferred in order to begin a
  Professional feature.

After this gate, the next work is **Professional Evaluation**, not Professional
Paid. Evaluation and Paid share the Professional capability set but differ in
their entitlement term; the evaluation path must prove activation, expiry, and
Community Safe Mode before a paid plan is offered.

## Source of truth

- [Community Edition](community.md) — product behavior and platform scope.
- [Professional Edition](professional.md) — planned Evaluation and Paid scope.
- [Portable recovery ADR](../adr/0005-portable-recovery-envelope.md) and
  [credential recovery ADR](../adr/0007-credential-recovery.md).
- [Local staff-session contract](../contracts/local-staff-session-v1.md).
- [Release verification](../runbooks/release-verification.md).
