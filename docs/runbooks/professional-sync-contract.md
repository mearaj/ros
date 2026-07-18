# Professional sync contract

## Current implementation boundary

This repository has four local foundations for Professional sync:

- the encrypted local database exposes pending operations as immutable audit
  envelopes and records a cloud acknowledgement as a separate immutable fact;
- the cloud migration set defines the intended tenant-scoped immutable event
  log;
- `services/api/src/sync.rs` has a pure, tested validator for a bounded,
  actor-bound audit-chain batch; and
- the API contains a feature-gated route, a short-lived Ed25519 device-token
  verifier, and a PostgreSQL transaction adapter for tenant-scoped acceptance
  and stable replay acknowledgements.

That is not a deployed sync service. The route is registered locally but is
disabled by default and fails closed unless its database and token-verifier
configuration is supplied. There is no production token issuer, device and
actor provisioning workflow, founder-provisioned tenant credential, proven
PostgreSQL role, cloud environment, reporting projection, activation screen,
or deployment. The OpenAPI document describes the implemented local transport
boundary; it is not evidence that Gotigin Cloud is live.

The local `PendingSyncOperation` read model includes the source event's
`actor_id`. The transport must send that exact value because the shared audit
hash binds it; a transport cannot swap actors without invalidating the
envelope.

The Community owner UI exposes a separate privacy-safe sync-queue projection
(`load_community_sync_queue`) that shows only sequence, event type, entity
type, and created time. That Flutter-facing view must never receive payloads,
hashes, device identifiers, actor identifiers, or correlation material.

## Exact wire envelope

One request represents a single candidate, contiguous suffix of one device's
audit chain. branch_id and device_id occur once at batch level and are inputs
to the hash of every operation. The current validator intentionally does not
accept a client-supplied anchor: the persistence transaction must load
the authoritative anchor itself.

| Field | Required form and validation rule |
| --- | --- |
| branch_id, device_id, operation_id, audit_event_id, actor_id | Canonical lowercase, hyphenated RFC 4122 UUIDv7: 36 characters, version nibble 7, and RFC 4122 variant nibble 8, 9, a, or b. Uppercase, braced, compact, non-v7, and non-RFC-4122 forms fail. |
| operations | One through 200 entries, in the submitted order. |
| sequence | A positive signed 64-bit device sequence, below i64::MAX so a successor remains representable. |
| event_type | One to 200 ASCII bytes matching ^[a-z][a-z0-9._-]{0,199}$. |
| payload_json | Non-empty compact JSON whose bytes exactly equal serde_json::to_string(serde_json::from_str(payload_json)), with a maximum of 1,048,576 UTF-8 bytes. This is the precise serializer rule currently enforced; do not substitute a loosely defined “minified JSON” routine. |
| occurred_at_utc | A valid RFC 3339 UTC instant in exactly YYYY-MM-DDTHH:mm:ss.sssZ form: 24 ASCII bytes, uppercase T/Z, exactly three fractional digits, and no numeric offset. |
| previous_hash, event_hash | Padded RFC 4648 standard Base64 representing exactly 32 bytes. URL-safe Base64, omitted padding, malformed padding, and non-canonical pad bits fail because the validator decodes and re-encodes the value exactly. |

previous_hash is omitted only when the durable device anchor is empty. For any
non-empty anchor, it must equal that anchor's event hash. Within the batch,
each later operation's previous_hash must equal the immediately preceding
operation's event_hash.

event_hash is recomputed over the exact canonical branch ID, actor ID, device
ID, audit-event ID, sequence, event type, JSON bytes, timestamp bytes, and
predecessor hash. The validator rejects an actor substitution even if all
other fields look valid. It verifies attribution; the authenticated transport
must additionally decide whether that actor may act for the claimed
tenant and branch.

The validator rejects duplicate operation_id values and duplicate
audit_event_id values inside one request. OpenAPI can describe array bounds
but cannot express keyed uniqueness for object properties, so this is a
runtime validation rule rather than a JSON Schema uniqueItems claim.

## Durable anchor and transaction boundary

For a device with no accepted facts, the authoritative anchor is sequence 0
with no event hash. Otherwise it is the last durable (sequence, event_hash)
pair for that exact device. A valid candidate batch starts one sequence after
the anchor; it cannot skip, repeat, or reorder a sequence. Its first
predecessor hash must equal the anchor hash, then every later predecessor must
equal the preceding submitted event hash.

The PostgreSQL adapter loads and holds that anchor in the same tenant-scoped
transaction that inserts new immutable facts. It must not trust
a client anchor, validate against a stale read, or advance the anchor outside
the fact insert. Concurrent writers for one device therefore need an explicit
serialization/locking strategy; the adapter uses a device-row lock for that
purpose. The pure validator still has no database or authentication side
effects and can be tested separately from the adapter.

## Persistence and idempotency behavior

The feature-gated HTTP/persistence path is designed to perform these
responsibilities in one tenant-scoped transaction. They remain deployment
requirements and require PostgreSQL integration proof; pure-validator tests
alone do not prove them:

1. Derive organization, branch, device, and actor authorization from verified
   claims rather than from untrusted body values. Set the PostgreSQL tenant
   context before querying tenant-owned data.
2. Resolve submitted operation_id records against durable idempotency state
   before treating an envelope as new. An exact replay returns the original
   stable acknowledgement; it must not insert a second financial or audit
   fact, and it must not advance the device anchor again.
3. Reject a reuse of an existing operation ID with different immutable
   content, a conflicting audit-event identity, or a chain that cannot follow
   the durable anchor. Do not repair either case with last-write-wins.
4. For a request that mixes a replay with new work, reconcile or reject it
   deterministically before giving a single new contiguous suffix to the
   validator. Never validate an old replay against a newer anchor as if it
   were a new fact.
5. Insert accepted immutable facts and durable idempotency/acknowledgement data
   atomically. Any reporting projection added later must join that same
   transaction. Only after commit may the transport return `accepted` or
   `already_accepted` acknowledgements.

The client records a returned acknowledgement as a separate immutable local
fact. Repeating its same server_event_id is a no-op; a different server event
ID for the same operation is a sync-integrity fault. An acknowledgement that
does not bind the submitted operation and audit-event identities is invalid.
Network failure before the local acknowledgement remains committed leaves the
operation pending; a retry must not create a duplicate financial event.

## Activation and recovery boundary

1. The owner explicitly activates Professional and creates a verified recovery
   snapshot before any registration or transfer.
2. The device obtains an authenticated, tenant-scoped session and registers a
   public device identity. The SQLCipher database key is never uploaded.
3. A Community database requires a separately designed encrypted baseline
   upload for facts created before activation. It is not correct to fabricate
   historical outbox operations.

Production token issuance, device/actor provisioning, cloud storage,
retry/backoff, user-facing conflict handling, and the activation workflow
remain required Professional work. The local route must remain disabled by
default until its PostgreSQL and non-owner-role integration suite passes; it
must not be treated as a deployed endpoint.

## Cloud database isolation boundary

Cloud migration 0002_tenant_integrity_and_forced_rls.sql makes a sync event's
organization agree structurally with both its branch and device, and enables
forced PostgreSQL RLS on organizations, branches, devices, and sync events.
Migration 0003_sync_event_actor_and_immutability.sql adds the non-null actor
identity and rejects UPDATE, DELETE, and TRUNCATE on accepted sync_events
facts at the database layer. Migration
0004_sync_acknowledgements_and_device_grants.sql adds canonical UUIDv7 server
event identities, stable acceptance timestamps, exact canonical payload text,
branch-scoped device and actor authorization facts, and append-only
revocations. The grant/revocation tables also use forced RLS.

0003 and 0004 deliberately provide no guessed backfill. They apply directly to
an empty event log. If an environment already has accepted events, migration
must stop and a separately reviewed recovery procedure must establish the
original actor enrollment, canonical payload bytes, and stable server event
identity for every affected event. Rewriting history, assigning a generic
actor, or reserializing JSON would invalidate the security model.

A device/branch or actor/branch revocation is permanent for that identity.
Restoring access requires a newly registered identity, never deleting the
revocation. The `*_by_actor_id` fields preserve the verified provisioning
actor supplied by the future administrative workflow; the schema cannot prove
that workflow by itself. A request already in flight when revocation begins
needs an explicit, integration-tested transaction-ordering policy. Requests
started after a committed revocation must be denied.

Each API transaction must set `app.organization_id` from verified
claims using transaction-local scope before it issues any tenant query. The
runtime API role must be a non-owner without BYPASSRLS and without UPDATE,
DELETE, or TRUNCATE privileges on immutable fact tables; a separate controlled
role applies migrations and provisioning facts. Database administrators can
alter schema-level protections, so the controlled migration role must remain
separate from the runtime API role. These migrations have not been exercised
against a founder-provisioned PostgreSQL environment with the final non-owner
role, so cross-tenant denial, stable replay, revocation ordering, and
immutability integration proof remain release blockers rather than completed
claims.
