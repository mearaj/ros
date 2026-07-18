# Catalogue Availability Contract v1

**Status:** Implemented Community Edition local contract  
**Scope:** Temporarily pausing or resuming an active menu item

`is_available` is a checkout eligibility state, not a deletion mechanism. An
owner or manager may mark a non-archived product sold out, then resume it when
it can be sold again. Both commands require the current product revision and a
3–500-character reason.

## Invariants

- A sold-out product remains visible to catalogue managers, with its image,
  price, revision, and retained history.
- The POS excludes sold-out products, and the Rust sale transaction rejects
  them again inside the encrypted database. A stale client cannot sell one.
- Every transition appends a device-hash-chained audit event and a future-sync
  outbox envelope. No history is overwritten or deleted.
- A sale can use only an available product. Archiving a product remains a
  separate, more permanent removal from the active catalogue.
- Price changes are allowed while sold out so an owner can prepare a correct
  price before resuming sale. The change preserves the unavailable state,
  increments the revision, and is independently audited/syncable.
- Optimistic revisions reject stale pause/resume/price commands. Repeating the
  current availability state is a conflict rather than a silent no-op.

## Interface behavior

The Menu workspace labels each item as **selling now** or **sold out**. Its
management menu asks for a reason before `Mark sold out` or `Resume selling`.
The counter never treats this Flutter filter as the authority; it is only a
clear, responsive reflection of the Rust/SQLCipher enforcement above.
