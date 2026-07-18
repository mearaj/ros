-- Low-stock policy is separate from immutable stock movements. Every change
-- appends a reasoned threshold fact; the latest fact is the active policy.
CREATE TABLE IF NOT EXISTS inventory_low_stock_threshold_events (
    inventory_low_stock_threshold_event_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL REFERENCES branches(branch_id),
    product_id TEXT NOT NULL REFERENCES products(product_id),
    threshold_quantity INTEGER NOT NULL CHECK (threshold_quantity >= 0),
    reason TEXT NOT NULL,
    occurred_at_utc TEXT NOT NULL,
    occurred_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS inventory_low_stock_threshold_events_latest
    ON inventory_low_stock_threshold_events (
        branch_id, product_id, occurred_at_utc DESC,
        inventory_low_stock_threshold_event_id DESC
    );

CREATE TRIGGER IF NOT EXISTS inventory_low_stock_threshold_events_cannot_be_updated
BEFORE UPDATE ON inventory_low_stock_threshold_events
BEGIN
    SELECT RAISE(ABORT, 'low-stock threshold events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS inventory_low_stock_threshold_events_cannot_be_deleted
BEFORE DELETE ON inventory_low_stock_threshold_events
BEGIN
    SELECT RAISE(ABORT, 'low-stock threshold events cannot be deleted');
END;
