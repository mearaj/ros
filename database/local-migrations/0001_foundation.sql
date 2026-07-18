CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    applied_at_utc TEXT NOT NULL,
    checksum TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS audit_events (
    event_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    occurred_at_utc TEXT NOT NULL,
    previous_hash BLOB,
    event_hash BLOB NOT NULL,
    synced_at_utc TEXT,
    UNIQUE (device_id, sequence)
) STRICT;

CREATE INDEX IF NOT EXISTS audit_events_branch_time
    ON audit_events (branch_id, occurred_at_utc);

