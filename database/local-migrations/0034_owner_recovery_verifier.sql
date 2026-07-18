-- Stores only the Argon2id verifier for the Owner recovery passphrase used by
-- portable envelopes (ADR 0005 / ADR 0007). The passphrase itself is never kept.

CREATE TABLE IF NOT EXISTS owner_recovery_verifiers (
    owner_recovery_verifier_id TEXT PRIMARY KEY NOT NULL,
    organization_id TEXT NOT NULL
        REFERENCES organizations (organization_id) ON DELETE RESTRICT,
    argon2id_hash TEXT NOT NULL,
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    superseded_at_utc TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS owner_recovery_verifiers_active
    ON owner_recovery_verifiers (organization_id, created_at_utc)
    WHERE superseded_at_utc IS NULL;

CREATE TRIGGER IF NOT EXISTS owner_recovery_verifiers_cannot_be_deleted
BEFORE DELETE ON owner_recovery_verifiers
BEGIN
    SELECT RAISE(ABORT, 'owner recovery verifiers must be preserved');
END;
