# Split-payment ledger contract v1

## Scope

This contract defines the required financial behavior for Community Edition
split tender (cash, card, and UPI on one invoice). Sale allocation validation,
method-bounded refunds, and the POS split-tender UI are implemented. Split
allocations are compared to the trusted payable preview (after discount and
tax), not a Flutter-invented total.

## Sale allocations

- A finalized invoice may have one or more immutable recorded payment rows.
- Every allocation has a positive integer minor-unit amount and one of the
  supported payment methods. No payment credential, card number, UPI handle,
  or processor response is stored.
- Inside the same SQLCipher transaction that finalizes an invoice, Rust must
  prove `sum(payment.amount_minor) == invoice.total_minor`. A duplicate-click,
  overflow, empty allocation, negative allocation, unknown method, or unequal
  sum rolls back the entire order, invoice, allocation, audit, outbox, and
  inventory transaction.
- The invoice receipt/report projection must show the complete allocation set,
  not invent a primary method from Flutter state. Cash-drawer expected cash is
  derived from the immutable cash allocations.

## Refund allocations

- A refund must be represented as one or more immutable payment-method
  allocations. For each original method, cumulative refunds may not exceed the
  cumulative recorded payment amount for that method.
- A partial refund distributes from the original recorded allocations in a
  deterministic recorded-at/payment-ID order unless a future owner-authorized
  refund-allocation UI explicitly chooses a valid distribution.
- Financial reporting and drawer closure subtract refund allocations by method,
  never by a single invoice-level method label.

## Delivery status

1. Payment allocation read models and invoice display projection — done.
2. Per-method remaining-balance refund enforcement — done.
3. Rust sale/refund commands, trigger, report, drawer, and audit regressions —
   done.
4. Flutter/Rust bridge and bounded POS split-tender UI — done.

Remaining founder-gated work (dual approval for large discounts/refunds, GST
claims, portable recovery) is intentionally outside this contract.
