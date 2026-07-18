-- Full invoice voids are new immutable facts. A void never rewrites the
-- original invoice, payment, or sale audit event. Refunded invoices cannot
-- be voided; voided invoices cannot be refunded.

CREATE TABLE IF NOT EXISTS invoice_voids (
    invoice_void_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    invoice_id TEXT NOT NULL UNIQUE
        REFERENCES invoices (invoice_id) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 3 AND 500),
    voided_at_utc TEXT NOT NULL,
    voided_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS invoice_voids_branch_time
    ON invoice_voids (branch_id, voided_at_utc);

CREATE TRIGGER IF NOT EXISTS invoice_voids_cannot_be_updated
BEFORE UPDATE ON invoice_voids
BEGIN
    SELECT RAISE(ABORT, 'voids are immutable; record a new correction');
END;

CREATE TRIGGER IF NOT EXISTS invoice_voids_cannot_be_deleted
BEFORE DELETE ON invoice_voids
BEGIN
    SELECT RAISE(ABORT, 'voids must be preserved for audit');
END;

CREATE TRIGGER IF NOT EXISTS invoice_voids_must_match_invoice_without_refunds
BEFORE INSERT ON invoice_voids
WHEN NOT EXISTS (
    SELECT 1
    FROM invoices
    WHERE invoices.invoice_id = NEW.invoice_id
      AND invoices.branch_id = NEW.branch_id
)
OR EXISTS (
    SELECT 1
    FROM invoice_refunds
    WHERE invoice_refunds.invoice_id = NEW.invoice_id
)
BEGIN
    SELECT RAISE(ABORT, 'void must match its invoice and cannot follow a refund');
END;

CREATE TRIGGER IF NOT EXISTS invoice_refunds_cannot_follow_void
BEFORE INSERT ON invoice_refunds
WHEN EXISTS (
    SELECT 1
    FROM invoice_voids
    WHERE invoice_voids.invoice_id = NEW.invoice_id
)
BEGIN
    SELECT RAISE(ABORT, 'refunds cannot follow a voided invoice');
END;
