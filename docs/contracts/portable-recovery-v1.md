# Portable recovery v1

Envelope version: `ros.recovery.v1` (ADR 0005 / ADR 0007).

A portable backup is a verified encrypted snapshot plus a passphrase-wrapped
recovery envelope. Clean-install restore unwraps the SQLCipher key, verifies
integrity, and writes a new destination without overwriting a live database.

Owner PIN recovery uses the same recovery passphrase verifier.
