-- Staff identities are immutable. Changing operational authority appends a
-- reasoned role fact so prior attribution remains intact for audit and sync.
CREATE TABLE IF NOT EXISTS staff_role_events (
    staff_role_event_id TEXT PRIMARY KEY NOT NULL,
    staff_id TEXT NOT NULL REFERENCES staff_accounts(staff_id),
    role TEXT NOT NULL CHECK (role IN ('manager', 'cashier', 'kitchen')),
    reason TEXT NOT NULL,
    occurred_at_utc TEXT NOT NULL,
    occurred_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS staff_role_events_latest
    ON staff_role_events (staff_id, occurred_at_utc DESC, staff_role_event_id DESC);

CREATE TRIGGER IF NOT EXISTS staff_role_events_cannot_be_updated
BEFORE UPDATE ON staff_role_events
BEGIN
    SELECT RAISE(ABORT, 'staff role events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS staff_role_events_cannot_be_deleted
BEFORE DELETE ON staff_role_events
BEGIN
    SELECT RAISE(ABORT, 'staff role events cannot be deleted');
END;
