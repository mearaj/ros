# OpenAI Build Week — reviewer guide

**Start here** if you are reviewing Restaurant Operating System (ROS) for
OpenAI Build Week (13–21 July 2026).

This folder is a map, not a second copy of the engineering docs. Canonical
product rules, ADRs, contracts, and runbooks live under `docs/` and are linked
below.

## Attribution

The core architecture, planning, and a substantial portion of the
implementation were completed using ChatGPT and Codex during OpenAI Build Week.
After promotional and paid Codex usage, remaining implementation, testing, and
polish continued with conventional development tools and other assistants.
Repository history and `docs/chat-sessions/` show where Codex / GPT-5.6 and
other agents contributed.

Enterprise-grade Restaurant Operating System developed by **Mearaj Bhagad**,
Founder of **GOTIGIN SOFTWARE & HARDWARE PRIVATE LIMITED**, using Rust,
Flutter, ChatGPT 5.6, Codex, and other AI assistants.

## What this product is

ROS is a **local-first restaurant operating system** (POS, kitchen display,
menu, inventory, reports, staff PINs, encrypted local database)—not only a
checkout screen. Commercial intent and philosophy:

- [Product vision](../RESTAURANT_OPERATING_SYSTEM_PRODUCT_VISION.md)
- [Root README](../README.md)
- [Edition definitions](../docs/editions/README.md)

## Honest status for reviewers

| Area | Status |
| --- | --- |
| **Community Edition** (single branch, offline) | Active delivery focus; substantial local POS/KDS/ops loop implemented |
| **Professional / Enterprise** | Documented and scaffolded; not sold as live until release evidence exists |
| **Linux download** | Signed AppImage packaged under `ros-website/public/downloads/` (GPG-verifiable). Not yet a Community “supported release” without [release-verification](../docs/runbooks/release-verification.md) evidence |
| **Windows download** | Packaging path ready; build on Win11 VM (`generate-release-signing-keys.ps1` → `provision-sqlcipher-artifacts.ps1` → `build-windows-release.ps1`) |
| **Android download** | Deferred this pass; website marks Android as planned |
| **Multi-device LAN Hub** | Specified as Community core; pairing/runtime still incomplete |
| **Android / iOS as “supported”** | Targeted; not release-accepted until secure-store and device gates pass |

Authoritative Community rules and acceptance gate:

- [Community Edition](../docs/editions/community.md)
- [Community delivery contract](../docs/editions/community-delivery-contract.md)

## Architecture (one picture)

```text
Flutter / Dart UI
    → flutter_rust_bridge
    → Rust domain, storage, security
    → SQLCipher-encrypted SQLite (per restaurant profile)
```

Professional cloud (API, Postgres, owner dashboard) is present as foundation
code and is **disabled / not deployed** by default.

Key decisions: [docs/adr/](../docs/adr/) (start with
[0001](../docs/adr/0001-flutter-rust-client-architecture.md) and
[0002](../docs/adr/0002-local-database-encryption.md)).

## How to verify locally

Use the developer runbook (debug path; not the Release packaging path):

→ [docs/runbooks/local-development.md](../docs/runbooks/local-development.md)

Typical commands (from repo root; see runbook for full gates):

```bash
cargo test --locked --workspace
cd apps/ros && flutter pub get --enforce-lockfile && flutter test
flutter build linux --debug
```

End-user walkthrough of what the app does today:

→ [docs/runbooks/community-user-guide.md](../docs/runbooks/community-user-guide.md)

## Release packaging & downloads

Website download filenames and signed artifact flow:

→ [docs/runbooks/release-packaging.md](../docs/runbooks/release-packaging.md)  
→ [ros-website/](../ros-website/README.md)  
→ Outputs: `ros-website/public/downloads/`

**This pass:** Linux AppImage packaged and GPG-signed; Windows via Win11 VM
checklist in the packaging runbook; Android deferred.

Fail-closed production SQLCipher policy:

→ [docs/runbooks/sqlcipher-artifact-manifest.md](../docs/runbooks/sqlcipher-artifact-manifest.md)  
→ Provision: `./scripts/provision-sqlcipher-artifacts.sh` (Linux) /
`.\scripts\provision-sqlcipher-artifacts.ps1` (Windows VM)

Publish evidence checklist:

→ [docs/runbooks/release-verification.md](../docs/runbooks/release-verification.md)

Still founder-gated before public publish (GCP, SQLCipher binaries, commercial
signing trust, printer lab, legal claims):

→ [docs/runbooks/founder-intervention-log.md](../docs/runbooks/founder-intervention-log.md)

## Roadmap & non-negotiable gates

→ [PLAN.md](../PLAN.md) (stages, delivery log, §15 release gates)

## Security

→ [docs/security/threat-model.md](../docs/security/threat-model.md)  
→ [docs/security/data-classification.md](../docs/security/data-classification.md)

## Behavior contracts (domain rules)

Versioned specs under [docs/contracts/](../docs/contracts/) (pricing, kitchen,
inventory, recovery, diagnostics, etc.).

## Full documentation index

→ [docs/README.md](../docs/README.md)

## License & trademarks

→ [LICENSE](../LICENSE) (Apache-2.0)  
→ [TRADEMARKS.md](../TRADEMARKS.md)

## Optional provenance

Long AI session archives (not product docs):

→ [docs/chat-sessions/](../docs/chat-sessions/README.md)
