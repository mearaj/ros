-- PostgreSQL foundation. Applied only by the Professional cloud service.
CREATE TABLE organizations (organization_id uuid PRIMARY KEY, display_name text NOT NULL);
CREATE TABLE branches (branch_id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations, display_name text NOT NULL);
CREATE TABLE devices (device_id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations, revoked_at timestamptz);
CREATE TABLE sync_events (
  operation_id uuid PRIMARY KEY,
  organization_id uuid NOT NULL REFERENCES organizations,
  branch_id uuid NOT NULL REFERENCES branches,
  device_id uuid NOT NULL REFERENCES devices,
  audit_event_id uuid NOT NULL UNIQUE,
  sequence bigint NOT NULL CHECK (sequence > 0),
  event_type text NOT NULL,
  payload_json jsonb NOT NULL,
  occurred_at_utc timestamptz NOT NULL,
  previous_hash bytea,
  event_hash bytea NOT NULL,
  received_at_utc timestamptz NOT NULL DEFAULT now(),
  UNIQUE (device_id, sequence)
);
ALTER TABLE branches ENABLE ROW LEVEL SECURITY;
ALTER TABLE devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE sync_events ENABLE ROW LEVEL SECURITY;
-- The service sets app.organization_id from verified authentication claims
-- inside every transaction. Missing context denies access by default.
CREATE POLICY tenant_branches ON branches
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);
CREATE POLICY tenant_devices ON devices
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);
CREATE POLICY tenant_events ON sync_events
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);
