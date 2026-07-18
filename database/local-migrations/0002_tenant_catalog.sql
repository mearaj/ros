CREATE TABLE IF NOT EXISTS organizations (
    organization_id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 160),
    created_at_utc TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS branches (
    branch_id TEXT PRIMARY KEY NOT NULL,
    organization_id TEXT NOT NULL
        REFERENCES organizations (organization_id) ON DELETE RESTRICT,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 160),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    time_zone TEXT NOT NULL CHECK (length(trim(time_zone)) BETWEEN 1 AND 64),
    created_at_utc TEXT NOT NULL,
    archived_at_utc TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS categories (
    category_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 120),
    name_key TEXT NOT NULL CHECK (length(name_key) > 0),
    sort_order INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    updated_by_actor_id TEXT NOT NULL,
    archived_at_utc TEXT,
    archived_by_actor_id TEXT,
    archive_reason TEXT,
    UNIQUE (branch_id, category_id),
    CHECK (
        (archived_at_utc IS NULL AND archived_by_actor_id IS NULL AND archive_reason IS NULL)
        OR
        (
            archived_at_utc IS NOT NULL
            AND archived_by_actor_id IS NOT NULL
            AND length(trim(archive_reason)) > 0
        )
    )
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS categories_active_name_key
    ON categories (branch_id, name_key)
    WHERE archived_at_utc IS NULL;

CREATE TABLE IF NOT EXISTS products (
    product_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    category_id TEXT,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 160),
    name_key TEXT NOT NULL CHECK (length(name_key) > 0),
    sku TEXT,
    barcode TEXT,
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    is_available INTEGER NOT NULL DEFAULT 1 CHECK (is_available IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    updated_by_actor_id TEXT NOT NULL,
    archived_at_utc TEXT,
    archived_by_actor_id TEXT,
    archive_reason TEXT,
    FOREIGN KEY (branch_id, category_id)
        REFERENCES categories (branch_id, category_id) ON DELETE RESTRICT,
    CHECK (archived_at_utc IS NULL OR is_available = 0),
    CHECK (
        (archived_at_utc IS NULL AND archived_by_actor_id IS NULL AND archive_reason IS NULL)
        OR
        (
            archived_at_utc IS NOT NULL
            AND archived_by_actor_id IS NOT NULL
            AND length(trim(archive_reason)) > 0
        )
    )
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS products_branch_sku
    ON products (branch_id, sku)
    WHERE sku IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS products_branch_barcode
    ON products (branch_id, barcode)
    WHERE barcode IS NOT NULL;

CREATE INDEX IF NOT EXISTS products_sale_menu
    ON products (branch_id, category_id, sort_order, name_key)
    WHERE archived_at_utc IS NULL AND is_available = 1;

CREATE TRIGGER IF NOT EXISTS audit_events_cannot_be_updated
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS audit_events_cannot_be_deleted
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit events are append-only');
END;

CREATE TRIGGER IF NOT EXISTS categories_cannot_be_deleted
BEFORE DELETE ON categories
BEGIN
    SELECT RAISE(ABORT, 'categories must be archived, not deleted');
END;

CREATE TRIGGER IF NOT EXISTS products_cannot_be_deleted
BEFORE DELETE ON products
BEGIN
    SELECT RAISE(ABORT, 'products must be archived, not deleted');
END;

CREATE TRIGGER IF NOT EXISTS products_must_use_branch_currency_on_insert
BEFORE INSERT ON products
WHEN (
    SELECT currency_code
    FROM branches
    WHERE branch_id = NEW.branch_id
) <> NEW.currency_code
BEGIN
    SELECT RAISE(ABORT, 'product currency must match its branch currency');
END;

CREATE TRIGGER IF NOT EXISTS products_must_use_branch_currency_on_update
BEFORE UPDATE OF branch_id, currency_code ON products
WHEN (
    SELECT currency_code
    FROM branches
    WHERE branch_id = NEW.branch_id
) <> NEW.currency_code
BEGIN
    SELECT RAISE(ABORT, 'product currency must match its branch currency');
END;
