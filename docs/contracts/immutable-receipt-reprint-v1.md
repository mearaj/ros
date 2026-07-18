# Immutable Receipt Reprint Contract v1

**Status:** Implemented Community Edition local contract  
**Scope:** On-screen, copied-text, and plain-text file exports of finalized invoices

A receipt reprint is reconstructed only from immutable order-line snapshots,
the finalized invoice, recorded payment allocations, and retained refund facts.
It never reads the current menu name or price, so later catalogue changes cannot
rewrite a historical receipt.

## Rules

- Only an active counter-capable staff session in the invoice's branch may load
  the selected receipt. Kitchen-only sessions are denied in Rust.
- The detail projection is branch-scoped and bounded to one invoice. It does
  not expose database paths, keys, audit-chain internals, or unrelated customer
  data.
- Split allocations appear independently and in their recorded sequence. Any
  retained refund total is displayed separately; the original invoice/payment
  facts remain unchanged.
- The Flutter receipt sheet may copy this reconstructed text to the local
  clipboard or save it as a plain `.txt` file through an explicit platform
  save dialog.
- ESC/POS and simple PDF encoders are available from the same immutable text
  (`ros_storage::encode_escpos_receipt` / `encode_simple_pdf_receipt`) per
  ADR 0008. Physical Epson TM-T82X class acceptance remains a publication gate.
  Email and tax-compliance claims remain out of scope.
