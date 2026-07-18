# Customer privacy contract v1

## Scope

This contract covers Community Edition customer records stored in the encrypted
local database. It intentionally distinguishes optional customer association
from a sale: a counter sale can be completed without creating or selecting a
customer.

## Data and consent rules

- A customer has an immutable identity record and append-only profile
  revisions. The current active revision may contain a display name, optional
  phone number, optional email address, and an explicit marketing-consent
  flag.
- Supplying contact details never implies marketing consent. The counter UI
  presents consent as an unchecked, separate choice.
- Flutter receives only the current active profile projection required for
  counter selection. It does not receive historic revisions or anonymized
  records.
- Customer enrollment is permitted to Owner, Manager, and Cashier sessions.
  Profile corrections and anonymization require Owner or Manager authority.
  Rust resolves that authority from the active local staff session.

## Corrections and anonymization

- A correction appends a new active profile revision with a required reason;
  it never updates an earlier profile.
- Anonymization appends a current `anonymized` revision containing the fixed
  display value `Anonymized customer`, no phone number, no email address, and
  no marketing consent. The record therefore disappears from future counter
  selection.
- No customer or customer-profile row may be updated or deleted. SQLite
  triggers reject direct changes as well as application code paths.
- Existing orders retain their customer ID, preserving financial and audit
  history. Future sales revalidate that a selected customer is active and
  belongs to the current branch in the same Rust-owned transaction that writes
  the sale.

## Audit and synchronization

Customer creation, profile correction, and anonymization each write a hashed
audit event and future-sync outbox envelope atomically with their profile
fact. Audit payloads use customer IDs and revisions, not contact values.

## Boundary

This is a local privacy-control foundation, not a legal retention policy or a
guarantee of erasure from a restaurant owner's external paper records,
screenshots, or independently exported backups. Any cloud retention, export,
or recovery policy requires the founder's legal and commercial decisions.
