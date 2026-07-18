# Restaurant Operating System

## Product Vision & Licensing Strategy

> This document defines the initial product vision, licensing strategy,
> and engineering philosophy.

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

# Product Editions

The edition descriptions below are product and licensing targets. They are not
an implementation-status table. The repository currently implements the
offline Community foundation and disabled Professional sync foundations;
14-day evaluation activation, signed annual entitlements, multi-branch
administration, production cloud deployment, and Enterprise commercial
controls remain gated work. No Professional or Enterprise capability should be
marketed as available until its release evidence exists.

## Community Edition (Free Forever)

Target: - Independent restaurants - Cafes - Small restaurants

Features:

-   Single branch
-   Unlimited usage
-   Unlimited menu items
-   Unlimited orders
-   Unlimited invoices
-   Unlimited tables
-   Unlimited employees
-   Inventory
-   Purchases
-   Expenses
-   Reports
-   Kitchen Display System (KDS)
-   Offline-first
-   Local database

Limitations:

-   Maximum 1 branch
-   No cloud synchronization
-   No centralized owner dashboard

This edition should feel complete and should never feel like a crippled
trial.

------------------------------------------------------------------------

## Professional Evaluation

Duration: - 14 Days

Purpose:

Allow restaurants to evaluate every Professional feature.

Features:

-   Up to 5 branches
-   Cloud synchronization
-   Central owner dashboard
-   Cross-branch reporting
-   Automatic backups
-   Remote management
-   Advanced user roles
-   Priority support

After evaluation:

Restaurants that only need a single branch should be able to continue
using Community Edition.

Do not lock customers out of their data.

------------------------------------------------------------------------

## Professional Edition

Annual subscription.

Includes every Professional capability.

Maximum: - 5 branches

Designed for growing restaurant businesses.

------------------------------------------------------------------------

## Enterprise Edition

Annual subscription.

Pricing depends on:

-   Number of branches
-   Deployment requirements
-   Support level
-   Custom integrations

Possible enterprise capabilities:

-   Unlimited branches
-   Organization analytics
-   Dedicated support
-   SLA
-   On-premise deployment
-   SSO
-   Custom APIs

------------------------------------------------------------------------

# Target Capability Matrix

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
