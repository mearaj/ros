# Professional API service

The API is intentionally not treated as deployed merely because its route,
validator, and PostgreSQL schema exist. The current binary registers a bounded
Professional sync endpoint, but it is disabled by default and returns 503
unless the complete local sync runtime is configured. When enabled, it verifies
short-lived Ed25519 device JWTs, binds body scope to verified tenant/branch/
device claims, sets transaction-local PostgreSQL tenant context, serializes a
device anchor, authorizes branch membership and actors, and commits immutable
events before returning stable acknowledgements. Enabled sync also checks the
required schema contract during startup and, after token verification and
local device throttling, before each request body is parsed or admitted.

That local implementation is not production readiness. Before Gotigin enables
or deploys it, the service still requires all of the following together:

1. TLS termination and verified OIDC bearer authentication.
2. A production token issuer plus audited device, actor, grant, and revocation
   provisioning; the repository verifier does not issue credentials.
3. A separately controlled migration/provisioning role and a least-privilege
   non-owner runtime role without `BYPASSRLS`.
4. Reporting projections committed in the same transaction when those
   projections are introduced.
5. Distributed/edge tenant abuse controls and an OpenTelemetry collector/
   exporter. The runtime emits a deliberately small privacy-safe JSON request
   event (`ros_api::http`) and enforces bounded global in-flight work plus a
   per-organization/device request window, transport body bounds, and batch
   bounds. For production, configure a tracing OpenTelemetry exporter
   (OTLP/gRPC or HTTP) against the Gotigin collector, with redaction that never
   includes Authorization headers, tokens, PINs, or sync payloads. Process-local
   stderr events are not substitutes for coordinated controls, durable security
   telemetry, alerting, or sampling across replicas and the public edge.
   Owner dashboard routes under `/v1/owner/*` are documented in
   [`openapi.yaml`](openapi.yaml); see `apps/owner-dashboard/` for the staging UI.
6. Production-like load, failover, restore, and least-privilege runtime-role
   drills in the selected cloud environment. The repository's disposable
   PostgreSQL suite covers schema isolation, replay/concurrency, revocation,
   rollback, and fail-closed schema admission; it does not replace those
   operational exercises.

The canonical transport contract is [`openapi.yaml`](openapi.yaml), and the
initial database foundation is
[`../../database/cloud-migrations/0001_tenant_event_log.sql`](../../database/cloud-migrations/0001_tenant_event_log.sql).
Migration `0002_tenant_integrity_and_forced_rls.sql` adds composite
organization/branch and organization/device foreign keys for sync events and
forces RLS on every tenant table. Migration 0003 makes accepted events
append-only and actor-bound. Migration 0004 adds canonical stable
acknowledgements plus append-only branch device/actor grants and revocations.
The API database role must not own these tables or have `BYPASSRLS`; the
controlled migration/provisioning role is separate.

## Local operational boundary

With Professional sync disabled (the default), the service has no cloud
dependency:

```bash
cargo run -p api
curl http://127.0.0.1:3000/healthz
curl http://127.0.0.1:3000/readyz
```

`ROS_API_LISTEN_ADDRESS` may set an explicit IP-address-and-port listener;
`ROS_DEPLOYMENT_ENV` accepts only `development`, `staging`, or `production`.
`/healthz` reports process liveness. `/readyz` reports sync as `disabled` in
this mode and does not assert tenant, OIDC, or cloud readiness. Liveness never
probes PostgreSQL and bypasses the application-work semaphore so saturated work
does not turn a healthy process into a restart loop. It remains subject to the
hard request deadline and response policy.

Every route is wrapped in the same operational boundary. Its configuration is
strict: values must be canonical positive base-10 integers with no sign or
surrounding whitespace, and startup rejects missing bounds or a queue timeout
larger than the complete request deadline.

| Variable | Default | Accepted range | Meaning |
| --- | ---: | ---: | --- |
| `ROS_API_REQUEST_TIMEOUT_MS` | `15000` | `100..=120000` | Complete request deadline, including queue time |
| `ROS_API_QUEUE_TIMEOUT_MS` | `100` | `1..=5000` | Maximum wait for an application-work permit; must not exceed the request deadline |
| `ROS_API_MAX_IN_FLIGHT` | `256` | `1..=4096` | Non-liveness requests executing concurrently in one process |

When capacity is occupied beyond the queue timeout, the API returns a bounded
`503 server_busy` problem with `Retry-After: 1`. A deadline returns a bounded
`504 request_timeout` problem. These limits apply outside the sync handler and
therefore do not change its security order: device authentication and the
device-local rate window still precede the schema probe, content-type check,
JSON parse, and database transaction. Production edge timeouts and connection
limits should be aligned with—not used to silently weaken—these service bounds.

The API accepts an inbound `X-Request-ID` only when it is one canonical
lowercase, hyphenated UUIDv4 or UUIDv7. It replaces missing, duplicated,
malformed, noncanonical, other-version, or oversized values with a server
UUIDv7, passes only that safe value downstream, and echoes it on every response.
All responses set `Cache-Control: no-store`, legacy `Pragma: no-cache`,
`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, a deny-by-default
content security policy, `Referrer-Policy: no-referrer`, same-origin resource
policy, and a restrictive permissions policy. HSTS belongs at the TLS
terminator because local development intentionally speaks HTTP.

Structured request events are JSON on stderr and contain only the safe request
ID, a fixed route label, a fixed method label, status, duration, and a fixed
outcome. Unknown paths are always `unmatched`; no URL, query, bearer value,
database URL, body, payload, restaurant/tenant identity, actor identity, branch
identity, or device identity is logged. The subscriber enables only the
repository-owned `ros_api::http` and `ros_api::lifecycle` INFO targets, avoiding
accidental dependency/SQL trace activation. An OpenTelemetry exporter,
collector backend, retention/access policy, alerting, and edge correlation are
still deployment work.

The local sync path is enabled only when all of these are supplied:

- `ROS_ENABLE_PROFESSIONAL_SYNC=true`
- `ROS_DATABASE_URL` for the migrated PostgreSQL runtime role
- `ROS_DEVICE_TOKEN_PUBLIC_KEY_FILE` for an Ed25519 public key
- `ROS_DEVICE_TOKEN_ISSUER` for exact issuer validation
- `ROS_DEVICE_TOKEN_AUDIENCE` for exact audience validation
- optional `ROS_SYNC_REQUESTS_PER_MINUTE` (default `120`, accepted range
  `1..=10000`) for the process-local organization/device admission window

Outside development, the PostgreSQL URL must use `sslmode=verify-full` and the
issuer must be an absolute HTTPS URL. Development may use a local PostgreSQL
transport and test issuer; that exception must not be copied into staging or
production configuration.

When enabled, startup fails closed if the key, database, or required migration
contract is unavailable. `/readyz` and every sync admission re-check the
required tables, columns, validated constraints, RLS/forced-RLS flags, tenant
policies, and enabled immutability triggers. This is a bounded readiness
signal, not proof of correct runtime privileges, provisioned identities,
production issuer availability, or resistance to an administrator deliberately
replacing an object with a malicious same-named definition. The database URL
is secret-bearing configuration and must come from a local secret facility or
deployment secret manager, never a committed file or shell-history example.

Cloud account selection, database credentials, production issuer
configuration, deployment, and domain/DNS are founder-owned external inputs.
None belongs in source control or a local development fallback.
