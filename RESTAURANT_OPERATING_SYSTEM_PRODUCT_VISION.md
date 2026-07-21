# Restaurant Operating System

## Product Vision & Licensing Strategy

> This document defines the initial product vision, licensing strategy,
> and engineering philosophy.
>
> **Edition rules (Community / Professional / Enterprise)** live in
> [`docs/editions/`](docs/editions/README.md). Do not treat this file as the
> edition source of truth.

## Product Vision

Restaurant Operating System is **not** just a Point of Sale application.

It is a complete operating system for restaurants that manages daily
operations through a modern, secure, reliable and beautiful experience.

This product is intended to become the flagship commercial product of
**GOTIGIN SOFTWARE & HARDWARE PRIVATE LIMITED**.

------------------------------------------------------------------------

## Core Principles

1.  Production quality over hackathon quality.
2.  Owner-first philosophy.
3.  Offline-first.
4.  Fast and intuitive.
5.  Secure and reliable.
6.  Long-term maintainability.
7.  Modular architecture.
8.  Beautiful user experience.
9.  Community Edition should be genuinely useful.
10. Businesses should upgrade because they grow, not because the
    software is artificially limited.

------------------------------------------------------------------------

## Product editions (summary)

Canonical definitions, acceptance gates, and platform claims:

- [docs/editions/README.md](docs/editions/README.md)
- [Community](docs/editions/community.md) ·
  [Professional](docs/editions/professional.md) ·
  [Enterprise](docs/editions/enterprise.md)

Commercial defaults (grace, branch ceilings, Safe Mode):
[ADR 0009](docs/adr/0009-commercial-edition-terms.md).

Edition topology and switching:
[ADR 0010](docs/adr/0010-edition-data-topology-and-switching.md).

Capability matrix (targets, not an implementation-status table):

  Capability            Community   Professional   Enterprise
  --------------------- ----------- -------------- ------------
  Single Branch         Yes         Yes            Yes
  Multi Branch          No          Up to 5        Entitlement branch capacity
  Offline Mode          Yes         Yes            Yes
  Cloud Sync            No          Yes            Yes
  Owner Dashboard       No          Yes            Yes
  Automatic Backups     No          Yes            Yes
  API Access            No          Yes            Yes
  Custom Integrations   No          No             Yes

Enterprise entries describe capabilities that may be included by contract;
they do not promise SSO, on-premise deployment, custom APIs, integrations, or
unlimited branches in the first release.

------------------------------------------------------------------------

# Engineering Philosophy

There must always be a single codebase.

Licensing unlocks capabilities only.

Customers should never reinstall or migrate to upgrade.

Architect for long-term maintainability, security, performance, and
extensibility.

Whenever multiple architectural choices exist:

-   Explain trade-offs.
-   Recommend the best long-term approach.
-   Challenge weak assumptions.
-   Prioritize simplicity and reliability.
