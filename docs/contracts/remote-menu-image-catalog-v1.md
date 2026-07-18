# Remote menu-image catalogue v1

## Status

**Client integration is ready; service provisioning is pending.** The client
uses the release-controlled provisional origin `https://ros.gotigin.com` and
shows a clear, non-blocking unavailable state until that service is deployed.
A URL entered by a restaurant operator is not an acceptable catalogue origin:
it would weaken the offline-first, licensing, and content-integrity model.

## Product model

The app has three image sources:

1. **Embedded app photos** — the offline default. The target catalogue is 100
   curated, food-only world-food thumbnails. The current 20-image pack is a
   quality-controlled seed, not a claim that the 100-image acceptance target
   is complete. Every future entry needs the same provenance record as the
   existing pack before it is embedded.
2. **Gotigin online catalogue** — a larger, searchable optional collection.
   It augments, never replaces, the embedded offline pack.
3. **Restaurant-owned upload** — selected from the device after the operator
   confirms its rights to use the image.

## Required service contract

The client calls:

```text
GET {origin}/v1/menu-images/search?q={query}&cursor={opaque-cursor}&limit=24
```

The response must be JSON and contain a versioned page plus immutable image
descriptors:

```json
{
  "schema_version": 1,
  "next_cursor": "opaque-or-null",
  "items": [
    {
      "image_id": "01J...",
      "display_name": "Chicken biryani",
      "tags": ["indian", "rice", "main"],
      "thumbnail_url": "https://images.gotigin.com/menu/.../thumb.webp",
      "image_url": "https://images.gotigin.com/menu/.../source.webp",
      "content_sha256": "lowercase-64-character-hex",
      "content_type": "image/webp",
      "width": 1600,
      "height": 1200,
      "licence": "Pexels License",
      "licence_url": "https://www.pexels.com/legal-pages/license/",
      "attribution_required": false
    }
  ]
}
```

## Non-negotiable client rules

- The release configuration owns an HTTPS allow-list for the catalogue origin;
  restaurant input cannot change it.
- The client displays only compact thumbnails in its transient cache. It must
  enforce response pagination, MIME type, byte limits, dimensions, and a
  cache quota before decoding.
- Selecting a catalogue image downloads its immutable content, verifies the
  published SHA-256, then passes bytes through the same Rust normalisation and
  encrypted append-only image-version flow as a restaurant upload. A database
  record must never depend on a live third-party URL.
- The UI remains fully usable without the service. Failed search, expired
  cache, captive portals, or a removed remote image must not affect existing
  menu items or the counter.
- The catalogue service stores source licence/provenance and runs a review
  process that rejects recognizable people, logos, packaging, and trademarks.

## Embedded 100-image acceptance gate

Before changing the UI label from “app photos” to “100 app photos,” the pack
must contain exactly 100 distinct, food-focused assets, each with a source
page, contributor, licence record, downloaded date, and derivative SHA-256 in
the provenance manifest. Assets remain small menu thumbnails; the embedded
pack must be measured as part of each Android, iOS, Linux, and desktop release
size review.
