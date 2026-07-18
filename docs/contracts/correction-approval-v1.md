# Correction approval v1

Policy version: `correction-approval.v1` (ADR 0006).

Refunds, voids, large discounts, and negative stock adjustments require a
distinct Owner/Manager approver. The requester and approver authenticate with
separate PIN credentials. Requests, decisions, and consumptions are append-only
local facts and sync-outbox-backed.

See migrations `0031_correction_approvals.sql` and the `DualPersonApproval`
storage API.
