# ADR 0006: Dual-person correction approval

**Status:** Accepted  
**Date:** 18 July 2026  
**Approver:** Gotigin engineering (founder-accountable product default)

## Context

Refunds, voids, and large discounts are currently single-actor Owner/Manager
commands. Stage 2/3 acceptance requires configurable dual-person approval.

## Decision

Default Community correction-approval policy `correction-approval.v1`:

| Action | Requires second person | Eligible approvers | Threshold / notes |
|--------|------------------------|--------------------|-------------------|
| Full or partial refund | Yes | Owner or Manager, distinct actor | Any amount |
| Invoice void | Yes | Owner or Manager, distinct actor | Always |
| Discount at POS | Yes when percentage ≥ 20% or fixed amount ≥ 50000 minor units of branch currency | Owner or Manager, distinct actor | Below threshold: requester Owner/Manager alone |
| Stock adjustment (negative) | Yes | Owner or Manager, distinct actor | Any negative adjustment |
| Day-close | No second person | Owner or Manager | Append-only; reopen remains unsupported |

Offline behavior:

1. Requester creates an append-only `approval_request` with action, entity refs,
   amount snapshot, reason, and policy version.
2. Approver authenticates with their own PIN in a separate short-lived approval
   challenge; Rust verifies distinct `actor_id` and eligible role.
3. Approval decision is append-only; the consuming mutation commits in the same
   transaction as the decision consumption marker.
4. Requests expire after 15 minutes if unused; expired requests cannot be
   consumed. No silent auto-approval.

## Consequences

- Refund/void/discount/adjustment commands accept an optional or required
  `approval_request_id` according to policy evaluation.
- Flutter prompts for a second staff PIN; authorization remains Rust-owned.
