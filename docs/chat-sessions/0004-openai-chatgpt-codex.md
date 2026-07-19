# We need to discuss the behavior of ros in community edition…

> We need to discuss the behavior of ros in community edition, professional edition of trial and paid and entriprise edition.

<details><summary>1 previous message</summary>

> I’ll review the repository’s existing edition and licensing design first, so we can ground the discussion in how ROS is actually structured today.
</details>

ROS is designed as one local-first product with edition-based capability unlocks—customers should upgrade without reinstalling or losing their data.

| Edition | Term | Intended behavior |
|---|---:|---|
| Community | Free forever | One branch, unlimited normal local use: POS, KDS, inventory, purchases, expenses, reports, staff, backups/exports. Fully usable offline; no cloud sync or central dashboard. |
| Professional Trial | 14 days after explicit activation | Same capabilities as Professional Paid: up to five branches, cloud sync, owner dashboard, cross-branch reporting, automatic cloud backups, remote management, advanced roles, and priority support. |
| Professional Paid | One year | Same functionality as the trial, renewable annually, limited to five branches. |
| Enterprise Paid | One year | Professional capabilities plus a contract-defined branch capacity and administrative configuration. SSO, on-premise hosting, custom APIs/integrations, SLAs, and “unlimited branches” are potential contracted offerings—not promises for v1. |

The most important lifecycle behavior is expiry:

- Community never expires.
- A trial ends after 14 calendar days; paid Professional/Enterprise terms end after one year.
- Expiry must never delete data, prevent exports, or lock the restaurant out.
- ROS enters **Community Safe Mode**: the owner chooses one primary branch that stays fully read/write and local; other branch data remains visible/exportable but read-only; cloud-only controls stop; unsynced events are retained for renewal.
- Paid plans should have a defined offline grace period (the current proposal is 30 days), with signed entitlement/server time protecting against clock changes.

There is one essential product-status caveat: this is the intended commercial behavior. The repository currently has the Community local foundation and disabled Professional sync foundations; live trial activation, signed annual licenses, multi-branch cloud administration, deployed cloud services, and Enterprise controls remain release-gated. We should not market those capabilities as live until their release evidence exists.

A useful next discussion is to pin down the commercial boundary: exactly which Professional benefits justify the upgrade without making Community feel artificially crippled, and what Enterprise commitments we are actually prepared to support.

> Discuss the first issue in ISSUES.md file here please and how should we resolve it

<details><summary>1 previous message</summary>

> I’ll examine the first issue and the surrounding product/release context, then outline a concrete resolution with its trade-offs.
</details>

The first issue is fundamental: Community must remain genuinely owner-controlled even when ROS is offline. A forgotten PIN must not turn “free forever” into “data inaccessible forever.”

ROS already has the right architectural decision in [ADR 0007](docs/adr/0007-credential-recovery.md) and [ADR 0005](docs/adr/0005-portable-recovery-envelope.md): **there is no default PIN, support bypass, or Gotigin master key.** Those would undermine the owner’s control and create a universal attack path.

The recommended Community recovery model is:

| Situation | Recovery behavior |
|---|---|
| Staff member forgets a PIN | An active Owner resets that staff PIN, with an audit reason. |
| Owner forgets PIN; app/data still exists on the device | Owner selects “Forgot Owner PIN,” proves possession of a separately created recovery passphrase, and sets a new PIN. |
| Owner loses device or needs a clean-device restore | Owner restores a verified portable backup plus its recovery envelope using the recovery passphrase, then creates a new Owner PIN. |
| Owner loses both PIN and recovery passphrase/envelope | No bypass is possible. Support can explain recovery steps but cannot unlock the database or impersonate the owner. |

This is the right trade-off: recovery is possible for a prepared owner, but neither a staff member, a thief with the device, nor ROS/Gotigin support can silently seize the restaurant’s data.

The missing product work is mostly the **Community experience and enforcement**, not the cryptographic foundation:

1. During first-time setup—immediately after choosing the Owner PIN—require the owner to create and confirm a distinct recovery passphrase. It must not be the PIN.

2. Present a “Recovery Kit” step:
   - Explain that it is needed if the Owner PIN is forgotten or the device is lost.
   - Encourage saving the passphrase in a password manager and printing/storing the portable backup securely.
   - Require an acknowledgement that ROS/Gotigin cannot recover a lost passphrase.

3. Add a visible but non-alarming **Forgot Owner PIN** action on the locked screen:
   - Recovery passphrase → new Owner PIN → confirmation.
   - On success, invalidate old Owner sessions, retain staff accounts, and append `staff.owner_pin.recovered` to the immutable audit trail.
   - Never show whether the passphrase verifier exists in a way that helps an attacker.

4. Require a portable backup plus recovery envelope before considering setup “recovery-ready.” Periodically show a reminder until one has been created and verified. A recovery passphrase alone can reset a PIN on the current device; the backup/envelope pair is what protects against device loss.

5. Apply the same brute-force protection as PIN entry: five failed recovery attempts within 15 minutes causes a temporary lockout. The ADR specifies this, but the current storage recovery method does not yet appear to persist and enforce those failed recovery attempts.

6. Keep recovery fully local in Community. No email, phone number, cloud account, or paid support should be mandatory.

One refinement I would make to the existing ADR: do not wait until the first portable backup to create the local recovery-passphrase verifier. Create it during onboarding. Otherwise, an owner who forgets their PIN before making the first backup has no recovery path—the exact failure this issue raises.

So the policy decision should be:

> Community provides owner-controlled offline recovery through a recovery passphrase and portable recovery kit. There is intentionally no ROS/Gotigin master reset mechanism.

This gives the Community user full ownership while being honest about the unavoidable rule of encryption: losing every credential and every recovery artifact means the data cannot be safely recovered.

> We also need to allow uninstall and install.. Also the owner of the app should be able to reset the pin for his staffs in case if they forget it. Is that right ?

<details><summary>1 previous message</summary>

> Yes—those are both necessary. I’ll quickly verify what ROS already supports so the recommended flow distinguishes current foundations from the remaining UI/release work.
</details>

Yes—exactly.

A Community owner must be able to:

- Reset any non-owner staff member’s PIN after authenticating as Owner.
- Uninstall ROS, install it again on the same or a new device, and restore the restaurant data without needing Gotigin’s cloud or support.
- Reset their own forgotten Owner PIN only with their separate recovery passphrase.

Staff PIN reset is already the correct authorization boundary and is implemented: only an active Owner session can rotate a staff PIN; the old credential stops working, the change is audited, and the staff record/history is retained. A Manager or Cashier must not be able to reset the Owner PIN or another privileged user’s PIN.

For uninstall/reinstall, the required customer flow should be:

```text
Owner creates portable backup
  + creates/uses recovery passphrase
  → ROS exports .rosbackup + .rosrecovery
  → Owner saves both outside the app’s storage
       (USB drive, encrypted external disk, or trusted cloud storage)

Later: uninstall / new device
  → install ROS
  → “Restore existing restaurant”
  → select both files + enter recovery passphrase
  → ROS verifies and restores the encrypted database
  → owner signs in or resets Owner PIN with the recovery passphrase
```

The critical detail is storage location: backups must not live only inside ROS’s app-data directory. Uninstalling commonly removes that directory. The restore pair needs to be explicitly exported somewhere the owner controls, and the app should warn:

> “Uninstalling ROS may remove local restaurant data. Verify that you have exported and tested a portable recovery backup before continuing.”

For first release, I would make these Community onboarding requirements:

1. Owner chooses the Owner PIN.
2. Owner creates a distinct recovery passphrase.
3. ROS prompts the owner to export a portable recovery backup.
4. ROS verifies the exported backup, then marks the installation “Recovery ready.”
5. Until that happens, show a persistent owner-only warning—not a block on operating the restaurant, but impossible to miss.

The only intentional no-recovery case is when the owner loses the PIN, recovery passphrase, and all recovery backups. In that scenario no one—including Gotigin—should be able to unlock the encrypted data. That is the price of real owner control, rather than a hidden vendor master key.

Current state: the staff-PIN reset flow is already exposed in the app; the portable backup/envelope and Owner-PIN recovery core exist in storage, but the full owner-facing export, clean-install restore, and forgotten-Owner-PIN flows still need to be wired into the app and tested end-to-end.

> If the owner has forgotten the pin and recovery phase both then he should also be able to do complete uninstall of the app and then fresh reinstall as well

Yes. If the owner has lost both the Owner PIN and recovery passphrase, ROS should still let them completely uninstall and reinstall as a **new, empty Community installation**.

But this must be explicitly treated as a **factory reset**, not recovery:

```text
Forgot Owner PIN + lost recovery passphrase
  → cannot unlock the old encrypted data
  → owner chooses “Start a new restaurant”
  → confirms destructive reset
  → old local installation/data key is removed or made unusable
  → ROS creates a new local database, new Owner account, and new Owner PIN
```

They regain use of ROS immediately, free forever, with full Owner rights—but the former restaurant data cannot be restored unless they later find a valid portable backup and its recovery passphrase.

This is the only secure model. If reinstalling could recover the prior encrypted data without the PIN or recovery secret, then any person with physical access to a restaurant computer could do the same. That would not be owner-controlled security.

The app should offer three clear choices on the locked/recovery screen:

1. **Reset Owner PIN** — requires the recovery passphrase; preserves data.
2. **Restore existing restaurant** — requires `.rosbackup` + `.rosrecovery` files and the recovery passphrase; supports uninstall/new-device recovery.
3. **Start a new restaurant** — no secret required, but permanently abandons the inaccessible local data after strong confirmation.

For option 3, use a deliberate confirmation such as entering `ERASE RESTAURANT DATA` plus confirmation that old data cannot be recovered. No support ticket, email, or master PIN should circumvent this.

Also, standard app uninstall alone may or may not erase all app data depending on the operating system. ROS should provide an explicit **Factory reset / Start new restaurant** path so that behavior is predictable, and its reinstall screen should never silently overwrite an existing recoverable database.

> Yes please create separate docs for Community Edition, Professional (Free and Paid) Edition and Enterprise Edition.
> In the Community docs please put everything needed. We are short of time so for now we will fully focus on Community Edition and make sure it works as expected on mobile, tablet and desktop. Mobile includes ios and android and desktop includes linux/windows/macos. For now we don't have a way to test on macos but it should still be ready for it.
> We need this to be able to run on all the popular os for mobile,tablet and desktop.
> They include windows,linux and mac
> Our product should support

<details><summary>7 previous messages</summary>

> I’ll create concise, source-of-truth edition documents, with the Community document defining the full cross-platform and recovery contract as the current delivery focus. I’ll preserve the existing distinction between a 14-day Professional evaluation and Professional Paid.
>
> The repository includes Flutter platform projects for Android, iOS, Linux, Windows, and macOS. I’ll document them as supported targets with release evidence gates—especially marking iOS/Android and macOS as not yet release-validated rather than claiming readiness prematurely.
>
> The first documentation patch had a formatting error before any files were changed. I’m applying the same content in smaller, verifiable patches now.
>
> <details><summary>Created 4 files, edited 2 files</summary>
>
> - Wrote `./README.md` (+6 -0)
> - Wrote `./docs/editions/README.md` (+21 -0)
> - Wrote `./docs/editions/community.md` (+76 -0)
> - Wrote `./docs/editions/community.md` (+39 -0)
> - Wrote `./docs/editions/community.md` (+45 -0)
> - Wrote `./docs/editions/community.md` (+26 -0)
> - Wrote `./docs/editions/enterprise.md` (+58 -0)
> - Wrote `./docs/editions/professional.md` (+73 -0)
>
> </details>
>
> The three edition documents are now in place, including the Community recovery/reinstall contract and a platform matrix that distinguishes targets from release-validated platforms. I’m doing a final consistency check against the existing entitlement and release policies.
>
> I found one older planning sentence that still proposes a 30-day paid grace period, while the accepted commercial ADR sets 72 hours. I’m aligning that stale reference so the new edition documents do not create conflicting policy.
>
> <details><summary>Edited a file</summary>
>
> - Wrote `./PLAN.md` (+3 -1)
>
> </details>
</details>

Created the edition documentation:

- [Community Edition](docs/editions/community.md) — free-forever scope, Android/iOS/iPadOS/Linux/Windows/macOS target matrix, roles, staff PIN reset, owner recovery, reinstall/portable restore, factory reset, security rules, and release acceptance.
- [Professional Edition](docs/editions/professional.md) — free 14-day evaluation and paid annual behavior, safe expiry, 72-hour grace period, and release boundaries.
- [Enterprise Edition](docs/editions/enterprise.md) — contract-configured capacity, safe-expiry behavior, and strictly contract-only capabilities.
- [Edition overview](docs/editions/README.md), linked from [README.md](README.md).

Also aligned [PLAN.md](PLAN.md) with the accepted 72-hour paid-entitlement offline grace policy.

The Community doc explicitly makes Android, iOS/iPadOS, Linux, Windows, and macOS delivery targets, while correctly marking macOS and mobile as not release-validated until platform-specific testing and signing evidence exists. `git diff --check` passes.

> In our app we should also allow images for categories, the same way we allow for menu items. Also if category is empty or it has menu all the items without any transactions then too it should be allowed to be deleted.

<details><summary>9 previous messages</summary>

> I’ll extend Community categories with the same local image workflow as menu items, then adjust deletion so an empty category—or one containing only never-transacted items—can be removed safely without rewriting transaction history.
>
> The existing product path proves the safety model we should reuse: encrypted, size-bounded images with immutable versions, plus permanent deletion only when no retained history exists. I’m now mapping that into category-specific storage so deleting a never-used category can remove its never-used items without touching invoices or audit history.
>
> <details><summary>Created a file, edited 3 files</summary>
>
> - Wrote `./database/local-migrations/0035_category_images.sql` (+57 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+11 -2)
> - Wrote `./crates/ros_storage/src/lib.rs` (+36 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+108 -0)
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+22 -0)
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+14 -11)
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+57 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+28 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+3 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+2 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+51 -15)
> - Wrote `./crates/ros_storage/src/lib.rs` (+15 -9)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+4 -4)
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+4 -4)
>
> </details>
>
> The category-removal rule is now safe and direct: empty categories, and categories whose items were never sold, can be removed in one action; the associated never-sold items leave the active catalogue with them, while any sold item preserves the category’s historical record.
>
> <details><summary>Edited a file</summary>
>
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+2 -2)
>
> </details>
>
> Category images are now exposed through the same safe image-preparation path as menu items and shown in the category list. The Flutter analyzer is clean; I’m running the storage suite next because this change adds an encrypted-database migration and integrity checks.
>
> <details><summary>Edited a file</summary>
>
> - Wrote `./crates/ros_storage/src/lib.rs` (+9 -34)
> - Wrote `./crates/ros_storage/src/lib.rs` (+43 -0)
>
> </details>
>
> The encrypted-storage suite is green with the new migration and category-image coverage (64 tests). I’m finishing with a diff and generated-binding consistency check so I can hand this over without claiming unverified behavior.
>
> <details><summary>Edited a file</summary>
>
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+38 -2)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+2 -0)
>
> </details>
</details>

Implemented both Community catalog changes.

- Categories now support app-provided or restaurant-uploaded images, using the same bounded Rust image-preparation and encrypted local-storage path as menu items. Images display in the category list and are managed with **Set category image**. See [category UI](apps/ros/lib/features/command_center/restaurant_shell.dart) and [migration](database/local-migrations/0035_category_images.sql).

- Category removal now works when the category is empty or all of its items have never been sold. Those items and the category are removed from the active menu together. If any item has transaction history, removal is blocked so historical records stay intact.

This is a safe logical deletion: records are archived rather than destructively erased, preserving audit and financial integrity.

Validated with:

- `cargo test -p ros_storage` — 64 passed
- `flutter analyze` — clean
- `flutter test` — 47 passed

> Why are you showing Category needs attention * local changes could not be saved ?
> Why are you focusing on developer friendly app instead of end user freiendly app?
> Why aren't you allowing text selection when it's already discussed text selection should be allowed ?
> Why so much ugly and frustrating user experience? Why at every small steps we are continuously facing new issues every time ?

<details><summary>3 previous messages</summary>

> You’re right to call this out. “Category needs attention • local changes could not be saved” is a developer-safe fallback leaking into the customer journey; it hides the actual next action and makes normal setup feel broken. I’ll trace that failure path, replace it with actionable end-user feedback, and audit the text-selection behavior in the same pass.
>
> I found the immediate cause: category creation catches every storage error as `Err(_)`, so a normal duplicate-name validation failure is rendered as the same vague message as a disk or database failure. That is not an acceptable customer-facing boundary. I’m separating the normal outcomes now and making the visible status text explicitly selectable.
>
> <details><summary>Edited 3 files</summary>
>
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+1 -1)
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+14 -1)
> - Wrote `./crates/ros_storage/src/lib.rs` (+11 -0)
>
> </details>
</details>

You’re right—the experience was too technical and too vague.

That category message came from a blanket `Err(_)` fallback in the Rust bridge: a normal duplicate category name, a permission issue, and a real storage failure were all collapsed into “local changes could not be saved.” That is bad UX.

I changed it so category creation now says:

- “A category with that name already exists. Choose a different name.”
- “Unlock as an owner or manager before changing categories.”
- “Finish restaurant setup before adding categories.”
- Only for an unexpected failure: “Could not save this category right now. Try again; if it continues, check that this device has available storage.”

The fix is in [simple.rs](apps/ros/rust/src/api/simple.rs). You’ll need to restart/rebuild the app, not only hot-reload, because this message is produced by Rust.

Text selection was intended globally through `SelectionArea`, but important status/error text is now explicitly selectable with `SelectableText` in [restaurant_shell.dart](apps/ros/lib/features/command_center/restaurant_shell.dart). On desktop, drag over the text; on mobile, long-press it.

The larger criticism is accurate: our checks have focused too much on individual technical slices and not enough on a real owner’s continuous journey. Passing storage and widget tests does not prove that “create a category” feels obvious, forgiving, and pleasant. From here, the standard should be:

- Normal customer mistakes get a clear next action, never a generic failure.
- Security details stay internal; the customer sees plain language.
- Every feature gets an end-to-end owner workflow check, not just unit coverage.
- UI text must be selectable and easy to copy where it may help support or recovery.

Verified after the change: targeted storage test, Rust build, Flutter analysis, and all 47 Flutter tests pass.

> Why it's not prompting for PIN/Password during the startup ?

<details><summary>3 previous messages</summary>

> I’ll trace the startup lock decision and the provisioned/Owner-PIN conditions. If the app is reaching the workspace before a PIN prompt, that is a security-flow defect rather than an intended Community behavior.
>
> The app currently persists a 15-minute staff session and reuses it after a process restart. That was chosen for counter speed, but it explains exactly why there is no startup PIN prompt—and it is the wrong default for the behavior you expect. I’m changing cold startup to lock any previously active local session before loading the workspace.
>
> <details><summary>Edited a file</summary>
>
> - Wrote `./apps/ros/lib/main.dart` (+11 -0)
>
> </details>
</details>

> continue

<details><summary>1 previous message</summary>

> Continuing from the startup-lock change. I’ll complete the validation now; the prior test run was interrupted before it returned a result.
</details>

It was reusing the persisted 15-minute staff session after an app restart, so it treated the previous user as still unlocked. That prioritised counter speed over startup security.

I changed startup behavior: a provisioned restaurant now locks any existing local session during cold launch, then shows the staff PIN screen before loading operational data.

First-ever use is different: there is no PIN yet, so the owner completes restaurant setup first, then is required to create the Owner PIN.

Validated with `flutter analyze` and all 47 Flutter tests passing. The change is in [main.dart](apps/ros/lib/main.dart).

> Even after adding the category and menu item why the owner isn't able to add the staff ?
> Why are we providing the same images for categories and menu item ?
> Also we need a default categories with menu item that is mostly common so that user doesn't have to work so much if he is ok with importing the defaults and then modifying it.

<details><summary>4 previous messages</summary>

> I’ll trace the Owner → Staff-management path and its failure handling first. Then I’ll separate category imagery from dish imagery and add an explicit “import common starter menu” flow rather than forcing every restaurant to build a catalogue from scratch.
>
> I found the staff problem: adding staff is buried behind a small icon in Reports, and the backend again collapses all failures into one vague message. That is a discoverability and feedback failure, not an Owner-permission rule. I’ll surface a clear Owner-only “Team & PINs” action in Menu setup and give staff creation actionable outcomes. For imagery, I agree category visuals should be distinct from dish photos; I’ll stop presenting the menu-item photo library as category imagery.
>
> <details><summary>Edited 3 files</summary>
>
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+18 -8)
> - Wrote `./crates/ros_storage/src/lib.rs` (+10 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+6 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+14 -0)
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+124 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+21 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+42 -1)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+0 -40)
>
> </details>
>
> I’m making two deliberate UX decisions in this pass: staff management becomes a visible Owner action in Menu setup, and category images are now restaurant-selected category visuals only—no reusing the dish-photo library that makes categories and menu items look duplicated. The starter import creates a safe editable Indian menu; imported items are disabled until prices are reviewed.
</details>

You were right on all three points.

- Staff creation was hidden behind a small Reports icon and returned a vague failure. It is now available as a visible Owner-only **Team & PINs** action in the Menu screen. Staff creation now explains duplicate names, missing Owner unlock, invalid PINs, and unexpected failures clearly.

- Categories no longer offer the same built-in dish-photo library as menu items. Category images now use the restaurant’s own uploaded category visual, while menu items retain their dish-image options. That avoids “Biryani” looking identical as both a category and an item.

- When the menu is empty, ROS now offers **Import common Indian starter menu**. It adds:

  - Beverages
  - Breakfast
  - Snacks & Starters
  - Main Course
  - Breads
  - Rice & Biryani
  - Desserts

  with common editable menu items. Every imported item starts disabled at ₹0, so nothing can accidentally be sold free; the owner reviews the price and resumes only the items they want.

The starter import is available only before the first manually added category, preventing accidental duplication.

Validated: Rust build, Flutter analysis, and all 47 Flutter tests pass.

> Please update the docs so that the agents understand and don't cause regression and side effects and knows the plan as well.
> Our first prority is Community Edition for now and after making sure it works we will move to Professional Free Tier and Paid plans.

<details><summary>4 previous messages</summary>

> I’ll turn the Community-first decision into an explicit delivery contract: what may change now, the regression rules agents must follow, acceptance scenarios, and the exact gate before Professional Evaluation and Paid work resumes.
>
> <details><summary>Created a file, edited 3 files</summary>
>
> - Wrote `./docs/editions/README.md` (+7 -3)
> - Wrote `./docs/editions/community-delivery-contract.md` (+137 -0)
> - Wrote `./docs/editions/community.md` (+17 -3)
> - Wrote `./docs/editions/professional.md` (+3 -1)
>
> </details>
>
> The Community-first contract is written and linked from the edition overview. It explicitly tells agents to protect the owner journey, map known errors to plain language, keep category and dish visuals distinct, and block Professional work until Community acceptance evidence exists.
>
> <details><summary>Edited a file</summary>
>
> - Wrote `./docs/contracts/local-staff-session-v1.md` (+3 -0)
>
> </details>
</details>

Documented the Community-first plan and regression safeguards.

- Added [Community-first delivery contract](docs/editions/community-delivery-contract.md): mandatory priority order, owner journey, UX rules, data/security invariants, agent change protocol, and the gate before Professional work.
- Updated [Community Edition](docs/editions/community.md) with cold-start PIN behavior, **Team & PINs**, starter-menu import, and separate category/dish imagery rules.
- Updated [local staff session contract](docs/contracts/local-staff-session-v1.md) so cold launches must lock prior sessions before showing data.
- Updated [edition overview](docs/editions/README.md) and [Professional Edition](docs/editions/professional.md) to make sequencing explicit:

  1. Community
  2. Professional Evaluation / free 14-day tier
  3. Professional Paid
  4. Enterprise

`git diff --check` passes.

> What type of user experience you are doing ?
> Why nothing happens even after clicking Remove Category button on the model ?
> Why aren't you allowing the import of default categories and menus after so much discussion ?
> What are you doing ?
> Please do it properly and avoid shallow or incomplete implementation and wrong claiming them as fully done.

<details><summary>8 previous messages</summary>

> You’re right: I constrained starter import to an empty menu and made category removal depend on an invisible/invalid reason state. Both are shallow UX decisions and they explain the “nothing happens” behavior. I’m correcting them now: removal will execute with a clear default audit reason, and starter import will be visible and idempotent at any time—adding only missing categories and items.
>
> <details><summary>Edited 2 files</summary>
>
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+74 -27)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+24 -46)
>
> </details>
>
> The two blocked actions are now being made executable rather than conditional dead ends: Remove Category no longer demands silent text input, and starter-menu import is no longer limited to an empty restaurant. Repeating it will skip existing entries instead of duplicating them.
>
> The test failure confirms the new control also pushed normal categories below the expected visible layout—a regression, not just a test update. I’m moving starter import into the existing category action row so it stays discoverable without disrupting the menu-management flow.
>
> <details><summary>Edited 4 files</summary>
>
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+10 -16)
> - Wrote `./apps/ros/test/widget_test.dart` (+8 -6)
> - Wrote `./docs/editions/community-delivery-contract.md` (+3 -3)
> - Wrote `./docs/editions/community.md` (+6 -5)
>
> </details>
>
> I also updated the tests and product contract to match the intended behavior: starter import is now repeatable and idempotent, rather than an empty-menu-only feature.
>
> <details><summary>Edited a file</summary>
>
> - Wrote `./apps/ros/test/widget_test.dart` (+2 -2)
> - Wrote `./apps/ros/test/widget_test.dart` (+8 -2)
> - Wrote `./apps/ros/test/widget_test.dart` (+6 -14)
>
> </details>
>
> The two reported interactions now have direct paths, and the focused widget regressions pass. I’m running the complete storage and Flutter suites again before handing this back, because this is exactly the kind of change that must not be claimed from a partial check.
</details>

You were right. The prior behavior was incomplete:

- **Remove Category** silently required a typed reason; clicking its button with an empty reason did nothing.
- **Starter import** was hidden once any category existed, which made it unusable in the situation you described.

Both are corrected.

- **Remove Category** now needs only a clear confirmation. It uses the system audit reason “Owner removed unused category,” then removes an empty/never-sold category. Sold categories remain protected with an actionable status.
- **Starter menu** is now always visible beside **Add category**. It can run at any time and is idempotent: ROS adds only missing common Indian categories/items and skips entries already present.
- Imported items remain **disabled at ₹0** until the owner reviews prices and deliberately resumes them—no accidental free sales.
- The Community docs now state that starter import is repeatable and does not duplicate existing starter entries.

The relevant changes are in [menu workflow UI](apps/ros/lib/features/command_center/restaurant_shell.dart), [starter-import behavior](apps/ros/rust/src/api/simple.rs), and [Community guidance](docs/editions/community.md).

Validation completed:

- `cargo test -p ros_storage` — 64 passed
- `flutter analyze` — clean
- `flutter test` — 47 passed
- formatting and diff checks passed

Please fully restart the app to load the Rust-side starter-import change; hot reload alone is not enough.

<details><summary>11 previous messages</summary>

> Context compaction
>
> Source: automatic
> Status: completed
>
> Why aren't we providing all the flexible options for adding the images for categories the way we do for menu items ?
> I don't understand why are you doing incomplete implementations and shallow implementations ?
> Why don't we have the default images as well from the app the way we have for menu ?
> Please use your common sense properly instead of forcing me to inform you of every small things again and again.
>
> You’re right: category images should offer the same choices as menu-item images, including built-in defaults. I’m correcting that parity gap now and will verify the relevant flow rather than treating the storage support as sufficient.
>
> I found the exact gap: the backend already accepted a built-in category image, but the visible category action went straight to a file picker—so users could not choose any app defaults, search the catalogue, see a preview, clear an image, or confirm rights for an upload. I’m using the image-asset workflow to add a distinct category library, rather than recycling dish photos.
>
> I’m treating “all flexible options” literally: category images will have separate app-provided category artwork, verified Gotigin catalogue choices with retained licence provenance, restaurant uploads with an explicit rights confirmation, and removal. This requires a small migration because catalogue attribution/provenance must not be discarded.
>
> <details><summary>Created 11 files, edited 5 files</summary>
>
> - Wrote `./database/local-migrations/0036_category_image_catalog_provenance.sql` (+88 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+26 -2)
> - Wrote `./crates/ros_storage/src/lib.rs` (+129 -14)
> - Wrote `./crates/ros_storage/src/lib.rs` (+36 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+13 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+11 -2)
> - Wrote `./crates/ros_storage/src/lib.rs` (+96 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+1 -1)
> - Wrote `./apps/ros/rust/src/api/simple.rs` (+61 -4)
> - Wrote `./apps/ros/assets/category-source/category_beverages.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_breads.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_breakfast.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_desserts.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_fast_food.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_healthy.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_mains.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_rice.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_specials.svg` (+4 -0)
> - Wrote `./apps/ros/assets/category-source/category_starters.svg` (+4 -0)
> - Wrote `./apps/ros/pubspec.yaml` (+1 -0)
> - Wrote `./apps/ros/lib/features/catalog/menu_item_image.dart` (+69 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+10 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+28 -1)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+3 -3)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+112 -19)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+275 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+73 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+0 -8)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+29 -5)
>
> </details>
>
> The category flow is now a deliberate three-source chooser instead of a hidden file upload. I’ve also added a proper “remove image” action that preserves category and audit history. Next I’m compiling the Rust migration/provenance work before regenerating the Flutter bridge.
>
> <details><summary>Edited 4 files</summary>
>
> - Wrote `./crates/ros_storage/src/lib.rs` (+1 -0)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+1 -1)
> - Wrote `./docs/editions/community-delivery-contract.md` (+11 -4)
> - Wrote `./docs/editions/community.md` (+7 -3)
> - Wrote `./crates/ros_storage/src/lib.rs` (+1 -1)
>
> </details>
>
> The encrypted-storage test suite now passes with the new category catalogue-provenance and image-clear coverage (65 tests). I’m running the full Flutter suite next, including the updated bridge and asset references.
>
> <details><summary>Edited 2 files</summary>
>
> - Wrote `./apps/ros/test/widget_test.dart` (+47 -0)
> - Wrote `./apps/ros/test/widget_test.dart` (+2 -1)
> - Wrote `./apps/ros/test/widget_test.dart` (+1 -1)
> - Wrote `./apps/ros/test/widget_test.dart` (+1 -1)
> - Wrote `./apps/ros/test/widget_test.dart` (+1 -1)
> - Wrote `./apps/ros/test/widget_test.dart` (+1 -1)
> - Wrote `./apps/ros/lib/features/command_center/restaurant_shell.dart` (+1 -1)
> - Wrote `./apps/ros/test/widget_test.dart` (+1 -1)
>
> </details>
>
> The new UI regression test caught a small bottom-sheet overflow; I fixed it by making the category image chooser scroll on shorter screens. The focused widget tests and analysis now pass. I’m doing the final full validation and diff hygiene check.
</details>

You were right. Category images now have the complete, distinct flow:

- App-provided category artwork (10 dedicated category visuals, not dish photos)
- Verified Gotigin catalogue photos with retained licence/integrity provenance
- Restaurant uploads with an explicit image-rights confirmation
- Remove current image without deleting the category or history

The user-facing workflow is in [restaurant_shell.dart](./apps/ros/lib/features/command_center/restaurant_shell.dart:3498); separate category rendering is in [menu_item_image.dart](./apps/ros/lib/features/catalog/menu_item_image.dart:75). The encrypted persistence and audit safeguards are covered by [migration 0036](./database/local-migrations/0036_category_image_catalog_provenance.sql:1) and updated Community regression rules in [the delivery contract](./docs/editions/community-delivery-contract.md:53).

Verified: 65 Rust storage tests, 48 Flutter tests, Flutter analysis, Rust check, and diff hygiene all pass.

Please fully restart/rebuild the app before testing this flow; hot reload will not apply the Rust bridge and database migration.

> we need a script so that we can completely uninstall the app locally, we will use it as long as we haven't published the so that migration worries won't be needed and properly planned app would be there in the first production release.

<details><summary>7 previous messages</summary>

> I’ll add a deliberately explicit local-development uninstall/reset script that removes the installed app data and secure local credentials, so developers can start clean before the first production release. It will require confirmation and avoid touching source, builds, or backups unless explicitly requested.
>
> The script will use the same Rust secure-store adapter as the app on desktop, rather than guessing how each OS stores credentials. It will be development-only, confirmation-gated, and supports Android plus iOS Simulator separately; physical iOS cannot be honestly “fully reset” by a host script because Keychain records may persist after uninstall.
>
> <details><summary>Created 2 files, edited 3 files</summary>
>
> - Wrote `./crates/ros_storage/src/lib.rs` (+37 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+131 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+10 -27)
> - Wrote `./crates/ros_storage/src/lib.rs` (+26 -0)
> - Wrote `./crates/ros_storage/src/lib.rs` (+0 -4)
> - Wrote `./apps/ros/rust/src/bin/ros_local_uninstall.rs` (+64 -0)
> - Wrote `./apps/ros/rust/Cargo.toml` (+5 -0)
> - Wrote `./scripts/uninstall-local-ros.py` (+245 -0)
> - Wrote `./scripts/uninstall-local-ros.py` (+3 -1)
>
> </details>
>
> The dry run correctly refused because a local ROS desktop process is currently running, so the script will not risk deleting files underneath it. I’ll leave that process untouched and continue with static and test validation.
>
> <details><summary>Edited 3 files</summary>
>
> - Wrote `./scripts/uninstall-local-ros.py` (+2 -3)
> - Wrote `./docs/runbooks/local-development.md` (+47 -0)
> - Wrote `./apps/ros/README.md` (+5 -0)
>
> </details>
>
> The tooling and runbook now distinguish a clean pre-release development reset from real customer recovery or release uninstall. The desktop reset clears database and secure credential together, then removes the local support directory and debug bundle; it deliberately refuses to run while the app is open.
>
> The reset has a storage-level test now: it proves the development database, WAL sidecars, and matching secure credential are all cleared together (66 Rust tests pass). I’m running the final app checks and confirming the repository has no whitespace or generation issues.
</details>

Done. Use this for a clean unpublished local reset:

```bash
python3 scripts/uninstall-local-ros.py --target desktop --dry-run
python3 scripts/uninstall-local-ros.py --target desktop --yes
```

It refuses if ROS is still open, then clears the Development database, SQLite sidecars, matching secure-store credential, support directory, and local debug bundle. I did not run the destructive command against the currently open app.

The script is [uninstall-local-ros.py](./scripts/uninstall-local-ros.py:100), backed by the same Rust secure-store integration in [ros_local_uninstall.rs](./apps/ros/rust/src/bin/ros_local_uninstall.rs:46). Instructions and Android/iOS Simulator commands are in [local-development.md](./docs/runbooks/local-development.md:28).

It is strictly development-only; production migrations still need proper planning before release.

Verified: 66 Rust tests and 48 Flutter tests pass, plus Rust and Flutter static checks.

> There is one more addition requirement we need to discuss.
> We need to support LAN feature because even in the single branch, there could be multiple desks. We need to discuss on how we will support this feature. This is very very important.

<details><summary>1 previous message</summary>

> Agreed—“single branch” must mean one restaurant location, not one device. LAN operation should let several counters, kitchen screens, and owner terminals share the same live branch while remaining usable if internet is down. I’ll first map the current local/sync boundaries so the discussion is grounded in what the app already has versus what must be designed.
</details>

Yes—this should be a core Community feature. A “single branch” restaurant can have many counters, kitchen screens, and owner devices; it should not mean one device.

The current product is single-installation local storage, with future cloud-sync outbox foundations—not LAN sharing—so LAN needs a deliberate architecture rather than a quick shared-database shortcut. [Community Edition](./docs/editions/community.md:18) [current architecture](./README.md:31)

My recommendation is a Branch Hub model:

```text
Internet optional

Counter 1 ─┐
Counter 2 ─┼─ encrypted LAN ─ Branch Hub ─ SQLCipher database
Kitchen ───┤                    (one authoritative branch)
Owner ─────┘
```

The Branch Hub runs on one always-on desktop or mini-PC in the restaurant. Every desk, tablet, and kitchen display joins it over the local network. The hub is the only writer to the branch database; clients never share a database file or independently edit SQLite over the network.

This gives us the right operational behaviour:

- Internet failure: all desks continue working over LAN.
- A desk loses LAN: it shows “Reconnecting to your restaurant hub”; it must not finalize invoices, payments, stock changes, or kitchen sends independently.
- Hub restart: desks reconnect and receive the missed events.
- Duplicate taps/retries: commands use idempotency IDs, so a sale is committed once.
- Power loss: the hub’s existing durable SQLCipher/WAL transaction model protects finalized work.
- Hub loss: owner restores a verified backup to a replacement hub; automatic failover should be a later, separately designed capability.

Security cannot be optional:

- Owner pairs a new device using a short-lived QR/one-time code plus confirmation on both screens.
- Each paired device receives its own cryptographic identity; all LAN traffic uses mutual TLS.
- Staff enter their PIN to the hub over that protected connection; PINs and database keys never move around as local files.
- Owner can see, rename, revoke, and re-pair devices from **Branch devices**.
- mDNS discovery is convenience only; it is never trust or authorization.

We should build it in these stages:

1. Define a LAN ADR and protocol contract: pairing, device lifecycle, command idempotency, event stream, reconnect, version compatibility, revocation, backup/recovery.
2. Create the Rust Branch Hub service and encrypted client cache/event projection.
3. Make POS, KDS, tables, staff, inventory, and reports use hub commands plus real-time event updates.
4. Add LAN health UX and owner device management.
5. Acceptance-test two counters plus KDS: disconnect/reconnect, duplicate retries, owner PIN reset propagation, revoked device rejection, hub restart, and backup restore.

I would put LAN multi-desk support in Community Edition because it is local, offline from the internet, and essential to ordinary single-branch operations. Professional should add cloud synchronization, multi-branch, remote owner access, and central control—not be the first place a restaurant gets multiple desks.

The key product decision is: are you comfortable making the first LAN release require one always-on desktop/mini-PC as the Branch Hub, with Android/iOS/tablet/desktop devices joining as clients? That is the reliable first design; making a phone or iPad host the branch creates background-service and reliability problems.

<details><summary>3 previous messages</summary>

> Context compaction
>
> Source: automatic
> Status: completed
>
> Yeah that is right, remember we will have only single app that works in different modes, community edition, professional free and paid, enterprise edition.
> So we need to be very careful, LAN is only supported in community edition and for other editions we will use WAN only.
> Does that sounds right ?
>
> Yes. I’m validating this against the existing edition boundaries so we can turn it into a firm rule without creating an accidental mixed-mode system.
</details>

Yes. That is a sound and clean edition boundary—provided “WAN only” describes cross-device coordination, not whether a cashier may continue selling during an internet outage.

| Operating mode in the single ROS app | Authority and multi-device transport |
|---|---|
| Community | Local Branch Hub over encrypted LAN; no cloud account or internet required |
| Professional Free/Trial | Gotigin cloud over WAN only |
| Professional Paid | Gotigin cloud over WAN only |
| Enterprise | Gotigin cloud over WAN only |

The app should contain all capabilities, but activate exactly one data topology per branch. A Professional or Enterprise branch must never also discover a Community Branch Hub, replicate over LAN, or dual-write to local and cloud authorities.

Two important rules:

- Professional/Enterprise devices retain an encrypted local cache and durable outbox so a desk can safely continue its own POS work while the WAN is temporarily down. Cross-desk live coordination resumes through the cloud when connectivity returns; no LAN fallback is used.
- Moving Community → Professional, or back to Community Safe Mode, must be an explicit verified migration—not an automatic switch. There must always be one authoritative copy, never a temporary split-brain between hub and cloud.

This fits the current product promise: Community is local/offline and cloud-free, while Professional adds cloud coordination without requiring a reinstall. [Community Edition](docs/editions/community.md:5) [Professional Edition](docs/editions/professional.md:5)

I would record this as the core topology rule before we implement LAN:

> **Community uses LAN Branch Hub multi-desk operation. Professional Free, Professional Paid, and Enterprise use Gotigin WAN cloud coordination only. LAN pairing, discovery, hub hosting, and local replication are disabled outside Community mode.**

> Yes do it please, but we need to allow the user to switch to community edition and non community edition as well.
> Just tell me that, is it a good idea ?
