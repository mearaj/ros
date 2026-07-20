# Professional Edition

## Product position

Professional Edition is ROS for a growing restaurant organization that needs
multiple branches and cloud coordination while retaining local, offline
restaurant operation. It is an entitlement upgrade to a Community installation;
it must not require reinstalling or discarding local data.

Professional devices coordinate through the Gotigin cloud over WAN. They keep
encrypted local caches and durable outboxes for temporary internet outages, but
do not discover, host, pair with, or replicate through a Community LAN Hub.
This is one mode of the same ROS app, not a separate application.

Professional is planned and release-gated. Community acceptance is the current
delivery priority; Professional Evaluation work begins only after the
Community-first delivery contract is satisfied. Until its cloud deployment,
authentication, entitlement, synchronization, backup, and acceptance evidence
exists, it must not be sold or represented as live.

## Professional Evaluation — free trial

Professional Evaluation is free, starts only after explicit owner activation,
and provides the same capabilities as Professional Paid for **14 calendar
days**. It is not a separate product tier.

At expiry, ROS enters Community Safe Mode. No restaurant data is deleted,
hidden, or made unexportable:

- the Owner selects one primary branch that remains locally read/write;
- other branch data remains visible and exportable but read-only;
- cloud synchronization, central controls, and multi-branch cloud functions
  pause; and
- unsynchronized local events are retained for safe later renewal.

## Professional Paid

Professional Paid is a renewable annual entitlement for up to **five
branches**. It includes the Professional capability set:

- Organization, branch, user, device, and role management.
- Up to five branches with continued local/offline POS at each branch.
- Authenticated cloud synchronization with idempotent event handling,
  acknowledgement, retry, visible sync health, and no last-write-wins handling
  of financial facts.
- Central owner dashboard, branch switching/filtering, cross-branch reporting,
  remote device/sync monitoring, alerts, and automatic encrypted cloud backup
  scheduling.
- Advanced roles, multi-device staff access, device revocation, session
  management, and a versioned authenticated API after it is published.

The annual entitlement has a **72-hour offline grace period** when it cannot
be refreshed. Afterwards, it follows the same Community Safe Mode behavior as
evaluation expiry. Renewal restores entitlement without reinstalling or
deleting data.

## Non-negotiable behavior

- Cloud failure must not stop local restaurant POS or remove the restaurant's
  local access to its data.
- A Community-to-Professional upgrade creates a verified pre-sync backup,
  establishes organization/branch/device identity, uploads a resumable
  baseline, verifies it, revokes LAN write authority, and only then enables
  cloud writes.
- The Owner may deliberately return to Community without reinstalling. The
  Owner selects one branch and a supported Branch Hub; all known cloud outboxes
  are reconciled, a verified snapshot is restored to the Hub, and cloud write
  authority is revoked before LAN writes begin. Other branches remain read-only
  and exportable and are never merged into the selected Community branch.
- Financial records remain append-only. Sync never replaces one database file
  with another and never resolves financial conflicts by last-write-wins.
- Entitlement expiry affects cloud and multi-branch coordination, not data
  ownership or export access.
- Expiry Safe Mode is not an unsafe automatic LAN conversion. During a WAN
  outage, retained outboxes remain durable; LAN pairing starts only after the
  selected branch has reconciled and completed a verified topology cutover.

## Release boundary

The repository may contain Professional foundations, including local sync
contracts and an owner-dashboard skeleton. Those foundations are not evidence
of a commercially available Professional service. Required release evidence
includes deployed tenant isolation, authentication/authorization, signed
entitlements, trial activation/expiry, safe renewal, backup/restore, sync
failure/retry, multi-branch reporting, device revocation, and production
operational support.

See [the edition overview](README.md),
[ADR 0009](../adr/0009-commercial-edition-terms.md),
[ADR 0010](../adr/0010-edition-data-topology-and-switching.md), and the
[Professional sync contract](../runbooks/professional-sync-contract.md).
