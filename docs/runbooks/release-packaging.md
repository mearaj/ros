# Release packaging

This runbook packages **signed Community Edition release artifacts** into
`ros-website/public/downloads/` using the filenames in
`ros-website/src/lib/downloads.ts`.

It does not replace [release-verification.md](release-verification.md) or the
Community acceptance gate in
[community-delivery-contract.md](../editions/community-delivery-contract.md).
A packaged binary is publishable only when those gates have evidence for the
platforms you claim as supported.

Authoritative constraints:

- Release/profile builds use the `production-sqlcipher` feature graph and
  refuse to link until reviewed artifacts exist under `third_party/sqlcipher/`
  ([sqlcipher-artifact-manifest.md](sqlcipher-artifact-manifest.md),
  [founder-intervention-log.md](founder-intervention-log.md)).
- Do not work around a missing artifact with the development SQLCipher feature,
  a system library, or an unsigned ad-hoc library
  ([local-development.md](local-development.md)).
- Android and iOS remain unsupported local-database release targets until
  secure-store adapters and real-device tests are accepted
  ([community.md](../editions/community.md),
  [release-verification.md](release-verification.md)).
- Desktop public release still requires production SQLCipher, signing, and
  release acceptance for each claimed OS ([community.md](../editions/community.md)).

## Current packaging status (Build Week pass)

| Platform | Packaging status |
| --- | --- |
| **Linux x86_64** | Signed AppImage published under `ros-website/public/downloads/` (`ros-linux-x86_64.AppImage` + `.sig`). Still not a Community “supported release” until [release-verification.md](release-verification.md) evidence exists. |
| **Windows 10+ x64** | Scripts + SQLCipher provision helper are in-repo; **build on a Win11 VM** (cannot cross-compile from Linux). |
| **Android** | **Deferred** this pass. Do not run `build-android-release.sh` or claim an APK is ready. Website marks Android as planned until a later APK ship. |

## Prerequisites (every platform you will publish)

1. **Production SQLCipher 4.17.x artifacts** for that platform under
   `third_party/sqlcipher/`, with a completed `MANIFEST.toml` (`reviewed_by`,
   `reviewed_at_utc`, per-target `[[artifact]]` + sha256). Obtain via:
   - Linux: `./scripts/provision-sqlcipher-artifacts.sh`
   - Windows (MSVC host/VM): `.\scripts\provision-sqlcipher-artifacts.ps1`
   See [sqlcipher-artifact-manifest.md](sqlcipher-artifact-manifest.md).
2. **Production signing identities** under `secrets/signing/` (gitignored).
   Generate once per release officer / build machine set.
3. Flutter tooling for the target OS, plus the platform extras listed below.

## One-time: generate production signing keys

Secrets live under `secrets/signing/` and are gitignored. Commit only the
README and the **public** GnuPG key under `ros-website/public/keys/`.

```bash
# Linux host: Android PKCS#12 keystore + GnuPG release key
./scripts/generate-release-signing-keys.sh

# Windows 10/11 build host (or Win11 VM): Authenticode PFX (+ GnuPG if needed)
.\scripts\generate-release-signing-keys.ps1
```

Copy `secrets/signing/gpg/` between Linux and Windows build hosts so every
platform’s download `.sig` verifies with the same public key.

| Material | Signs |
| --- | --- |
| `secrets/signing/android/ros-release.p12` | Android APK (v1/v2/v3) via `apps/ros/android/key.properties` |
| `secrets/signing/windows/ros-codesign.pfx` | Windows PE + installer (Authenticode / `signtool`) |
| `secrets/signing/gpg/gotigin-ros-release.sec` | Detached `.sig` next to each website download |

Replace self-generated Authenticode material with a commercially trusted
code-signing certificate before relying on Windows SmartScreen trust.

## Build per platform

| Host | Script | Output in `ros-website/public/downloads/` |
| --- | --- | --- |
| Linux | `./scripts/build-linux-release.sh` | `ros-linux-x86_64.AppImage` + `.sig` |
| Linux (with Android SDK/NDK) | `./scripts/build-android-release.sh` | `ros-android.apk` + `.sig` |
| Windows 10/11 | `.\scripts\build-windows-release.ps1` | `ros-windows-x64.exe` + `.sig` |

Windows product compatibility: **Windows 10 x64 and later**.

Scripts require production SQLCipher artifacts for the build target. They do
not fall back to the development SQLCipher path.

### Linux notes

- Requires `third_party/sqlcipher` artifact for `x86_64-unknown-linux-gnu`
  via `./scripts/provision-sqlcipher-artifacts.sh`.
- Downloads `appimagetool` into `tools/appimagetool/` (gitignored) on first use.
- **Keep Flutter’s bundle layout** in the AppImage: `ros`, `lib/`, and `data/`
  must sit next to each other (see `build-linux-release.sh`). Relocating
  `libapp.so` under `usr/lib` breaks AOT (`FlutterEngineCreateAOTData` /
  Invalid ELF path).
- **Ship `libsqlite3.so.0` → SQLCipher** in `AppDir/lib/` (provision + packaging
  scripts do this). SQLCipher’s shared object is often loaded as
  `libsqlite3.so.0`. GTK/tinysparql also load that SONAME; if the AppImage only
  ships `libsqlcipher.so`, the dynamic linker can bind `sqlite3_open*` to
  **system** SQLite and `sqlite3_key` to SQLCipher. Symptom:
  `encrypted local storage could not be opened` on first Release create.
  Verify after packaging:

  ```bash
  ./ros-linux-x86_64.AppImage --appimage-extract
  ls -la squashfs-root/lib/libsqlite3.so.0   # must resolve to SQLCipher
  ```

- Debug and Release use **separate** DB files and keyring services
  (`restaurant-os.development.db` vs `restaurant-os.db`). A healthy Debug
  install does not mean Release storage is fine.

### Windows 10+ — Win11 VM checklist

Run these on the Windows build VM (shared repo checkout; copy
`secrets/signing/gpg/` from the Linux host so `.sig` files use the same key).
Do **not** try to cross-compile Windows from Linux.

#### 0. Sync the repo onto the VM

- Prefer a **local NTFS copy** on the VM disk (e.g. `C:\ros-build`), not only a
  VirtualBox/VMware shared folder (`Z:`). Flutter's Windows build creates
  plugin symlinks; shared folders usually fail with `ERROR_ACCESS_DENIED`.
- Enable **Developer Mode** (Settings → System → For developers) so symlinks
  work without elevation on the local disk.
- Example:

  ```powershell
  robocopy Z:\WindowsShared\output C:\ros-build /E /XD .git build .dart_tool
  cd C:\ros-build
  ```

- Keep `third_party\sqlcipher\windows-x86_64\` (already provisioned) and
  `secrets\signing\` in that local tree.
- Copy from Linux if needed (do not commit secrets):
  - `secrets/signing/gpg/` (private + public GnuPG material used for `.sig`)
- After a successful build, copy
  `ros-website\public\downloads\ros-windows-x64.exe` + `.sig` back to the
  Linux share / website tree.

#### 1. Install prerequisites (once per VM)

| Tool | Purpose |
| --- | --- |
| Flutter (stable) + `flutter config --enable-windows-desktop` | Windows Release build |
| **Rust / rustup** (MSVC host) | Cargokit builds `rust_lib_ros`; pin `1.97.0` via `rustup toolchain install 1.97.0` |
| Visual Studio 2022/2026 with **Desktop development with C++** (or Build Tools + MSVC x64/x86) | MSVC + CMake; `provision-sqlcipher-artifacts.ps1` finds `VsDevCmd.bat` via `vswhere` |
| Windows 10/11 SDK | Includes `signtool.exe` |
| [Inno Setup 6](https://jrsoftware.org/isinfo.php) | Builds `ros-windows-x64.exe` installer |
| [Gpg4win](https://www.gpg4win.org/) | Detached `.sig` (`gpg` on PATH) |
| OpenSSL Win64 **full** (not Light) | `provision-sqlcipher-artifacts.ps1` + Release link of `sqlcipher.lib` |
| LLVM **or** VS “C++ Clang Compiler for Windows” | `libclang.dll` for `buildtime_bindgen` (`LIBCLANG_PATH`) |

Open a **Developer PowerShell for VS** (or ensure `VsDevCmd` / MSVC is on PATH).

If scripts are blocked (`running scripts is disabled on this system`), allow local
scripts for this user once:

```powershell
Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned
```

Or for a single run without changing policy:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\generate-release-signing-keys.ps1
```

Confirm:

```powershell
flutter doctor -v
flutter config --enable-windows-desktop
gpg --version
# signtool from Windows SDK; Inno Setup's ISCC on PATH or default install path
```

#### 2. One-time Authenticode PFX (Windows-only)

```powershell
cd <repo-root>
.\scripts\generate-release-signing-keys.ps1
```

Creates `secrets/signing/windows/ros-codesign.pfx` (gitignored). Self-signed is
fine for packaging; replace with a commercially trusted cert before SmartScreen
trust matters.

#### 3. Provision Windows SQLCipher 4.17.0

```powershell
.\scripts\provision-sqlcipher-artifacts.ps1
```

Expect:

- `third_party/sqlcipher/windows-x86_64/sqlcipher.lib` + headers
- `MANIFEST.toml` entry for `x86_64-pc-windows-msvc` (Linux entry preserved if
  already present)

#### 4. Build and publish the signed installer

```powershell
.\scripts\build-windows-release.ps1
```

Expect under `ros-website/public/downloads/`:

- `ros-windows-x64.exe`
- `ros-windows-x64.exe.sig`

Minimum OS for the installer: **Windows 10 x64 and later**.

Verify:

```powershell
gpg --import ros-website\public\keys\gotigin-ros-release.pub.asc
gpg --verify ros-website\public\downloads\ros-windows-x64.exe.sig `
             ros-website\public\downloads\ros-windows-x64.exe
```

Then mark Windows as `available` in `ros-website/src/lib/downloads.ts` only
when that binary is actually published for users.

### Android notes

One universal release APK covers phones, tablets, and other Android form
factors. Gradle refuses release packaging without a complete
`android/key.properties` (never falls back to the debug key).

Packaging an APK is not the same as claiming Android as a supported release
target. Per Community edition docs, Android stays targeted until secure-store
adapters, real-device tests, production SQLCipher linkage for Android ABIs, and
signing evidence are accepted. **This packaging pass skips Android entirely**
(no APK published; website status is planned).

## After packaging

1. Verify signatures (example for Linux):

   ```bash
   gpg --import ros-website/public/keys/gotigin-ros-release.pub.asc
   gpg --verify ros-website/public/downloads/ros-linux-x86_64.AppImage.sig \
                ros-website/public/downloads/ros-linux-x86_64.AppImage
   ```

2. Run the automated and manual gates in
   [release-verification.md](release-verification.md) for each platform you
   will offer.
3. Record Community acceptance evidence per
   [community-delivery-contract.md](../editions/community-delivery-contract.md)
   before treating the build as a supported public release.

## Related

- [sqlcipher-artifact-manifest.md](sqlcipher-artifact-manifest.md)
- [release-verification.md](release-verification.md)
- [founder-intervention-log.md](founder-intervention-log.md)
- [local-development.md](local-development.md)
- [community.md](../editions/community.md)
