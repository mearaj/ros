# Community financial CSV export v1

## Purpose and scope

Community Edition can prepare a narrow owner-controlled financial CSV for a
single local branch. It is a convenience export of immutable financial
aggregates, not a tax return, statutory ledger, accountant-certified report,
or customer-data export.

The export contains only UTC-day and tender aggregates for recorded payments,
refunds, and operating expenses, followed by an all-time summary. Amounts are
integer minor units and the currency code is explicit. It intentionally omits
customer information, free-text descriptions, product names, invoice IDs,
staff/device identifiers, credentials, audit payloads, and database material.

## Authorization and integrity boundary

The Rust storage layer—not the Flutter button—enforces all of these before
returning any bytes:

1. A currently active local Owner session is required. Manager, Cashier, and
   Kitchen sessions are denied.
2. The active Community branch must exist.
3. SQLCipher integrity, the local schema contract, foreign keys, and every
   device audit chain must verify successfully.
4. Invoice, payment, refund, expense, branch-currency, and tender facts must
   satisfy the export consistency checks.
5. The report is built inside the same immediate SQLite transaction as those
   checks, so another local writer cannot change the financial snapshot between
   verification and generation.

The export is deliberately capped at 50,000 aggregate records and 4 MiB. A
larger or inconsistent history fails safely instead of exhausting device memory
or silently truncating the report.

## File handling

Rust never writes plaintext CSV to a default path. After the Owner explicitly
requests the export, Flutter presents the native save dialog and passes the
prepared bytes to the chosen platform destination. Cancelling the dialog saves
nothing. Browser export is intentionally unavailable because it lacks a
reviewed explicit destination-selection boundary.

Once an Owner saves a CSV outside SQLCipher, its storage, access control,
retention, and onward sharing are controlled by that chosen destination. The
app does not claim to encrypt, revoke, or erase such copies.

## Wire format

The document is UTF-8 RFC 4180 CSV with CRLF records. The header is:

```text
record_type,accounting_date_utc,payment_method,gross_sales_minor,refund_minor,net_sales_minor,expense_minor,currency_code
```

`record_type` is one of `sale_payment`, `refund`, `expense`, or `summary`.
The aggregate rows populate only their applicable amount column; `summary`
uses `payment_method=all`. Dynamic cells are constrained to storage-owned UTC
dates, known tender names, ISO-style currency codes, and decimal integers, so
the export does not turn a corrupted value into a spreadsheet formula.

## Explicit non-claims

- This is not PDF or printer support.
- This is not GST, e-invoicing, PCI, statutory-accounting, or tax compliance.
- This is not a portable SQLCipher backup or clean-install recovery mechanism.
- This does not export customer personal data; any such export needs the
  founder-approved legal retention and privacy policy.
