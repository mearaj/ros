#!/usr/bin/env bash
# Verify the Professional PostgreSQL migration set against an ephemeral,
# explicitly local database. This is a schema/authorization test only: it
# neither deploys the API nor contacts any Gotigin Cloud environment.

set -euo pipefail

readonly repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly migrations_directory="$repository_root/database/cloud-migrations"
readonly database_url="${ROS_TEST_DATABASE_URL:?ROS_TEST_DATABASE_URL is required}"

if [[ "${ROS_ALLOW_CLOUD_SCHEMA_TESTS:-}" != "1" ]]; then
  echo 'Refusing to run: set ROS_ALLOW_CLOUD_SCHEMA_TESTS=1 for an explicitly disposable local database.' >&2
  exit 64
fi

if ! command -v psql >/dev/null 2>&1; then
  echo 'psql is required to verify the cloud migration set.' >&2
  exit 69
fi

# Do not let a convenient copy-and-paste target a remote or production
# database. The GitHub Actions service and documented local invocation both
# use a loopback PostgreSQL URI. A unix socket, DSN keywords, and unrecognised
# URI forms are deliberately rejected rather than guessed.
if [[ ! "$database_url" =~ ^postgres(ql)?:// ]]; then
  echo 'ROS_TEST_DATABASE_URL must be a postgresql:// URI targeting loopback.' >&2
  exit 64
fi

authority="${database_url#*://}"
authority="${authority%%/*}"
authority="${authority##*@}"
if [[ "$authority" == \[* ]]; then
  database_host="${authority#\[}"
  database_host="${database_host%%\]*}"
else
  database_host="${authority%%:*}"
fi

case "$database_host" in
  127.0.0.1 | ::1 | localhost) ;;
  *)
    echo 'Refusing to run cloud schema verification against a non-loopback database host.' >&2
    exit 64
    ;;
esac

readonly schema_name="ros_cloud_verify_${RANDOM}_$$"
readonly legacy_schema_name="ros_cloud_legacy_verify_${RANDOM}_$$"
readonly runtime_role="ros_cloud_runtime_verify_${RANDOM}_$$"
created_schema=0
created_legacy_schema=0
created_role=0

psql_unscoped() {
  PGOPTIONS='' psql --no-psqlrc --quiet --set=ON_ERROR_STOP=1 "$database_url" "$@"
}

psql_in_schema() {
  local target_schema="$1"
  shift
  PGOPTIONS="-c search_path=${target_schema},public" \
    psql --no-psqlrc --quiet --set=ON_ERROR_STOP=1 "$database_url" "$@"
}

cleanup() {
  local exit_status=$?
  set +e
  if [[ "$created_schema" == '1' ]]; then
    psql_unscoped --command "DROP SCHEMA IF EXISTS \"$schema_name\" CASCADE;" >/dev/null 2>&1
  fi
  if [[ "$created_legacy_schema" == '1' ]]; then
    psql_unscoped --command "DROP SCHEMA IF EXISTS \"$legacy_schema_name\" CASCADE;" >/dev/null 2>&1
  fi
  if [[ "$created_role" == '1' ]]; then
    psql_unscoped --command "DROP ROLE IF EXISTS \"$runtime_role\";" >/dev/null 2>&1
  fi
  exit "$exit_status"
}
trap cleanup EXIT

expect_failure() {
  local description="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    echo "Expected failure did not occur: $description" >&2
    exit 1
  fi
}

assert_scalar() {
  local expected="$1"
  local description="$2"
  local query="$3"
  local actual
  actual="$(psql_in_schema "$schema_name" --tuples-only --no-align --command "$query")"
  if [[ "$actual" != "$expected" ]]; then
    echo "Assertion failed ($description): expected '$expected', got '$actual'." >&2
    exit 1
  fi
}

shopt -s nullglob
migrations=("$migrations_directory"/[0-9][0-9][0-9][0-9]_*.sql)
if [[ "${#migrations[@]}" -eq 0 ]]; then
  echo 'No numbered cloud migrations were found.' >&2
  exit 66
fi

psql_unscoped --command "CREATE SCHEMA \"$schema_name\";"
created_schema=1

for migration in "${migrations[@]}"; do
  printf 'Applying %s\n' "${migration#"$repository_root"/}"
  psql_in_schema "$schema_name" --single-transaction --file "$migration"
done

# The migrations must not invent actor identities for historical accepted
# facts. Exercise that fail-closed rule independently from the empty-schema
# happy path above.
psql_unscoped --command "CREATE SCHEMA \"$legacy_schema_name\";"
created_legacy_schema=1
psql_in_schema "$legacy_schema_name" --single-transaction --file "${migrations_directory}/0001_tenant_event_log.sql"
psql_in_schema "$legacy_schema_name" --single-transaction --file "${migrations_directory}/0002_tenant_integrity_and_forced_rls.sql"
psql_in_schema "$legacy_schema_name" <<'SQL'
INSERT INTO organizations (organization_id, display_name)
VALUES ('11111111-1111-4111-8111-111111111111', 'Legacy tenant');
INSERT INTO branches (branch_id, organization_id, display_name)
VALUES ('11111111-1111-4111-8111-111111111112', '11111111-1111-4111-8111-111111111111', 'Legacy branch');
INSERT INTO devices (device_id, organization_id)
VALUES ('11111111-1111-4111-8111-111111111113', '11111111-1111-4111-8111-111111111111');
INSERT INTO sync_events (
  operation_id, organization_id, branch_id, device_id, audit_event_id,
  sequence, event_type, payload_json, occurred_at_utc, previous_hash, event_hash
) VALUES (
  '11111111-1111-4111-8111-111111111114',
  '11111111-1111-4111-8111-111111111111',
  '11111111-1111-4111-8111-111111111112',
  '11111111-1111-4111-8111-111111111113',
  '11111111-1111-4111-8111-111111111115',
  1,
  'sale.created',
  '{"amount":"100"}'::jsonb,
  '2026-07-17T00:00:00Z',
  NULL,
  decode(repeat('ab', 32), 'hex')
);
SQL
expect_failure 'migration 0003 refuses historical events without source actor identities' \
  psql_in_schema "$legacy_schema_name" --single-transaction --file "${migrations_directory}/0003_sync_event_actor_and_immutability.sql"

# Use a true non-owner role for RLS verification. The ephemeral CI database is
# initialized with an administrative migration role, so this setup does not
# model a deployed credential or grant any privilege outside this temporary
# test cluster.
psql_unscoped --command "CREATE ROLE \"$runtime_role\" NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;"
created_role=1
psql_unscoped --command "GRANT \"$runtime_role\" TO CURRENT_USER;"
psql_unscoped --command "GRANT USAGE ON SCHEMA \"$schema_name\" TO \"$runtime_role\";"
psql_unscoped --command "GRANT SELECT, INSERT ON ALL TABLES IN SCHEMA \"$schema_name\" TO \"$runtime_role\";"

psql_in_schema "$schema_name" <<'SQL'
INSERT INTO organizations (organization_id, display_name) VALUES
  ('11111111-1111-4111-8111-111111111111', 'Tenant A'),
  ('22222222-2222-4222-8222-222222222222', 'Tenant B');
INSERT INTO branches (branch_id, organization_id, display_name) VALUES
  ('11111111-1111-4111-8111-111111111112', '11111111-1111-4111-8111-111111111111', 'A branch'),
  ('22222222-2222-4222-8222-222222222223', '22222222-2222-4222-8222-222222222222', 'B branch');
INSERT INTO devices (device_id, organization_id) VALUES
  ('11111111-1111-4111-8111-111111111113', '11111111-1111-4111-8111-111111111111'),
  ('22222222-2222-4222-8222-222222222224', '22222222-2222-4222-8222-222222222222');
INSERT INTO device_branch_grants (
  organization_id, device_id, branch_id, granted_by_actor_id
) VALUES (
  '11111111-1111-4111-8111-111111111111',
  '11111111-1111-4111-8111-111111111113',
  '11111111-1111-4111-8111-111111111112',
  '11111111-1111-4111-8111-111111111116'
);
INSERT INTO branch_actors (
  organization_id, branch_id, actor_id, enrolled_by_actor_id
) VALUES (
  '11111111-1111-4111-8111-111111111111',
  '11111111-1111-4111-8111-111111111112',
  '11111111-1111-4111-8111-111111111116',
  '11111111-1111-4111-8111-111111111116'
);
INSERT INTO sync_events (
  operation_id, organization_id, branch_id, device_id, audit_event_id, actor_id,
  sequence, event_type, payload_json, payload_canonical, occurred_at_utc,
  previous_hash, event_hash, server_event_id
) VALUES (
  '11111111-1111-4111-8111-111111111114',
  '11111111-1111-4111-8111-111111111111',
  '11111111-1111-4111-8111-111111111112',
  '11111111-1111-4111-8111-111111111113',
  '11111111-1111-4111-8111-111111111115',
  '11111111-1111-4111-8111-111111111116',
  1,
  'sale.created',
  '{"amount":"100"}'::jsonb,
  '{"amount":"100"}',
  '2026-07-17T00:00:00Z',
  NULL,
  decode(repeat('ab', 32), 'hex'),
  '018f0000-0000-7000-8000-000000000001'
);
INSERT INTO device_branch_revocations (
  organization_id, device_id, branch_id, revoked_by_actor_id, reason
) VALUES (
  '11111111-1111-4111-8111-111111111111',
  '11111111-1111-4111-8111-111111111113',
  '11111111-1111-4111-8111-111111111112',
  '11111111-1111-4111-8111-111111111116',
  'test revocation'
);
INSERT INTO branch_actor_revocations (
  organization_id, branch_id, actor_id, revoked_by_actor_id, reason
) VALUES (
  '11111111-1111-4111-8111-111111111111',
  '11111111-1111-4111-8111-111111111112',
  '11111111-1111-4111-8111-111111111116',
  '11111111-1111-4111-8111-111111111116',
  'test revocation'
);
SQL

assert_scalar '8' 'all tenant tables have RLS enabled and forced' "
  SELECT count(*)
  FROM pg_class
  WHERE relnamespace = '$schema_name'::regnamespace
    AND relname IN (
      'organizations', 'branches', 'devices', 'sync_events',
      'device_branch_grants', 'device_branch_revocations',
      'branch_actors', 'branch_actor_revocations'
    )
    AND relrowsecurity
    AND relforcerowsecurity;
"
assert_scalar '8' 'all tenant tables have a tenant policy' "
  SELECT count(DISTINCT tablename)
  FROM pg_policies
  WHERE schemaname = '$schema_name'
    AND tablename IN (
      'organizations', 'branches', 'devices', 'sync_events',
      'device_branch_grants', 'device_branch_revocations',
      'branch_actors', 'branch_actor_revocations'
    )
    AND policyname LIKE 'tenant_%';
"
assert_scalar '0' 'non-owner sees no rows without tenant context' "
  SET ROLE \"$runtime_role\";
  SELECT count(*) FROM organizations;
"
assert_scalar '1' 'non-owner sees only Tenant A under Tenant A context' "
  SET ROLE \"$runtime_role\";
  BEGIN;
  SET LOCAL app.organization_id = '11111111-1111-4111-8111-111111111111';
  SELECT count(*) FROM organizations;
  COMMIT;
"
assert_scalar '0' 'non-owner cannot see Tenant A under Tenant B context' "
  SET ROLE \"$runtime_role\";
  BEGIN;
  SET LOCAL app.organization_id = '22222222-2222-4222-8222-222222222222';
  SELECT count(*) FROM sync_events;
  COMMIT;
"
expect_failure 'RLS rejects an insert whose organization differs from the tenant context' \
  psql_in_schema "$schema_name" --command "
    SET ROLE \"$runtime_role\";
    BEGIN;
    SET LOCAL app.organization_id = '22222222-2222-4222-8222-222222222222';
    INSERT INTO branches (branch_id, organization_id, display_name)
    VALUES ('11111111-1111-4111-8111-111111111117', '11111111-1111-4111-8111-111111111111', 'forbidden');
    ROLLBACK;
  "

# Cross-tenant foreign keys and acknowledgement identity constraints must
# still hold for a migration owner, where RLS is deliberately not relied on.
expect_failure 'a sync event cannot bind a Tenant A event to a Tenant B branch' \
  psql_in_schema "$schema_name" --command "
    INSERT INTO sync_events (
      operation_id, organization_id, branch_id, device_id, audit_event_id, actor_id,
      sequence, event_type, payload_json, payload_canonical, occurred_at_utc,
      previous_hash, event_hash, server_event_id
    ) VALUES (
      '11111111-1111-4111-8111-111111111118',
      '11111111-1111-4111-8111-111111111111',
      '22222222-2222-4222-8222-222222222223',
      '11111111-1111-4111-8111-111111111113',
      '11111111-1111-4111-8111-111111111119',
      '11111111-1111-4111-8111-111111111116',
      2, 'sale.created', '{\"amount\":\"101\"}'::jsonb, '{\"amount\":\"101\"}',
      '2026-07-17T00:00:01Z', decode(repeat('ab', 32), 'hex'),
      decode(repeat('cd', 32), 'hex'), '018f0000-0000-7000-8000-000000000002'
    );
  "
expect_failure 'server acknowledgement identity must be UUIDv7' \
  psql_in_schema "$schema_name" --command "
    INSERT INTO sync_events (
      operation_id, organization_id, branch_id, device_id, audit_event_id, actor_id,
      sequence, event_type, payload_json, payload_canonical, occurred_at_utc,
      previous_hash, event_hash, server_event_id
    ) VALUES (
      '11111111-1111-4111-8111-111111111118',
      '11111111-1111-4111-8111-111111111111',
      '11111111-1111-4111-8111-111111111112',
      '11111111-1111-4111-8111-111111111113',
      '11111111-1111-4111-8111-111111111119',
      '11111111-1111-4111-8111-111111111116',
      2, 'sale.created', '{\"amount\":\"101\"}'::jsonb, '{\"amount\":\"101\"}',
      '2026-07-17T00:00:01Z', decode(repeat('ab', 32), 'hex'),
      decode(repeat('cd', 32), 'hex'), '11111111-1111-4111-8111-111111111120'
    );
  "
expect_failure 'payload canonical text must match the stored JSON value' \
  psql_in_schema "$schema_name" --command "
    INSERT INTO sync_events (
      operation_id, organization_id, branch_id, device_id, audit_event_id, actor_id,
      sequence, event_type, payload_json, payload_canonical, occurred_at_utc,
      previous_hash, event_hash, server_event_id
    ) VALUES (
      '11111111-1111-4111-8111-111111111118',
      '11111111-1111-4111-8111-111111111111',
      '11111111-1111-4111-8111-111111111112',
      '11111111-1111-4111-8111-111111111113',
      '11111111-1111-4111-8111-111111111119',
      '11111111-1111-4111-8111-111111111116',
      2, 'sale.created', '{\"amount\":\"101\"}'::jsonb, '{\"amount\":\"102\"}',
      '2026-07-17T00:00:01Z', decode(repeat('ab', 32), 'hex'),
      decode(repeat('cd', 32), 'hex'), '018f0000-0000-7000-8000-000000000002'
    );
  "

# Every accepted event and authorization fact is append-only. Check all three
# mutation paths, not merely revoked SQL privileges, so accidental future
# grants cannot silently erase evidence.
immutable_tables=(
  sync_events
  device_branch_grants
  device_branch_revocations
  branch_actors
  branch_actor_revocations
)
for immutable_table in "${immutable_tables[@]}"; do
  expect_failure "$immutable_table rejects UPDATE" \
    psql_in_schema "$schema_name" --command "UPDATE $immutable_table SET organization_id = organization_id;"
  expect_failure "$immutable_table rejects DELETE" \
    psql_in_schema "$schema_name" --command "DELETE FROM $immutable_table;"
  expect_failure "$immutable_table rejects TRUNCATE" \
    psql_in_schema "$schema_name" --command "TRUNCATE TABLE $immutable_table CASCADE;"
done

printf 'Cloud migrations verified in disposable schema %s.\n' "$schema_name"
