# Owner, manager, cashier, and kitchen quick guides

For the full walkthrough (first-run, POS, Kitchen, Menu, backups, and recovery),
read the **[Community Edition user guide](community-user-guide.md)**.

## Owner

1. Install the desktop build, choose **Community**, choose **Hub** (even on a
   single-device restaurant), and create the restaurant + Owner PIN + recovery
   passphrase.
2. Enroll Manager/Cashier/Kitchen staff with unique PINs. Those roles may
   unlock on the Hub device; the Hub keeps serving LAN clients while locked or
   while unlocked as a non-Owner role.
3. Configure tax rates and menu products (with optional recipes/stock). Import
   the starter menu if you want a fast trial — items start at ₹1 and selling.
4. Create a portable backup + recovery envelope before the first busy service;
   store both files outside the app data directory.
5. Review **More** daily; close the UTC accounting day after reconciliation.
6. If you forget both Owner PIN and recovery passphrase, start a **new**
   restaurant profile rather than deleting files by hand. The old profile stays
   in local history until you remember a secret or restore from a portable kit.

## Manager

1. Unlock with Manager PIN.
2. Oversee POS discounts, sold-out toggles, inventory receipts, and expenses.
3. Approve refunds/voids as the second person when a cashier/manager requests them.
4. Never share your PIN; approval always uses a distinct actor.

## Cashier

1. Unlock with Cashier PIN.
2. Take dine-in/takeaway orders, send to Kitchen, collect cash/card/UPI/split.
3. Ask an Owner/Manager to approve refunds or voids — you cannot self-approve.

## Kitchen

1. Unlock with Kitchen PIN.
2. Progress tickets only; prices and payment data are never shown.
3. Acknowledge stop-work notices for cancelled tickets.
