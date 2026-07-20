# ADR 0010: Edition data topology and safe switching

**Status:** Accepted  
**Date:** 19 July 2026  
**Approver:** Gotigin engineering (founder-accountable product default)

## Context

ROS is one application that can operate as Community, Professional Evaluation,
Professional Paid, or Enterprise. A single Community branch still needs
multiple counters, kitchen displays, tablets, and owner devices when the
internet is unavailable.

Community therefore needs local-network coordination, while Professional and
Enterprise need cloud coordination across branches. Enabling both authorities
at once would create split-brain sales, inventory, kitchen, and staff state.
Edition changes must also preserve restaurant ownership and must not require a
new application installation.

## Decision

ROS ships one client and Rust core, but each branch activates exactly one
authoritative data topology:

| Edition | Authoritative topology |
| --- | --- |
| Community | One local Branch Hub over encrypted LAN |
| Professional Evaluation | Gotigin cloud over WAN |
| Professional Paid | Gotigin cloud over WAN |
| Enterprise | Gotigin cloud over WAN |

A one-device Community installation runs the Hub and client together. A
multi-device Community branch uses an always-on supported desktop or mini-PC
as the Hub. The Hub is the only writer to the branch SQLCipher database.
Clients use authenticated commands and an event stream; they never open or
share the database file.

**Hub service versus staff session.** Choosing Hub is a device-role decision.
Owner, Manager, Cashier, and Kitchen may all unlock the Hub UI so a
single-device restaurant can operate POS and Kitchen on the same machine.
Locking the UI or unlocking as a non-Owner role must not stop the Hub from
serving paired LAN clients. Only Owner may administer pairing, revocation,
portable recovery, and local restaurant profile history. The Hub stops when
the process exits, the machine sleeps or powers off, or Owner explicitly stops
the Hub.

Professional and Enterprise clients use encrypted local caches and durable
outboxes so an already authorized device can continue safe local work during a
temporary WAN outage. Cross-device and cross-branch coordination resumes only
through the cloud. LAN discovery, pairing, Hub hosting, and LAN replication
remain disabled in cloud topology.

LAN transport uses mutually authenticated device identities. Owner-approved,
short-lived pairing establishes each identity. mDNS may discover a Hub but
never grants trust. The Owner can inspect, rename, and revoke paired devices.
Commands carry stable idempotency identifiers and event cursors.

## Edition switching

The Owner may move between Community and an entitled cloud edition from the
same app. This is a controlled migration, not a settings toggle:

1. Explain the consequences and run topology-specific preflight checks.
2. Quiesce new writes and reconcile every known outbox with the current
   authority.
3. Create and verify an owner-controlled pre-cutover backup.
4. Transfer a resumable, checksummed baseline to the new authority.
5. Verify financial totals, event continuity, schema/protocol compatibility,
   branch identity, and device authorization.
6. Commit one durable cutover epoch.
7. Revoke the old topology's write grants before enabling writes on the new
   topology.

Community to cloud migration enrolls the organization, branch, and devices,
uploads the Hub baseline, verifies it, then disables LAN authority.

Cloud to Community migration requires the Owner to select exactly one branch
and provision a supported Hub. Other branches are never merged into it. They
remain read-only and exportable and can be retained as verified portable
archives. After reconciliation, the selected branch is restored and verified
on the Hub; cloud write grants are then revoked before LAN writes begin.

Rollback is automatic only before the cutover commit. After either authority
has accepted a post-cutover write, returning to the former topology requires a
new forward migration; ROS must never copy the old snapshot back over newer
facts.

## Expiry and unavailable WAN

Professional expiry enters Community Safe Mode without deleting data or
blocking export or already-authorized local POS. It does **not** silently turn
several disconnected cloud clients into independent Branch Hubs.

If the WAN is unavailable, each device retains its durable outbox and the app
clearly states that cross-device state may be delayed. When connectivity is
available, a safety-only reconciliation may drain retained branch events
without restoring paid cloud controls. The Owner can then complete the
verified cloud-to-Community migration for the selected branch. LAN pairing is
not enabled until that cutover succeeds.

## Consequences

- Edition changes do not require reinstalling ROS or abandoning data.
- At no time may a branch accept writes from both LAN Hub and cloud authority.
- LAN is a core Community capability, not a Professional feature.
- Cloud editions remain locally outage-tolerant, but do not use LAN as a
  hidden fallback.
- Hub loss requires verified restore to a replacement Hub. Automatic Hub
  failover is outside the first LAN release.
- The LAN protocol, Hub runtime, pairing lifecycle, cache/event projection,
  migration state machine, and failure-recovery acceptance tests must be
  completed before LAN or topology switching is represented as available.
