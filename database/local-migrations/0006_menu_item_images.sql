-- Menu images are deliberately modeled outside the financial product record.
-- A replacement creates a new immutable image version; the assignment is a
-- small, revisioned pointer. This keeps restaurant-owned images inside the
-- encrypted SQLCipher database and retains the history needed to explain a
-- catalogue change without making images part of an invoice snapshot.

CREATE UNIQUE INDEX IF NOT EXISTS products_branch_product_identity
    ON products (branch_id, product_id);

CREATE TABLE IF NOT EXISTS product_image_versions (
    image_version_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    source_kind TEXT NOT NULL
        CHECK (source_kind IN ('built_in', 'restaurant_upload')),
    asset_key TEXT,
    content_type TEXT,
    image_bytes BLOB,
    pixel_width INTEGER,
    pixel_height INTEGER,
    byte_length INTEGER NOT NULL CHECK (byte_length BETWEEN 0 AND 65536),
    content_sha256 BLOB,
    created_at_utc TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL,
    FOREIGN KEY (branch_id, product_id)
        REFERENCES products (branch_id, product_id) ON DELETE RESTRICT,
    UNIQUE (branch_id, product_id, image_version_id),
    CHECK (
        (
            source_kind = 'built_in'
            AND asset_key IS NOT NULL
            AND length(trim(asset_key)) BETWEEN 1 AND 64
            AND content_type IS NULL
            AND image_bytes IS NULL
            AND pixel_width IS NULL
            AND pixel_height IS NULL
            AND byte_length = 0
            AND content_sha256 IS NULL
        )
        OR
        (
            source_kind = 'restaurant_upload'
            AND asset_key IS NULL
            AND content_type = 'image/jpeg'
            AND image_bytes IS NOT NULL
            AND pixel_width BETWEEN 1 AND 320
            AND pixel_height BETWEEN 1 AND 240
            AND byte_length = length(image_bytes)
            AND byte_length BETWEEN 1 AND 65536
            AND content_sha256 IS NOT NULL
            AND length(content_sha256) = 32
        )
    )
) STRICT;

CREATE TABLE IF NOT EXISTS product_image_assignments (
    product_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL,
    image_version_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    assigned_at_utc TEXT NOT NULL,
    assigned_by_actor_id TEXT NOT NULL,
    FOREIGN KEY (branch_id, product_id, image_version_id)
        REFERENCES product_image_versions (branch_id, product_id, image_version_id)
        ON DELETE RESTRICT,
    UNIQUE (branch_id, product_id)
) STRICT;

CREATE INDEX IF NOT EXISTS product_image_assignments_branch_product
    ON product_image_assignments (branch_id, product_id);

CREATE TRIGGER IF NOT EXISTS product_image_versions_cannot_be_updated
BEFORE UPDATE ON product_image_versions
BEGIN
    SELECT RAISE(ABORT, 'product image versions are immutable');
END;

CREATE TRIGGER IF NOT EXISTS product_image_versions_cannot_be_deleted
BEFORE DELETE ON product_image_versions
BEGIN
    SELECT RAISE(ABORT, 'product image versions are append-only');
END;

CREATE TRIGGER IF NOT EXISTS product_image_assignments_cannot_be_deleted
BEFORE DELETE ON product_image_assignments
BEGIN
    SELECT RAISE(ABORT, 'product image assignments are retained for audit');
END;

CREATE TRIGGER IF NOT EXISTS product_image_assignments_identity_is_immutable
BEFORE UPDATE OF product_id, branch_id ON product_image_assignments
BEGIN
    SELECT RAISE(ABORT, 'product image assignment identity is immutable');
END;

CREATE TRIGGER IF NOT EXISTS product_image_assignments_must_advance_revision
BEFORE UPDATE ON product_image_assignments
WHEN NEW.image_version_id = OLD.image_version_id
    OR NEW.revision <> OLD.revision + 1
    OR NEW.assigned_at_utc < OLD.assigned_at_utc
BEGIN
    SELECT RAISE(ABORT, 'product image assignment must advance to a new version');
END;
