-- A sent kitchen ticket is never silently removed. Cancellation and kitchen
-- acknowledgement are distinct immutable facts, each tied to its own audit
-- event so they remain independently synchronizable and verifiable.

CREATE TABLE IF NOT EXISTS kitchen_ticket_cancellation_notices (
    kitchen_ticket_cancellation_notice_id TEXT PRIMARY KEY NOT NULL,
    kitchen_ticket_id TEXT NOT NULL UNIQUE
        REFERENCES kitchen_tickets (kitchen_ticket_id) ON DELETE RESTRICT,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    draft_order_id TEXT NOT NULL,
    draft_revision INTEGER NOT NULL CHECK (draft_revision >= 1),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 3 AND 500),
    occurred_at_utc TEXT NOT NULL,
    occurred_by_actor_id TEXT NOT NULL,
    audit_event_id TEXT NOT NULL UNIQUE
        REFERENCES audit_events (event_id) ON DELETE RESTRICT,
    FOREIGN KEY (draft_order_id, draft_revision)
        REFERENCES draft_order_revisions (draft_order_id, revision) ON DELETE RESTRICT
) STRICT;

CREATE INDEX IF NOT EXISTS kitchen_ticket_cancellation_notices_branch_occurred
    ON kitchen_ticket_cancellation_notices (branch_id, occurred_at_utc DESC);

CREATE TABLE IF NOT EXISTS kitchen_ticket_cancellation_acknowledgements (
    kitchen_ticket_cancellation_acknowledgement_id TEXT PRIMARY KEY NOT NULL,
    kitchen_ticket_cancellation_notice_id TEXT NOT NULL UNIQUE
        REFERENCES kitchen_ticket_cancellation_notices (
            kitchen_ticket_cancellation_notice_id
        ) ON DELETE RESTRICT,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    acknowledged_at_utc TEXT NOT NULL,
    acknowledged_by_actor_id TEXT NOT NULL,
    audit_event_id TEXT NOT NULL UNIQUE
        REFERENCES audit_events (event_id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX IF NOT EXISTS kitchen_ticket_cancellation_acknowledgements_branch_time
    ON kitchen_ticket_cancellation_acknowledgements (branch_id, acknowledged_at_utc DESC);

CREATE TRIGGER IF NOT EXISTS kitchen_ticket_cancellation_notices_cannot_be_updated
BEFORE UPDATE ON kitchen_ticket_cancellation_notices
BEGIN
    SELECT RAISE(ABORT, 'kitchen ticket cancellation notices are immutable');
END;

CREATE TRIGGER IF NOT EXISTS kitchen_ticket_cancellation_notices_cannot_be_deleted
BEFORE DELETE ON kitchen_ticket_cancellation_notices
BEGIN
    SELECT RAISE(ABORT, 'kitchen ticket cancellation notices must be preserved');
END;

CREATE TRIGGER IF NOT EXISTS kitchen_ticket_cancellation_acknowledgements_cannot_be_updated
BEFORE UPDATE ON kitchen_ticket_cancellation_acknowledgements
BEGIN
    SELECT RAISE(ABORT, 'kitchen ticket cancellation acknowledgements are immutable');
END;

CREATE TRIGGER IF NOT EXISTS kitchen_ticket_cancellation_acknowledgements_cannot_be_deleted
BEFORE DELETE ON kitchen_ticket_cancellation_acknowledgements
BEGIN
    SELECT RAISE(ABORT, 'kitchen ticket cancellation acknowledgements must be preserved');
END;
