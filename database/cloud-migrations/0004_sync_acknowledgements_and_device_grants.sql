-- Provider-neutral Professional sync persistence contract.
--
-- This migration adds the identities required by the feature-gated
-- authenticated sync route to return stable acknowledgements, authorize a
-- device for one branch, and verify every local actor named by an uploaded
-- audit event.
--
-- This migration deliberately fails if an experimental deployment already
-- accepted events. payload_canonical and server_event_id cannot be recovered
-- from the old row, and the new branch-actor foreign key requires the actual
-- historical actor enrollment. The original canonical byte string
-- participates in the audit hash. Such a deployment needs a separately
-- reviewed, source-backed recovery migration rather than invented values.

ALTER TABLE sync_events
  ADD COLUMN payload_canonical text NOT NULL,
  ADD COLUMN server_event_id uuid NOT NULL,
  ADD COLUMN accepted_at_utc timestamptz NOT NULL DEFAULT now();

ALTER TABLE sync_events
  ADD CONSTRAINT sync_events_server_event_id_unique UNIQUE (server_event_id),
  ADD CONSTRAINT sync_events_server_event_id_canonical_v7
    CHECK (
      server_event_id::text ~
        '^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
    ),
  ADD CONSTRAINT sync_events_event_hash_length CHECK (octet_length(event_hash) = 32),
  ADD CONSTRAINT sync_events_previous_hash_length
    CHECK (previous_hash IS NULL OR octet_length(previous_hash) = 32),
  ADD CONSTRAINT sync_events_event_type_canonical
    CHECK (octet_length(event_type) BETWEEN 1 AND 200 AND event_type ~ '^[a-z][a-z0-9._-]*$'),
  ADD CONSTRAINT sync_events_payload_canonical_matches_json
    CHECK (payload_canonical::jsonb = payload_json),
  ADD CONSTRAINT sync_events_payload_canonical_size
    CHECK (octet_length(payload_canonical) BETWEEN 1 AND 1048576);

CREATE TABLE device_branch_grants (
  organization_id uuid NOT NULL,
  device_id uuid NOT NULL,
  branch_id uuid NOT NULL,
  granted_at_utc timestamptz NOT NULL DEFAULT now(),
  granted_by_actor_id uuid NOT NULL,
  PRIMARY KEY (organization_id, device_id, branch_id),
  FOREIGN KEY (organization_id, device_id)
    REFERENCES devices (organization_id, device_id),
  FOREIGN KEY (organization_id, branch_id)
    REFERENCES branches (organization_id, branch_id)
);

-- A revocation is permanent for this device/branch identity. Restoring cloud
-- access requires a separately registered device identity; deleting or
-- rewriting either fact is never a reactivation mechanism.
CREATE TABLE device_branch_revocations (
  organization_id uuid NOT NULL,
  device_id uuid NOT NULL,
  branch_id uuid NOT NULL,
  revoked_at_utc timestamptz NOT NULL DEFAULT now(),
  revoked_by_actor_id uuid NOT NULL,
  reason text NOT NULL CHECK (
    octet_length(reason) BETWEEN 1 AND 500
    AND reason = btrim(reason)
    AND reason !~ '[[:cntrl:]]'
  ),
  PRIMARY KEY (organization_id, device_id, branch_id),
  FOREIGN KEY (organization_id, device_id, branch_id)
    REFERENCES device_branch_grants (organization_id, device_id, branch_id)
);

CREATE TABLE branch_actors (
  organization_id uuid NOT NULL,
  branch_id uuid NOT NULL,
  actor_id uuid NOT NULL,
  enrolled_at_utc timestamptz NOT NULL DEFAULT now(),
  enrolled_by_actor_id uuid NOT NULL,
  PRIMARY KEY (organization_id, branch_id, actor_id),
  FOREIGN KEY (organization_id, branch_id)
    REFERENCES branches (organization_id, branch_id)
);

-- Actor revocation is likewise permanent for this branch-scoped actor
-- identity. A later human re-enrollment must receive a new actor identity so
-- historical attribution remains unambiguous.
CREATE TABLE branch_actor_revocations (
  organization_id uuid NOT NULL,
  branch_id uuid NOT NULL,
  actor_id uuid NOT NULL,
  revoked_at_utc timestamptz NOT NULL DEFAULT now(),
  revoked_by_actor_id uuid NOT NULL,
  reason text NOT NULL CHECK (
    octet_length(reason) BETWEEN 1 AND 500
    AND reason = btrim(reason)
    AND reason !~ '[[:cntrl:]]'
  ),
  PRIMARY KEY (organization_id, branch_id, actor_id),
  FOREIGN KEY (organization_id, branch_id, actor_id)
    REFERENCES branch_actors (organization_id, branch_id, actor_id)
);

ALTER TABLE sync_events
  ADD CONSTRAINT sync_events_actor_authorized_for_branch
  FOREIGN KEY (organization_id, branch_id, actor_id)
  REFERENCES branch_actors (organization_id, branch_id, actor_id);

ALTER TABLE device_branch_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_branch_grants FORCE ROW LEVEL SECURITY;
ALTER TABLE device_branch_revocations ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_branch_revocations FORCE ROW LEVEL SECURITY;
ALTER TABLE branch_actors ENABLE ROW LEVEL SECURITY;
ALTER TABLE branch_actors FORCE ROW LEVEL SECURITY;
ALTER TABLE branch_actor_revocations ENABLE ROW LEVEL SECURITY;
ALTER TABLE branch_actor_revocations FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_device_branch_grants ON device_branch_grants
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid)
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);

CREATE POLICY tenant_device_branch_revocations ON device_branch_revocations
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid)
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);

CREATE POLICY tenant_branch_actors ON branch_actors
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid)
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);

CREATE POLICY tenant_branch_actor_revocations ON branch_actor_revocations
  USING (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid)
  WITH CHECK (organization_id = NULLIF(current_setting('app.organization_id', true), '')::uuid);

CREATE FUNCTION professional_security_facts_reject_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION
    USING ERRCODE = '55000',
          MESSAGE = 'Professional authorization facts are append-only';
  RETURN NULL;
END;
$$;

CREATE TRIGGER device_branch_grants_reject_update
BEFORE UPDATE ON device_branch_grants FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER device_branch_grants_reject_delete
BEFORE DELETE ON device_branch_grants FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER device_branch_grants_reject_truncate
AFTER TRUNCATE ON device_branch_grants FOR EACH STATEMENT
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER device_branch_revocations_reject_update
BEFORE UPDATE ON device_branch_revocations FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER device_branch_revocations_reject_delete
BEFORE DELETE ON device_branch_revocations FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER device_branch_revocations_reject_truncate
AFTER TRUNCATE ON device_branch_revocations FOR EACH STATEMENT
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER branch_actors_reject_update
BEFORE UPDATE ON branch_actors FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER branch_actors_reject_delete
BEFORE DELETE ON branch_actors FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER branch_actors_reject_truncate
AFTER TRUNCATE ON branch_actors FOR EACH STATEMENT
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER branch_actor_revocations_reject_update
BEFORE UPDATE ON branch_actor_revocations FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER branch_actor_revocations_reject_delete
BEFORE DELETE ON branch_actor_revocations FOR EACH ROW
EXECUTE FUNCTION professional_security_facts_reject_mutation();
CREATE TRIGGER branch_actor_revocations_reject_truncate
AFTER TRUNCATE ON branch_actor_revocations FOR EACH STATEMENT
EXECUTE FUNCTION professional_security_facts_reject_mutation();

REVOKE UPDATE, DELETE, TRUNCATE ON TABLE
  device_branch_grants,
  device_branch_revocations,
  branch_actors,
  branch_actor_revocations
FROM PUBLIC;
