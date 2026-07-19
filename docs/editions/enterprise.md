# Enterprise Edition

## Product position

Enterprise Edition is an annual, contract-configured ROS offering for
organizations that need Professional capabilities at a larger branch capacity
and an agreed operational relationship with Gotigin.

Enterprise is not a vague promise of every possible integration or deployment
model. Each entitlement and commercial agreement must state the exact branch
capacity, enabled capabilities, support scope, security responsibilities, and
term.

## Included baseline

Enterprise includes the available Professional capability set and an annual
signed entitlement with branch capacity configured in the commercial agreement.
The current software default ceiling is 50 branches until a signed contract
sets a different authorized capacity.

Enterprise uses the same local-first and safe-expiry principles:

- Each branch continues normal local operation when offline.
- Cloud synchronization and organization-level controls are added capabilities,
  not prerequisites for local POS.
- Annual expiry follows the Community Safe Mode path: no deletion, no loss of
  export access, one primary branch remains read/write locally, and other
  branch data is retained safely.
- A 72-hour offline grace period applies when a paid entitlement cannot be
  refreshed.

## Contract-only capabilities

The following may be offered only when specifically contracted, built, tested,
and accepted. They must not be advertised as generally available merely
because Enterprise exists:

- Higher or unlimited branch capacity.
- Organization analytics.
- Dedicated support and service-level agreements.
- On-premise or customer-managed deployment.
- Single sign-on.
- Custom APIs and third-party integrations.
- Custom data residency, retention, migration, or operational requirements.

For every contracted capability, ROS needs an explicit technical scope,
security review, support boundary, delivery acceptance criteria, and lifecycle
plan before it is enabled for a customer.

## Release boundary

Enterprise commercial controls, administration, on-premise delivery, SSO,
custom APIs, integrations, and contractual operational commitments remain
roadmap work unless separately evidenced. A signed agreement cannot turn an
unimplemented feature into a supported capability.

See [the edition overview](README.md), [Professional Edition](professional.md),
and [ADR 0009](../adr/0009-commercial-edition-terms.md).
