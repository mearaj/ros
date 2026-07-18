# Professional owner dashboard

Minimal Stage 4/5 owner dashboard for Restaurant Operating System Professional.

## Run

1. Start the Professional API (`services/api`) with sync enabled against staging
   Postgres.
2. Serve this directory with any static file server, for example:

```bash
cd apps/owner-dashboard
python3 -m http.server 8080
```

3. Open `http://127.0.0.1:8080` and point the API base URL at the Professional
   API (default `http://127.0.0.1:3000`).

## Pages

- Organization overview and entitlement state
- Branch list / switcher metadata
- Device status and revocation action
- Sync health / backlog summary
- Consolidated sales totals (from API reporting endpoints)

Authentication uses a short-lived owner bearer token pasted into the page for
local staging; production will replace this with OIDC/passkey session cookies
once the identity issuer is provisioned (ADR 0003).
