# Production SQLCipher artifact manifest

Release/profile builds select the `production-sqlcipher` feature graph. That
graph refuses to link until a reviewed artifact directory exists.

## Expected layout

```text
third_party/sqlcipher/
  MANIFEST.toml
  linux-x86_64/
    libsqlcipher.so   # shared (+ libsqlite3.so.0 symlink; SONAME interpose)
    include/
    SHA256SUMS
  windows-x86_64/
    sqlcipher.lib
    SHA256SUMS
  macos-x86_64/
    libsqlcipher.a
    SHA256SUMS
  macos-aarch64/
    libsqlcipher.a
    SHA256SUMS
```

Provision helpers (pinned official SQLCipher **4.17.0** source tarball + checksum):

- `./scripts/provision-sqlcipher-artifacts.sh` — Linux shared `libsqlcipher.so`
  (patches `__thread xoshiro_s` to process-global so GCC `-fPIC` links into the
  Flutter Rust cdylib; OpenSSL still backs the cipher). Also installs
  `libsqlite3.so.0` → SQLCipher so AppImage/GTK cannot split-bind system
  SQLite’s `sqlite3_open*` against SQLCipher’s `sqlite3_key` (that failure
  surfaces as “encrypted local storage could not be opened”).
- `.\scripts\provision-sqlcipher-artifacts.ps1` — Windows static `sqlcipher.lib`
  on an MSVC host / Win11 VM

See [release-packaging.md](release-packaging.md) Linux notes for the AppImage
verification checklist.

## MANIFEST.toml schema

```toml
schema_version = 1
sqlcipher_version = "4.17.0"
openssl_fips_note = "reviewed build notes live beside SHA256SUMS"
reviewed_by = "Gotigin security"
reviewed_at_utc = "YYYY-MM-DDTHH:MM:SSZ"

[[artifact]]
target = "x86_64-unknown-linux-gnu"
path = "linux-x86_64/libsqlcipher.so"
sha256 = "hex..."
provenance_url = "https://..."
```

## Build behavior

`crates/ros_storage/build.rs` now:

1. Parses `MANIFEST.toml`.
2. Selects the Cargo `TARGET` triple artifact.
3. Verifies the on-disk SHA-256.
4. Emits explicit `rustc-link-search` / `rustc-link-lib` directives.
5. Never falls back to pkg-config or an arbitrary system SQLCipher.

Release packaging scripts also export `SQLITE3_LIB_DIR` / `SQLITE3_INCLUDE_DIR`
so `libsqlite3-sys` stays aligned with the reviewed artifact.

Until reviewed binaries are placed, the production feature panics at build time
with a pointer to this runbook. Development continues on the bundled
development SQLCipher feature only (`flutter build … --debug` / cargokit
debug). Release packaging scripts require these artifacts and do not fall back
to the development feature; see [release-packaging.md](release-packaging.md)
and [local-development.md](local-development.md).
