# Local audit timeline contract v1

## Scope

This contract describes the Community Edition owner-facing view of the recent
local audit trail. It is a read-only operational aid, not an export API or a
replacement for Professional cloud anchoring.

## Access and verification

- Only the active local `Owner` session can load the timeline.
- Before the timeline is returned, Rust verifies SQLCipher integrity, the
  schema contract, foreign keys, and every local device audit chain.
- The timeline is branch-scoped, ordered newest first, and bounded to the most
  recent 100 events so an extensive history cannot freeze the client.

## Safe projection

Flutter receives only an event sequence, event type, and UTC timestamp. It
never receives the JSON audit payload, event hash, predecessor hash, actor ID,
device ID, credential material, or raw database data. This keeps the report
screen from becoming an accidental diagnostic-data or credential-exfiltration
surface.

## Integrity boundary

The timeline is derived from the existing append-only `audit_events` table; it
does not create, edit, acknowledge, or delete an audit event. Professional
remote anchoring, cross-device audit search, and a formal customer-data export
remain outside this Community v1 contract.
