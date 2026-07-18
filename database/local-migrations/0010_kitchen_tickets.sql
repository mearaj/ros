CREATE TABLE IF NOT EXISTS kitchen_tickets (
    kitchen_ticket_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    draft_order_id TEXT NOT NULL
        REFERENCES draft_orders (draft_order_id) ON DELETE RESTRICT,
    draft_revision INTEGER NOT NULL CHECK (draft_revision >= 1),
    ticket_state TEXT NOT NULL
        CHECK (ticket_state IN ('new', 'preparing', 'ready', 'completed')),
    table_label_snapshot TEXT,
    line_snapshot_json TEXT NOT NULL CHECK (json_valid(line_snapshot_json)),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    updated_by_actor_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    UNIQUE (draft_order_id, draft_revision)
) STRICT;

CREATE INDEX IF NOT EXISTS kitchen_tickets_branch_state_created
    ON kitchen_tickets (branch_id, ticket_state, created_at_utc);

CREATE TRIGGER IF NOT EXISTS kitchen_tickets_cannot_be_deleted
BEFORE DELETE ON kitchen_tickets
BEGIN
    SELECT RAISE(ABORT, 'kitchen tickets must be preserved for audit');
END;
