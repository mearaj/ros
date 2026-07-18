-- Accountable operating expenses are immutable branch facts. Corrections are
-- separate future facts; a recorded expense is never edited or deleted.
CREATE TABLE expenses (
    expense_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL REFERENCES branches (branch_id) ON DELETE RESTRICT,
    category TEXT NOT NULL CHECK (length(trim(category)) BETWEEN 1 AND 120),
    description TEXT NOT NULL CHECK (length(trim(description)) BETWEEN 3 AND 500),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    currency_code TEXT NOT NULL,
    payment_method TEXT NOT NULL CHECK (payment_method IN ('cash', 'card', 'upi')),
    incurred_at_utc TEXT NOT NULL,
    recorded_by_actor_id TEXT NOT NULL
) STRICT;
CREATE INDEX expenses_branch_incurred_at
    ON expenses (branch_id, incurred_at_utc DESC);
CREATE TRIGGER expenses_cannot_be_updated
BEFORE UPDATE ON expenses
BEGIN SELECT RAISE(ABORT, 'expenses are immutable'); END;
CREATE TRIGGER expenses_cannot_be_deleted
BEFORE DELETE ON expenses
BEGIN SELECT RAISE(ABORT, 'expenses must be preserved'); END;
