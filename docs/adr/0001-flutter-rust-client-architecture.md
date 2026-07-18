# ADR 0001: Flutter/Dart UI with a Rust-owned operational core

**Status:** Accepted  
**Date:** 16 July 2026

## Context

Restaurant Operating System must serve desktop POS terminals, tablets, and
mobile devices from one product codebase. It must remain offline-first, fast,
and safe for financial and operational workflows.

## Decision

Use Flutter and Dart for the adaptive client experience. Use Rust for database
access, business-critical domain commands, cryptographic/key-management
adapters, migrations, synchronization, and non-UI services.

Flutter communicates with the Rust core through generated
flutter_rust_bridge bindings. No business-critical database rule may exist
only in Dart, and Flutter must not open the local database directly.

## Consequences

- Flutter can provide one carefully designed interface across desktop, tablet,
  and mobile form factors.
- Rust provides a small trusted computing base for the parts of the product
  that need durable, audited behavior.
- The generated bridge replaces hand-written platform channel glue and is
  covered by contract tests.
- The app has a native-library build requirement on every supported target;
  native builds are therefore part of CI and release verification.

