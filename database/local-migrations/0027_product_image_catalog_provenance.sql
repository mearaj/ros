-- Gotigin catalogue selections use the same compact, encrypted JPEG storage
-- contract as restaurant uploads, but they must not lose the immutable source
-- record that made the no-attribution selection acceptable.  A companion
-- table avoids rebuilding the already-deployed product_image_versions table.

CREATE TABLE product_image_catalog_provenance (
    image_version_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
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
    FOREIGN KEY (branch_id, product_id, image_version_id)
        REFERENCES product_image_versions (branch_id, product_id, image_version_id)
        ON DELETE RESTRICT,
    FOREIGN KEY (audit_event_id)
        REFERENCES audit_events (event_id)
        ON DELETE RESTRICT,
    UNIQUE (branch_id, product_id, image_version_id)
) STRICT;

CREATE INDEX product_image_catalog_provenance_branch_product
    ON product_image_catalog_provenance (branch_id, product_id);

CREATE TRIGGER product_image_catalog_provenance_must_match_version_and_audit
BEFORE INSERT ON product_image_catalog_provenance
WHEN NOT EXISTS (
    SELECT 1
    FROM product_image_versions AS image_version
    JOIN audit_events AS audit
      ON audit.event_id = NEW.audit_event_id
    WHERE image_version.image_version_id = NEW.image_version_id
      AND image_version.branch_id = NEW.branch_id
      AND image_version.product_id = NEW.product_id
      AND image_version.source_kind = 'restaurant_upload'
      AND audit.branch_id = NEW.branch_id
      AND audit.event_type IN (
          'catalog.product.image.assigned',
          'catalog.product.image.replaced'
      )
      AND json_valid(audit.payload_json)
      AND json_extract(audit.payload_json, '$.entity_id') = NEW.image_version_id
      AND json_extract(audit.payload_json, '$.product_id') = NEW.product_id
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
    SELECT RAISE(ABORT, 'catalog image provenance must match its image version and audit');
END;

CREATE TRIGGER product_image_catalog_provenance_cannot_be_updated
BEFORE UPDATE ON product_image_catalog_provenance
BEGIN
    SELECT RAISE(ABORT, 'catalog image provenance is immutable');
END;

CREATE TRIGGER product_image_catalog_provenance_cannot_be_deleted
BEFORE DELETE ON product_image_catalog_provenance
BEGIN
    SELECT RAISE(ABORT, 'catalog image provenance is append-only');
END;
