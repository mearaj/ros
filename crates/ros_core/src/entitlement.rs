//! Provider-neutral entitlement rules.
//!
//! This module evaluates already-authenticated entitlement facts. It does not
//! issue, sign, persist, refresh, or transport entitlement credentials.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

use crate::EntityId;

pub const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
pub const EVALUATION_TERM: DurationSeconds = DurationSeconds::new(14 * SECONDS_PER_DAY);
pub const ANNUAL_TERM_365_DAYS: DurationSeconds = DurationSeconds::new(365 * SECONDS_PER_DAY);
pub const ANNUAL_TERM_366_DAYS: DurationSeconds = DurationSeconds::new(366 * SECONDS_PER_DAY);
pub const PROFESSIONAL_BRANCH_CAPACITY: u32 = 5;
/// ADR 0009 software default ceiling until a signed Enterprise contract
/// overrides capacity at issuance time.
pub const MAX_ENTERPRISE_BRANCH_CAPACITY: u32 = 50;
/// Offline grace after entitlements cannot be refreshed (ADR 0009).
pub const OFFLINE_GRACE_PERIOD: DurationSeconds = DurationSeconds::new(72 * SECONDS_PER_DAY);

/// A non-negative UTC instant represented as whole seconds since the Unix
/// epoch. Entitlement boundaries use the half-open interval `[start, end)`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnixSeconds(u64);

impl UnixSeconds {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, duration: DurationSeconds) -> Option<Self> {
        self.0.checked_add(duration.0).map(Self)
    }

    fn elapsed_since(self, earlier: Self) -> DurationSeconds {
        debug_assert!(self >= earlier);
        DurationSeconds(self.0 - earlier.0)
    }
}

/// A non-negative whole-second duration. Keeping durations distinct from
/// instants prevents accidental timestamp arithmetic at call sites.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DurationSeconds(u64);

impl DurationSeconds {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitlementEdition {
    Community,
    Evaluation,
    Professional,
    Enterprise,
}

/// Validated entitlement facts. Community is an untimed local baseline;
/// every other edition has an exact, finite validity interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitlementClaims {
    edition: EntitlementEdition,
    issued_at: Option<UnixSeconds>,
    not_before: Option<UnixSeconds>,
    expires_at: Option<UnixSeconds>,
    enterprise_branch_capacity: Option<NonZeroU32>,
}

impl EntitlementClaims {
    /// The Community edition has no expiry and requires no remote claim.
    pub const fn community() -> Self {
        Self {
            edition: EntitlementEdition::Community,
            issued_at: None,
            not_before: None,
            expires_at: None,
            enterprise_branch_capacity: None,
        }
    }

    /// A Professional evaluation is exactly fourteen 24-hour days.
    pub fn evaluation(
        issued_at: UnixSeconds,
        not_before: UnixSeconds,
        expires_at: UnixSeconds,
    ) -> Result<Self, EntitlementClaimError> {
        validate_timed_claim(issued_at, not_before, expires_at)?;
        if expires_at.elapsed_since(not_before) != EVALUATION_TERM {
            return Err(EntitlementClaimError::InvalidEvaluationTerm);
        }

        Ok(Self::timed(
            EntitlementEdition::Evaluation,
            issued_at,
            not_before,
            expires_at,
            None,
        ))
    }

    /// A Professional term is one calendar-year duration expressed as either
    /// 365 or 366 whole UTC days by the authoritative issuer.
    pub fn professional(
        issued_at: UnixSeconds,
        not_before: UnixSeconds,
        expires_at: UnixSeconds,
    ) -> Result<Self, EntitlementClaimError> {
        validate_annual_claim(issued_at, not_before, expires_at)?;
        Ok(Self::timed(
            EntitlementEdition::Professional,
            issued_at,
            not_before,
            expires_at,
            None,
        ))
    }

    /// An Enterprise term is annual and carries a positive writable-branch
    /// capacity. The claim deliberately contains capacity, not billing data.
    pub fn enterprise(
        issued_at: UnixSeconds,
        not_before: UnixSeconds,
        expires_at: UnixSeconds,
        branch_capacity: u32,
    ) -> Result<Self, EntitlementClaimError> {
        validate_annual_claim(issued_at, not_before, expires_at)?;
        let branch_capacity = NonZeroU32::new(branch_capacity)
            .filter(|capacity| capacity.get() <= MAX_ENTERPRISE_BRANCH_CAPACITY)
            .ok_or(EntitlementClaimError::InvalidEnterpriseBranchCapacity)?;

        Ok(Self::timed(
            EntitlementEdition::Enterprise,
            issued_at,
            not_before,
            expires_at,
            Some(branch_capacity),
        ))
    }

    fn timed(
        edition: EntitlementEdition,
        issued_at: UnixSeconds,
        not_before: UnixSeconds,
        expires_at: UnixSeconds,
        enterprise_branch_capacity: Option<NonZeroU32>,
    ) -> Self {
        Self {
            edition,
            issued_at: Some(issued_at),
            not_before: Some(not_before),
            expires_at: Some(expires_at),
            enterprise_branch_capacity,
        }
    }

    pub const fn edition(&self) -> EntitlementEdition {
        self.edition
    }

    pub const fn issued_at(&self) -> Option<UnixSeconds> {
        self.issued_at
    }

    pub const fn not_before(&self) -> Option<UnixSeconds> {
        self.not_before
    }

    pub const fn expires_at(&self) -> Option<UnixSeconds> {
        self.expires_at
    }

    pub const fn enterprise_branch_capacity(&self) -> Option<NonZeroU32> {
        self.enterprise_branch_capacity
    }

    fn timed_boundaries(&self) -> Option<(UnixSeconds, UnixSeconds)> {
        self.not_before.zip(self.expires_at)
    }
}

fn validate_timed_claim(
    issued_at: UnixSeconds,
    not_before: UnixSeconds,
    expires_at: UnixSeconds,
) -> Result<(), EntitlementClaimError> {
    if issued_at > not_before {
        return Err(EntitlementClaimError::IssuedAfterNotBefore);
    }
    if expires_at <= not_before {
        return Err(EntitlementClaimError::ExpiryNotAfterNotBefore);
    }
    Ok(())
}

fn validate_annual_claim(
    issued_at: UnixSeconds,
    not_before: UnixSeconds,
    expires_at: UnixSeconds,
) -> Result<(), EntitlementClaimError> {
    validate_timed_claim(issued_at, not_before, expires_at)?;
    let term = expires_at.elapsed_since(not_before);
    if !matches!(term, ANNUAL_TERM_365_DAYS | ANNUAL_TERM_366_DAYS) {
        return Err(EntitlementClaimError::InvalidAnnualTerm);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitlementClaimError {
    IssuedAfterNotBefore,
    ExpiryNotAfterNotBefore,
    InvalidEvaluationTerm,
    InvalidAnnualTerm,
    InvalidEnterpriseBranchCapacity,
}

impl fmt::Display for EntitlementClaimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IssuedAfterNotBefore => {
                formatter.write_str("The entitlement was issued after its not-before instant.")
            }
            Self::ExpiryNotAfterNotBefore => {
                formatter.write_str("The entitlement expiry must be after its not-before instant.")
            }
            Self::InvalidEvaluationTerm => {
                formatter.write_str("An evaluation term must be exactly fourteen days.")
            }
            Self::InvalidAnnualTerm => {
                formatter.write_str("An annual term must be exactly 365 or 366 whole UTC days.")
            }
            Self::InvalidEnterpriseBranchCapacity => {
                write!(
                    formatter,
                    "Enterprise branch capacity must be between 1 and {MAX_ENTERPRISE_BRANCH_CAPACITY}."
                )
            }
        }
    }
}

impl Error for EntitlementClaimError {}

/// Local policy inputs that remain independent of any billing provider. A
/// zero grace duration moves directly to Community safe mode at expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntitlementPolicy {
    grace_period: DurationSeconds,
    clock_rollback_tolerance: DurationSeconds,
}

impl EntitlementPolicy {
    pub const fn new(
        grace_period: DurationSeconds,
        clock_rollback_tolerance: DurationSeconds,
    ) -> Self {
        Self {
            grace_period,
            clock_rollback_tolerance,
        }
    }

    pub const fn grace_period(self) -> DurationSeconds {
        self.grace_period
    }

    pub const fn clock_rollback_tolerance(self) -> DurationSeconds {
        self.clock_rollback_tolerance
    }
}

/// The current wall-clock observation plus the greatest trusted instant
/// persisted by the caller. The evaluator never moves this high-water mark
/// backwards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntitlementClock {
    observed_at: UnixSeconds,
    trusted_high_water: Option<UnixSeconds>,
}

impl EntitlementClock {
    pub const fn new(observed_at: UnixSeconds, trusted_high_water: Option<UnixSeconds>) -> Self {
        Self {
            observed_at,
            trusted_high_water,
        }
    }

    pub const fn observed_at(self) -> UnixSeconds {
        self.observed_at
    }

    pub const fn trusted_high_water(self) -> Option<UnixSeconds> {
        self.trusted_high_water
    }
}

/// Known branches in authoritative, stable activation order. UI sort order
/// must never be supplied here because Enterprise writable slots depend on
/// this sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchTopology {
    primary_branch: EntityId,
    known_branches: Vec<EntityId>,
}

impl BranchTopology {
    pub fn new(
        primary_branch: EntityId,
        known_branches_in_activation_order: Vec<EntityId>,
    ) -> Result<Self, BranchTopologyError> {
        if known_branches_in_activation_order.is_empty() {
            return Err(BranchTopologyError::NoKnownBranches);
        }

        let mut unique = HashSet::with_capacity(known_branches_in_activation_order.len());
        if known_branches_in_activation_order
            .iter()
            .any(|branch| !unique.insert(branch.clone()))
        {
            return Err(BranchTopologyError::DuplicateKnownBranch);
        }
        if !unique.contains(&primary_branch) {
            return Err(BranchTopologyError::PrimaryBranchNotKnown);
        }

        Ok(Self {
            primary_branch,
            known_branches: known_branches_in_activation_order,
        })
    }

    pub fn primary_branch(&self) -> &EntityId {
        &self.primary_branch
    }

    pub fn known_branches_in_activation_order(&self) -> &[EntityId] {
        &self.known_branches
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchTopologyError {
    NoKnownBranches,
    DuplicateKnownBranch,
    PrimaryBranchNotKnown,
}

impl fmt::Display for BranchTopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoKnownBranches => formatter.write_str("At least one known branch is required."),
            Self::DuplicateKnownBranch => {
                formatter.write_str("Known branches must not contain duplicates.")
            }
            Self::PrimaryBranchNotKnown => {
                formatter.write_str("The primary branch must be a known branch.")
            }
        }
    }
}

impl Error for BranchTopologyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitlementState {
    Active,
    Grace,
    CommunitySafeMode,
    FailClosed(EntitlementFailClosedReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitlementFailClosedReason {
    ClockRollback,
    NotYetValid,
}

/// A known branch is never hidden merely because an entitlement changes. It
/// either remains writable or becomes readable/exportable. Unknown branch
/// identities are denied instead of being inferred from untrusted input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchAccess {
    ReadWrite,
    ReadOnlyExport,
    Denied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitlementDecision {
    edition: EntitlementEdition,
    state: EntitlementState,
    effective_at: UnixSeconds,
    next_high_water: UnixSeconds,
    observed_rollback: Option<DurationSeconds>,
    known_branches: Vec<EntityId>,
    writable_branches: Vec<EntityId>,
}

impl EntitlementDecision {
    pub const fn edition(&self) -> EntitlementEdition {
        self.edition
    }

    pub const fn state(&self) -> EntitlementState {
        self.state
    }

    /// The maximum of observed time and the trusted high-water mark. Term
    /// evaluation never rewinds, even when a small rollback is tolerated.
    pub const fn effective_at(&self) -> UnixSeconds {
        self.effective_at
    }

    /// Persist this value atomically with any entitlement-authorized write.
    pub const fn next_high_water(&self) -> UnixSeconds {
        self.next_high_water
    }

    pub const fn observed_rollback(&self) -> Option<DurationSeconds> {
        self.observed_rollback
    }

    pub fn branch_access(&self, branch_id: &EntityId) -> BranchAccess {
        if !self.known_branches.contains(branch_id) {
            BranchAccess::Denied
        } else if self.writable_branches.contains(branch_id) {
            BranchAccess::ReadWrite
        } else {
            BranchAccess::ReadOnlyExport
        }
    }
}

/// Evaluates a validated entitlement without I/O or hidden clock access.
/// Callers remain responsible for authenticating claims and loading/persisting
/// the high-water mark and branch topology from trusted storage.
pub fn evaluate_entitlement(
    claims: &EntitlementClaims,
    clock: EntitlementClock,
    policy: EntitlementPolicy,
    branches: &BranchTopology,
) -> EntitlementDecision {
    let observed_at = clock.observed_at;
    let high_water = clock.trusted_high_water.unwrap_or(observed_at);
    let observed_rollback =
        (observed_at < high_water).then(|| high_water.elapsed_since(observed_at));
    let effective_at = observed_at.max(high_water);
    let next_high_water = effective_at;

    let state =
        if observed_rollback.is_some_and(|rollback| rollback > policy.clock_rollback_tolerance) {
            EntitlementState::FailClosed(EntitlementFailClosedReason::ClockRollback)
        } else {
            evaluate_term_state(claims, effective_at, policy.grace_period)
        };

    let writable_capacity = match state {
        EntitlementState::FailClosed(_) => 0,
        EntitlementState::CommunitySafeMode => 1,
        EntitlementState::Active | EntitlementState::Grace => match claims.edition {
            EntitlementEdition::Enterprise => claims
                .enterprise_branch_capacity
                .expect("validated Enterprise claims carry branch capacity")
                .get(),
            EntitlementEdition::Evaluation | EntitlementEdition::Professional => {
                PROFESSIONAL_BRANCH_CAPACITY
            }
            EntitlementEdition::Community => 1,
        },
    };

    let writable_branches = select_writable_branches(branches, writable_capacity);
    EntitlementDecision {
        edition: claims.edition,
        state,
        effective_at,
        next_high_water,
        observed_rollback,
        known_branches: branches.known_branches.clone(),
        writable_branches,
    }
}

fn evaluate_term_state(
    claims: &EntitlementClaims,
    effective_at: UnixSeconds,
    grace_period: DurationSeconds,
) -> EntitlementState {
    let Some((not_before, expires_at)) = claims.timed_boundaries() else {
        return EntitlementState::Active;
    };

    if effective_at < not_before {
        EntitlementState::FailClosed(EntitlementFailClosedReason::NotYetValid)
    } else if effective_at < expires_at {
        EntitlementState::Active
    } else if effective_at.elapsed_since(expires_at) < grace_period {
        EntitlementState::Grace
    } else {
        EntitlementState::CommunitySafeMode
    }
}

fn select_writable_branches(branches: &BranchTopology, capacity: u32) -> Vec<EntityId> {
    if capacity == 0 {
        return Vec::new();
    }

    let capacity = capacity as usize;
    let mut writable = Vec::with_capacity(capacity.min(branches.known_branches.len()));
    writable.push(branches.primary_branch.clone());
    for branch in &branches.known_branches {
        if writable.len() == capacity {
            break;
        }
        if branch != &branches.primary_branch {
            writable.push(branch.clone());
        }
    }
    writable
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: UnixSeconds = UnixSeconds::new(2_000_000_000);
    const NO_GRACE: EntitlementPolicy =
        EntitlementPolicy::new(DurationSeconds::ZERO, DurationSeconds::ZERO);

    fn at_offset(start: UnixSeconds, seconds: u64) -> UnixSeconds {
        start
            .checked_add(DurationSeconds::new(seconds))
            .expect("test timestamp must fit")
    }

    fn annual_expiry(start: UnixSeconds) -> UnixSeconds {
        start
            .checked_add(ANNUAL_TERM_365_DAYS)
            .expect("test timestamp must fit")
    }

    fn fixed_id(value: u64) -> EntityId {
        EntityId::parse(&format!("01890f3e-0000-7000-8000-{value:012x}"))
            .expect("fixed test UUIDv7")
    }

    fn branches(count: usize, primary_index: usize) -> (BranchTopology, Vec<EntityId>) {
        let known = (0..count)
            .map(|index| fixed_id(index as u64 + 1))
            .collect::<Vec<_>>();
        let topology = BranchTopology::new(known[primary_index].clone(), known.clone())
            .expect("valid topology");
        (topology, known)
    }

    fn decision_at(
        claims: &EntitlementClaims,
        at: UnixSeconds,
        policy: EntitlementPolicy,
        branches: &BranchTopology,
    ) -> EntitlementDecision {
        evaluate_entitlement(claims, EntitlementClock::new(at, None), policy, branches)
    }

    #[test]
    fn timed_claims_reject_invalid_ordering() {
        let expiry = at_offset(START, EVALUATION_TERM.get());
        assert_eq!(
            EntitlementClaims::evaluation(at_offset(START, 1), START, expiry),
            Err(EntitlementClaimError::IssuedAfterNotBefore)
        );
        assert_eq!(
            EntitlementClaims::evaluation(START, START, START),
            Err(EntitlementClaimError::ExpiryNotAfterNotBefore)
        );
        assert_eq!(
            EntitlementClaims::professional(START, START, at_offset(START, 1)),
            Err(EntitlementClaimError::InvalidAnnualTerm)
        );
    }

    #[test]
    fn evaluation_is_exactly_fourteen_days() {
        assert!(
            EntitlementClaims::evaluation(START, START, at_offset(START, EVALUATION_TERM.get()))
                .is_ok()
        );
        assert_eq!(
            EntitlementClaims::evaluation(
                START,
                START,
                at_offset(START, EVALUATION_TERM.get() - 1)
            ),
            Err(EntitlementClaimError::InvalidEvaluationTerm)
        );
        assert_eq!(
            EntitlementClaims::evaluation(
                START,
                START,
                at_offset(START, EVALUATION_TERM.get() + 1)
            ),
            Err(EntitlementClaimError::InvalidEvaluationTerm)
        );
    }

    #[test]
    fn annual_terms_accept_only_whole_365_or_366_day_intervals() {
        for term in [ANNUAL_TERM_365_DAYS, ANNUAL_TERM_366_DAYS] {
            assert!(
                EntitlementClaims::professional(START, START, at_offset(START, term.get())).is_ok()
            );
        }

        assert_eq!(
            EntitlementClaims::professional(
                START,
                START,
                at_offset(START, ANNUAL_TERM_365_DAYS.get() + 1)
            ),
            Err(EntitlementClaimError::InvalidAnnualTerm)
        );
    }

    #[test]
    fn enterprise_capacity_is_positive_and_bounded() {
        for invalid in [0, MAX_ENTERPRISE_BRANCH_CAPACITY + 1, u32::MAX] {
            assert_eq!(
                EntitlementClaims::enterprise(START, START, annual_expiry(START), invalid),
                Err(EntitlementClaimError::InvalidEnterpriseBranchCapacity)
            );
        }
        let valid = EntitlementClaims::enterprise(START, START, annual_expiry(START), 3)
            .expect("positive Enterprise capacity");
        assert_eq!(
            valid.enterprise_branch_capacity().map(NonZeroU32::get),
            Some(3)
        );
    }

    #[test]
    fn community_never_expires_and_keeps_only_primary_writable() {
        let (topology, known) = branches(2, 1);
        let decision = decision_at(
            &EntitlementClaims::community(),
            UnixSeconds::new(u64::MAX),
            NO_GRACE,
            &topology,
        );

        assert_eq!(decision.state(), EntitlementState::Active);
        assert_eq!(decision.branch_access(&known[1]), BranchAccess::ReadWrite);
        assert_eq!(
            decision.branch_access(&known[0]),
            BranchAccess::ReadOnlyExport
        );
    }

    #[test]
    fn evaluation_boundaries_are_half_open_and_grace_is_explicit() {
        let expiry = at_offset(START, EVALUATION_TERM.get());
        let claim = EntitlementClaims::evaluation(START, START, expiry).expect("valid trial");
        let (topology, _) = branches(1, 0);
        let policy = EntitlementPolicy::new(DurationSeconds::new(60), DurationSeconds::new(5));

        assert_eq!(
            decision_at(&claim, UnixSeconds::new(START.get() - 1), policy, &topology).state(),
            EntitlementState::FailClosed(EntitlementFailClosedReason::NotYetValid)
        );
        assert_eq!(
            decision_at(&claim, START, policy, &topology).state(),
            EntitlementState::Active
        );
        assert_eq!(
            decision_at(
                &claim,
                UnixSeconds::new(expiry.get() - 1),
                policy,
                &topology
            )
            .state(),
            EntitlementState::Active
        );
        assert_eq!(
            decision_at(&claim, expiry, policy, &topology).state(),
            EntitlementState::Grace
        );
        assert_eq!(
            decision_at(&claim, at_offset(expiry, 59), policy, &topology).state(),
            EntitlementState::Grace
        );
        assert_eq!(
            decision_at(&claim, at_offset(expiry, 60), policy, &topology).state(),
            EntitlementState::CommunitySafeMode
        );
    }

    #[test]
    fn zero_grace_enters_safe_mode_at_the_expiry_instant() {
        let expiry = annual_expiry(START);
        let claim = EntitlementClaims::professional(START, START, expiry).expect("annual claim");
        let (topology, _) = branches(1, 0);

        assert_eq!(
            decision_at(&claim, expiry, NO_GRACE, &topology).state(),
            EntitlementState::CommunitySafeMode
        );
    }

    #[test]
    fn rollback_above_tolerance_fails_closed_without_hiding_known_data() {
        let claim = EntitlementClaims::community();
        let (topology, known) = branches(2, 0);
        let decision = evaluate_entitlement(
            &claim,
            EntitlementClock::new(UnixSeconds::new(900), Some(UnixSeconds::new(1_000))),
            EntitlementPolicy::new(DurationSeconds::ZERO, DurationSeconds::new(99)),
            &topology,
        );

        assert_eq!(
            decision.state(),
            EntitlementState::FailClosed(EntitlementFailClosedReason::ClockRollback)
        );
        assert_eq!(decision.effective_at(), UnixSeconds::new(1_000));
        assert_eq!(decision.next_high_water(), UnixSeconds::new(1_000));
        assert_eq!(
            decision.observed_rollback(),
            Some(DurationSeconds::new(100))
        );
        for branch in known {
            assert_eq!(
                decision.branch_access(&branch),
                BranchAccess::ReadOnlyExport
            );
        }
    }

    #[test]
    fn rollback_at_tolerance_uses_high_water_without_extending_the_term() {
        let expiry = at_offset(START, EVALUATION_TERM.get());
        let claim = EntitlementClaims::evaluation(START, START, expiry).expect("valid trial");
        let (topology, _) = branches(1, 0);
        let observed = UnixSeconds::new(expiry.get() - 100);
        let decision = evaluate_entitlement(
            &claim,
            EntitlementClock::new(observed, Some(expiry)),
            EntitlementPolicy::new(DurationSeconds::ZERO, DurationSeconds::new(100)),
            &topology,
        );

        assert_eq!(decision.state(), EntitlementState::CommunitySafeMode);
        assert_eq!(decision.effective_at(), expiry);
        assert_eq!(
            decision.observed_rollback(),
            Some(DurationSeconds::new(100))
        );
    }

    #[test]
    fn high_water_advances_only_forward() {
        let claim = EntitlementClaims::community();
        let (topology, _) = branches(1, 0);
        let forward = evaluate_entitlement(
            &claim,
            EntitlementClock::new(UnixSeconds::new(1_100), Some(UnixSeconds::new(1_000))),
            NO_GRACE,
            &topology,
        );
        assert_eq!(forward.next_high_water(), UnixSeconds::new(1_100));
        assert_eq!(forward.observed_rollback(), None);

        let equal = evaluate_entitlement(
            &claim,
            EntitlementClock::new(UnixSeconds::new(1_100), Some(UnixSeconds::new(1_100))),
            NO_GRACE,
            &topology,
        );
        assert_eq!(equal.next_high_water(), UnixSeconds::new(1_100));
        assert_eq!(equal.observed_rollback(), None);
    }

    #[test]
    fn enterprise_capacity_uses_primary_then_stable_activation_order() {
        let claim = EntitlementClaims::enterprise(START, START, annual_expiry(START), 2)
            .expect("valid Enterprise claim");
        let (topology, known) = branches(4, 2);
        let decision = decision_at(&claim, START, NO_GRACE, &topology);

        assert_eq!(decision.branch_access(&known[2]), BranchAccess::ReadWrite);
        assert_eq!(decision.branch_access(&known[0]), BranchAccess::ReadWrite);
        assert_eq!(
            decision.branch_access(&known[1]),
            BranchAccess::ReadOnlyExport
        );
        assert_eq!(
            decision.branch_access(&known[3]),
            BranchAccess::ReadOnlyExport
        );
        assert_eq!(decision.branch_access(&fixed_id(999)), BranchAccess::Denied);
    }

    #[test]
    fn expired_paid_term_falls_back_to_primary_branch_safe_mode() {
        let expiry = annual_expiry(START);
        let claim =
            EntitlementClaims::enterprise(START, START, expiry, 3).expect("valid Enterprise claim");
        let (topology, known) = branches(3, 1);
        let decision = decision_at(&claim, expiry, NO_GRACE, &topology);

        assert_eq!(decision.state(), EntitlementState::CommunitySafeMode);
        assert_eq!(decision.branch_access(&known[1]), BranchAccess::ReadWrite);
        assert_eq!(
            decision.branch_access(&known[0]),
            BranchAccess::ReadOnlyExport
        );
        assert_eq!(
            decision.branch_access(&known[2]),
            BranchAccess::ReadOnlyExport
        );
    }

    #[test]
    fn community_is_single_branch_while_evaluation_and_professional_allow_five() {
        let expiry = annual_expiry(START);
        let community = EntitlementClaims::community();
        let professional_claims = [
            EntitlementClaims::evaluation(START, START, at_offset(START, EVALUATION_TERM.get()))
                .expect("valid evaluation"),
            EntitlementClaims::professional(START, START, expiry).expect("valid Professional"),
        ];
        let (topology, known) = branches(6, 2);

        let community_decision = decision_at(&community, START, NO_GRACE, &topology);
        assert_eq!(
            community_decision.branch_access(&known[2]),
            BranchAccess::ReadWrite
        );
        for branch in known.iter().filter(|branch| *branch != &known[2]) {
            assert_eq!(
                community_decision.branch_access(branch),
                BranchAccess::ReadOnlyExport
            );
        }

        for claim in professional_claims {
            let decision = decision_at(&claim, START, NO_GRACE, &topology);
            for branch in &known[..5] {
                assert_eq!(decision.branch_access(branch), BranchAccess::ReadWrite);
            }
            assert_eq!(
                decision.branch_access(&known[5]),
                BranchAccess::ReadOnlyExport
            );
        }
    }

    #[test]
    fn topology_rejects_empty_duplicate_and_missing_primary_inputs() {
        let primary = fixed_id(1);
        assert_eq!(
            BranchTopology::new(primary.clone(), Vec::new()),
            Err(BranchTopologyError::NoKnownBranches)
        );
        assert_eq!(
            BranchTopology::new(primary.clone(), vec![primary.clone(), primary.clone()]),
            Err(BranchTopologyError::DuplicateKnownBranch)
        );
        assert_eq!(
            BranchTopology::new(primary, vec![fixed_id(2)]),
            Err(BranchTopologyError::PrimaryBranchNotKnown)
        );
    }
}
