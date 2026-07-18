-- Price changes are mutable catalogue facts, but only through a revisioned
-- command that emits an audit event. A product may be physically deleted only
-- while it has no financial, image-version, or synchronization history.
-- Audit events deliberately survive such a deletion and explain it.

CREATE TABLE IF NOT EXISTS product_deletion_authorizations (
    product_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    audit_event_id TEXT NOT NULL UNIQUE
        REFERENCES audit_events (event_id) ON DELETE RESTRICT,
    deleted_at_utc TEXT NOT NULL
) STRICT;

CREATE TRIGGER IF NOT EXISTS product_deletion_authorizations_cannot_be_updated
BEFORE UPDATE ON product_deletion_authorizations
BEGIN
    SELECT RAISE(ABORT, 'product deletion authorizations are immutable');
END;

CREATE TRIGGER IF NOT EXISTS product_deletion_authorizations_cannot_be_deleted
BEFORE DELETE ON product_deletion_authorizations
BEGIN
    SELECT RAISE(ABORT, 'product deletion authorizations are append-only');
END;

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
    OR EXISTS (
        SELECT 1
        FROM order_lines
        WHERE order_lines.product_id = OLD.product_id
    )
    OR EXISTS (
        SELECT 1
        FROM product_image_versions
        WHERE product_image_versions.branch_id = OLD.branch_id
          AND product_image_versions.product_id = OLD.product_id
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
