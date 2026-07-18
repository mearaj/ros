# Cloud schema verification

`scripts/verify-cloud-migrations.sh` is a fail-closed integration check for
the Professional PostgreSQL migration set. It runs only when both safeguards
are present:

- `ROS_ALLOW_CLOUD_SCHEMA_TESTS=1`; and
- `ROS_TEST_DATABASE_URL` is a `postgresql://` URI whose host is exactly
  `127.0.0.1`, `::1`, or `localhost`.

The script creates random, disposable schemas and a non-owner, no-login role;
it drops both on exit. It does not deploy the API, provision a tenant, create
Gotigin Cloud credentials, or contact any public service.

For a locally started disposable PostgreSQL database named `ros_ci`:

```bash
ROS_ALLOW_CLOUD_SCHEMA_TESTS=1 \
ROS_TEST_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5432/ros_ci?sslmode=disable' \
./scripts/verify-cloud-migrations.sh
```

The runner applies each numbered migration inside a transaction, verifies
that a historical non-empty event log blocks the actor-identity migration, and
then checks:

- RLS and `FORCE ROW LEVEL SECURITY` on every tenant table;
- non-owner isolation with missing, matching, and mismatching tenant context;
- cross-organization foreign-key integrity;
- acknowledgement UUIDv7 and payload/JSON semantic-match constraints; and
- database triggers blocking update, delete, and truncate of accepted facts
  and authorization facts.

The CI cloud-schema job then applies the same migrations to the disposable
database's `public` schema and runs the ignored Rust/PostgreSQL API integration
suite. That suite verifies atomic acceptance and replay, concurrent retry
idempotency, revocation enforcement, startup refusal of an incomplete schema,
and request-admission refusal after an immutability trigger is disabled. Test
JWT signing keys are generated only in process; no reusable private key is
stored in the repository.

This validates the checked-in schema in a disposable test cluster. It is not
evidence that a production runtime role, cloud account, issuer, deployment,
or operational controls have been provisioned. Exact payload byte
canonicalization is enforced by the Rust transport validator; PostgreSQL's
JSONB equality check proves semantic equality only.
