//! Refuses a production SQLCipher link until a reviewed local artifact
//! manifest lists a checksum-verified library for this target. This is
//! intentionally stronger than relying on the `rusqlite/sqlcipher` feature,
//! which can otherwise discover an arbitrary system library through pkg-config.

use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PRODUCTION_SQLCIPHER");
    println!("cargo:rerun-if-changed=../../third_party/sqlcipher");

    if std::env::var_os("CARGO_FEATURE_PRODUCTION_SQLCIPHER").is_none() {
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sqlcipher_root = manifest_dir.join("../../third_party/sqlcipher");
    let manifest_path = sqlcipher_root.join("MANIFEST.toml");
    let Ok(manifest) = fs::read_to_string(&manifest_path) else {
        panic!(
            "production-sqlcipher requires third_party/sqlcipher/MANIFEST.toml; see docs/runbooks/sqlcipher-artifact-manifest.md"
        );
    };
    if !manifest.contains("reviewed_by = \"")
        || manifest.contains("reviewed_by = \"\"")
        || !manifest.contains("[[artifact]]")
    {
        panic!(
            "production-sqlcipher MANIFEST.toml is incomplete: set reviewed_by, reviewed_at_utc, and at least one [[artifact]] with sha256; see docs/runbooks/sqlcipher-artifact-manifest.md"
        );
    }
    panic!(
        "production-sqlcipher artifacts are declared but linker wiring is still incomplete; place reviewed binaries under third_party/sqlcipher/<target>/ and finish controlled rustc-link-* emission per docs/runbooks/sqlcipher-artifact-manifest.md"
    );
}
