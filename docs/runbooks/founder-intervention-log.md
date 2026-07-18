# Founder intervention log

This log separates decisions that can safely be implemented by the team from
actions that need an accountable Gotigin officer, commercial decision, or
credential owner. Work must continue on all other items.

## Resolved for engineering (18 July 2026)

| # | Decision | Record |
|---|----------|--------|
| 1 | First cloud target Google Cloud `asia-south1`; provider-neutral architecture retained; diagnostics URL and retention defaults set | [ADR 0003](../adr/0003-cloud-provider-selection.md) |
| 4 | First thermal profile: ESC/POS 80 mm / Epson TM-T82X class | [ADR 0008](../adr/0008-thermal-printer-support.md) |
| 5 | Offline grace 72h; Professional 5 branches; Enterprise ceiling 50; Safe Mode rules | [ADR 0009](../adr/0009-commercial-edition-terms.md) |
| 7 | Portable recovery envelope `ros.recovery.v1` with Owner passphrase | [ADR 0005](../adr/0005-portable-recovery-envelope.md) |
| 8 | Dual-person correction approval defaults | [ADR 0006](../adr/0006-dual-person-correction-approval.md) |
| 9 | Owner PIN recovery bound to recovery envelope; no remote bypass | [ADR 0007](../adr/0007-credential-recovery.md) |

## Still requiring credential or physical provisioning before public publish

1. **GCP billing account / project credentials** — create the real
   development/staging/production projects and wire secrets into the deploy
   pipeline. OpenTofu modules under `infra/` are ready for that binding.
2. **Production SQLCipher 4.17.x artifacts** — place reviewed platform binaries
   and checksums under `third_party/sqlcipher/` per
   [sqlcipher-artifact-manifest.md](sqlcipher-artifact-manifest.md). Builds
   remain fail-closed until those files exist.
3. **Code-signing identities and store accounts** — Apple/Windows/Linux signing
   material for publication; see
   [release-verification.md](release-verification.md).
4. **Physical Epson TM-T82X class unit** — lab acceptance evidence before
   marketing claims beyond “ESC/POS profile implemented”.
5. **Legal/compliance review** — still required before GST, e-invoicing, PCI,
   or accessibility-certification claims.

## Not blockers for offline Community development

- Live payment-gateway accounts. The local product records payment method only
  and deliberately stores no payment credentials or card data.
- Mobile release support. Android and iOS remain fail-closed until their secure
  storage adapters are reviewed and real-device tested.

## Decision record

When an item is resolved, record the decision, approver, date, and evidence in
the applicable ADR or release checklist; do not store credentials in this file.
