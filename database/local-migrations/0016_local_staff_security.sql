-- Local staff identities are immutable. A later PIN reset or revocation is a
-- new append-only fact, never a rewrite of who was originally provisioned.
CREATE TABLE IF NOT EXISTS staff_accounts (
    staff_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL REFERENCES branches(branch_id),
    display_name TEXT NOT NULL,
    name_key TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner', 'manager', 'cashier', 'kitchen')),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    UNIQUE (branch_id, name_key)
) STRICT;

CREATE TABLE IF NOT EXISTS staff_pin_credentials (
    staff_pin_credential_id TEXT PRIMARY KEY NOT NULL,
    staff_id TEXT NOT NULL REFERENCES staff_accounts(staff_id),
    argon2id_hash TEXT NOT NULL,
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS staff_pin_credentials_latest
    ON staff_pin_credentials (staff_id, created_at_utc DESC, staff_pin_credential_id DESC);

CREATE TABLE IF NOT EXISTS staff_status_events (
    staff_status_event_id TEXT PRIMARY KEY NOT NULL,
    staff_id TEXT NOT NULL REFERENCES staff_accounts(staff_id),
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
    occurred_at_utc TEXT NOT NULL,
    occurred_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS staff_status_events_latest
    ON staff_status_events (staff_id, occurred_at_utc DESC, staff_status_event_id DESC);

-- A session is an append-only local record. Only the most recent event is
-- considered. This preserves a durable answer to who unlocked the counter
-- while still permitting lock and expiry transitions.
CREATE TABLE IF NOT EXISTS local_staff_session_events (
    local_staff_session_event_id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL,
    staff_id TEXT NOT NULL REFERENCES staff_accounts(staff_id),
    event_type TEXT NOT NULL CHECK (event_type IN ('unlocked', 'locked')),
    occurred_at_utc TEXT NOT NULL,
    expires_at_utc TEXT,
    audit_event_id TEXT REFERENCES audit_events(event_id),
    CHECK (
        (event_type = 'unlocked' AND expires_at_utc IS NOT NULL)
        OR (event_type = 'locked' AND expires_at_utc IS NULL)
    )
) STRICT;

CREATE INDEX IF NOT EXISTS local_staff_session_events_latest
    ON local_staff_session_events (device_id, occurred_at_utc DESC, local_staff_session_event_id DESC);

CREATE TABLE IF NOT EXISTS staff_pin_attempts (
    staff_pin_attempt_id TEXT PRIMARY KEY NOT NULL,
    staff_id TEXT NOT NULL REFERENCES staff_accounts(staff_id),
    attempted_at_utc TEXT NOT NULL,
    succeeded INTEGER NOT NULL CHECK (succeeded IN (0, 1))
) STRICT;

CREATE INDEX IF NOT EXISTS staff_pin_attempts_throttle
    ON staff_pin_attempts (staff_id, succeeded, attempted_at_utc DESC);

-- Existing Community installations already have a stable owner actor. Turn it
-- into the immutable owner account but deliberately give it no credential: the
-- first owner must choose a PIN in the current app, never inherit a default.
INSERT INTO staff_accounts (
    staff_id, branch_id, display_name, name_key, role, created_at_utc, created_by_actor_id
)
SELECT
    identity.owner_actor_id,
    branch.branch_id,
    'Owner',
    'owner',
    'owner',
    identity.created_at_utc,
    identity.owner_actor_id
FROM local_installation_identity AS identity
JOIN branches AS branch
WHERE identity.singleton = 1
  AND NOT EXISTS (
      SELECT 1 FROM staff_accounts AS staff
      WHERE staff.staff_id = identity.owner_actor_id
  );

INSERT INTO staff_status_events (
    staff_status_event_id, staff_id, status, occurred_at_utc, occurred_by_actor_id
)
SELECT
    staff.staff_id,
    staff.staff_id,
    'active',
    staff.created_at_utc,
    staff.created_by_actor_id
FROM staff_accounts AS staff
WHERE NOT EXISTS (
    SELECT 1 FROM staff_status_events AS status
    WHERE status.staff_id = staff.staff_id
);

CREATE TRIGGER IF NOT EXISTS staff_accounts_cannot_be_updated
BEFORE UPDATE ON staff_accounts
BEGIN
    SELECT RAISE(ABORT, 'staff accounts are immutable');
END;

CREATE TRIGGER IF NOT EXISTS staff_accounts_cannot_be_deleted
BEFORE DELETE ON staff_accounts
BEGIN
    SELECT RAISE(ABORT, 'staff accounts cannot be deleted');
END;

CREATE TRIGGER IF NOT EXISTS staff_pin_credentials_cannot_be_updated
BEFORE UPDATE ON staff_pin_credentials
BEGIN
    SELECT RAISE(ABORT, 'staff PIN credentials are immutable');
END;

CREATE TRIGGER IF NOT EXISTS staff_pin_credentials_cannot_be_deleted
BEFORE DELETE ON staff_pin_credentials
BEGIN
    SELECT RAISE(ABORT, 'staff PIN credentials cannot be deleted');
END;

CREATE TRIGGER IF NOT EXISTS staff_status_events_cannot_be_updated
BEFORE UPDATE ON staff_status_events
BEGIN
    SELECT RAISE(ABORT, 'staff status events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS staff_status_events_cannot_be_deleted
BEFORE DELETE ON staff_status_events
BEGIN
    SELECT RAISE(ABORT, 'staff status events cannot be deleted');
END;

CREATE TRIGGER IF NOT EXISTS local_staff_session_events_cannot_be_updated
BEFORE UPDATE ON local_staff_session_events
BEGIN
    SELECT RAISE(ABORT, 'staff session events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS local_staff_session_events_cannot_be_deleted
BEFORE DELETE ON local_staff_session_events
BEGIN
    SELECT RAISE(ABORT, 'staff session events cannot be deleted');
END;

CREATE TRIGGER IF NOT EXISTS staff_pin_attempts_cannot_be_updated
BEFORE UPDATE ON staff_pin_attempts
BEGIN
    SELECT RAISE(ABORT, 'staff PIN attempts are immutable');
END;

CREATE TRIGGER IF NOT EXISTS staff_pin_attempts_cannot_be_deleted
BEFORE DELETE ON staff_pin_attempts
BEGIN
    SELECT RAISE(ABORT, 'staff PIN attempts cannot be deleted');
END;
