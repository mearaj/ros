# Inventory ledger v1

Inventory will be modelled as append-only movements, not a mutable stock value.

Each movement records: UUIDv7 identity, branch, product, signed whole-unit
quantity, type (`opening`, `purchase`, `sale`, `waste`, `adjustment`), reason
where required, actor, device, UTC timestamp, and optional source document.

The current balance is derived as `SUM(quantity_delta)` for a branch/product.
Sale deduction, stock checks, movement insertion, audit event, and sync-outbox
event must commit in the same local transaction. A movement cannot be updated
or deleted. A negative balance is rejected unless an explicitly approved
backorder policy is added in a later version.

This contract intentionally separates product sellability from stock tracking:
restaurants can sell untracked menu items, while a tracked item cannot be
finalized below zero. Purchases, waste, and adjustments require their own
reasoned owner/manager commands and correction flows.

Current implementation: the encrypted local schema, open-time contract
verification, derived-balance read, and opening-stock, purchase, waste, and
adjustment commands are implemented. They are immutable, audited, outbox-backed
movements; opening stock is permitted only as the first movement, and
stock-control commands require the Owner or Manager role. Waste
and adjustment require a reason; the database rejects any movement that would
take a tracked balance below zero. A product becomes tracked at its first
movement; completed sales deduct tracked product quantities in the same
transaction as the order, invoice, payment, audit, and sync-outbox facts. UI
is available from the Community menu workspace for viewing balances and
recording validated movements.

Low-stock policy is now included as a narrow extension: an Owner or Manager
can append a reasoned non-negative threshold only after an item is explicitly
stock-tracked. No threshold is assumed by default. An Owner or Manager can
also append a reasoned clear event; the latest threshold-or-clear event is the
current policy. A threshold controls the inventory view's **low stock**
indicator when its derived balance is at or below that threshold; a clear event
removes only that indicator. Neither action changes stock, an existing
movement, or whether checkout rejects an actual stock overdraw. Both event
types are immutable, audit-chained, and sync-outbox-backed. Suppliers, purchase
documents, and product recipes (BOM) are covered by
[suppliers-purchases-recipes-v1.md](suppliers-purchases-recipes-v1.md).
Forecasting remains outside this contract.
