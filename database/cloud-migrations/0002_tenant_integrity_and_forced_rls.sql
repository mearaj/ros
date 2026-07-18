-- Professional cloud hardening. This migration is forward-only and must be
-- applied by the controlled migration role before the application role is
-- allowed to accept sync traffic.

-- A sync event cannot name a branch or device from another organization even
-- if a future application query accidentally omits an organization predicate.
ALTER TABLE branches
  ADD CONSTRAINT branches_organization_branch_unique
  UNIQUE (organization_id, branch_id);

ALTER TABLE devices
  ADD CONSTRAINT devices_organization_device_unique
  UNIQUE (organization_id, device_id);

ALTER TABLE sync_events
  ADD CONSTRAINT sync_events_branch_same_organization
  FOREIGN KEY (organization_id, branch_id)
  REFERENCES branches (organization_id, branch_id);

ALTER TABLE sync_events
  ADD CONSTRAINT sync_events_device_same_organization
  FOREIGN KEY (organization_id, device_id)
  REFERENCES devices (organization_id, device_id);

-- RLS is enabled on every tenant table and forced to apply even to the table
-- owner. A migration/admin role still needs BYPASSRLS only for controlled
-- maintenance outside request handling; the API role must not receive it.
ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;
ALTER TABLE organizations FORCE ROW LEVEL SECURITY;
ALTER TABLE branches FORCE ROW LEVEL SECURITY;
ALTER TABLE devices FORCE ROW LEVEL SECURITY;
ALTER TABLE sync_events FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_organizations ON organizations
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid)
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);

-- The initial policies already reject reads without app.organization_id. WITH
-- CHECK closes the corresponding insert/update path as well.
ALTER POLICY tenant_branches ON branches
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);

ALTER POLICY tenant_devices ON devices
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);

ALTER POLICY tenant_events ON sync_events
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);
