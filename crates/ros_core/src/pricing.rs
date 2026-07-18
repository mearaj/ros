//! Provider- and jurisdiction-neutral tax and order-discount calculations.
//!
//! This module deliberately works only in integer currency minor units. It
//! defines arithmetic and snapshot invariants, not a jurisdiction's tax law.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use unicode_normalization::UnicodeNormalization;

use crate::{ActorRole, CurrencyCode, Money};

/// One basis point is one hundredth of one percent.
pub const BASIS_POINTS_DENOMINATOR: u32 = 10_000;
/// A defensive v1 ceiling. A later jurisdiction policy may impose a lower one.
pub const MAX_COMBINED_TAX_BASIS_POINTS: u32 = BASIS_POINTS_DENOMINATOR;
pub const MAX_DISCOUNT_BASIS_POINTS: u32 = BASIS_POINTS_DENOMINATOR;
pub const MAX_PRICING_LINES: usize = 200;
pub const MAX_TAX_RATES_PER_LINE: usize = 8;
pub const MAX_LINE_QUANTITY: i64 = 1_000_000;

const MAX_LINE_REFERENCE_CHARACTERS: usize = 128;
const MAX_LINE_NAME_CHARACTERS: usize = 160;
const MAX_TAX_NAME_CHARACTERS: usize = 120;
const MAX_DISCOUNT_REASON_CHARACTERS: usize = 500;

/// The only rounding rule used by the v1 pricing calculation.
///
/// Exact nonnegative rational values are rounded to the nearest minor unit;
/// an exact half is rounded up. This common commercial rule is simple to
/// reproduce on every platform and avoids silently favouring a lower charge
/// at midpoint ties. Negative monetary inputs are rejected before calculation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PricingRoundingRule {
    NearestMinorUnitHalfUp,
}

/// Whether named tax components are absent, added to the supplied price, or
/// already included in it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaxTreatmentKind {
    NoTax,
    Exclusive,
    Inclusive,
}

/// A normalized, bounded component of an aggregate tax profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxRate {
    name: String,
    name_key: String,
    basis_points: u32,
}

impl TaxRate {
    pub fn new(name: &str, basis_points: u32) -> Result<Self, PricingError> {
        if basis_points > MAX_COMBINED_TAX_BASIS_POINTS {
            return Err(PricingError::InvalidTaxRate);
        }

        let (name, name_key) =
            normalize_text(name, MAX_TAX_NAME_CHARACTERS).ok_or(PricingError::InvalidTaxName)?;
        Ok(Self {
            name,
            name_key,
            basis_points,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn name_key(&self) -> &str {
        &self.name_key
    }

    pub const fn basis_points(&self) -> u32 {
        self.basis_points
    }
}

/// A validated line-level tax profile. Taxable profiles contain one to eight
/// uniquely named components whose combined rate is at most 100% in v1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxTreatment {
    kind: TaxTreatmentKind,
    rates: Vec<TaxRate>,
}

impl TaxTreatment {
    pub const fn no_tax() -> Self {
        Self {
            kind: TaxTreatmentKind::NoTax,
            rates: Vec::new(),
        }
    }

    pub fn exclusive(rates: Vec<TaxRate>) -> Result<Self, PricingError> {
        Self::taxable(TaxTreatmentKind::Exclusive, rates)
    }

    pub fn inclusive(rates: Vec<TaxRate>) -> Result<Self, PricingError> {
        Self::taxable(TaxTreatmentKind::Inclusive, rates)
    }

    fn taxable(kind: TaxTreatmentKind, mut rates: Vec<TaxRate>) -> Result<Self, PricingError> {
        if rates.is_empty() {
            return Err(PricingError::EmptyTaxProfile);
        }
        if rates.len() > MAX_TAX_RATES_PER_LINE {
            return Err(PricingError::TooManyTaxRates);
        }

        // Canonical ordering makes component apportionment independent of UI
        // input order and gives later invoice snapshots a stable representation.
        rates.sort_by(|left, right| left.name_key.cmp(&right.name_key));
        if rates
            .windows(2)
            .any(|pair| pair[0].name_key == pair[1].name_key)
        {
            return Err(PricingError::DuplicateTaxName);
        }

        let combined = rates
            .iter()
            .try_fold(0_u32, |total, rate| total.checked_add(rate.basis_points));
        if combined.is_none_or(|total| total > MAX_COMBINED_TAX_BASIS_POINTS) {
            return Err(PricingError::CombinedTaxRateTooHigh);
        }

        Ok(Self { kind, rates })
    }

    pub const fn kind(&self) -> TaxTreatmentKind {
        self.kind
    }

    pub fn rates(&self) -> &[TaxRate] {
        &self.rates
    }

    fn combined_basis_points(&self) -> u32 {
        self.rates.iter().map(TaxRate::basis_points).sum()
    }
}

/// A validated input line. The supplied unit price is gross for inclusive
/// treatment and net for no-tax or exclusive treatment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingLineInput {
    line_reference: String,
    line_reference_key: String,
    display_name: String,
    unit_price: Money,
    quantity: i64,
    tax_treatment: TaxTreatment,
}

impl PricingLineInput {
    pub fn new(
        line_reference: &str,
        display_name: &str,
        unit_price: Money,
        quantity: i64,
        tax_treatment: TaxTreatment,
    ) -> Result<Self, PricingError> {
        let (line_reference, line_reference_key) =
            normalize_text(line_reference, MAX_LINE_REFERENCE_CHARACTERS)
                .ok_or(PricingError::InvalidLineReference)?;
        let (display_name, _) = normalize_text(display_name, MAX_LINE_NAME_CHARACTERS)
            .ok_or(PricingError::InvalidLineName)?;
        if unit_price.minor_units() < 0 {
            return Err(PricingError::NegativeUnitPrice);
        }
        if !(1..=MAX_LINE_QUANTITY).contains(&quantity) {
            return Err(PricingError::InvalidQuantity);
        }

        Ok(Self {
            line_reference,
            line_reference_key,
            display_name,
            unit_price,
            quantity,
            tax_treatment,
        })
    }

    pub fn line_reference(&self) -> &str {
        &self.line_reference
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn unit_price(&self) -> &Money {
        &self.unit_price
    }

    pub const fn quantity(&self) -> i64 {
        self.quantity
    }

    pub fn tax_treatment(&self) -> &TaxTreatment {
        &self.tax_treatment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscountKind {
    Fixed,
    Percentage,
}

/// Authorization intent carried with a discount. This is metadata for the
/// later storage/session boundary, not proof that an actor was authorized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscountAuthorizationIntent {
    ManagerOrOwnerApproval,
}

impl DiscountAuthorizationIntent {
    /// Roles intended to approve a discount once transactional authorization
    /// is integrated. A UI decision alone must never be treated as approval.
    pub const fn permits_role(self, role: ActorRole) -> bool {
        matches!(role, ActorRole::Owner | ActorRole::Manager)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiscountMethod {
    Fixed {
        amount_minor: i64,
    },
    Percentage {
        basis_points: u32,
        maximum_minor: Option<i64>,
    },
}

/// A validated order-level discount request. Its required reason is preserved
/// in normalized display and comparison forms for a later audit snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderDiscount {
    method: DiscountMethod,
    reason: String,
    reason_key: String,
}

impl OrderDiscount {
    pub fn fixed(amount_minor: i64, reason: &str) -> Result<Self, PricingError> {
        if amount_minor < 0 {
            return Err(PricingError::NegativeDiscount);
        }
        Self::new(DiscountMethod::Fixed { amount_minor }, reason)
    }

    pub fn percentage(
        basis_points: u32,
        maximum_minor: Option<i64>,
        reason: &str,
    ) -> Result<Self, PricingError> {
        if basis_points > MAX_DISCOUNT_BASIS_POINTS {
            return Err(PricingError::InvalidDiscountRate);
        }
        if maximum_minor.is_some_and(|maximum| maximum < 0) {
            return Err(PricingError::InvalidDiscountCap);
        }
        Self::new(
            DiscountMethod::Percentage {
                basis_points,
                maximum_minor,
            },
            reason,
        )
    }

    fn new(method: DiscountMethod, reason: &str) -> Result<Self, PricingError> {
        let (reason, reason_key) = normalize_text(reason, MAX_DISCOUNT_REASON_CHARACTERS)
            .ok_or(PricingError::InvalidDiscountReason)?;
        Ok(Self {
            method,
            reason,
            reason_key,
        })
    }

    pub const fn kind(&self) -> DiscountKind {
        match self.method {
            DiscountMethod::Fixed { .. } => DiscountKind::Fixed,
            DiscountMethod::Percentage { .. } => DiscountKind::Percentage,
        }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn reason_key(&self) -> &str {
        &self.reason_key
    }

    pub const fn authorization_intent(&self) -> DiscountAuthorizationIntent {
        DiscountAuthorizationIntent::ManagerOrOwnerApproval
    }

    pub const fn fixed_amount_minor(&self) -> Option<i64> {
        match self.method {
            DiscountMethod::Fixed { amount_minor } => Some(amount_minor),
            DiscountMethod::Percentage { .. } => None,
        }
    }

    pub const fn percentage_basis_points(&self) -> Option<u32> {
        match self.method {
            DiscountMethod::Percentage { basis_points, .. } => Some(basis_points),
            DiscountMethod::Fixed { .. } => None,
        }
    }

    pub const fn maximum_minor(&self) -> Option<i64> {
        match self.method {
            DiscountMethod::Percentage { maximum_minor, .. } => maximum_minor,
            DiscountMethod::Fixed { .. } => None,
        }
    }
}

/// Immutable facts for one named tax component on one calculated line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxComponentBreakdown {
    name: String,
    name_key: String,
    basis_points: u32,
    tax_before_discount_minor: i64,
    tax_minor: i64,
}

impl TaxComponentBreakdown {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn name_key(&self) -> &str {
        &self.name_key
    }

    pub const fn basis_points(&self) -> u32 {
        self.basis_points
    }

    pub const fn tax_before_discount_minor(&self) -> i64 {
        self.tax_before_discount_minor
    }

    pub const fn tax_minor(&self) -> i64 {
        self.tax_minor
    }
}

/// Immutable calculated facts for one input line, suitable for copying into a
/// later append-only invoice snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingLineBreakdown {
    line_reference: String,
    display_name: String,
    quantity: i64,
    unit_price_minor: i64,
    tax_treatment: TaxTreatmentKind,
    input_price_total_minor: i64,
    net_before_discount_minor: i64,
    allocated_discount_minor: i64,
    net_minor: i64,
    tax_components: Vec<TaxComponentBreakdown>,
    tax_before_discount_minor: i64,
    tax_minor: i64,
    payable_before_discount_minor: i64,
    payable_minor: i64,
}

impl PricingLineBreakdown {
    pub fn line_reference(&self) -> &str {
        &self.line_reference
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub const fn quantity(&self) -> i64 {
        self.quantity
    }

    pub const fn unit_price_minor(&self) -> i64 {
        self.unit_price_minor
    }

    pub const fn tax_treatment(&self) -> TaxTreatmentKind {
        self.tax_treatment
    }

    pub const fn input_price_total_minor(&self) -> i64 {
        self.input_price_total_minor
    }

    pub const fn net_before_discount_minor(&self) -> i64 {
        self.net_before_discount_minor
    }

    pub const fn allocated_discount_minor(&self) -> i64 {
        self.allocated_discount_minor
    }

    pub const fn net_minor(&self) -> i64 {
        self.net_minor
    }

    pub fn tax_components(&self) -> &[TaxComponentBreakdown] {
        &self.tax_components
    }

    pub const fn tax_before_discount_minor(&self) -> i64 {
        self.tax_before_discount_minor
    }

    pub const fn tax_minor(&self) -> i64 {
        self.tax_minor
    }

    pub const fn payable_before_discount_minor(&self) -> i64 {
        self.payable_before_discount_minor
    }

    pub const fn payable_minor(&self) -> i64 {
        self.payable_minor
    }
}

/// Immutable order-discount facts retained by a successful calculation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscountBreakdown {
    kind: DiscountKind,
    reason: String,
    reason_key: String,
    fixed_amount_minor: Option<i64>,
    percentage_basis_points: Option<u32>,
    maximum_minor: Option<i64>,
    calculated_minor: i64,
    applied_minor: i64,
    authorization_intent: DiscountAuthorizationIntent,
}

impl DiscountBreakdown {
    pub const fn kind(&self) -> DiscountKind {
        self.kind
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn reason_key(&self) -> &str {
        &self.reason_key
    }

    pub const fn fixed_amount_minor(&self) -> Option<i64> {
        self.fixed_amount_minor
    }

    pub const fn percentage_basis_points(&self) -> Option<u32> {
        self.percentage_basis_points
    }

    pub const fn maximum_minor(&self) -> Option<i64> {
        self.maximum_minor
    }

    /// The method result after an explicit percentage cap but before the
    /// nonnegative-order cap.
    pub const fn calculated_minor(&self) -> i64 {
        self.calculated_minor
    }

    /// The amount actually allocated, capped at the order's net subtotal.
    pub const fn applied_minor(&self) -> i64 {
        self.applied_minor
    }

    pub const fn authorization_intent(&self) -> DiscountAuthorizationIntent {
        self.authorization_intent
    }
}

/// Complete immutable calculation output. Every total is in `currency()`
/// minor units, and all fields can be recomputed from `lines()`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingBreakdown {
    currency: CurrencyCode,
    rounding_rule: PricingRoundingRule,
    lines: Vec<PricingLineBreakdown>,
    discount: Option<DiscountBreakdown>,
    input_price_subtotal_minor: i64,
    net_before_discount_minor: i64,
    discount_minor: i64,
    net_minor: i64,
    tax_before_discount_minor: i64,
    tax_minor: i64,
    payable_before_discount_minor: i64,
    payable_minor: i64,
}

impl PricingBreakdown {
    pub fn currency(&self) -> &str {
        self.currency.as_str()
    }

    pub const fn rounding_rule(&self) -> PricingRoundingRule {
        self.rounding_rule
    }

    pub fn lines(&self) -> &[PricingLineBreakdown] {
        &self.lines
    }

    pub fn discount(&self) -> Option<&DiscountBreakdown> {
        self.discount.as_ref()
    }

    /// Sum of supplied extended prices. Inclusive lines include tax here;
    /// exclusive lines do not, so this is an input fact rather than a payable.
    pub const fn input_price_subtotal_minor(&self) -> i64 {
        self.input_price_subtotal_minor
    }

    pub const fn net_before_discount_minor(&self) -> i64 {
        self.net_before_discount_minor
    }

    pub const fn discount_minor(&self) -> i64 {
        self.discount_minor
    }

    pub const fn net_minor(&self) -> i64 {
        self.net_minor
    }

    pub const fn tax_before_discount_minor(&self) -> i64 {
        self.tax_before_discount_minor
    }

    pub const fn tax_minor(&self) -> i64 {
        self.tax_minor
    }

    pub const fn payable_before_discount_minor(&self) -> i64 {
        self.payable_before_discount_minor
    }

    pub const fn payable_minor(&self) -> i64 {
        self.payable_minor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedLine {
    input: PricingLineInput,
    input_total: i64,
    net_before_discount: i64,
    component_tax_before_discount: Vec<i64>,
}

/// Calculates a bounded order using checked arithmetic only.
pub fn calculate_pricing(
    lines: Vec<PricingLineInput>,
    discount: Option<OrderDiscount>,
) -> Result<PricingBreakdown, PricingError> {
    if lines.is_empty() {
        return Err(PricingError::EmptyPricingLines);
    }
    if lines.len() > MAX_PRICING_LINES {
        return Err(PricingError::TooManyPricingLines);
    }

    let currency = CurrencyCode::parse(lines[0].unit_price.currency())
        .map_err(|_| PricingError::InvalidCurrency)?;
    let mut references = HashSet::with_capacity(lines.len());
    let mut prepared = Vec::with_capacity(lines.len());

    for input in lines {
        if input.unit_price.currency() != currency.as_str() {
            return Err(PricingError::CurrencyMismatch {
                expected: currency.as_str().to_owned(),
                actual: input.unit_price.currency().to_owned(),
            });
        }
        if !references.insert(input.line_reference_key.clone()) {
            return Err(PricingError::DuplicateLineReference);
        }

        let input_total =
            checked_i64(i128::from(input.unit_price.minor_units()) * i128::from(input.quantity))?;
        let (net_before_discount, component_tax_before_discount) =
            calculate_before_discount(&input.tax_treatment, input_total)?;
        prepared.push(PreparedLine {
            input,
            input_total,
            net_before_discount,
            component_tax_before_discount,
        });
    }

    let input_price_subtotal = checked_sum(prepared.iter().map(|line| line.input_total))?;
    let net_before_discount = checked_sum(prepared.iter().map(|line| line.net_before_discount))?;
    let (discount_breakdown, discount_minor) = calculate_discount(discount, net_before_discount)?;
    let weights = prepared
        .iter()
        .map(|line| line.net_before_discount)
        .collect::<Vec<_>>();
    let tie_keys = prepared
        .iter()
        .map(|line| line.input.line_reference_key.as_str())
        .collect::<Vec<_>>();
    let allocated_discounts = allocate_proportionally(discount_minor, &weights, &tie_keys)?;

    let mut calculated_lines = Vec::with_capacity(prepared.len());
    for (line, allocated_discount) in prepared.into_iter().zip(allocated_discounts) {
        let net_minor = line
            .net_before_discount
            .checked_sub(allocated_discount)
            .ok_or(PricingError::AmountOverflow)?;
        let component_tax = calculate_after_discount(&line, net_minor)?;
        let tax_before_discount = checked_sum(line.component_tax_before_discount.iter().copied())?;
        let tax_minor = checked_sum(component_tax.iter().copied())?;
        let payable_before_discount = line
            .net_before_discount
            .checked_add(tax_before_discount)
            .ok_or(PricingError::AmountOverflow)?;
        let payable_minor = net_minor
            .checked_add(tax_minor)
            .ok_or(PricingError::AmountOverflow)?;

        let tax_components = line
            .input
            .tax_treatment
            .rates
            .iter()
            .zip(line.component_tax_before_discount.iter())
            .zip(component_tax.iter())
            .map(|((rate, before), after)| TaxComponentBreakdown {
                name: rate.name.clone(),
                name_key: rate.name_key.clone(),
                basis_points: rate.basis_points,
                tax_before_discount_minor: *before,
                tax_minor: *after,
            })
            .collect();

        calculated_lines.push(PricingLineBreakdown {
            line_reference: line.input.line_reference,
            display_name: line.input.display_name,
            quantity: line.input.quantity,
            unit_price_minor: line.input.unit_price.minor_units(),
            tax_treatment: line.input.tax_treatment.kind,
            input_price_total_minor: line.input_total,
            net_before_discount_minor: line.net_before_discount,
            allocated_discount_minor: allocated_discount,
            net_minor,
            tax_components,
            tax_before_discount_minor: tax_before_discount,
            tax_minor,
            payable_before_discount_minor: payable_before_discount,
            payable_minor,
        });
    }

    let net_minor = checked_sum(calculated_lines.iter().map(PricingLineBreakdown::net_minor))?;
    let tax_before_discount = checked_sum(
        calculated_lines
            .iter()
            .map(PricingLineBreakdown::tax_before_discount_minor),
    )?;
    let tax_minor = checked_sum(calculated_lines.iter().map(PricingLineBreakdown::tax_minor))?;
    let payable_before_discount = checked_sum(
        calculated_lines
            .iter()
            .map(PricingLineBreakdown::payable_before_discount_minor),
    )?;
    let payable_minor = checked_sum(
        calculated_lines
            .iter()
            .map(PricingLineBreakdown::payable_minor),
    )?;

    debug_assert_eq!(net_before_discount - discount_minor, net_minor);
    debug_assert!(payable_minor >= 0);

    Ok(PricingBreakdown {
        currency,
        rounding_rule: PricingRoundingRule::NearestMinorUnitHalfUp,
        lines: calculated_lines,
        discount: discount_breakdown,
        input_price_subtotal_minor: input_price_subtotal,
        net_before_discount_minor: net_before_discount,
        discount_minor,
        net_minor,
        tax_before_discount_minor: tax_before_discount,
        tax_minor,
        payable_before_discount_minor: payable_before_discount,
        payable_minor,
    })
}

fn calculate_before_discount(
    treatment: &TaxTreatment,
    input_total: i64,
) -> Result<(i64, Vec<i64>), PricingError> {
    match treatment.kind {
        TaxTreatmentKind::NoTax => Ok((input_total, Vec::new())),
        TaxTreatmentKind::Exclusive => {
            let taxes = treatment
                .rates
                .iter()
                .map(|rate| {
                    round_half_up(
                        i128::from(input_total) * i128::from(rate.basis_points),
                        i128::from(BASIS_POINTS_DENOMINATOR),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            // Detect a total that cannot later become a payable i64 now.
            input_total
                .checked_add(checked_sum(taxes.iter().copied())?)
                .ok_or(PricingError::AmountOverflow)?;
            Ok((input_total, taxes))
        }
        TaxTreatmentKind::Inclusive => {
            let combined_rate = treatment.combined_basis_points();
            if combined_rate == 0 {
                return Ok((input_total, vec![0; treatment.rates.len()]));
            }
            let total_tax = round_half_up(
                i128::from(input_total) * i128::from(combined_rate),
                i128::from(BASIS_POINTS_DENOMINATOR + combined_rate),
            )?;
            let net = input_total
                .checked_sub(total_tax)
                .ok_or(PricingError::AmountOverflow)?;
            let weights = treatment
                .rates
                .iter()
                .map(|rate| i64::from(rate.basis_points))
                .collect::<Vec<_>>();
            let tie_keys = treatment
                .rates
                .iter()
                .map(|rate| rate.name_key.as_str())
                .collect::<Vec<_>>();
            let taxes = allocate_proportionally(total_tax, &weights, &tie_keys)?;
            Ok((net, taxes))
        }
    }
}

fn calculate_after_discount(line: &PreparedLine, net_minor: i64) -> Result<Vec<i64>, PricingError> {
    match line.input.tax_treatment.kind {
        TaxTreatmentKind::NoTax => Ok(Vec::new()),
        TaxTreatmentKind::Exclusive => line
            .input
            .tax_treatment
            .rates
            .iter()
            .map(|rate| {
                round_half_up(
                    i128::from(net_minor) * i128::from(rate.basis_points),
                    i128::from(BASIS_POINTS_DENOMINATOR),
                )
            })
            .collect(),
        TaxTreatmentKind::Inclusive => {
            if line.net_before_discount == 0 {
                // At sub-minor-unit extremes, a positive inclusive amount can
                // decompose entirely into rounded tax. There is then no net
                // base against which to allocate an order discount.
                return Ok(line.component_tax_before_discount.clone());
            }
            line.component_tax_before_discount
                .iter()
                .map(|tax| {
                    round_half_up(
                        i128::from(*tax) * i128::from(net_minor),
                        i128::from(line.net_before_discount),
                    )
                })
                .collect()
        }
    }
}

fn calculate_discount(
    discount: Option<OrderDiscount>,
    net_subtotal: i64,
) -> Result<(Option<DiscountBreakdown>, i64), PricingError> {
    let Some(discount) = discount else {
        return Ok((None, 0));
    };

    let calculated_minor = match discount.method {
        DiscountMethod::Fixed { amount_minor } => amount_minor,
        DiscountMethod::Percentage {
            basis_points,
            maximum_minor,
        } => {
            let percentage = round_half_up(
                i128::from(net_subtotal) * i128::from(basis_points),
                i128::from(BASIS_POINTS_DENOMINATOR),
            )?;
            maximum_minor.map_or(percentage, |maximum| percentage.min(maximum))
        }
    };
    let applied_minor = calculated_minor.min(net_subtotal);
    let kind = discount.kind();
    let fixed_amount_minor = discount.fixed_amount_minor();
    let percentage_basis_points = discount.percentage_basis_points();
    let maximum_minor = discount.maximum_minor();
    let authorization_intent = discount.authorization_intent();
    let breakdown = DiscountBreakdown {
        kind,
        reason: discount.reason,
        reason_key: discount.reason_key,
        fixed_amount_minor,
        percentage_basis_points,
        maximum_minor,
        calculated_minor,
        applied_minor,
        authorization_intent,
    };
    Ok((Some(breakdown), applied_minor))
}

/// Largest-remainder apportionment with stable key ordering for ties.
fn allocate_proportionally(
    total: i64,
    weights: &[i64],
    tie_keys: &[&str],
) -> Result<Vec<i64>, PricingError> {
    debug_assert_eq!(weights.len(), tie_keys.len());
    if total == 0 {
        return Ok(vec![0; weights.len()]);
    }
    let total_weight = checked_sum(weights.iter().copied())?;
    if total < 0 || total_weight <= 0 {
        return Err(PricingError::AmountOverflow);
    }

    let mut result = Vec::with_capacity(weights.len());
    let mut remainders = Vec::with_capacity(weights.len());
    let mut floor_sum = 0_i64;
    for (index, weight) in weights.iter().copied().enumerate() {
        if weight < 0 {
            return Err(PricingError::AmountOverflow);
        }
        let numerator = i128::from(total) * i128::from(weight);
        let floor = checked_i64(numerator / i128::from(total_weight))?;
        floor_sum = floor_sum
            .checked_add(floor)
            .ok_or(PricingError::AmountOverflow)?;
        result.push(floor);
        remainders.push((index, numerator % i128::from(total_weight)));
    }

    remainders.sort_by(|(left_index, left), (right_index, right)| {
        right.cmp(left).then_with(|| {
            tie_keys[*left_index]
                .cmp(tie_keys[*right_index])
                .then(Ordering::Equal)
        })
    });
    let remaining = total
        .checked_sub(floor_sum)
        .ok_or(PricingError::AmountOverflow)?;
    let count = usize::try_from(remaining).map_err(|_| PricingError::AmountOverflow)?;
    if count > remainders.len() {
        return Err(PricingError::AmountOverflow);
    }
    for (index, _) in remainders.into_iter().take(count) {
        result[index] = result[index]
            .checked_add(1)
            .ok_or(PricingError::AmountOverflow)?;
    }
    Ok(result)
}

fn round_half_up(numerator: i128, denominator: i128) -> Result<i64, PricingError> {
    if numerator < 0 || denominator <= 0 {
        return Err(PricingError::AmountOverflow);
    }
    let rounded = numerator
        .checked_add(denominator / 2)
        .ok_or(PricingError::AmountOverflow)?
        / denominator;
    checked_i64(rounded)
}

fn checked_sum(values: impl IntoIterator<Item = i64>) -> Result<i64, PricingError> {
    values.into_iter().try_fold(0_i64, |total, value| {
        total.checked_add(value).ok_or(PricingError::AmountOverflow)
    })
}

fn checked_i64(value: i128) -> Result<i64, PricingError> {
    i64::try_from(value).map_err(|_| PricingError::AmountOverflow)
}

fn normalize_text(value: &str, maximum_characters: usize) -> Option<(String, String)> {
    let display = value.trim().nfc().collect::<String>();
    if display.is_empty()
        || display.chars().count() > maximum_characters
        || display.chars().any(char::is_control)
    {
        return None;
    }
    let key = display
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfc()
        .collect();
    Some((display, key))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PricingError {
    AmountOverflow,
    CombinedTaxRateTooHigh,
    CurrencyMismatch { expected: String, actual: String },
    DuplicateLineReference,
    DuplicateTaxName,
    EmptyPricingLines,
    EmptyTaxProfile,
    InvalidCurrency,
    InvalidDiscountCap,
    InvalidDiscountRate,
    InvalidDiscountReason,
    InvalidLineName,
    InvalidLineReference,
    InvalidQuantity,
    InvalidTaxName,
    InvalidTaxRate,
    NegativeDiscount,
    NegativeUnitPrice,
    TooManyPricingLines,
    TooManyTaxRates,
}

impl fmt::Display for PricingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmountOverflow => {
                formatter.write_str("The pricing amount exceeds the supported range.")
            }
            Self::CombinedTaxRateTooHigh => {
                formatter.write_str("The combined line tax rate cannot exceed 100% in pricing v1.")
            }
            Self::CurrencyMismatch { expected, actual } => write!(
                formatter,
                "All pricing lines must use {expected}; found {actual}."
            ),
            Self::DuplicateLineReference => {
                formatter.write_str("Pricing line references must be unique.")
            }
            Self::DuplicateTaxName => {
                formatter.write_str("Tax component names must be unique within a line profile.")
            }
            Self::EmptyPricingLines => formatter.write_str("Pricing requires at least one line."),
            Self::EmptyTaxProfile => {
                formatter.write_str("A taxable line requires at least one named tax rate.")
            }
            Self::InvalidCurrency => formatter.write_str("The pricing currency is invalid."),
            Self::InvalidDiscountCap => {
                formatter.write_str("A percentage discount cap cannot be negative.")
            }
            Self::InvalidDiscountRate => {
                formatter.write_str("A discount rate must be between 0 and 10,000 basis points.")
            }
            Self::InvalidDiscountReason => formatter.write_str(
                "A discount reason must be readable normalized text within 500 characters.",
            ),
            Self::InvalidLineName => formatter.write_str(
                "A pricing line name must be readable normalized text within 160 characters.",
            ),
            Self::InvalidLineReference => formatter.write_str(
                "A pricing line reference must be readable normalized text within 128 characters.",
            ),
            Self::InvalidQuantity => {
                formatter.write_str("A pricing quantity must be between 1 and 1,000,000.")
            }
            Self::InvalidTaxName => formatter
                .write_str("A tax name must be readable normalized text within 120 characters."),
            Self::InvalidTaxRate => {
                formatter.write_str("Each tax rate must be between 0 and 10,000 basis points.")
            }
            Self::NegativeDiscount => formatter.write_str("A fixed discount cannot be negative."),
            Self::NegativeUnitPrice => formatter.write_str("A unit price cannot be negative."),
            Self::TooManyPricingLines => {
                formatter.write_str("An order cannot contain more than 200 pricing lines.")
            }
            Self::TooManyTaxRates => {
                formatter.write_str("A line cannot contain more than eight tax components.")
            }
        }
    }
}

impl Error for PricingError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn money(value: i64) -> Money {
        Money::new(value, "INR").expect("valid money")
    }

    fn rate(name: &str, basis_points: u32) -> TaxRate {
        TaxRate::new(name, basis_points).expect("valid rate")
    }

    fn line(
        reference: &str,
        unit_price: i64,
        quantity: i64,
        tax: TaxTreatment,
    ) -> PricingLineInput {
        PricingLineInput::new(reference, reference, money(unit_price), quantity, tax)
            .expect("valid line")
    }

    #[test]
    fn zero_value_order_and_zero_rates_remain_zero() {
        for treatment in [
            TaxTreatment::no_tax(),
            TaxTreatment::exclusive(vec![rate("Zero", 0)]).expect("profile"),
            TaxTreatment::inclusive(vec![rate("Zero", 0)]).expect("profile"),
        ] {
            let result = calculate_pricing(
                vec![line("zero", 0, 1, treatment)],
                Some(OrderDiscount::fixed(10, "Courtesy").expect("discount")),
            )
            .expect("calculation");
            assert_eq!(result.net_before_discount_minor(), 0);
            assert_eq!(result.discount_minor(), 0);
            assert_eq!(result.tax_minor(), 0);
            assert_eq!(result.payable_minor(), 0);
        }
    }

    #[test]
    fn exact_half_rounds_up_for_exclusive_and_inclusive_tax() {
        let exclusive = calculate_pricing(
            vec![line(
                "exclusive",
                5,
                1,
                TaxTreatment::exclusive(vec![rate("Ten percent", 1_000)]).expect("profile"),
            )],
            None,
        )
        .expect("exclusive calculation");
        assert_eq!(exclusive.tax_minor(), 1);
        assert_eq!(exclusive.payable_minor(), 6);

        let inclusive = calculate_pricing(
            vec![line(
                "inclusive",
                1,
                1,
                TaxTreatment::inclusive(vec![rate("Hundred percent", 10_000)]).expect("profile"),
            )],
            None,
        )
        .expect("inclusive calculation");
        assert_eq!(inclusive.net_minor(), 0);
        assert_eq!(inclusive.tax_minor(), 1);
        assert_eq!(inclusive.payable_minor(), 1);
    }

    #[test]
    fn inclusive_tax_is_back_calculated_without_changing_gross() {
        let result = calculate_pricing(
            vec![line(
                "inclusive",
                110,
                1,
                TaxTreatment::inclusive(vec![rate("Component B", 500), rate("Component A", 500)])
                    .expect("profile"),
            )],
            None,
        )
        .expect("calculation");
        let line = &result.lines()[0];
        assert_eq!(line.net_minor(), 100);
        assert_eq!(line.tax_minor(), 10);
        assert_eq!(line.payable_minor(), 110);
        assert_eq!(line.tax_components()[0].name(), "Component A");
        assert_eq!(line.tax_components()[0].tax_minor(), 5);
        assert_eq!(line.tax_components()[1].tax_minor(), 5);
    }

    #[test]
    fn fixed_and_percentage_discounts_obey_all_caps() {
        let base = || vec![line("item", 1_000, 1, TaxTreatment::no_tax())];
        let fixed = calculate_pricing(
            base(),
            Some(OrderDiscount::fixed(2_000, "Recovery").expect("discount")),
        )
        .expect("fixed calculation");
        assert_eq!(
            fixed.discount().expect("snapshot").calculated_minor(),
            2_000
        );
        assert_eq!(fixed.discount_minor(), 1_000);
        assert_eq!(fixed.payable_minor(), 0);

        let percentage = calculate_pricing(
            base(),
            Some(OrderDiscount::percentage(5_000, Some(300), "Promotion").expect("discount")),
        )
        .expect("percentage calculation");
        assert_eq!(
            percentage.discount().expect("snapshot").calculated_minor(),
            300
        );
        assert_eq!(percentage.discount_minor(), 300);
        assert_eq!(percentage.payable_minor(), 700);

        let midpoint = calculate_pricing(
            vec![line("one", 1, 1, TaxTreatment::no_tax())],
            Some(OrderDiscount::percentage(5_000, None, "Midpoint").expect("discount")),
        )
        .expect("midpoint calculation");
        assert_eq!(midpoint.discount_minor(), 1);
        assert_eq!(midpoint.payable_minor(), 0);

        assert!(OrderDiscount::fixed(-1, "Invalid").is_err());
        assert!(OrderDiscount::percentage(1, Some(-1), "Invalid").is_err());
        assert!(OrderDiscount::percentage(10_001, None, "Invalid").is_err());
    }

    #[test]
    fn mixed_tax_lines_allocate_discount_and_recompute_tax_deterministically() {
        let result = calculate_pricing(
            vec![
                line("none", 100, 1, TaxTreatment::no_tax()),
                line(
                    "exclusive",
                    100,
                    1,
                    TaxTreatment::exclusive(vec![rate("Local tax", 1_000)]).expect("profile"),
                ),
                line(
                    "inclusive",
                    110,
                    1,
                    TaxTreatment::inclusive(vec![rate("Local tax", 1_000)]).expect("profile"),
                ),
            ],
            Some(OrderDiscount::fixed(30, "Manager recovery").expect("discount")),
        )
        .expect("calculation");

        assert_eq!(result.input_price_subtotal_minor(), 310);
        assert_eq!(result.net_before_discount_minor(), 300);
        assert_eq!(result.discount_minor(), 30);
        assert_eq!(result.net_minor(), 270);
        assert_eq!(result.tax_before_discount_minor(), 20);
        assert_eq!(result.tax_minor(), 18);
        assert_eq!(result.payable_before_discount_minor(), 320);
        assert_eq!(result.payable_minor(), 288);
        assert!(
            result
                .lines()
                .iter()
                .all(|line| line.allocated_discount_minor() == 10)
        );
    }

    #[test]
    fn every_arithmetic_boundary_is_checked() {
        let multiplied = PricingLineInput::new(
            "multiply",
            "Multiply",
            money(i64::MAX),
            2,
            TaxTreatment::no_tax(),
        )
        .expect("structurally valid");
        assert_eq!(
            calculate_pricing(vec![multiplied], None),
            Err(PricingError::AmountOverflow)
        );

        let payable = line(
            "tax overflow",
            i64::MAX,
            1,
            TaxTreatment::exclusive(vec![rate("Tax", 1)]).expect("profile"),
        );
        assert_eq!(
            calculate_pricing(vec![payable], None),
            Err(PricingError::AmountOverflow)
        );

        let summed = vec![
            line("maximum", i64::MAX, 1, TaxTreatment::no_tax()),
            line("one", 1, 1, TaxTreatment::no_tax()),
        ];
        assert_eq!(
            calculate_pricing(summed, None),
            Err(PricingError::AmountOverflow)
        );
    }

    #[test]
    fn names_reasons_rates_and_collections_are_bounded_and_normalized() {
        let normalized_rate = TaxRate::new("  Cafe\u{301} tax  ", 100).expect("rate");
        assert_eq!(normalized_rate.name(), "Café tax");
        assert_eq!(normalized_rate.name_key(), "café tax");
        let discount = OrderDiscount::fixed(1, "  Cafe\u{301} recovery  ").expect("discount");
        assert_eq!(discount.reason(), "Café recovery");
        assert!(TaxRate::new("Tax\nname", 1).is_err());
        assert!(TaxRate::new("Tax", MAX_COMBINED_TAX_BASIS_POINTS + 1).is_err());
        assert!(OrderDiscount::fixed(1, "Reason\nline").is_err());
        assert!(PricingLineInput::new(" ", "Name", money(1), 1, TaxTreatment::no_tax()).is_err());
        assert!(
            PricingLineInput::new(
                "id",
                "Name",
                money(1),
                MAX_LINE_QUANTITY + 1,
                TaxTreatment::no_tax()
            )
            .is_err()
        );
        assert!(TaxTreatment::exclusive(Vec::new()).is_err());
        assert!(TaxTreatment::exclusive(vec![rate("Same", 1), rate("same", 1)]).is_err());
        assert!(
            TaxTreatment::exclusive(
                (0..=MAX_TAX_RATES_PER_LINE)
                    .map(|index| rate(&format!("Tax {index}"), 1))
                    .collect()
            )
            .is_err()
        );
        assert!(TaxTreatment::exclusive(vec![rate("A", 6_000), rate("B", 4_001)]).is_err());
        assert_eq!(
            calculate_pricing(Vec::new(), None),
            Err(PricingError::EmptyPricingLines)
        );

        let too_many = (0..=MAX_PRICING_LINES)
            .map(|index| line(&format!("line {index}"), 1, 1, TaxTreatment::no_tax()))
            .collect();
        assert_eq!(
            calculate_pricing(too_many, None),
            Err(PricingError::TooManyPricingLines)
        );
    }

    #[test]
    fn line_references_and_currencies_are_unique_and_consistent() {
        let duplicate = vec![
            line("Table A", 1, 1, TaxTreatment::no_tax()),
            line("table a", 1, 1, TaxTreatment::no_tax()),
        ];
        assert_eq!(
            calculate_pricing(duplicate, None),
            Err(PricingError::DuplicateLineReference)
        );

        let usd = PricingLineInput::new(
            "usd",
            "USD",
            Money::new(1, "USD").expect("money"),
            1,
            TaxTreatment::no_tax(),
        )
        .expect("line");
        assert!(matches!(
            calculate_pricing(vec![line("inr", 1, 1, TaxTreatment::no_tax()), usd], None),
            Err(PricingError::CurrencyMismatch { .. })
        ));
    }

    #[test]
    fn remainder_allocation_uses_canonical_keys_not_input_order() {
        let calculate = |lines| {
            calculate_pricing(
                lines,
                Some(OrderDiscount::fixed(1, "Canonical tie").expect("discount")),
            )
            .expect("calculation")
        };
        let first = calculate(vec![
            line("Zulu", 1, 1, TaxTreatment::no_tax()),
            line("Alpha", 1, 1, TaxTreatment::no_tax()),
        ]);
        let second = calculate(vec![
            line("Alpha", 1, 1, TaxTreatment::no_tax()),
            line("Zulu", 1, 1, TaxTreatment::no_tax()),
        ]);
        for result in [first, second] {
            let alpha = result
                .lines()
                .iter()
                .find(|line| line.line_reference() == "Alpha")
                .expect("alpha line");
            let zulu = result
                .lines()
                .iter()
                .find(|line| line.line_reference() == "Zulu")
                .expect("zulu line");
            assert_eq!(alpha.allocated_discount_minor(), 1);
            assert_eq!(zulu.allocated_discount_minor(), 0);
        }

        let included = calculate_pricing(
            vec![line(
                "inclusive tie",
                1,
                1,
                TaxTreatment::inclusive(vec![rate("Zulu tax", 5_000), rate("Alpha tax", 5_000)])
                    .expect("profile"),
            )],
            None,
        )
        .expect("inclusive calculation");
        let components = included.lines()[0].tax_components();
        assert_eq!(components[0].name(), "Alpha tax");
        assert_eq!(components[0].tax_minor(), 1);
        assert_eq!(components[1].tax_minor(), 0);
    }

    #[test]
    fn discount_permission_is_explicit_intent_not_caller_authorization() {
        let intent = OrderDiscount::fixed(1, "Approved recovery")
            .expect("discount")
            .authorization_intent();
        assert!(intent.permits_role(ActorRole::Owner));
        assert!(intent.permits_role(ActorRole::Manager));
        assert!(!intent.permits_role(ActorRole::Cashier));
        assert!(!intent.permits_role(ActorRole::Kitchen));
    }

    #[test]
    fn small_domain_sweep_preserves_all_calculation_invariants() {
        let rates = [0, 1, 333, 1_000, 5_000, 10_000];
        for amount in 0..=40_i64 {
            for basis_points in rates {
                for treatment in [
                    TaxTreatment::no_tax(),
                    TaxTreatment::exclusive(vec![rate("Tax", basis_points)]).expect("profile"),
                    TaxTreatment::inclusive(vec![rate("Tax", basis_points)]).expect("profile"),
                ] {
                    for discount in 0..=amount + 2 {
                        let result = calculate_pricing(
                            vec![line("sweep", amount, 1, treatment.clone())],
                            Some(OrderDiscount::fixed(discount, "Sweep").expect("discount")),
                        )
                        .expect("bounded calculation");
                        let sum_net: i64 = result.lines().iter().map(|line| line.net_minor()).sum();
                        let sum_tax: i64 = result.lines().iter().map(|line| line.tax_minor()).sum();
                        let sum_payable: i64 =
                            result.lines().iter().map(|line| line.payable_minor()).sum();
                        let sum_discount: i64 = result
                            .lines()
                            .iter()
                            .map(|line| line.allocated_discount_minor())
                            .sum();

                        assert_eq!(sum_net, result.net_minor());
                        assert_eq!(sum_tax, result.tax_minor());
                        assert_eq!(sum_payable, result.payable_minor());
                        assert_eq!(sum_discount, result.discount_minor());
                        assert_eq!(
                            result.net_before_discount_minor() - result.discount_minor(),
                            result.net_minor()
                        );
                        assert_eq!(
                            result.net_minor() + result.tax_minor(),
                            result.payable_minor()
                        );
                        assert!(result.discount_minor() >= 0);
                        assert!(result.net_minor() >= 0);
                        assert!(result.tax_minor() >= 0);
                        assert!(result.payable_minor() >= 0);
                        assert!(result.payable_minor() <= result.payable_before_discount_minor());
                    }
                }
            }
        }
    }
}
