-- Opening stock establishes the initial balance exactly once. Later stock
-- changes must be purchase, sale, waste, or accountable adjustment facts.
DROP TRIGGER IF EXISTS inventory_movements_valid_direction;
CREATE TRIGGER inventory_movements_valid_direction
BEFORE INSERT ON inventory_movements
WHEN (NEW.movement_type IN ('opening', 'purchase') AND NEW.quantity_delta < 1)
  OR (NEW.movement_type IN ('sale', 'waste') AND NEW.quantity_delta > -1)
  OR (NEW.movement_type = 'adjustment' AND (NEW.reason IS NULL OR length(trim(NEW.reason)) < 3))
  OR (NEW.movement_type = 'opening' AND EXISTS (
      SELECT 1 FROM inventory_movements
      WHERE branch_id = NEW.branch_id AND product_id = NEW.product_id
  ))
BEGIN SELECT RAISE(ABORT, 'inventory movement direction, reason, or opening state is invalid'); END;
