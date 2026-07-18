-- Professional cloud hardening. This migration is forward-only. `actor_id`
-- participates in the local audit-event hash, so it must never be fabricated
-- for an already accepted cloud event.
--
-- PostgreSQL will reject this statement if sync_events already has rows. That
-- is intentional: a deployment with historical accepted facts must first use
-- a separately reviewed, source-backed recovery procedure that supplies the
-- original actor identity for every fact. Do not substitute a default actor.
ALTER TABLE sync_events
  ADD COLUMN actor_id uuid NOT NULL;

-- Accepted device events are facts, not a mutable queue. The API role must
-- have INSERT only; these triggers also fail closed if a broader privilege is
-- accidentally granted later. TRUNCATE is included because it is another way
-- to erase accepted facts without issuing DELETE statements.
CREATE FUNCTION sync_events_reject_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION
    USING ERRCODE = '55000',
          MESSAGE = 'accepted sync events are immutable';
  RETURN NULL;
END;
$$;

CREATE TRIGGER sync_events_reject_update
BEFORE UPDATE ON sync_events
FOR EACH ROW
EXECUTE FUNCTION sync_events_reject_mutation();

CREATE TRIGGER sync_events_reject_delete
BEFORE DELETE ON sync_events
FOR EACH ROW
EXECUTE FUNCTION sync_events_reject_mutation();

CREATE TRIGGER sync_events_reject_truncate
AFTER TRUNCATE ON sync_events
FOR EACH STATEMENT
EXECUTE FUNCTION sync_events_reject_mutation();

REVOKE UPDATE, DELETE, TRUNCATE ON TABLE sync_events FROM PUBLIC;
