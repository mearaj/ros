# Local development and build modes

This runbook is the Stage 1 handoff for developing and verifying the Restaurant
Operating System locally. It distinguishes a repeatable **Development** build
from the deliberately gated **Release** build. A release-labelled executable is
not automatically a distributable product.

## Build-mode contract

| Mode | How it is selected | Local data namespace | Intended use | Current status |
|---|---|---|---|---|
| Development | `flutter run` or `flutter build --debug` | Database: `restaurant-os.development.db`; desktop key-store service: `com.gotigin.ros.development.sqlcipher.v1` | Safe local development, tests, and demos | Supported on a Linux desktop session |
| Release | `flutter build --profile` or `flutter build --release` | Database: `restaurant-os.db`; desktop key-store service: `com.gotigin.ros.sqlcipher.v1` | Controlled, signed production packaging | Not publishable until its native SQLCipher artifact and release gates are accepted |

The Rust feature graph, not a Dart flag, is the authority for this choice:
Development uses the reproducible bundled SQLCipher test path, while
profile/release requires the `production-sqlcipher` path. The two modes do not
share a database or a desktop key-store entry. Do not use Development data for
a real restaurant or assume that it is an upgrade path to a Release install.

Flutter never receives a database key. On desktop, Rust obtains the key through
the operating-system secure store. Do not manually remove only the database or
only its key-store entry: a mismatch fails closed to protect existing data.
Existing installs that still hold the pre-rename services
(`com.gotigin.restaurantos.*.sqlcipher.v1`) are migrated on first open into the
current `com.gotigin.ros.*.sqlcipher.v1` entries.

## Reset an unpublished local installation

Before the first public production release, use the repository-owned reset
script when a developer needs a genuinely clean install rather than carrying
development migrations forward. It is **destructive**, development-only, and
never selects the production database or credential namespace.

Close ROS first, then inspect the target:

```bash
python3 scripts/uninstall-local-ros.py --target desktop --dry-run
```

To remove the desktop Development database, its SQLite sidecars, its matching
operating-system secure-store credential, its application-support directory,
and the local Flutter debug bundle:

```bash
python3 scripts/uninstall-local-ros.py --target desktop --yes
```

The script refuses to run while the local `ros` process is open. Its Rust
companion moves the encrypted database aside before it clears the credential;
if secure-store deletion fails, it restores the database files. It never reads
or prints a database key. Use `--keep-build` to preserve the debug bundle, or
`--app-path /path/to/ros.app` to remove an explicitly supplied local desktop
bundle as well.

Mobile commands remove only an unpublished local install on the selected test
target:

```bash
python3 scripts/uninstall-local-ros.py --target android --device <adb-serial> --yes
python3 scripts/uninstall-local-ros.py --target ios-simulator --device <simulator-udid> --yes
```

`--erase-ios-simulator` additionally erases the entire selected simulator and
is therefore never implicit. A physical iPhone/iPad is not offered as a
"complete uninstall" target: iOS Keychain records can survive an app uninstall
and require an app-owned recovery/reset design. This is especially important
once the mobile secure-store adapter is introduced.

This reset is for unpublished development only. It is not a replacement for a
customer backup, restore, ownership-recovery, update, or release-uninstall
path. Keep any test backup needed for validation outside the application-
support directory before running it.

## Prerequisites

- Flutter stable with Linux desktop support enabled.
- Rust `1.97.0`, including `clippy` and `rustfmt`.
- A graphical Linux desktop session with an unlocked Secret Service-compatible
  key store. A headless shell cannot prove the desktop key-store workflow.
- On Debian/Ubuntu, the native dependencies used by CI:

  ```bash
  sudo apt-get update
  sudo apt-get install --yes clang cmake libgtk-3-dev liblzma-dev ninja-build pkg-config
  ```

Install the pinned Rust tooling if it is not already available:

```bash
rustup toolchain install 1.97.0 --profile minimal --component clippy --component rustfmt
```

Enable and inspect Flutter's Linux desktop toolchain:

```bash
flutter config --enable-linux-desktop
flutter doctor -v
```

From the Flutter application directory, resolve Dart dependencies once:

```bash
cd apps/ros
flutter pub get
```

## Run the Development build

From the repository root:

```bash
cd apps/ros
flutter run -d linux
```

Or build an executable without launching it:

```bash
cd apps/ros
flutter build linux --debug
```

The app should report a local storage status rather than reveal paths, database
keys, or operating-system credential-store details. Use this Stage 1 smoke test:

1. Complete the Community restaurant setup.
2. In **Menu**, create a category and a menu item with an exact minor-unit
   price. Choose either an **app photo** or a JPEG, PNG, or WebP image the
   restaurant has rights to use; source files up to 32 MiB are decoded under
   bounded Rust limits, metadata-stripped, and compressed. The app accepts an
   image only when the prepared menu thumbnail is at or below 3 MiB, and
   restaurant-owned images need the rights confirmation.
3. Confirm the image appears on the saved item and its **Counter** tile. The
   Rust core stores uploads as metadata-stripped, bounded JPEG thumbnails in
   the encrypted local database; app photos are compact bundled WebP assets.
4. Create the six-to-twelve-digit **Owner PIN** when prompted. Confirm the
   app unlocks as Owner; use the floating lock action and unlock again before
   continuing. The local session expires after 15 minutes.
5. While unlocked as Owner, open **Reports** and use **Manage local staff** to
   add test Manager, Cashier, and Kitchen accounts with distinct PINs. Lock and
   unlock as each account. Confirm Manager can manage Menu, counter, Kitchen,
   and accountable operations; Cashier can operate the counter but cannot
   mutate Menu, Kitchen, staff, refunds, expenses, inventory, or cash-drawer
   controls; and Kitchen can progress Kitchen tickets but cannot operate the
   counter or mutate Menu. The interface should restrict unavailable actions,
   and Rust must reject the same unauthorized mutation if called directly.
   Lock again and continue as Owner or Manager.
6. From the item's Menu action, open **Modifiers** and add an optional
   non-negative price addition such as “Extra cheese”. A modifier's name and
   price cannot be edited in place: archive it and create a replacement when a
   correction is needed, so historical orders remain exact.
7. Open **Counter**, add the item, select the modifier, choose **Takeaway** or
   **Dine in**, choose **Cash**, **Card**, or **UPI**, then record the sale.
8. Confirm the cart and success receipt show the selected modifier and its
   price addition, plus an `INV-…` number and “saved locally”. Flutter carries
   modifier identities; Rust reloads the product and active modifier facts,
   calculates the trusted unit price, and snapshots the modifier names and
   prices before commit.
9. Confirm the success receipt shows “saved locally”.
   That response is shown only after one encrypted SQLite transaction has
   committed the order, immutable invoice, payment, audit events, and future
   sync-outbox entries.
10. Quit and relaunch the app. The existing restaurant, menu, selected image,
   invoice, and payment must be available without repeating setup. In
   **Reports**, open the recent invoice to confirm that its receipt reprint
   comes from immutable historical line, modifier, and payment snapshots.
11. In **Menu**, open the item's action menu and choose **Remove from active
   menu**. Enter a reason and archive it. Confirm it disappears from the active
   menu and Counter after the confirmation; it is deliberately retained in the
   encrypted database, audit chain, and any historic invoices rather than
   hard-deleted.
12. In **Counter**, add another item and choose **Dine in**. Hold the order for
   a table, add an optional kitchen instruction (maximum 500 characters), and
   select a modifier before saving it. Send the saved order to **Kitchen**.
   Confirm the kitchen ticket shows the instruction and modifier name but
   never prices, payment details, or the cancellation rationale. If the cart,
   instruction, or fulfillment changes after a save, confirm the action first
   requires saving a new draft revision before it can be sent; this prevents a
   stale snapshot from reaching the kitchen. Reopen the ticket and verify that
   its instruction and modifier snapshot remain unchanged. The behavior is
   defined in [Kitchen instructions v1](../contracts/kitchen-instructions-v1.md).

The Rust storage suite verifies the transaction boundaries, sequential invoice
numbers, audit chain, outbox rows, and database triggers that reject invoice,
payment, order, and outbox updates/deletes. It is the current way to verify
those persisted records beyond the one-time receipt view.

The original Stage 1 counter was an **immediate counter sale**: its cart was
memory-only until checkout. The current Community build additionally supports
held orders/tables, a privacy-safe KDS, local reports, receipt reprint, and
authenticated standard-role workspaces. It also supports product-bound
optional additions whose price and historical snapshot are enforced by Rust.
Provider-neutral taxes/discounts, dual-person refund/void approval, suppliers
and purchase documents, recipes/BOM stock deduction, ESC/POS+PDF receipt
encoding, and portable recovery envelopes are implemented in the local storage
surface. Hardware printer acceptance, production SQLCipher linkage, signed
packaging, entitlement activation against a deployed cloud, and the full
Section 15 release gates remain outside a publishable release until their
evidence is recorded.

If the application shows **“Local storage needs attention”**, first check that
the process is running in an unlocked graphical desktop session and that the
application-support directory is writable. Do not delete local files or
key-store entries as a troubleshooting shortcut; preserve both for recovery.

## Run the automated checks

Run the Rust checks from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Then run the Flutter checks:

```bash
cd apps/ros
flutter pub get --enforce-lockfile
dart format --output=none --set-exit-if-changed lib test integration_test
flutter analyze
flutter test
flutter test integration_test
flutter build linux --debug
```

These are the same categories of checks used by the repository CI workflow.
The native build is important: it validates that the generated Flutter/Rust
bridge and Cargokit integration can produce a desktop Development artifact.

For a release candidate, return to the repository root and generate the
non-overwriting locked dependency-evidence bundle described in
[Dependency evidence, SBOM, and open-source notices](dependency-evidence-and-sbom.md).
That bundle is useful review input; it is not by itself a publication SBOM or
completed license/notices review.

## When a Rust bridge API changes

Generated files are part of the contract. After editing
`apps/ros/rust/src/api/`, regenerate the bridge from the Flutter app
directory and include the generated changes with the Rust change:

```bash
cd apps/ros
flutter_rust_bridge_codegen generate
dart format lib test integration_test
```

Then rerun the Rust and Flutter checks above.

## Local diagnostics

Allow-listed technical events are written under the application-support
`diagnostics/` directory (outside SQLCipher). Bridge outcomes recorded today
include bootstrap, staff unlock/lock, owner PIN configure, sale preview/complete,
refund, void, day close, backup create/verify/restore, and storage integrity,
plus UI navigation/action breadcrumbs. Owners can open **Local
diagnostics** from Reports to export a redacted pack or voluntarily share it
after consent. Cloud upload requires building with:

```bash
flutter run --dart-define=ROS_DIAGNOSTICS_SHARE_URL=https://diagnostics.example.gotigin
```

If the define is unset, share prepares the pack locally and reports that intake
is unavailable. See
[local-diagnostics-v1.md](../contracts/local-diagnostics-v1.md).

## Release mode: intentionally fail closed today

Profile and Release builds are a distinct security mode, not an optimization
switch for the Development build. They must link a reviewed, pinned SQLCipher
4.17.x artifact and prove its provenance and checksum before packaging. The
current project has no accepted controlled artifact manifest or signed
desktop-key-store smoke-test evidence, so public Release packaging is blocked.

The build deliberately selects the production feature graph and never silently
falls back to the bundled Development SQLCipher source or an arbitrary system
SQLCipher library:

```bash
cd apps/ros
flutter build linux --release
```

The current production feature fails at the explicit artifact gate before
native linkage. A reviewed artifact manifest, checksum/provenance verifier,
and controlled linker configuration must be implemented before this can ever
produce an accepted Release artifact.

To inspect that gate without invoking the Flutter packaging tool, run the
feature graph with Development defaults disabled:

```bash
cargo check --locked -p ros_storage --no-default-features \
  --features production-sqlcipher,platform-keyring
```

It must fail with the explicit controlled-artifact refusal from
`crates/ros_storage/build.rs`; a successful command here would be a security
regression, not progress.

Do not work around that failure with a system SQLite library, a development
feature, environment-held key material, or an unsigned ad-hoc library. The
release checklist is in [PLAN.md](../../PLAN.md#15-non-negotiable-release-gates)
and the encryption decision records the required artifact controls in
[ADR 0002](../adr/0002-local-database-encryption.md).

Before a Release build can be accepted, the team must at least:

1. Add and verify the controlled SQLCipher 4.17.x artifact manifest,
   checksum, provenance, and platform linkage.
2. Build the Rust `production-sqlcipher` feature graph in controlled CI,
   without any Development linkage.
3. Run signed desktop create/reopen/missing-key recovery smoke tests against
   the native OS key store.
4. Complete the applicable functional, migration, backup/recovery, security,
   accessibility, signing, and operational gates in `PLAN.md`.

Android and iOS are not accepted local-database release targets yet: their
native secure-store adapters and signed-device tests are still required.
Their Android Gradle release tasks also fail closed unless a controlled
`android/key.properties` supplies all four signing settings shown in
`android/key.properties.example`; the project never signs a release artifact
with the debug key or silently produces an unsigned one.
