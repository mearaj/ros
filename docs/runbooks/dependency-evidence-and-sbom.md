# Dependency evidence, SBOM, and open-source notices

## Current boundary

The repository can create a repeatable baseline inventory from its locked Rust
and Flutter inputs. That inventory is useful review evidence, but it is not yet
a publication-grade software bill of materials (SBOM), an open-source notice,
a vulnerability scan, or a legal license decision.

The generator deliberately does not infer licenses. Rust `declared_license`
values are package-manifest metadata reported by Cargo and remain unreviewed.
Flutter's dependency command does not report licenses, so its inventory records
`null` rather than guessing. A missing declaration is a release-review item,
not evidence that a dependency has no obligations.

## Generate the baseline evidence

Prerequisites are the pinned Rust toolchain, Flutter, `jq`, and `sha256sum`.
Resolve the Flutter graph with the enforced lockfile as part of the normal
verification gate, then choose a new or empty output directory:

```bash
cd apps/ros
flutter pub get --enforce-lockfile
cd ../..
./scripts/generate-dependency-evidence.sh \
  target/release-evidence/dependencies-rc1
```

The script fails instead of overwriting a non-empty destination. It runs Cargo
metadata in locked, offline mode and creates:

- sorted Rust, Flutter application, and Cargokit Dart build-tool package
  inventories;
- exact Cargo, Flutter, Cargokit build-tool, Rust-toolchain, and Android Gradle
  wrapper input snapshots; and
- `SHA256SUMS` for every file in the evidence directory.

Given identical input files and resolved dependency graphs, the generated file
contents are deterministic. The output directory name is not embedded. Keep
the bundle with the exact source revision and release-candidate evidence.

## What the baseline does not cover

The inventory is broader than a single packaged artifact in some places and
narrower in others. It can include development packages, while it does not
prove which native libraries, operating-system frameworks, Flutter engine
components, fonts, images, Gradle plugins, or platform packaging components
entered a particular artifact. It also does not collect license texts or test
license compatibility. The Gradle wrapper snapshot records its current
configuration but is not Gradle dependency verification or a dependency lock.

The embedded menu-image provenance is reviewed separately in
[menu image provenance](../assets/menu-image-provenance.md); it must still be
included in the release's asset and notice review where applicable.

## Publication gate

Before labeling an artifact publishable, the accountable release owner must
attach evidence that:

1. the build used an approved, immutable source revision and enforced lockfiles;
2. each release target and profile has an artifact-specific dependency graph,
   including native SQLCipher, Flutter engine, Gradle/native packaging, and
   embedded assets;
3. a schema-valid SPDX or CycloneDX SBOM was produced by a pinned, reviewed tool,
   validated, checksummed, and bound to the signed artifact;
4. automated vulnerability results were reviewed with dated dispositions and
   do not silently suppress unresolved findings;
5. every dependency and embedded asset has reviewed license/provenance evidence,
   all missing or ambiguous declarations are resolved, and required license
   texts and attributions appear in the shipped open-source notices; and
6. the SBOM, notices, source offer obligations where applicable, artifact
   checksums, signing evidence, and build provenance are retained together.

Until all six items exist for the exact artifact, release documentation must
say **dependency evidence generated; publication SBOM and notices pending**.
