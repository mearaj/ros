# Entitlement evaluation contract v1

## Status and scope

This is a provider-neutral domain foundation, not a finished licensing
service. `ros_core` can validate entitlement facts and deterministically decide
edition state and branch access. It does **not** yet issue or verify signed
tokens, contact a billing provider, refresh a subscription, select a trusted
clock source, or persist the clock high-water mark. Integrations must not treat
an untrusted API or UI payload as validated claims.

No entitlement transition deletes, archives, filters, or otherwise hides
restaurant data. A known branch that cannot safely write remains readable and
exportable.

## Edition terms

- **Community** is the untimed local baseline and never expires. Its explicit
  primary branch is read/write; any other known branch is read-only/exportable.
- **Evaluation** is exactly fourteen 24-hour UTC days and allows up to five
  writable branches.
- **Professional** is an annual term and allows up to five writable branches.
- **Enterprise** is an annual term with a positive writable-branch capacity
  carried by the claim. Capacities above 10,000 are malformed rather than
  becoming an unbounded resource policy.
- An annual UTC interval must be exactly 365 or 366 whole days. The future
  authoritative issuer is responsible for choosing the correct calendar-year
  interval.

Timed claims require `issued_at <= not_before < expires_at`. Their active
window is half-open: `[not_before, expires_at)`. Evaluation and annual term
lengths are checked after ordering, using unsigned whole-second values and
checked timestamp addition at call sites.

## State transitions

| Condition | State | Write behavior |
| --- | --- | --- |
| Community with a safe clock | Active | Primary branch only |
| Timed claim inside its active window | Active | Licensed branch capacity |
| Expired timed claim inside configured grace | Grace | Licensed branch capacity |
| Grace ended or configured grace is zero | Community safe mode | Primary branch only |
| Claim is not yet valid | Fail closed | No writes; known data remains readable/exportable |
| Clock rollback exceeds configured tolerance | Fail closed | No writes; known data remains readable/exportable |

Grace duration and rollback tolerance are explicit local policy inputs. No
default commercial grace period is asserted by this contract.

## Clock safety

Evaluation receives both the current observed UTC second and an optional
trusted high-water second loaded by the caller. It evaluates at the greater of
those values, so even a tolerated small rollback cannot extend an evaluation,
annual term, or grace period. A rollback strictly greater than the configured
tolerance fails closed. A rollback exactly equal to the tolerance continues at
the high-water instant.

The returned `next_high_water` never decreases. A future integration must
persist it atomically with every entitlement-authorized write. Clock rollback,
state transitions, and high-water changes should also become immutable audit
facts when persistence is implemented.

## Branch decisions

The evaluator requires one explicit primary branch and a duplicate-free list
of all known branches in stable, authoritative activation order. UI sorting is
not authoritative. Professional-family writable slots contain the primary
branch first, then the earliest other activated branches until the five-branch
or Enterprise claim capacity is reached.

- A writable licensed branch returns `ReadWrite`.
- A known branch outside capacity returns `ReadOnlyExport`.
- An unknown branch identity returns `Denied`.
- Community safe mode always reduces write access to the primary branch.
- Fail-closed states remove all write access, but every known branch remains
  `ReadOnlyExport`.

The storage integration must authenticate claims, protect stable branch order,
re-evaluate inside each write transaction, and enforce the returned branch
decision. A UI-only check is not an authorization boundary.
