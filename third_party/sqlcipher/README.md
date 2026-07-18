# SQLCipher production artifacts

This directory holds reviewed SQLCipher 4.17.x static libraries for release
builds. It intentionally contains **no** binaries until Gotigin places
checksum-verified artifacts here.

See [docs/runbooks/sqlcipher-artifact-manifest.md](../../docs/runbooks/sqlcipher-artifact-manifest.md).

Placeholder `MANIFEST.toml` documents the schema; `build.rs` treats a missing
or incomplete artifact set as a hard error when `production-sqlcipher` is
enabled.
