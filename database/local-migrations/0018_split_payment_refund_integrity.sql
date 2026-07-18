-- Split-tender foundation. Existing invoices have one payment, so their
-- migration sequence is safely initialized to 1. New sales may append further
-- immutable payment allocations within the same finalization transaction.
ALTER TABLE payments
    ADD COLUMN payment_sequence INTEGER NOT NULL DEFAULT 1
        CHECK (payment_sequence > 0);

CREATE UNIQUE INDEX IF NOT EXISTS payments_invoice_sequence
    ON payments (invoice_id, payment_sequence);

DROP INDEX IF EXISTS payments_one_recorded_payment_per_invoice;
DROP TRIGGER IF EXISTS payments_must_match_finalized_invoice;

CREATE TRIGGER IF NOT EXISTS payments_must_match_finalized_invoice
BEFORE INSERT ON payments
WHEN NOT EXISTS (
    SELECT 1
    FROM invoices
    WHERE invoices.invoice_id = NEW.invoice_id
      AND invoices.branch_id = NEW.branch_id
      AND invoices.currency_code = NEW.currency_code
      AND invoices.invoice_state = 'finalized'
)
OR NEW.amount_minor > (
    SELECT invoices.total_minor - COALESCE(SUM(payments.amount_minor), 0)
    FROM invoices
    LEFT JOIN payments ON payments.invoice_id = invoices.invoice_id
    WHERE invoices.invoice_id = NEW.invoice_id
      AND invoices.branch_id = NEW.branch_id
)
BEGIN
    SELECT RAISE(ABORT, 'payment must match its finalized invoice without over-allocation');
END;

-- A refund allocation must not exceed what was originally received through
-- that payment method after earlier immutable refunds of the same method.
-- This preserves cash-drawer and method reporting integrity for split tender.
DROP TRIGGER IF EXISTS invoice_refunds_must_match_invoice;

CREATE TRIGGER IF NOT EXISTS invoice_refunds_must_match_invoice
BEFORE INSERT ON invoice_refunds
WHEN NOT EXISTS (
    SELECT 1
    FROM invoices
    WHERE invoices.invoice_id = NEW.invoice_id
      AND invoices.branch_id = NEW.branch_id
      AND invoices.currency_code = NEW.currency_code
)
OR NEW.amount_minor > (
    SELECT
        COALESCE(SUM(payments.amount_minor), 0)
        - COALESCE((
            SELECT SUM(invoice_refunds.amount_minor)
            FROM invoice_refunds
            WHERE invoice_refunds.invoice_id = NEW.invoice_id
              AND invoice_refunds.payment_method_snapshot = NEW.payment_method_snapshot
        ), 0)
    FROM payments
    WHERE payments.invoice_id = NEW.invoice_id
      AND payments.branch_id = NEW.branch_id
      AND payments.payment_method = NEW.payment_method_snapshot
)
BEGIN
    SELECT RAISE(ABORT, 'refund must match an available original payment-method balance');
END;
