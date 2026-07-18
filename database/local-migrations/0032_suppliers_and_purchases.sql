-- Supplier accounts and purchase documents that receive into the inventory
-- ledger via source_document_id on inventory_movements.

CREATE TABLE IF NOT EXISTS suppliers (
    supplier_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 120),
    name_key TEXT NOT NULL CHECK (length(trim(name_key)) BETWEEN 1 AND 120),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    archived_at_utc TEXT,
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    UNIQUE (branch_id, name_key)
) STRICT;

CREATE TABLE IF NOT EXISTS purchase_documents (
    purchase_document_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    supplier_id TEXT NOT NULL
        REFERENCES suppliers (supplier_id) ON DELETE RESTRICT,
    supplier_reference TEXT CHECK (
        supplier_reference IS NULL
        OR length(trim(supplier_reference)) BETWEEN 1 AND 120
    ),
    currency_code TEXT NOT NULL CHECK (
        length(currency_code) = 3
        AND currency_code = upper(currency_code)
    ),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 3 AND 500),
    received_at_utc TEXT NOT NULL,
    received_by_actor_id TEXT NOT NULL,
    UNIQUE (branch_id, supplier_id, supplier_reference)
) STRICT;

CREATE TABLE IF NOT EXISTS purchase_document_lines (
    purchase_document_line_id TEXT PRIMARY KEY NOT NULL,
    purchase_document_id TEXT NOT NULL
        REFERENCES purchase_documents (purchase_document_id) ON DELETE RESTRICT,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    product_id TEXT NOT NULL
        REFERENCES products (product_id) ON DELETE RESTRICT,
    line_number INTEGER NOT NULL CHECK (line_number >= 1),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    unit_cost_minor INTEGER NOT NULL CHECK (unit_cost_minor >= 0),
    UNIQUE (purchase_document_id, line_number),
    UNIQUE (purchase_document_id, product_id)
) STRICT;

CREATE TRIGGER IF NOT EXISTS suppliers_cannot_be_deleted
BEFORE DELETE ON suppliers
BEGIN
    SELECT RAISE(ABORT, 'suppliers are archive-only');
END;

CREATE TRIGGER IF NOT EXISTS purchase_documents_cannot_be_updated
BEFORE UPDATE ON purchase_documents
BEGIN
    SELECT RAISE(ABORT, 'purchase documents are immutable');
END;

CREATE TRIGGER IF NOT EXISTS purchase_documents_cannot_be_deleted
BEFORE DELETE ON purchase_documents
BEGIN
    SELECT RAISE(ABORT, 'purchase documents must be preserved');
END;

CREATE TRIGGER IF NOT EXISTS purchase_document_lines_cannot_be_updated
BEFORE UPDATE ON purchase_document_lines
BEGIN
    SELECT RAISE(ABORT, 'purchase lines are immutable');
END;

CREATE TRIGGER IF NOT EXISTS purchase_document_lines_cannot_be_deleted
BEFORE DELETE ON purchase_document_lines
BEGIN
    SELECT RAISE(ABORT, 'purchase lines must be preserved');
END;
