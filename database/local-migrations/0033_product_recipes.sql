-- Product recipes (BOM): selling a finished product deducts ingredient stock
-- when recipe lines exist; otherwise tracked finished-good deduction remains.

CREATE TABLE IF NOT EXISTS product_recipes (
    product_recipe_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    finished_product_id TEXT NOT NULL
        REFERENCES products (product_id) ON DELETE RESTRICT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    archived_at_utc TEXT,
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS product_recipes_active_finished_product
    ON product_recipes (branch_id, finished_product_id)
    WHERE archived_at_utc IS NULL;

CREATE TABLE IF NOT EXISTS product_recipe_lines (
    product_recipe_line_id TEXT PRIMARY KEY NOT NULL,
    product_recipe_id TEXT NOT NULL
        REFERENCES product_recipes (product_recipe_id) ON DELETE RESTRICT,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    ingredient_product_id TEXT NOT NULL
        REFERENCES products (product_id) ON DELETE RESTRICT,
    quantity_per_unit INTEGER NOT NULL CHECK (quantity_per_unit > 0),
    line_number INTEGER NOT NULL CHECK (line_number >= 1),
    UNIQUE (product_recipe_id, line_number),
    UNIQUE (product_recipe_id, ingredient_product_id)
) STRICT;

CREATE TRIGGER IF NOT EXISTS product_recipes_cannot_be_deleted
BEFORE DELETE ON product_recipes
BEGIN
    SELECT RAISE(ABORT, 'recipes are archive-only');
END;

CREATE TRIGGER IF NOT EXISTS product_recipe_lines_cannot_be_updated
BEFORE UPDATE ON product_recipe_lines
BEGIN
    SELECT RAISE(ABORT, 'recipe lines are immutable; archive and recreate');
END;

CREATE TRIGGER IF NOT EXISTS product_recipe_lines_cannot_be_deleted
BEFORE DELETE ON product_recipe_lines
BEGIN
    SELECT RAISE(ABORT, 'recipe lines must be preserved');
END;

CREATE TRIGGER IF NOT EXISTS product_recipe_lines_reject_self_ingredient
BEFORE INSERT ON product_recipe_lines
WHEN NEW.ingredient_product_id = (
    SELECT finished_product_id
    FROM product_recipes
    WHERE product_recipe_id = NEW.product_recipe_id
)
BEGIN
    SELECT RAISE(ABORT, 'a product cannot be an ingredient of itself');
END;
