-- Dual-person correction approvals (ADR 0006). Requests and decisions are
-- append-only. A consuming mutation records approval_consumptions so a decision
-- cannot be reused.

CREATE TABLE IF NOT EXISTS correction_approval_requests (
    approval_request_id TEXT PRIMARY KEY NOT NULL,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    action_type TEXT NOT NULL CHECK (
        action_type IN (
            'refund',
            'void',
            'discount',
            'stock_adjustment'
        )
    ),
    target_entity_type TEXT NOT NULL CHECK (length(trim(target_entity_type)) BETWEEN 1 AND 64),
    target_entity_id TEXT NOT NULL CHECK (length(trim(target_entity_id)) BETWEEN 1 AND 64),
    amount_minor INTEGER,
    currency_code TEXT CHECK (
        currency_code IS NULL
        OR (
            length(currency_code) = 3
            AND currency_code = upper(currency_code)
        )
    ),
    policy_version TEXT NOT NULL CHECK (policy_version = 'correction-approval.v1'),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 3 AND 500),
    requested_at_utc TEXT NOT NULL,
    requested_by_actor_id TEXT NOT NULL,
    expires_at_utc TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS correction_approval_requests_branch_time
    ON correction_approval_requests (branch_id, requested_at_utc);

CREATE TABLE IF NOT EXISTS correction_approval_decisions (
    approval_decision_id TEXT PRIMARY KEY NOT NULL,
    approval_request_id TEXT NOT NULL UNIQUE
        REFERENCES correction_approval_requests (approval_request_id) ON DELETE RESTRICT,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'denied')),
    decided_at_utc TEXT NOT NULL,
    decided_by_actor_id TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS correction_approval_consumptions (
    approval_consumption_id TEXT PRIMARY KEY NOT NULL,
    approval_request_id TEXT NOT NULL UNIQUE
        REFERENCES correction_approval_requests (approval_request_id) ON DELETE RESTRICT,
    branch_id TEXT NOT NULL
        REFERENCES branches (branch_id) ON DELETE RESTRICT,
    consumed_at_utc TEXT NOT NULL,
    consumed_by_actor_id TEXT NOT NULL,
    resulting_entity_type TEXT NOT NULL,
    resulting_entity_id TEXT NOT NULL
) STRICT;

CREATE TRIGGER IF NOT EXISTS correction_approval_requests_cannot_be_updated
BEFORE UPDATE ON correction_approval_requests
BEGIN
    SELECT RAISE(ABORT, 'correction approval requests are immutable');
END;

CREATE TRIGGER IF NOT EXISTS correction_approval_requests_cannot_be_deleted
BEFORE DELETE ON correction_approval_requests
BEGIN
    SELECT RAISE(ABORT, 'correction approval requests must be preserved');
END;

CREATE TRIGGER IF NOT EXISTS correction_approval_decisions_cannot_be_updated
BEFORE UPDATE ON correction_approval_decisions
BEGIN
    SELECT RAISE(ABORT, 'correction approval decisions are immutable');
END;

CREATE TRIGGER IF NOT EXISTS correction_approval_decisions_cannot_be_deleted
BEFORE DELETE ON correction_approval_decisions
BEGIN
    SELECT RAISE(ABORT, 'correction approval decisions must be preserved');
END;

CREATE TRIGGER IF NOT EXISTS correction_approval_consumptions_cannot_be_updated
BEFORE UPDATE ON correction_approval_consumptions
BEGIN
    SELECT RAISE(ABORT, 'correction approval consumptions are immutable');
END;

CREATE TRIGGER IF NOT EXISTS correction_approval_consumptions_cannot_be_deleted
BEFORE DELETE ON correction_approval_consumptions
BEGIN
    SELECT RAISE(ABORT, 'correction approval consumptions must be preserved');
END;

CREATE TRIGGER IF NOT EXISTS correction_approval_decisions_require_distinct_actor
BEFORE INSERT ON correction_approval_decisions
WHEN NEW.decided_by_actor_id = (
    SELECT requested_by_actor_id
    FROM correction_approval_requests
    WHERE approval_request_id = NEW.approval_request_id
)
OR NEW.branch_id != (
    SELECT branch_id
    FROM correction_approval_requests
    WHERE approval_request_id = NEW.approval_request_id
)
BEGIN
    SELECT RAISE(ABORT, 'approver must be a distinct actor on the same branch');
END;
