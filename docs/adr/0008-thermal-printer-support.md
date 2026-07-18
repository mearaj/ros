# ADR 0008: First supported thermal printer

**Status:** Accepted  
**Date:** 18 July 2026  
**Approver:** Gotigin engineering (founder-accountable product default)

## Context

Receipt reprint today is immutable text only. Advertising printing requires a
named, physically testable model.

## Decision

1. **First supported profile:** ESC/POS compatible 80 mm thermal printers that
   accept raw TCP (port 9100) or USB/serial raw byte streams. Reference
   acceptance target: **Epson TM-T82X** class (ESC/POS, 80 mm).
2. **Software boundary:** Rust owns an immutable receipt projection → ESC/POS
   bytes encoder. Flutter only selects destination (network host/port or OS
   raw device path) and displays job status.
3. **PDF:** generate a simple A4/80 mm-equivalent PDF from the same immutable
   receipt projection for archive/email; PDF is not a substitute for the named
   thermal acceptance test.
4. **Until a physical unit is attached in CI/lab:** automated tests assert
   byte-level ESC/POS structure against fixtures; publication marketing may
   claim “ESC/POS 80 mm (Epson TM-T82X class)” only after founder records
   physical acceptance evidence in the release checklist.

## Consequences

- Printer/PDF work can proceed against the ESC/POS profile without waiting for
  every hardware SKU.
- Physical acceptance evidence remains a Stage 6 publication checkbox.
