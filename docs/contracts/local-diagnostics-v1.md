# Local diagnostics contract v1

## Purpose

Local diagnostics exist so an installation can recover from failures and so an
Owner may **voluntarily** share a redacted pack with Gotigin to diagnose an
issue or improve the product. Continuous anonymous analytics, session replay,
and background phone-home are out of scope for this contract.

## Local collection (always on)

The client appends allow-listed structured events to a rotating JSONL file under
the application-support `diagnostics/` directory. The file is intentionally
outside SQLCipher so bootstrap and open failures remain diagnosable.

Each event may include only:

| Field | Rule |
| --- | --- |
| `schema_version` | Constant `1` |
| `occurred_at_utc` | RFC3339 UTC from a trusted clock |
| `event_code` | Fixed enum string (see below) |
| `component` | One of `bootstrap`, `storage`, `bridge`, `ui`, `share` |
| `outcome` | One of `ok`, `denied`, `failed`, `busy`, `throttled`, `unavailable` |
| `duration_ms` | Non-negative integer when measured |
| `platform` | Coarse label such as `linux`, `windows`, `macos`, `android`, `ios`, `unknown` |
| `app_channel` | `development` or `production` feature channel label |
| `process_correlation` | Random per-process token; not a device, actor, or branch ID |
| `detail_code` | Optional secondary fixed enum (never free text) |

Unknown fields, oversize strings, or free-text payloads are rejected and must
not be written.

## Event codes (v1)

Bootstrap / storage: `bootstrap_open`, `storage_integrity`, `migration_apply`

Staff: `staff_unlock`, `staff_lock`, `owner_pin_configure`

Commerce: `sale_complete`, `sale_preview`, `invoice_refund`, `invoice_void`,
`accounting_day_close`

Backup: `backup_create`, `backup_verify`, `backup_restore`

UI breadcrumbs: `nav_overview`, `nav_pos`, `nav_kitchen`, `nav_inventory`,
`nav_reports`, `action_backup_create`, `action_backup_verify`,
`action_backup_restore`, `action_day_close`, `action_export_csv`,
`action_diagnostics_open`, `ui_error`, `ui_zone_error`

Share: `diagnostics_export`, `diagnostics_share_attempted`,
`diagnostics_cleared`

## Forbidden content

Logs, packs, and share requests must never include:

- SQLCipher / database keys or keyring secrets
- Owner/staff PIN plaintext, salts, or verifiers
- Customer name, phone, email, or marketing consent
- Staff display names
- Invoice, order, draft, payment, or customer identifiers
- Audit payloads, event hashes, or device/actor IDs
- Kitchen notes or cancellation free-text rationales
- Absolute filesystem paths or OS usernames
- Raw exception messages or stacks that may embed paths
- Payment PAN, UPI handles, or processor responses

## Owner export and voluntary share

1. Only an active Owner session may list, export, clear, or share diagnostics.
2. Export writes a bounded redacted pack (events + manifest) chosen by the
   Owner through a native save dialog. Rust never writes an unsolicited pack
   outside the diagnostics directory.
3. **Share with Gotigin** requires an explicit consent dialog each time. The
   dialog must state that sharing is optional, list what is included at a high
   level, and state that shared data is used to diagnose the issue and improve
   the product.
4. No background upload. No automatic crash upload.
5. If no diagnostics intake URL is configured, the client prepares the pack
   locally and reports that cloud share is unavailable; the Owner may still
   export the pack.

## Retention

- Local ring file: rotate before exceeding approximately 2 MiB; keep at most
  one rotated sibling.
- Cloud retention and lawful basis for retained packs remain founder decisions
  recorded outside this contract.

## Trust boundary

Flutter may supply only allow-listed breadcrumb codes. Rust validates every
record before append. User-facing `storage_status` strings remain separate from
diagnostic detail codes.
