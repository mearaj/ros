-- Category imagery is local catalogue presentation data. It follows the same
-- bounded, metadata-free image rules as product images, while remaining
-- separate from financial product and invoice facts.

ALTER TABLE categories ADD COLUMN image_asset_key TEXT;
ALTER TABLE categories ADD COLUMN image_bytes BLOB;
ALTER TABLE categories ADD COLUMN image_content_type TEXT;
ALTER TABLE categories ADD COLUMN image_pixel_width INTEGER;
ALTER TABLE categories ADD COLUMN image_pixel_height INTEGER;
ALTER TABLE categories ADD COLUMN image_sha256 BLOB;

CREATE TRIGGER IF NOT EXISTS categories_image_shape_on_insert
BEFORE INSERT ON categories
WHEN NOT (
    (NEW.image_asset_key IS NULL AND NEW.image_bytes IS NULL
        AND NEW.image_content_type IS NULL AND NEW.image_pixel_width IS NULL
        AND NEW.image_pixel_height IS NULL AND NEW.image_sha256 IS NULL)
    OR
    (NEW.image_asset_key IS NOT NULL AND length(trim(NEW.image_asset_key)) BETWEEN 1 AND 64
        AND NEW.image_bytes IS NULL AND NEW.image_content_type IS NULL
        AND NEW.image_pixel_width IS NULL AND NEW.image_pixel_height IS NULL
        AND NEW.image_sha256 IS NULL)
    OR
    (NEW.image_asset_key IS NULL AND NEW.image_bytes IS NOT NULL
        AND NEW.image_content_type = 'image/jpeg'
        AND NEW.image_pixel_width BETWEEN 1 AND 320
        AND NEW.image_pixel_height BETWEEN 1 AND 240
        AND length(NEW.image_bytes) BETWEEN 1 AND 65536
        AND NEW.image_sha256 IS NOT NULL AND length(NEW.image_sha256) = 32)
)
BEGIN
    SELECT RAISE(ABORT, 'category image shape is invalid');
END;

CREATE TRIGGER IF NOT EXISTS categories_image_shape_on_update
BEFORE UPDATE OF image_asset_key, image_bytes, image_content_type,
    image_pixel_width, image_pixel_height, image_sha256 ON categories
WHEN NOT (
    (NEW.image_asset_key IS NULL AND NEW.image_bytes IS NULL
        AND NEW.image_content_type IS NULL AND NEW.image_pixel_width IS NULL
        AND NEW.image_pixel_height IS NULL AND NEW.image_sha256 IS NULL)
    OR
    (NEW.image_asset_key IS NOT NULL AND length(trim(NEW.image_asset_key)) BETWEEN 1 AND 64
        AND NEW.image_bytes IS NULL AND NEW.image_content_type IS NULL
        AND NEW.image_pixel_width IS NULL AND NEW.image_pixel_height IS NULL
        AND NEW.image_sha256 IS NULL)
    OR
    (NEW.image_asset_key IS NULL AND NEW.image_bytes IS NOT NULL
        AND NEW.image_content_type = 'image/jpeg'
        AND NEW.image_pixel_width BETWEEN 1 AND 320
        AND NEW.image_pixel_height BETWEEN 1 AND 240
        AND length(NEW.image_bytes) BETWEEN 1 AND 65536
        AND NEW.image_sha256 IS NOT NULL AND length(NEW.image_sha256) = 32)
)
BEGIN
    SELECT RAISE(ABORT, 'category image shape is invalid');
END;
