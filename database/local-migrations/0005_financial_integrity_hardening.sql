-- Migration 5 deliberately recreates security-sensitive triggers rather than
-- using IF NOT EXISTS. It repairs a same-name trigger that might have been
-- pre-created in a partial or tampered database before migration 4 ran.

DROP TRIGGER IF EXISTS audit_events_cannot_be_updated;
CREATE TRIGGER audit_events_cannot_be_updated
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit events are immutable');
END;

DROP TRIGGER IF EXISTS audit_events_cannot_be_deleted;
CREATE TRIGGER audit_events_cannot_be_deleted
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit events are append-only');
END;

DROP TRIGGER IF EXISTS categories_cannot_be_deleted;
CREATE TRIGGER categories_cannot_be_deleted
BEFORE DELETE ON categories
BEGIN
    SELECT RAISE(ABORT, 'categories must be archived, not deleted');
END;

DROP TRIGGER IF EXISTS products_cannot_be_deleted;
CREATE TRIGGER products_cannot_be_deleted
BEFORE DELETE ON products
BEGIN
    SELECT RAISE(ABORT, 'products must be archived, not deleted');
END;

DROP TRIGGER IF EXISTS branch_document_sequences_cannot_be_deleted;
CREATE TRIGGER branch_document_sequences_cannot_be_deleted
BEFORE DELETE ON branch_document_sequences
BEGIN
    SELECT RAISE(ABORT, 'invoice sequence state must be preserved');
END;

DROP TRIGGER IF EXISTS branch_document_sequences_must_start_after_last_invoice;
CREATE TRIGGER branch_document_sequences_must_start_after_last_invoice
BEFORE INSERT ON branch_document_sequences
WHEN NEW.next_invoice_number <> COALESCE(
    (
        SELECT MAX(invoice_number) + 1
        FROM invoices
        WHERE branch_id = NEW.branch_id
    ),
    1
)
BEGIN
    SELECT RAISE(ABORT, 'invoice sequence must begin after the last invoice');
END;

DROP TRIGGER IF EXISTS branch_document_sequences_must_increment;
CREATE TRIGGER branch_document_sequences_must_increment
BEFORE UPDATE ON branch_document_sequences
WHEN NEW.branch_id <> OLD.branch_id
    OR NEW.next_invoice_number <> OLD.next_invoice_number + 1
    OR NEW.next_invoice_number <> COALESCE(
        (
            SELECT MAX(invoice_number) + 1
            FROM invoices
            WHERE branch_id = OLD.branch_id
        ),
        1
    )
BEGIN
    SELECT RAISE(ABORT, 'invoice sequence may only advance by one');
END;

DROP TRIGGER IF EXISTS orders_cannot_be_deleted;
CREATE TRIGGER orders_cannot_be_deleted
BEFORE DELETE ON orders
BEGIN
    SELECT RAISE(ABORT, 'orders must be preserved for audit');
END;

DROP TRIGGER IF EXISTS finalized_orders_cannot_be_updated;
CREATE TRIGGER finalized_orders_cannot_be_updated
BEFORE UPDATE ON orders
WHEN OLD.order_state IN ('finalized', 'void')
BEGIN
    SELECT RAISE(ABORT, 'finalized orders are immutable');
END;

DROP TRIGGER IF EXISTS order_lines_cannot_be_updated;
CREATE TRIGGER order_lines_cannot_be_updated
BEFORE UPDATE ON order_lines
BEGIN
    SELECT RAISE(ABORT, 'order lines are immutable');
END;

DROP TRIGGER IF EXISTS order_lines_cannot_be_deleted;
CREATE TRIGGER order_lines_cannot_be_deleted
BEFORE DELETE ON order_lines
BEGIN
    SELECT RAISE(ABORT, 'order lines must be preserved for audit');
END;

DROP TRIGGER IF EXISTS invoices_cannot_be_updated;
CREATE TRIGGER invoices_cannot_be_updated
BEFORE UPDATE ON invoices
BEGIN
    SELECT RAISE(ABORT, 'finalized invoices are immutable');
END;

DROP TRIGGER IF EXISTS invoices_cannot_be_deleted;
CREATE TRIGGER invoices_cannot_be_deleted
BEFORE DELETE ON invoices
BEGIN
    SELECT RAISE(ABORT, 'finalized invoices must be preserved for audit');
END;

DROP TRIGGER IF EXISTS payments_cannot_be_updated;
CREATE TRIGGER payments_cannot_be_updated
BEFORE UPDATE ON payments
BEGIN
    SELECT RAISE(ABORT, 'payments are immutable; use a reversal or refund');
END;

DROP TRIGGER IF EXISTS payments_cannot_be_deleted;
CREATE TRIGGER payments_cannot_be_deleted
BEFORE DELETE ON payments
BEGIN
    SELECT RAISE(ABORT, 'payments must be preserved for audit');
END;

DROP TRIGGER IF EXISTS sync_outbox_events_cannot_be_deleted;
CREATE TRIGGER sync_outbox_events_cannot_be_deleted
BEFORE DELETE ON sync_outbox_events
BEGIN
    SELECT RAISE(ABORT, 'sync outbox events are append-only');
END;

DROP TRIGGER IF EXISTS sync_outbox_events_cannot_be_updated;
CREATE TRIGGER sync_outbox_events_cannot_be_updated
BEFORE UPDATE ON sync_outbox_events
BEGIN
    SELECT RAISE(ABORT, 'sync outbox events are immutable');
END;

DROP TRIGGER IF EXISTS sync_acknowledgements_cannot_be_updated;
CREATE TRIGGER sync_acknowledgements_cannot_be_updated
BEFORE UPDATE ON sync_acknowledgements
BEGIN
    SELECT RAISE(ABORT, 'sync acknowledgements are append-only');
END;

DROP TRIGGER IF EXISTS sync_acknowledgements_cannot_be_deleted;
CREATE TRIGGER sync_acknowledgements_cannot_be_deleted
BEFORE DELETE ON sync_acknowledgements
BEGIN
    SELECT RAISE(ABORT, 'sync acknowledgements are append-only');
END;

DROP TRIGGER IF EXISTS order_lines_must_match_order_and_product;
CREATE TRIGGER order_lines_must_match_order_and_product
BEFORE INSERT ON order_lines
WHEN NOT EXISTS (
    SELECT 1
    FROM orders
    JOIN products ON products.product_id = NEW.product_id
    WHERE orders.order_id = NEW.order_id
      AND orders.branch_id = products.branch_id
      AND orders.currency_code = products.currency_code
      AND orders.currency_code = NEW.currency_code
      AND products.unit_price_minor = NEW.unit_price_minor
      AND NEW.line_total_minor = NEW.unit_price_minor * NEW.quantity
)
BEGIN
    SELECT RAISE(ABORT, 'order line must match its order and product snapshot');
END;

DROP TRIGGER IF EXISTS invoices_must_match_finalized_order;
CREATE TRIGGER invoices_must_match_finalized_order
BEFORE INSERT ON invoices
WHEN NOT EXISTS (
    SELECT 1
    FROM orders
    WHERE orders.order_id = NEW.order_id
      AND orders.branch_id = NEW.branch_id
      AND orders.currency_code = NEW.currency_code
      AND orders.order_state = 'finalized'
      AND orders.subtotal_minor = NEW.subtotal_minor
      AND EXISTS (
          SELECT 1
          FROM branch_document_sequences
          WHERE branch_document_sequences.branch_id = NEW.branch_id
            AND branch_document_sequences.next_invoice_number = NEW.invoice_number
      )
)
BEGIN
    SELECT RAISE(ABORT, 'invoice must match its finalized order');
END;

CREATE UNIQUE INDEX IF NOT EXISTS payments_one_recorded_payment_per_invoice
    ON payments (invoice_id);

DROP TRIGGER IF EXISTS payments_must_match_finalized_invoice;
CREATE TRIGGER payments_must_match_finalized_invoice
BEFORE INSERT ON payments
WHEN NOT EXISTS (
    SELECT 1
    FROM invoices
    WHERE invoices.invoice_id = NEW.invoice_id
      AND invoices.branch_id = NEW.branch_id
      AND invoices.currency_code = NEW.currency_code
      AND invoices.invoice_state = 'finalized'
      AND invoices.total_minor = NEW.amount_minor
)
BEGIN
    SELECT RAISE(ABORT, 'payment must match its finalized invoice');
END;

DROP TRIGGER IF EXISTS sync_outbox_must_match_audit_identity;
CREATE TRIGGER sync_outbox_must_match_audit_identity
BEFORE INSERT ON sync_outbox_events
WHEN NOT EXISTS (
    SELECT 1
    FROM audit_events
    WHERE audit_events.event_id = NEW.audit_event_id
      AND audit_events.branch_id = NEW.branch_id
      AND audit_events.device_id = NEW.device_id
)
BEGIN
    SELECT RAISE(ABORT, 'sync outbox event must match its audit identity');
END;
