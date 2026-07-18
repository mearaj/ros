-- Immutable, product-bound menu modifiers. A modifier's descriptive and
-- monetary facts are never edited: replacement means a new option, while an
-- archive preserves every prior selection for receipts, kitchen tickets, and
-- future synchronization.
CREATE TABLE IF NOT EXISTS product_modifier_options (
    modifier_option_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    product_id TEXT NOT NULL
        REFERENCES products (product_id) ON DELETE RESTRICT,
    display_name TEXT NOT NULL
        CHECK (length(trim(display_name)) BETWEEN 1 AND 120),
    name_key TEXT NOT NULL
        CHECK (length(trim(name_key)) BETWEEN 1 AND 120),
    price_delta_minor INTEGER NOT NULL CHECK (price_delta_minor >= 0),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    updated_by_actor_id TEXT NOT NULL,
    archived_at_utc TEXT,
    archived_by_actor_id TEXT,
    archive_reason TEXT,
    archive_audit_event_id TEXT UNIQUE
        REFERENCES audit_events (event_id) ON DELETE RESTRICT,
    CHECK (
        (archived_at_utc IS NULL
            AND archived_by_actor_id IS NULL
            AND archive_reason IS NULL
            AND archive_audit_event_id IS NULL)
        OR
        (archived_at_utc IS NOT NULL
            AND archived_by_actor_id IS NOT NULL
            AND archive_reason IS NOT NULL
            AND archive_audit_event_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX IF NOT EXISTS product_modifier_options_product_active
    ON product_modifier_options (branch_id, product_id, name_key, modifier_option_id)
    WHERE archived_at_utc IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS active_product_modifier_options_name
    ON product_modifier_options (branch_id, product_id, name_key)
    WHERE archived_at_utc IS NULL;

CREATE TRIGGER product_modifier_options_must_match_product_and_branch
BEFORE INSERT ON product_modifier_options
WHEN NOT EXISTS (
    SELECT 1
    FROM products
    WHERE products.product_id = NEW.product_id
      AND products.branch_id = NEW.branch_id
      AND products.currency_code = NEW.currency_code
      AND products.archived_at_utc IS NULL
)
BEGIN
    SELECT RAISE(ABORT, 'modifier option must match an active product and branch currency');
END;

CREATE TRIGGER product_modifier_options_immutable_facts
BEFORE UPDATE OF branch_id, product_id, display_name, name_key, price_delta_minor,
                 currency_code, created_at_utc, created_by_actor_id
ON product_modifier_options
BEGIN
    SELECT RAISE(ABORT, 'modifier option facts are immutable; create a replacement option');
END;

CREATE TRIGGER product_modifier_options_archive_only
BEFORE UPDATE ON product_modifier_options
WHEN OLD.archived_at_utc IS NOT NULL
    OR NEW.archived_at_utc IS NULL
    OR NEW.archived_by_actor_id IS NULL
    OR NEW.archive_reason IS NULL
    OR NEW.archive_audit_event_id IS NULL
    OR NEW.revision <> OLD.revision + 1
    OR NEW.updated_at_utc <> NEW.archived_at_utc
    OR NEW.updated_by_actor_id <> NEW.archived_by_actor_id
    OR NOT EXISTS (
        SELECT 1
        FROM audit_events
        WHERE audit_events.event_id = NEW.archive_audit_event_id
          AND audit_events.branch_id = NEW.branch_id
          AND audit_events.actor_id = NEW.archived_by_actor_id
          AND audit_events.event_type = 'catalog.product_modifier_option.archived'
    )
BEGIN
    SELECT RAISE(ABORT, 'modifier options may only be archived by their audited archive event');
END;

CREATE TRIGGER product_modifier_options_cannot_be_deleted
BEFORE DELETE ON product_modifier_options
BEGIN
    SELECT RAISE(ABORT, 'modifier options must be archived, not deleted');
END;

-- Existing order lines keep their historical effective selling price. New
-- rows also retain an exact option snapshot and the per-unit modifier total.
ALTER TABLE order_lines
    ADD COLUMN modifier_total_minor INTEGER NOT NULL DEFAULT 0
        CHECK (modifier_total_minor >= 0);

ALTER TABLE order_lines
    ADD COLUMN modifier_snapshot_json TEXT NOT NULL DEFAULT '[]'
        CHECK (json_valid(modifier_snapshot_json));

DROP TRIGGER IF EXISTS order_lines_must_match_order_and_product;
CREATE TRIGGER order_lines_must_match_order_and_product
BEFORE INSERT ON order_lines
WHEN
    json_type(NEW.modifier_snapshot_json) <> 'array'
    OR json_array_length(NEW.modifier_snapshot_json) > 20
    OR NEW.modifier_total_minor <> COALESCE(
        (
            SELECT SUM(CAST(json_extract(selected.value, '$.price_delta_minor') AS INTEGER))
            FROM json_each(NEW.modifier_snapshot_json) AS selected
        ),
        0
    )
    OR (
        SELECT COUNT(*)
        FROM json_each(NEW.modifier_snapshot_json)
    ) <> (
        SELECT COUNT(DISTINCT json_extract(selected.value, '$.modifier_option_id'))
        FROM json_each(NEW.modifier_snapshot_json) AS selected
    )
    OR EXISTS (
        SELECT 1
        FROM json_each(NEW.modifier_snapshot_json) AS selected
        WHERE NOT EXISTS (
            SELECT 1
            FROM product_modifier_options AS option
            JOIN orders ON orders.order_id = NEW.order_id
            WHERE option.modifier_option_id = json_extract(selected.value, '$.modifier_option_id')
              AND option.branch_id = orders.branch_id
              AND option.product_id = NEW.product_id
              AND option.archived_at_utc IS NULL
              AND option.display_name = json_extract(selected.value, '$.display_name_snapshot')
              AND option.price_delta_minor = CAST(json_extract(selected.value, '$.price_delta_minor') AS INTEGER)
              AND option.currency_code = NEW.currency_code
        )
    )
    OR NOT EXISTS (
        SELECT 1
        FROM orders
        JOIN products ON products.product_id = NEW.product_id
        WHERE orders.order_id = NEW.order_id
          AND orders.branch_id = products.branch_id
          AND orders.currency_code = products.currency_code
          AND orders.currency_code = NEW.currency_code
          AND NEW.unit_price_minor = products.unit_price_minor + NEW.modifier_total_minor
          AND NEW.line_total_minor = NEW.unit_price_minor * NEW.quantity
    )
BEGIN
    SELECT RAISE(ABORT, 'order line must match its active product and modifier snapshots');
END;

-- Retained modifier history is a hard blocker for the narrow unused-product
-- deletion exception. This preserves option identities even before a sale.
DROP TRIGGER IF EXISTS products_cannot_be_deleted;
CREATE TRIGGER products_cannot_be_deleted
BEFORE DELETE ON products
WHEN NOT EXISTS (
        SELECT 1
        FROM product_deletion_authorizations
        JOIN audit_events
            ON audit_events.event_id = product_deletion_authorizations.audit_event_id
        WHERE product_deletion_authorizations.product_id = OLD.product_id
          AND product_deletion_authorizations.branch_id = OLD.branch_id
          AND audit_events.branch_id = OLD.branch_id
          AND audit_events.event_type = 'catalog.product.deleted'
          AND CASE
              WHEN json_valid(audit_events.payload_json)
                  THEN json_extract(audit_events.payload_json, '$.entity_id')
              ELSE NULL
          END = OLD.product_id
    )
    OR EXISTS (SELECT 1 FROM order_lines WHERE order_lines.product_id = OLD.product_id)
    OR EXISTS (
        SELECT 1
        FROM product_image_versions
        WHERE product_image_versions.branch_id = OLD.branch_id
          AND product_image_versions.product_id = OLD.product_id
    )
    OR EXISTS (
        SELECT 1
        FROM product_modifier_options
        WHERE product_modifier_options.branch_id = OLD.branch_id
          AND product_modifier_options.product_id = OLD.product_id
    )
    OR EXISTS (
        SELECT 1
        FROM sync_outbox_events
        JOIN audit_events
            ON audit_events.event_id = sync_outbox_events.audit_event_id
        WHERE audit_events.branch_id = OLD.branch_id
          AND CASE
                WHEN json_valid(audit_events.payload_json)
                    THEN json_extract(audit_events.payload_json, '$.entity_id')
                ELSE NULL
              END = OLD.product_id
    )
BEGIN
    SELECT RAISE(ABORT, 'products with retained history must be archived');
END;
