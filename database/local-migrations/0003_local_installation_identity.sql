CREATE TABLE IF NOT EXISTS local_installation_identity (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    device_id TEXT NOT NULL UNIQUE,
    owner_actor_id TEXT NOT NULL UNIQUE,
    created_at_utc TEXT NOT NULL
) STRICT;

CREATE TRIGGER IF NOT EXISTS local_installation_identity_cannot_be_updated
BEFORE UPDATE ON local_installation_identity
BEGIN
    SELECT RAISE(ABORT, 'local installation identity is immutable');
END;

CREATE TRIGGER IF NOT EXISTS local_installation_identity_cannot_be_deleted
BEFORE DELETE ON local_installation_identity
BEGIN
    SELECT RAISE(ABORT, 'local installation identity cannot be deleted');
END;
