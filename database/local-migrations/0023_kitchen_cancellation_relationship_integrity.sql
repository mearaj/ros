-- Kitchen-cancellation facts reference several immutable records. Foreign
-- keys alone cannot express that every reference belongs to the same branch,
-- ticket, revision, and actor. These guards reject accidental or direct-SQL
-- cross-linking before a false stop-work or acknowledgement fact can exist.

CREATE TRIGGER IF NOT EXISTS kitchen_tickets_must_match_draft_revision
BEFORE INSERT ON kitchen_tickets
WHEN NOT EXISTS (
    SELECT 1
    FROM draft_orders AS draft
    JOIN draft_order_revisions AS draft_revision
      ON draft_revision.draft_order_id = draft.draft_order_id
     AND draft_revision.revision = NEW.draft_revision
    WHERE draft.draft_order_id = NEW.draft_order_id
      AND draft.branch_id = NEW.branch_id
      AND draft_revision.line_snapshot_json = NEW.line_snapshot_json
)
BEGIN
    SELECT RAISE(ABORT, 'kitchen ticket must match its branch draft revision');
END;

CREATE TRIGGER IF NOT EXISTS kitchen_ticket_cancellation_notices_must_match_ticket_and_audit
BEFORE INSERT ON kitchen_ticket_cancellation_notices
WHEN NOT EXISTS (
    SELECT 1
    FROM kitchen_tickets AS ticket
    JOIN draft_orders AS draft
      ON draft.draft_order_id = ticket.draft_order_id
    JOIN draft_order_revisions AS draft_revision
      ON draft_revision.draft_order_id = ticket.draft_order_id
     AND draft_revision.revision = ticket.draft_revision
    JOIN audit_events AS audit
      ON audit.event_id = NEW.audit_event_id
    WHERE ticket.kitchen_ticket_id = NEW.kitchen_ticket_id
      AND ticket.branch_id = NEW.branch_id
      AND ticket.draft_order_id = NEW.draft_order_id
      AND ticket.draft_revision = NEW.draft_revision
      AND ticket.line_snapshot_json = draft_revision.line_snapshot_json
      AND draft.branch_id = NEW.branch_id
      AND audit.branch_id = NEW.branch_id
      AND audit.actor_id = NEW.occurred_by_actor_id
)
BEGIN
    SELECT RAISE(ABORT, 'kitchen cancellation notice must match ticket, draft, and audit identity');
END;

CREATE TRIGGER IF NOT EXISTS kitchen_ticket_cancellation_acknowledgements_must_match_notice_and_audit
BEFORE INSERT ON kitchen_ticket_cancellation_acknowledgements
WHEN NOT EXISTS (
    SELECT 1
    FROM kitchen_ticket_cancellation_notices AS notice
    JOIN audit_events AS audit
      ON audit.event_id = NEW.audit_event_id
    WHERE notice.kitchen_ticket_cancellation_notice_id = NEW.kitchen_ticket_cancellation_notice_id
      AND notice.branch_id = NEW.branch_id
      AND audit.branch_id = NEW.branch_id
      AND audit.actor_id = NEW.acknowledged_by_actor_id
)
BEGIN
    SELECT RAISE(ABORT, 'kitchen cancellation acknowledgement must match notice and audit identity');
END;
