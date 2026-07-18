-- Security decisions must follow durable insertion order, not wall-clock
-- text. Restaurant devices can have their clocks corrected or moved
-- backwards while offline; that must never resurrect an older credential,
-- role, status, or session event.
CREATE TABLE local_security_fact_order (
    fact_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_kind TEXT NOT NULL CHECK (
        fact_kind IN (
            'staff_pin_credential',
            'staff_status',
            'staff_role',
            'staff_session',
            'staff_pin_attempt'
        )
    ),
    fact_id TEXT NOT NULL,
    UNIQUE (fact_kind, fact_id)
) STRICT;

-- Preserve the physical append order of pre-v26 facts exactly once. All of
-- these source tables already reject UPDATE and DELETE, so rowid is a safe
-- migration input; the explicit sequence below is the durable authority from
-- this migration onward (including across VACUUM).
INSERT INTO local_security_fact_order (fact_kind, fact_id)
SELECT 'staff_pin_credential', staff_pin_credential_id
FROM staff_pin_credentials
ORDER BY rowid;

INSERT INTO local_security_fact_order (fact_kind, fact_id)
SELECT 'staff_status', staff_status_event_id
FROM staff_status_events
ORDER BY rowid;

INSERT INTO local_security_fact_order (fact_kind, fact_id)
SELECT 'staff_role', staff_role_event_id
FROM staff_role_events
ORDER BY rowid;

INSERT INTO local_security_fact_order (fact_kind, fact_id)
SELECT 'staff_session', local_staff_session_event_id
FROM local_staff_session_events
ORDER BY rowid;

INSERT INTO local_security_fact_order (fact_kind, fact_id)
SELECT 'staff_pin_attempt', staff_pin_attempt_id
FROM staff_pin_attempts
ORDER BY rowid;

CREATE TRIGGER staff_pin_credentials_assign_monotonic_order
AFTER INSERT ON staff_pin_credentials
BEGIN
    INSERT INTO local_security_fact_order (fact_kind, fact_id)
    VALUES ('staff_pin_credential', NEW.staff_pin_credential_id);
END;

CREATE TRIGGER staff_status_events_assign_monotonic_order
AFTER INSERT ON staff_status_events
BEGIN
    INSERT INTO local_security_fact_order (fact_kind, fact_id)
    VALUES ('staff_status', NEW.staff_status_event_id);
END;

CREATE TRIGGER staff_role_events_assign_monotonic_order
AFTER INSERT ON staff_role_events
BEGIN
    INSERT INTO local_security_fact_order (fact_kind, fact_id)
    VALUES ('staff_role', NEW.staff_role_event_id);
END;

CREATE TRIGGER local_staff_session_events_assign_monotonic_order
AFTER INSERT ON local_staff_session_events
BEGIN
    INSERT INTO local_security_fact_order (fact_kind, fact_id)
    VALUES ('staff_session', NEW.local_staff_session_event_id);
END;

CREATE TRIGGER staff_pin_attempts_assign_monotonic_order
AFTER INSERT ON staff_pin_attempts
BEGIN
    INSERT INTO local_security_fact_order (fact_kind, fact_id)
    VALUES ('staff_pin_attempt', NEW.staff_pin_attempt_id);
END;

CREATE TRIGGER local_security_fact_order_cannot_be_updated
BEFORE UPDATE ON local_security_fact_order
BEGIN
    SELECT RAISE(ABORT, 'local security fact order is immutable');
END;

CREATE TRIGGER local_security_fact_order_cannot_be_deleted
BEFORE DELETE ON local_security_fact_order
BEGIN
    SELECT RAISE(ABORT, 'local security fact order cannot be deleted');
END;

-- The maximum successfully observed clock value is a bounded high-water
-- control. Session and PIN checks fail closed whenever the current wall clock
-- is behind it. This is deliberately derived mutable state rather than a
-- business fact: polling it must not create millions of history rows. The
-- triggers allow only a monotonic advance and forbid removal or replacement.
CREATE TABLE local_security_clock_state (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    high_water_utc TEXT NOT NULL
) STRICT;

INSERT INTO local_security_clock_state (singleton, high_water_utc)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

CREATE TRIGGER local_security_clock_state_monotonic_update_only
BEFORE UPDATE ON local_security_clock_state
WHEN NEW.singleton <> OLD.singleton
    OR NEW.high_water_utc < OLD.high_water_utc
BEGIN
    SELECT RAISE(ABORT, 'local security clock high-water mark cannot move backwards');
END;

CREATE TRIGGER local_security_clock_state_cannot_be_deleted
BEFORE DELETE ON local_security_clock_state
BEGIN
    SELECT RAISE(ABORT, 'local security clock state cannot be deleted');
END;
