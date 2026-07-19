# Community Edition

## Product promise

Community Edition is ROS for a single restaurant branch. It is free forever,
local-first, and fully owned by the restaurant operator. It must remain useful
without a Gotigin account, an internet connection, a subscription, or a cloud
service.

The restaurant owns and controls its local data. Gotigin does not hold a
master PIN, database key, or remote unlock mechanism.

## Scope

| Topic | Community behavior |
| --- | --- |
| Price and term | Free forever; no expiry or subscription |
| Restaurant capacity | One branch per local Community installation |
| Menu, orders, invoices, tables, staff | Unlimited normal local use |
| Connectivity | Local POS and operations work offline |
| Data | Encrypted local database; owner-controlled backup and recovery |
| Cloud synchronization and owner dashboard | Not included |
| Multi-branch operation | Not included |
| Upgrade | A later Professional activation must preserve local data and local operation |

Community is not a trial. Its limits are the natural limits of a local,
single-branch deployment—not artificial caps on a restaurant's day-to-day
work.

## Supported operating-system targets

ROS uses one Flutter client with a Rust business, storage, and security core.
The intended Community Edition target matrix is:

| Form factor | Operating systems | Release position |
| --- | --- | --- |
| Phone and tablet | Android and iOS/iPadOS | Targeted. Android and iOS are not publishable until secure-store adapters, real-device tests, production SQLCipher linkage, and store-signing evidence are accepted. |
| Desktop | Linux | Development build and CI verification exist; a public release still requires the production artifact, signing, and release gates. |
| Desktop | Windows | CI compile/test compatibility exists; a public release still requires production SQLCipher, secure-store smoke tests, signing, and release acceptance. |
| Desktop | macOS | Platform project is maintained as a target. No macOS device is currently available for final verification, so it must not be claimed as release-validated until a signed macOS build and clean-device smoke test are recorded. |

"Supported" in a public release means the exact platform has passed its own
build, install, secure-storage, recovery, offline-operation, accessibility,
and update/uninstall acceptance tests. It does not mean merely that a Flutter
project directory compiles.

Android tablets and iPads use the same Community product and data model as
phones. Desktop, tablet, and phone interfaces may adapt their layout, but must
not change the financial, authorization, or recovery rules.

## Local restaurant operation

Community is expected to provide the complete single-branch operating loop:

- Restaurant onboarding, settings, categories, products, images, taxes,
  modifiers, availability, and tables.
- Dine-in and takeaway POS, held orders, split tender, local cash/card/UPI
  payment recording, immutable invoices, receipt reprint, refunds, voids, and
  cash-drawer accounting.
- Kitchen Display System, kitchen instructions, cancellation acknowledgement,
  and ticket progress.
- Customers with explicit marketing consent, correction history, and
  anonymization.
- Inventory movements, purchases, waste, adjustments, low-stock alerts,
  expenses, reports, exports, day close, and owner audit history.
- Local diagnostics and verified backup/restore flows.

All financial and security-critical decisions are enforced by Rust and the
encrypted local database, not just by the Flutter interface. Financial facts,
staff-security events, and audit history are append-only. A correction creates
a new, reasoned fact; it must not rewrite an invoice or erase its history.

Some capabilities may be present in source while awaiting release evidence.
Hardware printing, tax or statutory-compliance claims, payment-gateway
processing, and platform-specific publishing must only be represented as
available after their respective acceptance gates pass.

## Roles and PINs

Every Community restaurant has one reserved **Owner** account. The Owner has
full local administrative authority. A first-run installation creates no
default PIN: the Owner chooses a six-to-twelve-digit PIN before restaurant
operations are exposed.

The Owner can create, change roles for, reset the PIN of, and revoke non-owner
staff. The local roles are Manager, Cashier, and Kitchen. Staff PINs are never
stored as plaintext; they are protected by Argon2id verifiers. Failed PIN
attempts are rate-limited, and device-local staff sessions expire.

| Action | Owner | Manager | Cashier | Kitchen |
| --- | --- | --- | --- | --- |
| Staff administration and PIN reset | Yes | No | No | No |
| Catalogue, inventory, expenses, refunds, drawer, cancellations | Yes | Yes | No | No |
| Counter sale, held order, send to kitchen | Yes | Yes | Yes | No |
| Kitchen-ticket progression | Yes | Yes | No | Yes |

A staff member who forgets a PIN asks an active Owner to set a new one through
**Team & PINs**. The previous PIN immediately ceases to authenticate; the
change is audited. A staff member must never be able to reset the Owner PIN.
Every cold app launch returns a provisioned restaurant to the PIN screen; an
inactivity-limited session does not survive a process restart.

## Menu setup shortcuts

Owners and Managers can build a menu manually or explicitly import the
**common Indian starter menu** at any time. It adds only missing common
categories and items as an editable starting point, so repeating the import
does not duplicate existing starter entries. Imported items are disabled at
zero price; the restaurant must review each price and deliberately resume each
item before sale.

Category visuals are separate from dish imagery. For every category, an Owner
or Manager can choose one of four visible options: app-provided category
artwork, a verified Gotigin catalogue photo, a restaurant-owned upload, or
remove the current image. The app artwork is a distinct category library, not
a reused menu-item dish-photo library. Restaurant uploads require a rights
confirmation; verified catalogue selections retain their checksum and licence
record locally even if the category image is later changed or removed.

## Owner recovery, reinstall, and factory reset

Community recovery must be fully local. It has three distinct paths:

| Owner situation | Safe action | Result |
| --- | --- | --- |
| Owner forgot the Owner PIN but still has the recovery passphrase | **Reset Owner PIN** | Passphrase proof permits a new Owner PIN; data is preserved and the event is audited. |
| Device was lost, replaced, or ROS was uninstalled | **Restore existing restaurant** | Select a portable backup (`.rosbackup`), its recovery envelope (`.rosrecovery`), and enter the recovery passphrase. ROS verifies and restores into a clean installation. |
| Owner lost the PIN, recovery passphrase, and usable recovery backup | **Start a new restaurant** | A deliberate factory reset creates a new empty Community restaurant. The inaccessible prior encrypted data is not recovered. |

The first two paths require an owner-created recovery passphrase. It is
separate from the shorter daily Owner PIN and is never stored by ROS or
Gotigin. The portable backup contains an encrypted database snapshot; the
recovery envelope wraps the database key with the recovery passphrase. Both
files and the passphrase are required for clean-install restoration.

During onboarding, ROS must guide the Owner to:

1. Set and confirm the Owner PIN.
2. Set and confirm a distinct recovery passphrase.
3. Create a portable backup and recovery envelope.
4. Save both files outside ROS's app-data directory, such as an encrypted
   external drive, a secure owner-controlled cloud folder, or another managed
   backup location.
5. Verify the backup and acknowledge that Gotigin cannot recover a lost
   passphrase or database key.

ROS should continue to display an Owner-only recovery-readiness warning until
a portable backup has been successfully created and verified. It must warn
before an uninstall/factory-reset operation that normal app uninstall can
remove local app data.

The locked/recovery screen must clearly offer:

1. **Unlock** with a staff PIN.
2. **Forgot Owner PIN** — recovery passphrase plus a new PIN; no data loss.
3. **Restore existing restaurant** — portable backup, envelope, and recovery
   passphrase.
4. **Start a new restaurant** — a destructive fresh start requiring explicit
   typed confirmation such as `ERASE RESTAURANT DATA`.

The factory-reset route is intentionally allowed without the old PIN or
passphrase, because it reveals no protected data. It must never silently
overwrite a recoverable database. Support may explain these steps but must
never ask for or receive a PIN, recovery passphrase, SQLCipher key, or raw
restaurant database.

## Security and data rules

- The encrypted database key stays in operating-system secure storage; Flutter
  does not receive it.
- PINs, recovery passphrases, database keys, and credential verifiers are not
  logged, exported in diagnostics, or synchronized.
- A fresh install or portable restore must never overwrite a live local
  database. Restore first verifies encryption, integrity, schema, and backup
  checksum.
- Losing all credentials and recovery artifacts is unrecoverable by design.
  A vendor bypass would defeat the product's owner-controlled security model.
- Community is offline-capable, not invulnerable to an owner with unrestricted
  operating-system access. ROS must not claim otherwise.

## Community release acceptance

Before declaring Community Edition available on a platform, record evidence
for the platform-specific release gates and this functional path:

1. Install a clean build and create a restaurant, Owner PIN, recovery
   passphrase, and verified portable recovery kit.
2. Verify locked-state redaction, Owner/Manager/Cashier/Kitchen authorization,
   a staff PIN reset by Owner, throttled invalid PIN attempts, and automatic
   session expiry.
3. Complete the stated local restaurant loop offline; restart and confirm
   invoices, reports, KDS, inventory, and audit records remain correct.
4. Verify a portable restore onto a clean installation and compare required
   report totals. Verify forgotten-Owner-PIN recovery with the passphrase.
5. Confirm a destructive factory reset requires explicit confirmation and does
   not silently overwrite a recoverable installation.
6. Perform the platform's signed install/update/uninstall, secure-store,
   encrypted-database, and release-artifact checks. On touch devices, include
   both phone and tablet layouts; on desktop, include keyboard workflows.

The detailed technical policy is in [ADR 0004](../adr/0004-rust-owned-local-key-store.md),
[ADR 0005](../adr/0005-portable-recovery-envelope.md),
[ADR 0007](../adr/0007-credential-recovery.md), the
[local staff-session contract](../contracts/local-staff-session-v1.md), and
the [release-verification runbook](../runbooks/release-verification.md).
