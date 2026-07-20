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
| Restaurant capacity | One branch; one local Hub may serve multiple paired devices |
| Menu, orders, invoices, tables, staff | Unlimited normal local use |
| Connectivity | Local POS and operations work without internet; multi-device coordination uses encrypted LAN |
| Data | Encrypted local database; owner-controlled backup and recovery |
| Cloud synchronization and owner dashboard | Not included |
| Multi-branch operation | Not included |
| Upgrade | A later Professional activation must preserve local data and local operation |

Community is not a trial. Its limits are the natural limits of a local,
single-branch deployment—not artificial caps on a restaurant's day-to-day
work.

## First-run edition and device role

ROS is one app. On first launch (and whenever no local restaurant profile is
active), it offers:

1. **Community Edition** — free forever, local-first, one branch.
2. **Paid editions** — Professional Evaluation (free trial), Professional Paid,
   and Enterprise. Those paths stay prepared in the product; Community delivery
   remains the active priority until its acceptance gate passes.

After **Community** is chosen, the device chooses its role:

| Device role | Meaning |
| --- | --- |
| **Hub** (Main / Primary) | This device owns the branch SQLCipher database and serves paired LAN clients. |
| **Client** (Secondary) | This device pairs to an existing Hub over encrypted LAN. It never hosts the first-release Hub database. |

A restaurant with only one computer still chooses **Hub**. On that machine the
Hub process and the operator UI run together so the same device can take
orders and, when other devices appear later, serve them.

## Hub service versus staff login

**Device role** and **staff session** are separate:

- The **Hub** keeps running and serving LAN clients while the local UI is
  unlocked as Owner, Manager, Cashier, or Kitchen, and while the local UI is
  locked awaiting a PIN. Locking or changing role must not stop the Hub.
- The Hub stops when the app/process exits, the machine sleeps or powers off,
  or an Owner explicitly stops Hub / shuts down the branch (if that control is
  offered).
- **Owner, Manager, Cashier, and Kitchen may all unlock on the Hub device.**
  One-device restaurants must be able to run POS and Kitchen on the same
  machine that hosts the Hub.
- Only an **Owner** session may perform Hub administration: staff lifecycle,
  device pairing/revocation, portable backup and recovery kit, local restaurant
  profile create/switch/archive, and destructive abandon of a profile.

Client devices unlock with Owner or other staff roles according to normal
authorization; they do not host the branch database.

## Multi-device LAN operation

One branch does not mean one device. Community supports counters, kitchen
displays, tablets, and owner devices through one local **Branch Hub**. On a
single desktop the Hub and client run together. A multi-device restaurant uses
an always-on supported desktop or mini-PC as the Hub; phones and tablets join
as clients and do not host the first-release Hub.

The Hub is the branch authority and the only writer to its SQLCipher database.
Paired clients send idempotent commands and receive ordered event updates over
mutually authenticated encrypted LAN connections. They never open a shared
SQLite file. Internet access and a Gotigin account are not required.

If a client loses the LAN, it shows that it is reconnecting and does not
independently finalize invoices, payments, stock movements, or kitchen sends.
After a Hub restart, clients resume from their last event cursor. The Owner
approves short-lived device pairing and can inspect, rename, and revoke devices.
Network discovery is only a convenience and never grants access.

Professional Evaluation, Professional Paid, and Enterprise use cloud
coordination over WAN instead. A branch never enables LAN Hub and cloud
authority at the same time. Changing edition uses the verified migration in
[ADR 0010](../adr/0010-edition-data-topology-and-switching.md), not an instant
transport toggle.

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
does not duplicate existing starter entries. Imported items arrive **available
to sell at a ₹1 placeholder price** so the restaurant can try POS immediately,
then replace prices in Menu when ready. Every imported category arrives with its distinct app
category artwork and every imported item with a matching built-in dish photo,
so the imported menu is presentable immediately; the owner can replace or
remove any of these visuals afterwards. Running import again also fills any
starter categories or items that still have no image, without overwriting
images the restaurant already chose.

Category visuals are separate from dish imagery. For every category, an Owner
or Manager can choose one of four visible options: app-provided category
artwork, a verified Gotigin catalogue photo, a restaurant-owned upload, or
remove the current image. The app artwork is a distinct category library, not
a reused menu-item dish-photo library. Restaurant uploads require a rights
confirmation; verified catalogue selections retain their checksum and licence
record locally even if the category image is later changed or removed.

## Counter selling (Owner, Manager, Cashier)

Before an item can be sold at the counter it must be **available** and have a
price greater than zero. Starter-menu imports arrive available at ₹1 so quick
setup and POS trials work immediately; replace those placeholder prices before
live service. Updating the price alone does not put a paused item on POS; the
item must also be resumed. After a price update on a paused item, ROS offers
**Resume selling**.

1. Unlock as Owner or Manager.
2. Optionally open **Menu** and import the **common Indian starter menu**, then
   open **POS** to try selling at the ₹1 placeholders.
3. When ready for real service, open **Menu**, find each item, choose
   **Update price**, enter the selling price and a short reason.
4. If an item was paused, choose **Resume selling** when prompted (or open
   Manage again and choose **Resume selling**).
5. Unlock as Owner, Manager, or Cashier and open **POS**.
6. Tap items to add them to **Current order**. Adjust quantities as needed.
7. Choose **Takeaway** or **Dine in**.
8. Choose **Payment received by**: Cash, Card, UPI, or Split.
9. Choose **Record … sale**. For Split, enter amounts that add up exactly to
   the order total.

ROS records the local invoice and payment entries. Card and UPI here mean
“payment received by that method,” not a payment-gateway charge.

## Owner recovery, reinstall, and local restaurant history

Community recovery must be fully local. There is no BIP39 seed phrase: the
Owner creates a distinct **recovery passphrase** (24–64 characters) used to
reset the Owner PIN and to wrap the portable recovery envelope.

| Owner situation | Safe action | Result |
| --- | --- | --- |
| Owner forgot the Owner PIN but still has the recovery passphrase | **Reset Owner PIN** | Passphrase proof permits a new Owner PIN on that restaurant profile; data is preserved and the event is audited. |
| Device was lost, replaced, or ROS was uninstalled | **Restore existing restaurant** | Select a portable backup (`.rosbackup`), its recovery envelope (`.rosrecovery`), and enter the recovery passphrase. ROS verifies and restores into a clean profile without overwriting another live profile. |
| Owner forgot both PIN and recovery passphrase on this device | **Start a new restaurant** | Creates a **new empty** local restaurant profile. The previous encrypted profile stays on the device in **local restaurant history**. It is not opened, rewritten, or re-owned. |
| Owner later remembers the PIN or passphrase for an old profile | **Open from history** | Unlock with the remembered PIN, or use **Forgot Owner PIN** with the passphrase on that profile. New profiles never become Owner of an old database without a secret. |

Daily unlock always uses a staff **PIN**. The recovery passphrase is not the
daily login password; it is only for Owner PIN recovery and portable restore.

The first two paths require an owner-created recovery passphrase. It is
separate from the shorter daily Owner PIN and is never stored by ROS or
Gotigin. The portable backup contains an encrypted database snapshot; the
recovery envelope wraps the database key with the recovery passphrase. Both
files and the passphrase are required for clean-install restoration.

During onboarding on a new Hub profile, ROS must guide the Owner to:

1. Set and confirm the Owner PIN.
2. Set and confirm a distinct recovery passphrase (verifier stored locally at
   onboarding, not only at first portable backup).
3. Create a portable backup and recovery envelope.
4. Save both files outside ROS's app-data directory, such as an encrypted
   external drive, a secure owner-controlled cloud folder, or another managed
   backup location.
5. Verify the backup and acknowledge that Gotigin cannot recover a lost
   passphrase or database key.

ROS should continue to display an Owner-only recovery-readiness warning until
a portable backup has been successfully created and verified. It must warn
before an uninstall operation that normal app uninstall can remove local app
data, including restaurant history on that device.

The locked/recovery screen must clearly offer:

1. **Unlock** with a staff PIN (Owner, Manager, Cashier, or Kitchen on Hub;
   roles as authorized on Client).
2. **Forgot Owner PIN** — recovery passphrase plus a new PIN; no data loss on
   that profile.
3. **Restore existing restaurant** — portable backup, envelope, and recovery
   passphrase into a new profile slot.
4. **Start a new restaurant** — creates a new empty profile; requires explicit
   confirmation that the previous profile stays locked in history until a
   secret returns. It must never silently overwrite or re-own a recoverable
   profile.
5. **Open another restaurant** — choose a profile from local history when more
   than one exists.

Support may explain these steps but must never ask for or receive a PIN,
recovery passphrase, SQLCipher key, or raw restaurant database.

Operators should follow the plain-language
[Community Edition user guide](../runbooks/community-user-guide.md) for
first-run and daily use.

## Security and data rules

- The encrypted database key stays in operating-system secure storage; Flutter
  does not receive it.
- PINs, recovery passphrases, database keys, and credential verifiers are not
  logged, exported in diagnostics, or synchronized.
- A fresh install, new local profile, or portable restore must never overwrite
  a live local database for another profile. Restore first verifies encryption,
  integrity, schema, and backup checksum.
- Losing all credentials and recovery artifacts for a profile leaves that
  profile locked. The Owner may start a **new** empty profile on the same
  device; that is not a vendor bypass and does not unlock the old ciphertext.
  A vendor master unlock would defeat the product's owner-controlled security
  model.
- Community is offline-capable, not invulnerable to an owner with unrestricted
  operating-system access. ROS must not claim otherwise.

## Community release acceptance

Before declaring Community Edition available on a platform, record evidence
for the platform-specific release gates and this functional path:

1. Install a clean build, choose Community, choose Hub (or Client when pairing),
   and create a restaurant, Owner PIN, recovery passphrase, and verified
   portable recovery kit.
2. Verify locked-state redaction, Owner/Manager/Cashier/Kitchen authorization
   **including unlock on the Hub device**, a staff PIN reset by Owner,
   throttled invalid PIN attempts, and automatic session expiry. Confirm the
   Hub continues serving clients while the Hub UI is locked or unlocked as a
   non-Owner role.
3. Complete the stated local restaurant loop offline; restart and confirm
   invoices, reports, KDS, inventory, and audit records remain correct.
4. Verify a portable restore onto a clean installation and compare required
   report totals. Verify forgotten-Owner-PIN recovery with the passphrase.
5. Confirm **Start a new restaurant** creates a new empty profile, leaves the
   prior profile in local history, and does not re-own or overwrite it. Confirm
   the old profile can be opened again after remembering the PIN or passphrase.
6. With two counters and one KDS, verify pairing, revocation, duplicate-command
   handling, reconnect with missed events, Owner PIN-reset propagation, Hub
   restart, and verified restore to a replacement Hub.
7. Perform the platform's signed install/update/uninstall, secure-store,
   encrypted-database, and release-artifact checks. On touch devices, include
   both phone and tablet layouts; on desktop, include keyboard workflows.

The detailed technical policy is in [ADR 0004](../adr/0004-rust-owned-local-key-store.md),
[ADR 0005](../adr/0005-portable-recovery-envelope.md),
[ADR 0007](../adr/0007-credential-recovery.md),
[ADR 0010](../adr/0010-edition-data-topology-and-switching.md), the
[local staff-session contract](../contracts/local-staff-session-v1.md), and
the [release-verification runbook](../runbooks/release-verification.md).
