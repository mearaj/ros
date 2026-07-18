CREATE TABLE IF NOT EXISTS restaurant_tables (
    table_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    display_name TEXT NOT NULL
        CHECK (length(trim(display_name)) BETWEEN 1 AND 120),
    name_key TEXT NOT NULL
        CHECK (length(trim(name_key)) BETWEEN 1 AND 120),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    updated_by_actor_id TEXT NOT NULL,
    UNIQUE (branch_id, name_key)
) STRICT;

CREATE TABLE IF NOT EXISTS draft_orders (
    draft_order_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    table_id TEXT
        REFERENCES restaurant_tables (table_id) ON DELETE RESTRICT,
    fulfillment TEXT NOT NULL CHECK (fulfillment IN ('dine_in', 'takeaway')),
    draft_state TEXT NOT NULL CHECK (draft_state IN ('open', 'sent_to_kitchen', 'cancelled')),
    currency_code TEXT NOT NULL
        CHECK (length(currency_code) = 3 AND currency_code GLOB '[A-Z][A-Z][A-Z]'),
    current_revision INTEGER NOT NULL CHECK (current_revision >= 1),
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    updated_by_actor_id TEXT NOT NULL,
    CHECK ((fulfillment = 'dine_in' AND table_id IS NOT NULL) OR fulfillment = 'takeaway')
) STRICT;

CREATE INDEX IF NOT EXISTS draft_orders_branch_state_updated
    ON draft_orders (branch_id, draft_state, updated_at_utc DESC);

CREATE UNIQUE INDEX IF NOT EXISTS one_open_dine_in_draft_per_table
    ON draft_orders (branch_id, table_id)
    WHERE draft_state IN ('open', 'sent_to_kitchen') AND table_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS draft_order_revisions (
    draft_order_revision_id TEXT PRIMARY KEY NOT NULL,
    draft_order_id TEXT NOT NULL
        REFERENCES draft_orders (draft_order_id) ON DELETE RESTRICT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    subtotal_minor INTEGER NOT NULL CHECK (subtotal_minor >= 0),
    line_count INTEGER NOT NULL CHECK (line_count > 0),
    line_snapshot_json TEXT NOT NULL CHECK (json_valid(line_snapshot_json)),
    saved_at_utc TEXT NOT NULL,
    saved_by_actor_id TEXT NOT NULL,
    UNIQUE (draft_order_id, revision)
) STRICT;

CREATE TRIGGER IF NOT EXISTS restaurant_tables_cannot_be_deleted
BEFORE DELETE ON restaurant_tables
BEGIN
    SELECT RAISE(ABORT, 'restaurant tables must be archived, not deleted');
END;

CREATE TRIGGER IF NOT EXISTS draft_orders_cannot_be_deleted
BEFORE DELETE ON draft_orders
BEGIN
    SELECT RAISE(ABORT, 'draft orders must be preserved for audit');
END;

CREATE TRIGGER IF NOT EXISTS draft_order_revisions_cannot_be_updated
BEFORE UPDATE ON draft_order_revisions
BEGIN
    SELECT RAISE(ABORT, 'draft order revisions are append-only');
END;

CREATE TRIGGER IF NOT EXISTS draft_order_revisions_cannot_be_deleted
BEFORE DELETE ON draft_order_revisions
BEGIN
    SELECT RAISE(ABORT, 'draft order revisions are append-only');
END;
