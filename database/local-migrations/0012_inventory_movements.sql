-- Append-only stock ledger. Balances are derived from movement facts.
CREATE TABLE IF NOT EXISTS inventory_movements (
    inventory_movement_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL REFERENCES branches (branch_id) ON DELETE RESTRICT,
    product_id TEXT NOT NULL REFERENCES products (product_id) ON DELETE RESTRICT,
    movement_type TEXT NOT NULL CHECK (movement_type IN ('opening', 'purchase', 'sale', 'waste', 'adjustment')),
    quantity_delta INTEGER NOT NULL CHECK (quantity_delta <> 0),
    reason TEXT,
    source_document_id TEXT,
    occurred_at_utc TEXT NOT NULL,
    occurred_by_actor_id TEXT NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS inventory_movements_balance_lookup
    ON inventory_movements (branch_id, product_id, occurred_at_utc);
CREATE TRIGGER IF NOT EXISTS inventory_movements_cannot_be_updated
BEFORE UPDATE ON inventory_movements
BEGIN SELECT RAISE(ABORT, 'inventory movements are immutable'); END;
CREATE TRIGGER IF NOT EXISTS inventory_movements_cannot_be_deleted
BEFORE DELETE ON inventory_movements
BEGIN SELECT RAISE(ABORT, 'inventory movements must be preserved'); END;
CREATE TRIGGER IF NOT EXISTS inventory_movements_valid_direction
BEFORE INSERT ON inventory_movements
WHEN (NEW.movement_type IN ('opening', 'purchase') AND NEW.quantity_delta < 1)
  OR (NEW.movement_type IN ('sale', 'waste') AND NEW.quantity_delta > -1)
  OR (NEW.movement_type = 'adjustment' AND (NEW.reason IS NULL OR length(trim(NEW.reason)) < 3))
BEGIN SELECT RAISE(ABORT, 'inventory movement direction or reason is invalid'); END;
CREATE TRIGGER IF NOT EXISTS inventory_movements_no_negative_balance
BEFORE INSERT ON inventory_movements
WHEN COALESCE((SELECT SUM(quantity_delta) FROM inventory_movements WHERE branch_id = NEW.branch_id AND product_id = NEW.product_id), 0) + NEW.quantity_delta < 0
BEGIN SELECT RAISE(ABORT, 'inventory balance cannot become negative'); END;
