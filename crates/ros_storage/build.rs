//! Refuses a production SQLCipher link until a reviewed local artifact
//! manifest lists a checksum-verified library for this target. This is
//! intentionally stronger than relying on the `rusqlite/sqlcipher` feature,
//! which can otherwise discover an arbitrary system library through pkg-config.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PRODUCTION_SQLCIPHER");
    println!("cargo:rerun-if-changed=../../third_party/sqlcipher");
    println!("cargo:rerun-if-env-changed=TARGET");

    if std::env::var_os("CARGO_FEATURE_PRODUCTION_SQLCIPHER").is_none() {
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sqlcipher_root = manifest_dir.join("../../third_party/sqlcipher");
    let manifest_path = sqlcipher_root.join("MANIFEST.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap_or_else(|_| {
        panic!(
            "production-sqlcipher requires third_party/sqlcipher/MANIFEST.toml; see docs/runbooks/sqlcipher-artifact-manifest.md"
        )
    });

    if !manifest.contains("reviewed_by = \"")
        || manifest.contains("reviewed_by = \"\"")
        || !manifest.contains("[[artifact]]")
    {
        panic!(
            "production-sqlcipher MANIFEST.toml is incomplete: set reviewed_by, reviewed_at_utc, and at least one [[artifact]] with sha256; see docs/runbooks/sqlcipher-artifact-manifest.md"
        );
    }

    let target = std::env::var("TARGET").unwrap_or_else(|_| {
        panic!("production-sqlcipher requires Cargo TARGET to select the artifact")
    });

    let artifact = select_artifact(&manifest, &target).unwrap_or_else(|| {
        panic!(
            "production-sqlcipher has no [[artifact]] for TARGET={target}; see docs/runbooks/sqlcipher-artifact-manifest.md"
        )
    });

    let library_path = sqlcipher_root.join(&artifact.path);
    if !library_path.is_file() {
        panic!(
            "production-sqlcipher artifact missing on disk: {}",
            library_path.display()
        );
    }

    let actual = sha256_hex(&library_path);
    let expected = artifact.sha256.trim_start_matches("sha256:");
    if !actual.eq_ignore_ascii_case(expected) {
        panic!(
            "production-sqlcipher checksum mismatch for {}: expected {expected}, got {actual}",
            library_path.display()
        );
    }

    let search_dir = library_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .unwrap_or_else(|_| library_path.parent().unwrap().to_path_buf());

    println!("cargo:rustc-link-search=native={}", search_dir.display());
    let link_kind = match library_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
    {
        "so" | "dylib" | "dll" => "dylib",
        _ => "static",
    };
    println!("cargo:rustc-link-lib={link_kind}=sqlcipher");

    // SQLCipher builds typically need the platform crypto provider.
    if target.contains("windows") {
        // Prefer OPENSSL_ROOT_DIR / OPENSSL_LIB_DIR so MSVC can find libcrypto.lib
        // under "C:\Program Files\OpenSSL-Win64\..." without relying on LIB alone.
        if let Ok(lib_dir) = std::env::var("OPENSSL_LIB_DIR") {
            println!("cargo:rustc-link-search=native={lib_dir}");
        } else if let Ok(root) = std::env::var("OPENSSL_ROOT_DIR") {
            for candidate in [
                format!(r"{root}\lib\VC\x64\MD"),
                format!(r"{root}\lib\VC\x64\MT"),
                format!(r"{root}\lib\VC\static"),
                format!(r"{root}\lib"),
            ] {
                let path = PathBuf::from(&candidate);
                if path.is_dir() {
                    println!("cargo:rustc-link-search=native={}", path.display());
                    break;
                }
            }
        }
        println!("cargo:rerun-if-env-changed=OPENSSL_ROOT_DIR");
        println!("cargo:rerun-if-env-changed=OPENSSL_LIB_DIR");
        println!("cargo:rustc-link-lib=dylib=libcrypto");
        println!("cargo:rustc-link-lib=dylib=libssl");
        println!("cargo:rustc-link-lib=dylib=crypt32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=bcrypt");
    } else if target.contains("apple") {
        println!("cargo:rustc-link-lib=dylib=crypto");
    } else {
        println!("cargo:rustc-link-lib=dylib=crypto");
        println!("cargo:rustc-link-lib=dylib=dl");
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=m");
    }

    let include_dir = search_dir.join("include");
    if include_dir.is_dir() {
        println!(
            "cargo:warning=production-sqlcipher: export SQLCIPHER_LIB_DIR={} SQLCIPHER_INCLUDE_DIR={} SQLCIPHER_STATIC=1 for libsqlite3-sys (sqlcipher feature uses SQLCIPHER_* env vars)",
            search_dir.display(),
            include_dir.display()
        );
    }

    println!(
        "cargo:rustc-env=ROS_PRODUCTION_SQLCIPHER_ARTIFACT={}",
        library_path.display()
    );
}

#[derive(Debug)]
struct ArtifactRef {
    path: String,
    sha256: String,
}

fn select_artifact(manifest: &str, target: &str) -> Option<ArtifactRef> {
    let mut current_target: Option<String> = None;
    let mut current_path: Option<String> = None;
    let mut current_sha: Option<String> = None;
    let mut matched: Option<ArtifactRef> = None;

    let flush = |current_target: &mut Option<String>,
                 current_path: &mut Option<String>,
                 current_sha: &mut Option<String>,
                 matched: &mut Option<ArtifactRef>,
                 target: &str| {
        if let (Some(t), Some(path), Some(sha256)) = (
            current_target.take(),
            current_path.take(),
            current_sha.take(),
        ) && t == target
        {
            *matched = Some(ArtifactRef { path, sha256 });
        }
    };

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line == "[[artifact]]" {
            flush(
                &mut current_target,
                &mut current_path,
                &mut current_sha,
                &mut matched,
                target,
            );
            continue;
        }
        if let Some(value) = toml_string_value(line, "target") {
            current_target = Some(value);
        } else if let Some(value) = toml_string_value(line, "path") {
            current_path = Some(value);
        } else if let Some(value) = toml_string_value(line, "sha256") {
            current_sha = Some(value);
        }
    }
    flush(
        &mut current_target,
        &mut current_path,
        &mut current_sha,
        &mut matched,
        target,
    );
    matched
}

fn toml_string_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = \"");
    if let Some(rest) = line.strip_prefix(&prefix) {
        return rest.strip_suffix('"').map(str::to_owned);
    }
    None
}

fn sha256_hex(path: &Path) -> String {
    // Follow symlinks so MANIFEST hashes match the real shared-object content.
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let bytes = fs::read(&canonical)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", canonical.display()));
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
