-- A kitchen note belongs to an immutable draft revision. `NULL` is the only
-- representation for an absent note; callers normalize blank input to NULL
-- before persistence. Existing revisions and tickets intentionally retain no
-- note rather than receiving a synthetic historical value.
ALTER TABLE draft_order_revisions
ADD COLUMN kitchen_note TEXT
    CHECK (
        kitchen_note IS NULL
        OR (
            length(kitchen_note) BETWEEN 1 AND 500
            AND kitchen_note = trim(kitchen_note)
            AND instr(kitchen_note, char(0)) = 0
            AND kitchen_note NOT GLOB '*[' || char(1) || '-' || char(31) || ']*'
            AND kitchen_note NOT GLOB '*[' || char(127) || '-' || char(159) || ']*'
        )
    );

-- The kitchen receives its own immutable snapshot. It must never read a
-- mutable current-draft field after a ticket is sent.
ALTER TABLE kitchen_tickets
ADD COLUMN kitchen_note_snapshot TEXT
    CHECK (
        kitchen_note_snapshot IS NULL
        OR (
            length(kitchen_note_snapshot) BETWEEN 1 AND 500
            AND kitchen_note_snapshot = trim(kitchen_note_snapshot)
            AND instr(kitchen_note_snapshot, char(0)) = 0
            AND kitchen_note_snapshot NOT GLOB '*[' || char(1) || '-' || char(31) || ']*'
            AND kitchen_note_snapshot NOT GLOB '*[' || char(127) || '-' || char(159) || ']*'
        )
    );

-- Kitchen state and optimistic revision are deliberately mutable operational
-- fields. The note snapshot is not: a direct SQL update cannot rewrite an
-- instruction already sent to preparation.
CREATE TRIGGER IF NOT EXISTS kitchen_ticket_note_snapshot_is_immutable
BEFORE UPDATE OF kitchen_note_snapshot ON kitchen_tickets
WHEN NEW.kitchen_note_snapshot IS NOT OLD.kitchen_note_snapshot
BEGIN
    SELECT RAISE(ABORT, 'kitchen ticket note snapshot is immutable');
END;

-- The existing ticket/draft trigger binds line snapshots. Bind the note as
-- well so a direct SQL insert cannot pair a valid ticket with an instruction
-- from another revision.
CREATE TRIGGER IF NOT EXISTS kitchen_tickets_kitchen_note_must_match_draft_revision
BEFORE INSERT ON kitchen_tickets
WHEN NOT EXISTS (
    SELECT 1
    FROM draft_orders AS draft
    JOIN draft_order_revisions AS draft_revision
      ON draft_revision.draft_order_id = draft.draft_order_id
     AND draft_revision.revision = NEW.draft_revision
    WHERE draft.draft_order_id = NEW.draft_order_id
      AND draft.branch_id = NEW.branch_id
      AND NEW.kitchen_note_snapshot IS draft_revision.kitchen_note
)
BEGIN
    SELECT RAISE(ABORT, 'kitchen ticket note must match its draft revision');
END;
