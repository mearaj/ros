# ADR 0007: Owner and staff credential recovery

**Status:** Accepted  
**Date:** 18 July 2026  
**Approver:** Gotigin engineering (founder-accountable product default)

## Context

There is no default Owner PIN and no bypass. Lost Owner credentials would
otherwise brick a restaurant installation.

## Decision

Credential recovery policy `credential-recovery.v1`:

1. **Staff (non-owner) PIN loss:** an active Owner session may rotate that
   staff member’s PIN with a required reason (already implemented). No second
   factor beyond Owner authority.
2. **Owner PIN loss on a working installation:** require the portable recovery
   envelope passphrase (ADR 0005) as identity proof, then allow a one-time
   Owner PIN reset that:
   - verifies the recovery passphrase against the latest Owner-exported
     recovery verifier stored locally (or supplied with the envelope),
   - appends an audit event `staff.owner_pin.recovered`,
   - never logs the passphrase or new PIN,
   - rate-limits failed recovery attempts (5 failures / 15 minutes).
3. **Owner PIN and passphrase both unavailable on this device:** the Owner may
   **Start a new restaurant**, which creates a new empty local profile. The
   prior encrypted profile remains in local restaurant history and is not
   opened, rewritten, or re-owned. If the Owner later remembers the PIN or
   recovery passphrase for that profile, they may unlock or recover it from
   history. Gotigin does not hold a master unlock key and must not invent a
   remote bypass.
4. **Escalation boundary:** support tickets may confirm process steps; they
   must never receive PINs, recovery passphrases, SQLCipher keys, or raw
   databases.

The recovery passphrase verifier must be created during Owner onboarding on a
profile, not deferred until the first portable backup. Portable backup +
envelope (ADR 0005) remains required for clean-device restore.

## Consequences

- Owner PIN recovery is bound to the onboarding recovery passphrase verifier,
  with portable envelope create/restore and local restaurant profile history
  for “start new without destroying old.”
- Community remains usable offline for recovery when the passphrase or
  portable kit exists; double-forget yields a new empty profile, not a vendor
  unlock of the old database.
