# ROS editions

These documents define the customer-facing behavior and release boundaries for
Restaurant Operating System (ROS). They are deliberately separate from the
implementation roadmap: a capability is not available for sale or marketing
until its acceptance and release evidence exists.

**Reviewers:** [`openai-build-week/README.md`](../../openai-build-week/README.md) ·
**Doc index:** [`docs/README.md`](../README.md)

- [Community Edition](community.md) — free, local-first, one-branch ROS.
- [Community Edition user guide](../runbooks/community-user-guide.md) — plain-language
  setup, daily use, and role quick guides.
- [Community-first delivery contract](community-delivery-contract.md) — active
  work order, UX rules, regression safeguards, and the gate before
  Professional work resumes.
- [Professional Edition](professional.md) — free 14-day evaluation and paid
  annual entitlement for growing multi-branch restaurants.
- [Enterprise Edition](enterprise.md) — annual, contract-configured
  organization offering.

ROS has one codebase and one app. First launch offers **Community** or **Paid
editions** (Professional Evaluation, Professional Paid, Enterprise). Community
then chooses **Hub** or **Client** device role. Each branch activates exactly
one authority: Community coordinates its devices through a local Branch Hub
over encrypted LAN; Professional Evaluation, Professional Paid, and Enterprise
coordinate through Gotigin cloud over WAN. Moving in either direction does not
require a reinstall, but it is a verified data-topology migration—not a
simultaneous LAN/cloud mode or an instant settings toggle. See
[ADR 0010](../adr/0010-edition-data-topology-and-switching.md).

On a Community Hub, Owner, Manager, Cashier, and Kitchen may unlock the UI.
The Hub service keeps serving LAN clients while the UI is locked or unlocked
as a non-Owner role. Only Owner administers pairing, portable recovery, and
local restaurant profiles. Forgot-both-secrets creates a new empty profile and
keeps the prior profile in local history until a PIN or passphrase returns.

The current delivery priority is the complete, dependable Community Edition on
Android, iOS, Linux, Windows, and macOS. The next tier after Community is
Professional Evaluation (free 14-day access), followed by Professional Paid.
Professional and Enterprise remain release-gated until their cloud, identity,
entitlement, and deployment evidence is complete.
