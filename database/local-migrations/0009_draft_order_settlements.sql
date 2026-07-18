CREATE TABLE IF NOT EXISTS draft_order_settlements (
    draft_order_id TEXT PRIMARY KEY NOT NULL
        REFERENCES draft_orders (draft_order_id) ON DELETE RESTRICT,
    order_id TEXT NOT NULL UNIQUE
        REFERENCES orders (order_id) ON DELETE RESTRICT,
    settled_revision INTEGER NOT NULL CHECK (settled_revision >= 1),
    settled_at_utc TEXT NOT NULL,
    settled_by_actor_id TEXT NOT NULL
) STRICT;

CREATE TRIGGER IF NOT EXISTS draft_order_settlements_cannot_be_updated
BEFORE UPDATE ON draft_order_settlements
BEGIN
    SELECT RAISE(ABORT, 'draft order settlements are immutable');
END;

CREATE TRIGGER IF NOT EXISTS draft_order_settlements_cannot_be_deleted
BEFORE DELETE ON draft_order_settlements
BEGIN
    SELECT RAISE(ABORT, 'draft order settlements are immutable');
END;
