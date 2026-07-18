# ADR 0009: Commercial offline-grace and edition capacity defaults

**Status:** Accepted  
**Date:** 18 July 2026  
**Approver:** Gotigin engineering (founder-accountable product default)

## Context

Professional trial expiry and Enterprise branch capacity need product defaults
before entitlement issuance and Safe Mode can be completed.

## Decision

Defaults for the defined release (pricing amounts remain commercially editable
without schema change):

| Term | Default |
|------|---------|
| Professional evaluation | 14 days from activation |
| Offline grace after entitlement cannot be refreshed | 72 hours of continued local Professional features, then Community Safe Mode |
| Community Safe Mode | Local POS/KDS/reports remain fully usable; cloud sync and multi-branch cloud features pause; no data deletion or hiding |
| Professional paid branch capacity | Up to 5 branches |
| Enterprise paid branch capacity | Configured per contract; software default ceiling 50 until a signed contract overrides |
| Annual expiry | Same Safe Mode path as trial expiry; renewal restores entitlements without reinstall |

## Consequences

- Entitlement evaluator and activation flows use these constants.
- Commercial price lists are outside this ADR; only operational safety defaults
  are locked here.
