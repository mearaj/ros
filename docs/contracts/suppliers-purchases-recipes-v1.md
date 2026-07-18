# Suppliers, purchases, and recipes v1

Extends the inventory ledger (inventory-ledger-v1):

- Suppliers are archive-only master data.
- Purchase documents receive lines into immutable `purchase` inventory
  movements via `source_document_id`.
- Product recipes (BOM) cause sale deductions against ingredient products when
  an active recipe exists; otherwise tracked finished-good deduction remains.

Schema: local migrations 0032–0033.
