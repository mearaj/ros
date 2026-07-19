-- A verified Gotigin catalogue image can also be used as a category visual.
-- Categories keep only their current display image, so preserve every remote
-- selection in an append-only companion record linked to its audited category
-- revision. This keeps the licence and integrity proof even after the image is
-- changed again.

CREATE TABLE category_image_catalog_provenance (
    category_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    category_revision INTEGER NOT NULL CHECK (category_revision > 0),
    catalog_image_id TEXT NOT NULL
        CHECK (
            length(trim(catalog_image_id)) BETWEEN 1 AND 128
            AND catalog_image_id = trim(catalog_image_id)
            AND catalog_image_id NOT GLOB '*[^A-Za-z0-9._:-]*'
        ),
    original_content_sha256 BLOB NOT NULL
        CHECK (length(original_content_sha256) = 32),
    licence_label TEXT NOT NULL
        CHECK (
            length(trim(licence_label)) BETWEEN 1 AND 160
            AND licence_label = trim(licence_label)
        ),
    licence_url TEXT NOT NULL
        CHECK (
            length(licence_url) BETWEEN 9 AND 2048
            AND licence_url = trim(licence_url)
            AND licence_url GLOB 'https://?*'
            AND instr(licence_url, '#') = 0
        ),
    service_origin TEXT NOT NULL
        CHECK (service_origin = 'https://ros.gotigin.com'),
    service_schema_version INTEGER NOT NULL
        CHECK (service_schema_version = 1),
    audit_event_id TEXT NOT NULL UNIQUE,
    PRIMARY KEY (category_id, category_revision),
    FOREIGN KEY (branch_id, category_id)
        REFERENCES categories (branch_id, category_id) ON DELETE RESTRICT,
    FOREIGN KEY (audit_event_id)
        REFERENCES audit_events (event_id) ON DELETE RESTRICT,
    UNIQUE (branch_id, category_id, category_revision)
) STRICT;

CREATE INDEX category_image_catalog_provenance_branch_category
    ON category_image_catalog_provenance (branch_id, category_id);

CREATE TRIGGER category_image_catalog_provenance_must_match_category_and_audit
BEFORE INSERT ON category_image_catalog_provenance
WHEN NOT EXISTS (
    SELECT 1
    FROM categories AS category
    JOIN audit_events AS audit
      ON audit.event_id = NEW.audit_event_id
    WHERE category.category_id = NEW.category_id
      AND category.branch_id = NEW.branch_id
      AND category.revision = NEW.category_revision
      AND category.image_asset_key IS NULL
      AND category.image_bytes IS NOT NULL
      AND audit.branch_id = NEW.branch_id
      AND audit.event_type = 'catalog.category.image.updated'
      AND json_valid(audit.payload_json)
      AND json_extract(audit.payload_json, '$.entity_id') = NEW.category_id
      AND json_extract(audit.payload_json, '$.revision') = NEW.category_revision
      AND json_extract(audit.payload_json, '$.after.source_kind') = 'gotigin_catalog'
      AND json_extract(audit.payload_json, '$.after.catalog.image_id') = NEW.catalog_image_id
      AND json_extract(audit.payload_json, '$.after.catalog.original_content_sha256')
          = lower(hex(NEW.original_content_sha256))
      AND json_extract(audit.payload_json, '$.after.catalog.licence_label') = NEW.licence_label
      AND json_extract(audit.payload_json, '$.after.catalog.licence_url') = NEW.licence_url
      AND json_extract(audit.payload_json, '$.after.catalog.service_origin') = NEW.service_origin
      AND json_extract(audit.payload_json, '$.after.catalog.service_schema_version')
          = NEW.service_schema_version
)
BEGIN
    SELECT RAISE(ABORT, 'category catalogue image provenance must match its image and audit');
END;

CREATE TRIGGER category_image_catalog_provenance_cannot_be_updated
BEFORE UPDATE ON category_image_catalog_provenance
BEGIN
    SELECT RAISE(ABORT, 'category catalogue image provenance is immutable');
END;

CREATE TRIGGER category_image_catalog_provenance_cannot_be_deleted
BEFORE DELETE ON category_image_catalog_provenance
BEGIN
    SELECT RAISE(ABORT, 'category catalogue image provenance is append-only');
END;
