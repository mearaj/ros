# Portable recovery v1

Envelope version: `ros.recovery.v1` (ADR 0005 / ADR 0007).

A portable backup is a verified encrypted snapshot plus a passphrase-wrapped
recovery envelope. Clean-install restore unwraps the SQLCipher key, verifies
integrity, and writes a new destination without overwriting a live database
or another local restaurant profile.

Owner PIN recovery uses the same recovery passphrase verifier. That verifier
must be created during Owner onboarding on the profile, not only when the
first portable kit is exported.

If the Owner forgets both PIN and passphrase on a device, they may start a
**new** empty restaurant profile. The prior profile remains in local history
and can be unlocked or recovered later if a secret returns. Starting a new
profile is not a master bypass and must not re-own the old database.

**Implementation status:** Community bridge + Flutter expose Owner onboarding
passphrase creation, Forgot Owner PIN, portable kit create/restore, and
multi-profile history (start new / open prior).
