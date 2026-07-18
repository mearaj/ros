-- Financial corrections are new immutable facts. A refund never rewrites an
-- invoice, payment, or original sale audit event.

CREATE TABLE IF NOT EXISTS invoice_refunds (
    invoice_refund_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    invoice_id TEXT NOT NULL
        REFERENCES invoices (invoice_id) ON DELETE RESTRICT,
    payment_method_snapshot TEXT NOT NULL
        CHECK (payment_method_snapshot IN ('cash', 'card', 'upi')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 3 AND 500),
    refunded_at_utc TEXT NOT NULL,
    refunded_by_actor_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS invoice_refunds_branch_time
    ON invoice_refunds (branch_id, refunded_at_utc);

CREATE TRIGGER IF NOT EXISTS invoice_refunds_cannot_be_updated
BEFORE UPDATE ON invoice_refunds
BEGIN
    SELECT RAISE(ABORT, 'refunds are immutable; record a new correction');
END;

CREATE TRIGGER IF NOT EXISTS invoice_refunds_cannot_be_deleted
BEFORE DELETE ON invoice_refunds
BEGIN
    SELECT RAISE(ABORT, 'refunds must be preserved for audit');
END;

CREATE TRIGGER IF NOT EXISTS invoice_refunds_must_match_invoice
BEFORE INSERT ON invoice_refunds
WHEN NOT EXISTS (
    SELECT 1
    FROM invoices
    JOIN payments ON payments.invoice_id = invoices.invoice_id
    WHERE invoices.invoice_id = NEW.invoice_id
      AND invoices.branch_id = NEW.branch_id
      AND invoices.currency_code = NEW.currency_code
      AND payments.payment_method = NEW.payment_method_snapshot
)
OR NEW.amount_minor > (
    SELECT invoices.total_minor - COALESCE(SUM(invoice_refunds.amount_minor), 0)
    FROM invoices
    LEFT JOIN invoice_refunds ON invoice_refunds.invoice_id = invoices.invoice_id
    WHERE invoices.invoice_id = NEW.invoice_id
      AND invoices.branch_id = NEW.branch_id
)
BEGIN
    SELECT RAISE(ABORT, 'refund must match its invoice and cannot exceed the recorded total');
END;
