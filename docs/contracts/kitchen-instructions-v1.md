# Kitchen instruction contract v1

An optional kitchen instruction is operational text attached to one saved
order revision, such as an allergen-handling or preparation request. It is not
a cancellation rationale, payment note, customer profile field, or staff
management reason.

## Boundary

- Counter users may add, replace, or clear an instruction only while the draft
  order is open.
- Storage trims the value, treats blank input as absent, rejects control
  characters, and accepts at most 500 characters.
- The instruction is retained in `draft_order_revisions`; a Kitchen ticket
  takes a separate immutable snapshot when the draft is sent.
- Reopening a kitchen-sent order retains its instruction in the new editable
  revision. A changed instruction becomes a later retained revision, never an
  edit to an earlier ticket.
- Kitchen Display receives the instruction snapshot and no management or
  cancellation rationale. It receives no prices or payment data.

## Integrity and privacy

- An instruction's text is not duplicated in the audit event payload or sync
  event metadata. Those payloads record only `kitchen_note_present`.
- Database triggers reject direct replacement of a ticket instruction and
  reject a ticket whose instruction does not match its bound draft revision.
- The instruction remains inside the encrypted local database and its
  verified encrypted backups. It should contain only information a kitchen
  needs to prepare that order; do not use it for payment credentials, identity
  documents, or internal staff allegations.

## Counter experience

The Counter offers **Add kitchen instruction** on an unsent order. The text is
saved with the next hold/open-order revision and becomes locked after Kitchen
send. The Kitchen Display labels it clearly as **Kitchen instruction** so it
is not confused with the cancellation stop-work signal.
