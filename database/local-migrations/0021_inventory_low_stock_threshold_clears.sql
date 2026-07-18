-- Clearing an alert is a retained policy event, never a delete of a prior
-- threshold. The current policy is whichever threshold/clear event is latest.
CREATE TABLE IF NOT EXISTS inventory_low_stock_threshold_clear_events (
    inventory_low_stock_threshold_clear_event_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL REFERENCES branches(branch_id),
    product_id TEXT NOT NULL REFERENCES products(product_id),
    reason TEXT NOT NULL,
    occurred_at_utc TEXT NOT NULL,
    occurred_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS inventory_low_stock_threshold_clear_events_latest
    ON inventory_low_stock_threshold_clear_events (
        branch_id, product_id, occurred_at_utc DESC,
        inventory_low_stock_threshold_clear_event_id DESC
    );

CREATE TRIGGER IF NOT EXISTS inventory_low_stock_threshold_clear_events_cannot_be_updated
BEFORE UPDATE ON inventory_low_stock_threshold_clear_events
BEGIN
    SELECT RAISE(ABORT, 'low-stock threshold clear events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS inventory_low_stock_threshold_clear_events_cannot_be_deleted
BEFORE DELETE ON inventory_low_stock_threshold_clear_events
BEGIN
    SELECT RAISE(ABORT, 'low-stock threshold clear events cannot be deleted');
END;
