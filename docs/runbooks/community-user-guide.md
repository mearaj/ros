# Community Edition user guide

This guide is for restaurant owners and staff who use **Restaurant Operating
System (ROS) Community Edition**. It explains the real screens and buttons in
the app today, in the order you meet them.

Read the short section **Two ideas that clear most confusion** before you
start. Most first-run frustration comes from mixing those up.

---

## What Community Edition is

Community Edition is a **local-first** restaurant system for **one branch**:

- Free forever — no Gotigin account, subscription, or internet required for
daily work.
- Your menu, orders, kitchen tickets, staff, and money records live in an
**encrypted database on this computer**.
- Gotigin **cannot** reset your Owner PIN, recover your passphrase, or unlock
your database. You stay in control of recovery.

If you have only one computer, that computer is still the **Hub**. You take
orders and run kitchen on the same machine.

---



## Two ideas that clear most confusion



### 1. Daily PIN ≠ recovery passphrase


| Secret                              | What it is for                                                          | Shape            |
| ----------------------------------- | ----------------------------------------------------------------------- | ---------------- |
| **Staff PIN** (including Owner PIN) | Unlock the app every day                                                | 6–12 digits      |
| **Recovery passphrase**             | Reset a forgotten **Owner** PIN, and unlock a **portable recovery kit** | 24–64 characters |


You type the **PIN** to open the app. You almost never type the passphrase
during normal service. Write the passphrase down and store it somewhere safe
**outside** this app (password manager, sealed envelope, encrypted drive).

### 2. Device role (Hub) ≠ who is logged in (staff)


| Concept           | Meaning                                                                    |
| ----------------- | -------------------------------------------------------------------------- |
| **Hub / Client**  | Chosen once at setup. **Hub** owns the restaurant database on this device. |
| **Staff session** | Who unlocked just now: Owner, Manager, Cashier, or Kitchen                 |


Locking the screen or unlocking as Cashier does **not** “turn off” the Hub.
Closing the app window or powering off the computer does.

On a one-computer restaurant: choose **Hub**, then unlock as Owner, Cashier,
or Kitchen as needed on that same machine.

---



## First-time setup (follow this order)

Do these steps once on a clean install.

### Step 1 — Choose Community

Screen: **Choose your edition**

1. Select **Community**.
2. Tap **Continue**.

(**Paid** is reserved for later Professional / Enterprise licensing. For this
guide, stay on Community.)

### Step 2 — Choose Hub

Screen: **Choose this device’s role**

1. Select **Hub** (even if this is your only computer).
2. Tap **Continue to security**.

Use **Back** if you need to change the edition.

> **Client** is for a second device that will join a Hub later. Pairing UI is
> still limited; for a first install, always use **Hub**.



### Step 3 — Create the restaurant

You land in **Menu** with setup open.

Screen title: **Create your local restaurant workspace**

Fill in:


| Field                          | What to enter                   |
| ------------------------------ | ------------------------------- |
| **Restaurant or company name** | Legal / trading name            |
| **First branch name**          | e.g. Koramangala, Main outlet   |
| **Operating currency**         | INR (current Community default) |
| **Time zone**                  | Usually `Asia/Kolkata`          |


Tap **Create local workspace**.

### Step 4 — Secure the restaurant (Owner PIN + passphrase)

Screen: **Secure your restaurant**

1. **Owner PIN** — 6 to 12 digits (daily unlock).
2. **Confirm owner PIN**.
3. **Recovery passphrase** — 24 to 64 characters (not the same as the PIN).
4. **Confirm recovery passphrase**.
5. Tap **Secure this device**.

Use **Back** only if you need to revise edition / Hub. After this step, the
Owner account exists and the app unlocks as Owner.

**Write down the passphrase now.** Without it, forgetting the Owner PIN is
much harder to fix.

### Step 5 — Import a starter menu (recommended for a first try)

Still in **Menu**:

1. Tap **Starter menu**.
2. Read the dialog **Import common starter menu?**
3. Tap **Import menu**.

What you get:

- Common Indian categories and dishes with photos.
- Every imported item is **ready to sell** at a **₹1 placeholder price** so you
can try POS immediately.
- Before real customers, open each item and set the real price (see
[Update prices](#update-prices-before-real-service)).

Tap **Not now** only if you prefer to add every category and item by hand.

### Step 6 — Try one sale

1. Open **POS**.
2. Tap a dish → add it to the order.
3. Choose **Takeaway** or **Dine in**.
4. Choose **Payment received by**: **Cash**, **Card**, **UPI**, or **Split**.
5. Tap **Record … sale**.

If POS says nothing is ready to sell, go back to **Menu** and check items show
**selling now** (not **paused — not on POS**).

### Step 7 — Create a portable recovery kit (do this before busy service)

Unlock as **Owner** → open **More** → under **Backup & recovery**:

1. Tap **Create portable recovery kit**.
2. Enter the **same recovery passphrase** (or another 24–64 character phrase you
  will keep with the kit).
3. Confirm and create the kit.

ROS writes two files under the app’s `portable-backups` folder:

- an encrypted backup, and
- a `.rosrecovery` envelope.

**Copy both files off this computer** (USB drive, encrypted cloud folder you
control, etc.). Gotigin cannot recreate them.

Also useful on the same screen:

- **Create verified local backup** — snapshot beside live data on this machine.
- **Verify local backup integrity** / **Restore verified backup** — same-install
checks (restore writes beside live data; it does not silently overwrite).

---



## The five main screens

Bottom navigation (or the left sidebar on a wide window):


| Tab          | What it is for                                                          | Who can open it         |
| ------------ | ----------------------------------------------------------------------- | ----------------------- |
| **Overview** | Status, open orders, kitchen queue, shortcut to start an order or setup | Any unlocked staff      |
| **POS**      | Take orders and record payment                                          | Owner, Manager, Cashier |
| **Kitchen**  | Kitchen Display — prepare tickets (no prices)                           | Owner, Manager, Kitchen |
| **Menu**     | Categories, products, prices, images, taxes, stock shortcuts            | Owner, Manager          |
| **More**     | Sales report, day close, backups, staff, expenses, drawer, diagnostics  | Owner, Manager          |


If a tab is blocked, the app explains why (for example Cashier opening **Menu**).

**Lock:** use the small lock button (tooltip **Lock staff session**) when you
leave the counter. Sessions also expire after about **15 minutes** of
inactivity. Closing and reopening the app always asks for a PIN again.

---



## Unlock screen (every day after setup)

Screen: **Unlock Restaurant Operating System**

1. Choose **Staff member**.
2. Enter that person’s **PIN**.
3. Tap **Unlock**.

Also on this screen:


| Link                     | When to use it                                                                                           |
| ------------------------ | -------------------------------------------------------------------------------------------------------- |
| **Forgot Owner PIN**     | You remember the recovery passphrase; you need a new Owner PIN. Restaurant data stays.                   |
| **Restaurant history**   | Open another restaurant profile on this device, or **Start new restaurant**.                             |
| **Restore portable kit** | New/clean device or reinstall: restore from backup + `.rosrecovery` + passphrase into a **new** profile. |




### Reset Owner PIN

1. Tap **Forgot Owner PIN**.
2. Enter **Recovery passphrase**.
3. Enter **New Owner PIN** and confirm.
4. Tap **Reset Owner PIN**.

Too many wrong passphrase attempts are temporarily blocked — wait and retry.

### Start a new restaurant (forgot both PIN and passphrase)

1. Tap **Restaurant history**.
2. Tap **Start new restaurant**.
3. Enter a **Restaurant label**.
4. Confirm.

This creates a **new empty** restaurant. The old encrypted restaurant **stays
on the device** in history. It is not deleted and is not taken over. If you
later remember the old PIN or passphrase, open that profile from history.

---



## Who can do what


| Action                                           | Owner | Manager | Cashier | Kitchen |
| ------------------------------------------------ | ----- | ------- | ------- | ------- |
| Unlock on the Hub computer                       | Yes   | Yes     | Yes     | Yes     |
| Take orders / record payment (POS)               | Yes   | Yes     | Yes     | No      |
| Run Kitchen Display                              | Yes   | Yes     | No      | Yes     |
| Edit menu, taxes, stock, expenses                | Yes   | Yes     | No      | No      |
| Add/revoke staff, rotate staff PINs              | Yes   | No      | No      | No      |
| Portable recovery kit / restaurant history admin | Yes   | No      | No      | No      |
| Export financial CSV                             | Yes   | No      | No      | No      |
| Approve refunds / voids (second person)          | Yes   | Yes     | No      | No      |


**Staff who forget their PIN** ask the Owner: **More** → **Manage local staff**
(or **Menu** → **Team & PINs**) → **Rotate PIN**. Staff cannot reset the Owner
PIN themselves.

---



## Menu — build and maintain what you sell

Open **Menu** as Owner or Manager.

### Add a category

1. Tap **New category** / **Add category**.
2. Name it (e.g. Beverages).
3. Optional: set category artwork (**Choose app category artwork**, Gotigin
  photos, or your own image with rights confirmation).



### Add a menu item

1. Enter **Menu item** name and **Price (INR)**.
2. Optional photo.
3. Tap **Add menu item**.

New items are available to sell unless you pause them.

### Update prices before real service

Starter-menu items arrive at **₹1** on purpose.

1. Find the item → open manage actions.
2. Tap **Update price**.
3. Enter the real price and a short reason.
4. Save.

If an item was **paused**, price alone does not put it on POS. Choose
**Resume selling** when asked (or **Resume selling** from manage actions).

### Pause / resume (sold out)

- **Mark sold out** — item disappears from POS; history stays.
- **Resume selling** — item returns to POS (must have price greater than zero).

Status text under each product: **selling now** or **paused — not on POS**.

### Tax treatment

Per item: **No tax**, **Exclusive**, or **Inclusive** (from manage actions /
tax controls).

### Modifiers

Add options (e.g. spice level) from the item’s **Modifiers** actions. Options
are snapshotted onto orders when sold.

---



## POS — take an order

Unlock as Owner, Manager, or Cashier → **POS**.

### Happy path

1. Tap dishes to add them to **Current order** (set modifiers if prompted).
2. Adjust quantities.
3. Optional: kitchen note / customer (where offered).
4. Choose **Takeaway** or **Dine in** (dine-in asks for **Table name or number**).
5. Choose **Payment received by**: **Cash**, **Card**, **UPI**, or **Split**.
6. Owner/Manager may apply an **Order discount** before payment.
7. Tap **Record Cash sale** / **Record Card sale** / **Record UPI sale** / record
  split.

**Card** and **UPI** mean “customer paid by that method.” ROS is not charging a
payment gateway.

### Hold and kitchen

- **Hold open order** — keep working later.
- **Send to kitchen** — kitchen sees the ticket.

After send, the cart is locked for cashiers. Owner/Manager can
**Reopen new revision** or **Cancel sent order**.

### Split payment

Choose **Split**, allocate Cash/Card/UPI amounts that **exactly** equal the
total, then confirm.

### Empty POS?

- No products → finish setup / import starter menu / add items in **Menu**.
- Products exist but all paused → **Resume selling** in **Menu**.

---



## Kitchen Display

Unlock as Owner, Manager, or Kitchen → **Kitchen**.

- Tickets show table or **Takeaway**, items, and optional kitchen instructions.
- **No prices or payment details** appear here.

Progress a ticket:

1. **Start preparing** (from new)
2. **Mark ready**
3. **Complete ticket**

If an order was cancelled at the counter:

- You see a stop-work notice.
- Tap **Acknowledge cancellation** so it leaves the active queue.

---



## More — money, team, and recovery

Unlock as Owner or Manager → **More** (**Local Sales Report**).

### Day numbers

- Totals for the selected **UTC accounting day** (Cash, Card, UPI, refunds,
discounts, tax, expenses).
- **Top items** and **Recent invoices** (view receipt; refund/void as allowed).

**Choose UTC accounting day** if you need another day. Display time zone is for
reading clocks; accounting days stay on UTC.

### Close the day (Owner or Manager)

1. Tap **Close UTC accounting day**.
2. Enter a reason.
3. Confirm **Close day**.

Closed days are not reopened in the app. Do this after you have reconciled.

### Export (Owner only)

**Export verified financial CSV** downloads trusted local totals for the day.

### Refunds and voids

From an invoice: **Record refund** or **Void invoice**.

These need a **second** active Owner or Manager PIN (not the same person
approving their own request). Cashiers cannot self-approve.

### Expenses and cash drawer

- **Open expense ledger** → **Record expense** (category, description, amount,
Cash/Card/UPI).
- **View cash drawer** → open with **Opening float (₹)**; later close with
**Counted cash (₹)**.



### Staff (Owner only)

**Manage local staff**:

- **Add local staff** — name, role (**Cashier** / **Kitchen** / **Manager**),
first PIN.
- **Change role**, **Rotate PIN**, **Revoke access**.



### Diagnostics (Owner)

**Local diagnostics** — save a technical pack on device; optional share with
Gotigin. Packs must not contain PINs, passphrases, or database keys.

---



## Backup and recovery cheat sheet


| Situation                                    | What to do                                                                              |
| -------------------------------------------- | --------------------------------------------------------------------------------------- |
| Normal day                                   | Unlock with staff PIN                                                                   |
| Forgot Owner PIN, have passphrase            | Unlock screen → **Forgot Owner PIN**                                                    |
| Forgot staff (non-Owner) PIN                 | Owner rotates PIN in **Manage local staff**                                             |
| New computer / reinstall                     | Unlock screen → **Restore portable kit** (backup + `.rosrecovery` + passphrase)         |
| Forgot PIN **and** passphrase on this device | **Restaurant history** → **Start new restaurant** (old profile stays locked in history) |
| Later remember old PIN/passphrase            | **Restaurant history** → **Open** that profile                                          |
| Before busy season                           | Owner creates portable kit and stores both files off-device                             |


Same-install **verified local backup** is for snapshots beside the live
database. **Portable recovery kit** is for moving to another machine or a
clean install.

---



## Common questions



### Why is everything ₹1 after import?

So you can try POS without typing dozens of prices first. Change prices in
**Menu** before real service.

### I updated the price but the item still isn’t on POS

The item is probably **paused**. Use **Resume selling**.

### I chose Client by mistake

Use **Back** during first-run if you still can. For a single computer you want
**Hub**. Client is for joining another Hub later.

### Why can’t the cashier open Menu or More?

By design. Catalogue and money admin are Owner/Manager. Cashier uses **POS**.
Kitchen uses **Kitchen**.

### Does locking the app stop orders on other devices?

No — locking only ends the **staff session** on this screen. The Hub role is
separate. (Closing the app entirely does stop this computer’s Hub process.)

### Will Gotigin support recover my restaurant if I lose everything?

No. Support can explain these steps but must never take your PIN, passphrase,
or database files. Keep a portable kit off-device.

### I ran the developer uninstall script

`bash scripts/uninstall-local-ros.sh` wipes the **local development** install on
this machine (including Owner). The next launch is a full first-run again. That
script is for developers, not for restaurant recovery.

---



## Suggested first hour checklist

1. [ ] Community → Hub
2. [ ] Create restaurant workspace
3. [ ] Set Owner PIN + recovery passphrase (written down safely)
4. [ ] Import starter menu
5. [ ] Record one Cash takeaway sale on POS
6. [ ] Open Kitchen and advance that ticket (unlock as Kitchen or Owner)
7. [ ] Update two item prices in Menu
8. [ ] Add one Cashier staff member
9. [ ] Create portable recovery kit and copy both files off this computer
10. [ ] Lock the session; unlock again as Cashier and take a second order

When that checklist feels natural, you understand the product loop Community
Edition is built around.

---

## Role quick guides

### Owner

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

### Manager

1. Unlock with Manager PIN.
2. Oversee POS discounts, sold-out toggles, inventory receipts, and expenses.
3. Approve refunds/voids as the second person when a cashier/manager requests them.
4. Never share your PIN; approval always uses a distinct actor.

### Cashier

1. Unlock with Cashier PIN.
2. Take dine-in/takeaway orders, send to Kitchen, collect cash/card/UPI/split.
3. Ask an Owner/Manager to approve refunds or voids — you cannot self-approve.

### Kitchen

1. Unlock with Kitchen PIN.
2. Progress tickets only; prices and payment data are never shown.
3. Acknowledge stop-work notices for cancelled tickets.

---

## Related documents

- Product rules: [Community Edition](../editions/community.md)
- Local developer reset: [Local development](local-development.md)
- Doc index: [docs/README.md](../README.md)

