-- Append-only UTC accounting-day close snapshots. Closing records the
-- reconciled totals at close time; there is no reopen path in this schema.
-- Dual-person reopen policy remains founder-gated.

CREATE TABLE IF NOT EXISTS accounting_day_closes (
    accounting_day_close_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    accounting_date_utc TEXT NOT NULL
        CHECK (length(accounting_date_utc) = 10),
    invoice_count INTEGER NOT NULL CHECK (invoice_count >= 0),
    total_minor INTEGER NOT NULL,
    cash_minor INTEGER NOT NULL,
    card_minor INTEGER NOT NULL,
    upi_minor INTEGER NOT NULL,
    refund_minor INTEGER NOT NULL CHECK (refund_minor >= 0),
    expense_minor INTEGER NOT NULL CHECK (expense_minor >= 0),
    discount_minor INTEGER NOT NULL CHECK (discount_minor >= 0),
    tax_minor INTEGER NOT NULL CHECK (tax_minor >= 0),
    currency_code TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 3 AND 500),
    closed_at_utc TEXT NOT NULL,
    closed_by_actor_id TEXT NOT NULL,
    UNIQUE (branch_id, accounting_date_utc)
) STRICT;

CREATE INDEX IF NOT EXISTS accounting_day_closes_branch_date
    ON accounting_day_closes (branch_id, accounting_date_utc DESC);

CREATE TRIGGER IF NOT EXISTS accounting_day_closes_cannot_be_updated
BEFORE UPDATE ON accounting_day_closes
BEGIN
    SELECT RAISE(ABORT, 'accounting day closes are immutable; reopen is not supported');
END;

CREATE TRIGGER IF NOT EXISTS accounting_day_closes_cannot_be_deleted
BEFORE DELETE ON accounting_day_closes
BEGIN
    SELECT RAISE(ABORT, 'accounting day closes must be preserved for audit');
END;
