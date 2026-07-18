-- Cash drawer accountability uses immutable open and close facts. A close is
-- a separate record so neither opening float nor counted cash can be edited.
CREATE TABLE cash_drawer_sessions (
    cash_drawer_session_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL REFERENCES branches (branch_id) ON DELETE RESTRICT,
    opening_cash_minor INTEGER NOT NULL CHECK (opening_cash_minor >= 0),
    currency_code TEXT NOT NULL,
    opened_at_utc TEXT NOT NULL,
    opened_by_actor_id TEXT NOT NULL
) STRICT;
CREATE TABLE cash_drawer_closures (
    cash_drawer_closure_id TEXT PRIMARY KEY NOT NULL,
    cash_drawer_session_id TEXT NOT NULL UNIQUE REFERENCES cash_drawer_sessions (cash_drawer_session_id) ON DELETE RESTRICT,
    counted_cash_minor INTEGER NOT NULL CHECK (counted_cash_minor >= 0),
    expected_cash_minor INTEGER NOT NULL CHECK (expected_cash_minor >= 0),
    variance_minor INTEGER NOT NULL,
    closed_at_utc TEXT NOT NULL,
    closed_by_actor_id TEXT NOT NULL
) STRICT;
CREATE INDEX cash_drawer_sessions_branch_opened_at ON cash_drawer_sessions (branch_id, opened_at_utc DESC);
CREATE TRIGGER cash_drawer_sessions_cannot_be_updated
BEFORE UPDATE ON cash_drawer_sessions BEGIN SELECT RAISE(ABORT, 'cash drawer sessions are immutable'); END;
CREATE TRIGGER cash_drawer_sessions_cannot_be_deleted
BEFORE DELETE ON cash_drawer_sessions BEGIN SELECT RAISE(ABORT, 'cash drawer sessions must be preserved'); END;
CREATE TRIGGER cash_drawer_closures_cannot_be_updated
BEFORE UPDATE ON cash_drawer_closures BEGIN SELECT RAISE(ABORT, 'cash drawer closures are immutable'); END;
CREATE TRIGGER cash_drawer_closures_cannot_be_deleted
BEFORE DELETE ON cash_drawer_closures BEGIN SELECT RAISE(ABORT, 'cash drawer closures must be preserved'); END;
