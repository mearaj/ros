# Production SQLCipher artifact manifest

Release/profile builds select the `production-sqlcipher` feature graph. That
graph refuses to link until a reviewed artifact directory exists.

## Expected layout

```text
third_party/sqlcipher/
  MANIFEST.toml
  linux-x86_64/
    libsqlcipher.a
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

## MANIFEST.toml schema

```toml
schema_version = 1
sqlcipher_version = "4.17.0"
openssl_fips_note = "reviewed build notes live beside SHA256SUMS"
reviewed_by = "Gotigin security"
reviewed_at_utc = "YYYY-MM-DDTHH:MM:SSZ"

[[artifact]]
target = "x86_64-unknown-linux-gnu"
path = "linux-x86_64/libsqlcipher.a"
sha256 = "hex..."
provenance_url = "https://..."
```

## Build behavior

`crates/ros_storage/build.rs` and `crates/ros_sqlcipher_ffi` must:

1. Parse `MANIFEST.toml`.
2. Select the host/target triple artifact.
3. Verify the on-disk SHA-256.
4. Emit explicit `rustc-link-search` / `rustc-link-lib` directives.
5. Never fall back to pkg-config or an arbitrary system SQLCipher.

Until reviewed binaries are placed, the production feature panics at build time
with a pointer to this runbook. Development continues on the bundled
development SQLCipher feature only.
