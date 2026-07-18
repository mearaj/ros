CREATE TABLE IF NOT EXISTS branch_document_sequences (
    branch_id TEXT PRIMARY KEY NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    next_invoice_number INTEGER NOT NULL
        CHECK (next_invoice_number BETWEEN 1 AND 9223372036854775807)
) STRICT;

CREATE TABLE IF NOT EXISTS orders (
    order_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    order_type TEXT NOT NULL
        CHECK (order_type IN ('dine_in', 'takeaway')),
    order_state TEXT NOT NULL
        CHECK (order_state IN ('open', 'finalized', 'void')),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    subtotal_minor INTEGER NOT NULL CHECK (subtotal_minor >= 0),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    finalized_at_utc TEXT,
    finalized_by_actor_id TEXT,
    CHECK (
        (order_state = 'open' AND finalized_at_utc IS NULL AND finalized_by_actor_id IS NULL)
        OR
        (order_state = 'finalized' AND finalized_at_utc IS NOT NULL AND finalized_by_actor_id IS NOT NULL)
        OR
        (order_state = 'void' AND finalized_at_utc IS NOT NULL AND finalized_by_actor_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX IF NOT EXISTS orders_branch_state_time
    ON orders (branch_id, order_state, created_at_utc);

CREATE TABLE IF NOT EXISTS order_lines (
    order_line_id TEXT PRIMARY KEY NOT NULL,
    order_id TEXT NOT NULL
        REFERENCES orders (order_id) ON DELETE RESTRICT,
    product_id TEXT NOT NULL
        REFERENCES products (product_id) ON DELETE RESTRICT,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    product_name_snapshot TEXT NOT NULL
        CHECK (length(trim(product_name_snapshot)) BETWEEN 1 AND 160),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    line_total_minor INTEGER NOT NULL CHECK (line_total_minor >= 0),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    created_at_utc TEXT NOT NULL,
    UNIQUE (order_id, line_number)
) STRICT;

CREATE INDEX IF NOT EXISTS order_lines_order
    ON order_lines (order_id, line_number);

CREATE TABLE IF NOT EXISTS invoices (
    invoice_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    order_id TEXT NOT NULL UNIQUE
        REFERENCES orders (order_id) ON DELETE RESTRICT,
    invoice_number INTEGER NOT NULL CHECK (invoice_number > 0),
    invoice_state TEXT NOT NULL CHECK (invoice_state = 'finalized'),
    subtotal_minor INTEGER NOT NULL CHECK (subtotal_minor >= 0),
    total_minor INTEGER NOT NULL CHECK (total_minor >= 0),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    finalized_at_utc TEXT NOT NULL,
    finalized_by_actor_id TEXT NOT NULL,
    UNIQUE (branch_id, invoice_number)
) STRICT;

CREATE INDEX IF NOT EXISTS invoices_branch_finalized
    ON invoices (branch_id, finalized_at_utc, invoice_number);

CREATE TABLE IF NOT EXISTS payments (
    payment_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    invoice_id TEXT NOT NULL
        REFERENCES invoices (invoice_id) ON DELETE RESTRICT,
    payment_method TEXT NOT NULL
        CHECK (payment_method IN ('cash', 'card', 'upi')),
    payment_state TEXT NOT NULL CHECK (payment_state = 'recorded'),
    amount_minor INTEGER NOT NULL CHECK (amount_minor >= 0),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    recorded_at_utc TEXT NOT NULL,
    recorded_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS payments_invoice
    ON payments (invoice_id, recorded_at_utc);

CREATE TABLE IF NOT EXISTS sync_outbox_events (
    operation_id TEXT PRIMARY KEY NOT NULL,
    audit_event_id TEXT NOT NULL UNIQUE
        REFERENCES audit_events (event_id) ON DELETE RESTRICT,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    device_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    created_at_utc TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS sync_outbox_events_branch_created
    ON sync_outbox_events (branch_id, created_at_utc);

CREATE TABLE IF NOT EXISTS sync_acknowledgements (
    operation_id TEXT PRIMARY KEY NOT NULL
        REFERENCES sync_outbox_events (operation_id) ON DELETE RESTRICT,
    server_event_id TEXT NOT NULL,
    acknowledged_at_utc TEXT NOT NULL
) STRICT;

CREATE TRIGGER IF NOT EXISTS orders_cannot_be_deleted
BEFORE DELETE ON orders
BEGIN
    SELECT RAISE(ABORT, 'orders must be preserved for audit');
END;

CREATE TRIGGER IF NOT EXISTS finalized_orders_cannot_be_updated
BEFORE UPDATE ON orders
WHEN OLD.order_state IN ('finalized', 'void')
BEGIN
    SELECT RAISE(ABORT, 'finalized orders are immutable');
END;

CREATE TRIGGER IF NOT EXISTS order_lines_cannot_be_updated
BEFORE UPDATE ON order_lines
BEGIN
    SELECT RAISE(ABORT, 'order lines are immutable');
END;

CREATE TRIGGER IF NOT EXISTS order_lines_cannot_be_deleted
BEFORE DELETE ON order_lines
BEGIN
    SELECT RAISE(ABORT, 'order lines must be preserved for audit');
END;

CREATE TRIGGER IF NOT EXISTS invoices_cannot_be_updated
BEFORE UPDATE ON invoices
BEGIN
    SELECT RAISE(ABORT, 'finalized invoices are immutable');
END;

CREATE TRIGGER IF NOT EXISTS invoices_cannot_be_deleted
BEFORE DELETE ON invoices
BEGIN
    SELECT RAISE(ABORT, 'finalized invoices must be preserved for audit');
END;

CREATE TRIGGER IF NOT EXISTS payments_cannot_be_updated
BEFORE UPDATE ON payments
BEGIN
    SELECT RAISE(ABORT, 'payments are immutable; use a reversal or refund');
END;

CREATE TRIGGER IF NOT EXISTS payments_cannot_be_deleted
BEFORE DELETE ON payments
BEGIN
    SELECT RAISE(ABORT, 'payments must be preserved for audit');
END;

CREATE TRIGGER IF NOT EXISTS sync_outbox_events_cannot_be_deleted
BEFORE DELETE ON sync_outbox_events
BEGIN
    SELECT RAISE(ABORT, 'sync outbox events are append-only');
END;

CREATE TRIGGER IF NOT EXISTS sync_outbox_events_cannot_be_updated
BEFORE UPDATE ON sync_outbox_events
BEGIN
    SELECT RAISE(ABORT, 'sync outbox events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS sync_acknowledgements_cannot_be_updated
BEFORE UPDATE ON sync_acknowledgements
BEGIN
    SELECT RAISE(ABORT, 'sync acknowledgements are append-only');
END;

CREATE TRIGGER IF NOT EXISTS sync_acknowledgements_cannot_be_deleted
BEFORE DELETE ON sync_acknowledgements
BEGIN
    SELECT RAISE(ABORT, 'sync acknowledgements are append-only');
END;

CREATE TRIGGER IF NOT EXISTS orders_must_use_branch_currency_on_insert
BEFORE INSERT ON orders
WHEN (
    SELECT currency_code
    FROM branches
    WHERE branch_id = NEW.branch_id
) <> NEW.currency_code
BEGIN
    SELECT RAISE(ABORT, 'order currency must match its branch currency');
END;

CREATE TRIGGER IF NOT EXISTS invoices_must_use_branch_currency_on_insert
BEFORE INSERT ON invoices
WHEN (
    SELECT currency_code
    FROM branches
    WHERE branch_id = NEW.branch_id
) <> NEW.currency_code
BEGIN
    SELECT RAISE(ABORT, 'invoice currency must match its branch currency');
END;

CREATE TRIGGER IF NOT EXISTS payments_must_use_branch_currency_on_insert
BEFORE INSERT ON payments
WHEN (
    SELECT currency_code
    FROM branches
    WHERE branch_id = NEW.branch_id
) <> NEW.currency_code
BEGIN
    SELECT RAISE(ABORT, 'payment currency must match its branch currency');
END;
