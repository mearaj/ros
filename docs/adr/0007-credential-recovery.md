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
3. **Owner PIN loss without recovery envelope:** fail closed. Support may guide
   clean-install restore from a portable backup + envelope only. Gotigin does
   not hold a master unlock key and must not invent a remote bypass.
4. **Escalation boundary:** support tickets may confirm process steps; they
   must never receive PINs, recovery passphrases, SQLCipher keys, or raw
   databases.

## Consequences

- Implementation adds Owner PIN recovery bound to the recovery-envelope secret.
- Community remains usable offline for recovery when the envelope exists.
