# Cash drawer ledger v1

Each branch may have one unclosed local drawer session. Opening records an
immutable opening float. Closing records a separate immutable closure fact;
neither record can be updated or deleted.

Expected cash is derived inside the close transaction:

`opening float + post-open cash payments - post-open cash refunds - post-open cash expenses`

The counted amount and signed variance are retained together with the session,
actor, timestamp, audit event, and future-sync outbox envelope. The app must
not silently force the count to match expected cash.

Opening and closing require Owner or Manager authority. Community currently
uses its local owner context; future local staff PIN sessions must supply a
trusted Owner/Manager context rather than bypassing this storage boundary.

The owner UI must obtain the currently unclosed session from an encrypted
storage read model. It must never reconstruct that state from Flutter memory
or assume that a previously displayed session ID remains open after restart.
