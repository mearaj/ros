-- Provider-neutral tax profiles and order-level pricing snapshots.
-- This migration does not encode GST, HSN/SAC, e-invoicing, or any other
-- jurisdiction-specific fiscal claim. Named rates are restaurant-configured
-- arithmetic inputs only.

CREATE TABLE IF NOT EXISTS branch_tax_rates (
    tax_rate_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    display_name TEXT NOT NULL
        CHECK (length(trim(display_name)) BETWEEN 1 AND 120),
    name_key TEXT NOT NULL
        CHECK (length(trim(name_key)) BETWEEN 1 AND 120),
    basis_points INTEGER NOT NULL
        CHECK (basis_points BETWEEN 0 AND 10000),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    updated_by_actor_id TEXT NOT NULL,
    archived_at_utc TEXT,
    archived_by_actor_id TEXT,
    archive_reason TEXT,
    archive_audit_event_id TEXT UNIQUE
        REFERENCES audit_events (event_id) ON DELETE RESTRICT,
    CHECK (
        (archived_at_utc IS NULL
            AND archived_by_actor_id IS NULL
            AND archive_reason IS NULL
            AND archive_audit_event_id IS NULL)
        OR
        (archived_at_utc IS NOT NULL
            AND archived_by_actor_id IS NOT NULL
            AND archive_reason IS NOT NULL
            AND archive_audit_event_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX IF NOT EXISTS branch_tax_rates_branch_active
    ON branch_tax_rates (branch_id, name_key, tax_rate_id)
    WHERE archived_at_utc IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS active_branch_tax_rates_name
    ON branch_tax_rates (branch_id, name_key)
    WHERE archived_at_utc IS NULL;

CREATE TRIGGER branch_tax_rates_immutable_facts
BEFORE UPDATE OF branch_id, display_name, name_key, basis_points,
                 created_at_utc, created_by_actor_id
ON branch_tax_rates
BEGIN
    SELECT RAISE(ABORT, 'branch tax rate facts are immutable; archive and create a replacement');
END;

CREATE TRIGGER branch_tax_rates_archive_only
BEFORE UPDATE ON branch_tax_rates
WHEN OLD.archived_at_utc IS NOT NULL
    OR NEW.archived_at_utc IS NULL
    OR NEW.archived_by_actor_id IS NULL
    OR NEW.archive_reason IS NULL
    OR NEW.archive_audit_event_id IS NULL
    OR NEW.revision <> OLD.revision + 1
    OR NEW.updated_at_utc <> NEW.archived_at_utc
    OR NEW.updated_by_actor_id <> NEW.archived_by_actor_id
BEGIN
    SELECT RAISE(ABORT, 'branch tax rates may only be archived through an audited revision');
END;

CREATE TRIGGER branch_tax_rates_cannot_be_deleted
BEFORE DELETE ON branch_tax_rates
BEGIN
    SELECT RAISE(ABORT, 'branch tax rates are archive-only');
END;

ALTER TABLE products ADD COLUMN tax_treatment TEXT NOT NULL DEFAULT 'no_tax'
    CHECK (tax_treatment IN ('no_tax', 'exclusive', 'inclusive'));

ALTER TABLE orders ADD COLUMN discount_minor INTEGER NOT NULL DEFAULT 0
    CHECK (discount_minor >= 0);
ALTER TABLE orders ADD COLUMN tax_minor INTEGER NOT NULL DEFAULT 0
    CHECK (tax_minor >= 0);
ALTER TABLE orders ADD COLUMN pricing_snapshot_json TEXT NOT NULL DEFAULT '{}';

ALTER TABLE invoices ADD COLUMN discount_minor INTEGER NOT NULL DEFAULT 0
    CHECK (discount_minor >= 0);
ALTER TABLE invoices ADD COLUMN tax_minor INTEGER NOT NULL DEFAULT 0
    CHECK (tax_minor >= 0);
ALTER TABLE invoices ADD COLUMN pricing_snapshot_json TEXT NOT NULL DEFAULT '{}';
