-- Customer identity is intentionally separated from mutable personal data.
-- Contact corrections and anonymization are appended as new profile facts so
-- a financial order can retain a stable customer relationship without keeping
-- stale contact details as the current profile.
CREATE TABLE IF NOT EXISTS customers (
    customer_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL REFERENCES branches(branch_id),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS customer_profile_revisions (
    customer_profile_revision_id TEXT PRIMARY KEY NOT NULL,
    customer_id TEXT NOT NULL REFERENCES customers(customer_id),
    revision INTEGER NOT NULL CHECK (revision > 0),
    display_name TEXT NOT NULL,
    phone_number TEXT,
    email_address TEXT,
    marketing_consent INTEGER NOT NULL CHECK (marketing_consent IN (0, 1)),
    profile_state TEXT NOT NULL CHECK (profile_state IN ('active', 'anonymized')),
    changed_at_utc TEXT NOT NULL,
    changed_by_actor_id TEXT NOT NULL,
    reason TEXT,
    UNIQUE (customer_id, revision)
) STRICT;

CREATE INDEX IF NOT EXISTS customer_profile_revisions_current
    ON customer_profile_revisions (customer_id, revision DESC);

ALTER TABLE orders ADD COLUMN customer_id TEXT REFERENCES customers(customer_id);
CREATE INDEX IF NOT EXISTS orders_branch_customer
    ON orders (branch_id, customer_id)
    WHERE customer_id IS NOT NULL;

CREATE TRIGGER IF NOT EXISTS customers_cannot_be_updated
BEFORE UPDATE ON customers
BEGIN
    SELECT RAISE(ABORT, 'customers are immutable');
END;

CREATE TRIGGER IF NOT EXISTS customers_cannot_be_deleted
BEFORE DELETE ON customers
BEGIN
    SELECT RAISE(ABORT, 'customers cannot be deleted; append an anonymized profile');
END;

CREATE TRIGGER IF NOT EXISTS customer_profile_revisions_cannot_be_updated
BEFORE UPDATE ON customer_profile_revisions
BEGIN
    SELECT RAISE(ABORT, 'customer profile revisions are immutable');
END;

CREATE TRIGGER IF NOT EXISTS customer_profile_revisions_cannot_be_deleted
BEFORE DELETE ON customer_profile_revisions
BEGIN
    SELECT RAISE(ABORT, 'customer profile revisions cannot be deleted');
END;
