# Kitchen Cancellation Contract v1

**Status:** Implemented Community Edition local contract  
**Scope:** A sent draft's cancellation or correction after its kitchen ticket
has been created

A kitchen ticket is an operational instruction, not a disposable projection.
Once a draft is sent, the original ticket and draft revision remain retained.
The only supported ways to change that instruction are a reasoned cancellation
or a reasoned reopen that creates a new draft revision.

## Sent-order boundary

- A draft in `sent_to_kitchen` state remains settlement-bound to its saved
  fulfillment and product/quantity snapshot. Counter editing and a second send
  are unavailable for that revision; storage independently rejects a
  settlement command with a different fulfillment or set of lines.
- Only an active Owner or Manager can cancel or reopen an unchanged sent
  draft. Both operations require the current draft revision and a 3–500
  character reason. Stale, completed, settled, or already-cancelled tickets
  are rejected rather than being silently changed.
- A cancellation marks the sent draft cancelled and writes an immutable,
  branch-scoped cancellation notice for its existing kitchen ticket.
- A reopen writes the same immutable notice for the original ticket, then
  creates the next editable draft revision. Sending that revision creates a
  distinct ticket; it never overwrites the original snapshot or ticket.

## Kitchen acknowledgement and privacy

- A cancellation notice is a stop-work signal. While it is unacknowledged, the
  ticket remains in the active KDS queue with cancellation pending and normal
  `new → preparing → ready → completed` progression is blocked.
- An active Kitchen, Manager, or Owner session may acknowledge the notice.
  The acknowledgement is a separate immutable fact. Only then does the old
  cancelled ticket leave the active KDS queue; it is not deleted, and it can
  never resume normal progression.
- KDS receives the ticket's table label and line snapshot plus the
  cancellation-pending signal. It does not receive the counter's free-text
  reason, audit payload, payment data, or prices.

## Retained evidence

The cancellation notice and acknowledgement each have their own append-only
audit event and future-sync outbox event. Their database rows are `STRICT`,
foreign-key bound to the original ticket/revision, and protected by
update/delete rejection triggers. This provides a locally verifiable record of
who requested stop work and who acknowledged it without exposing the rationale
to Kitchen.

## Manual acceptance evidence

For a release candidate, demonstrate all of the following on a clean local
restaurant:

1. Send a draft, restart or refresh the app, and confirm its sent identity is
   restored rather than becoming a new cart.
2. Confirm that the sent revision cannot be edited or sent again, then settle
   it without changing its saved fulfillment or product/quantity snapshot.
3. Send another draft; cancel it as Owner or Manager with a reason. Confirm
   KDS displays stop work without showing the reason and rejects normal ticket
   progression.
4. Acknowledge the stop-work notice from a Kitchen session. Confirm the ticket
   disappears only from the active queue and retained/audit history remains.
5. Reopen a third sent draft as Owner or Manager. Confirm a new draft revision
   and a distinct subsequent ticket are created, while the original ticket
   remains a cancellation record until Kitchen acknowledgement.

This local contract does not claim deployed Professional sync, printer/hardware
integration, or a complete production-release certification.
