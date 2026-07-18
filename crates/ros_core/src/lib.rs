//! The trusted, UI-independent domain core for Restaurant Operating System.
//!
//! This crate deliberately contains the financial and audit invariants that
//! must behave the same on desktop, tablet, mobile, and cloud services.

#![forbid(unsafe_code)]

pub mod entitlement;
pub mod pricing;

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use uuid::{Uuid, Variant, Version};

pub const PRODUCT_NAME: &str = "Restaurant Operating System";
pub const PRODUCT_SHORT_NAME: &str = "Restaurant Operating System";

/// A currency amount represented in minor units only.
///
/// Values such as rupees must never be represented using floating point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Money {
    minor_units: i64,
    currency: CurrencyCode,
}

impl Money {
    pub fn new(minor_units: i64, currency: impl AsRef<str>) -> Result<Self, DomainError> {
        Ok(Self {
            minor_units,
            currency: CurrencyCode::parse(currency.as_ref())?,
        })
    }

    pub fn minor_units(&self) -> i64 {
        self.minor_units
    }

    pub fn currency(&self) -> &str {
        self.currency.as_str()
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch {
                left: self.currency.as_str().to_owned(),
                right: other.currency.as_str().to_owned(),
            });
        }

        let minor_units = self
            .minor_units
            .checked_add(other.minor_units)
            .ok_or(DomainError::AmountOverflow)?;

        Ok(Self {
            minor_units,
            currency: self.currency.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrencyCode(String);

impl CurrencyCode {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(DomainError::InvalidCurrency(value.to_owned()));
        }

        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinancialMutationKind {
    InvoiceCreated,
    PaymentRecorded,
    InvoiceVoided,
    PaymentRefunded,
    StockAdjusted,
    ExpenseRecorded,
}

impl FinancialMutationKind {
    /// All financial changes are represented by a new event. None may silently
    /// delete or overwrite its historical predecessor.
    pub const fn is_append_only(self) -> bool {
        true
    }

    pub const fn requires_reason(self) -> bool {
        matches!(
            self,
            Self::InvoiceVoided | Self::PaymentRefunded | Self::StockAdjusted
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreStatus {
    pub product_name: &'static str,
    pub storage_owner: &'static str,
    pub financial_history: &'static str,
}

impl CoreStatus {
    pub fn summary(&self) -> String {
        format!(
            "{} • {} • {}",
            self.product_name, self.storage_owner, self.financial_history
        )
    }
}

pub fn initial_core_status() -> CoreStatus {
    CoreStatus {
        product_name: PRODUCT_SHORT_NAME,
        storage_owner: "Rust operational core",
        financial_history: "append-only financial history",
    }
}

pub fn sum_minor_units(values: impl IntoIterator<Item = i64>) -> Result<i64, DomainError> {
    values.into_iter().try_fold(0_i64, |total, value| {
        total.checked_add(value).ok_or(DomainError::AmountOverflow)
    })
}

/// A canonical UUIDv7 identity for durable entities and cross-device events.
///
/// UUIDv7 provides practical uniqueness without a cloud round trip and keeps
/// new records roughly ordered by creation time for local and cloud storage.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(Uuid);

impl EntityId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let parsed =
            Uuid::parse_str(value).map_err(|_| DomainError::InvalidIdentifier(value.to_owned()))?;

        if parsed.get_variant() != Variant::RFC4122
            || parsed.get_version() != Some(Version::SortRand)
            || parsed.hyphenated().to_string() != value
        {
            return Err(DomainError::InvalidIdentifier(value.to_owned()));
        }

        Ok(Self(parsed))
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(formatter)
    }
}

/// Stable domain-separated audit-envelope hash used by both the encrypted
/// local store and the future Professional sync verifier. The inputs are
/// length-prefixed so the encoding is unambiguous even when payload values
/// contain delimiters or arbitrary Unicode text.
pub const AUDIT_EVENT_HASH_PROTOCOL_V1: &[u8] = b"gotigin.restaurant-os.audit-event.v1";

/// The exact immutable fields covered by an audit-event hash. Callers must
/// pass canonical UUIDv7 strings for identities before computing or verifying
/// a hash; identity parsing remains a separate boundary concern.
pub struct AuditEventHashInput<'input> {
    pub event_id: &'input str,
    pub branch_id: &'input str,
    pub actor_id: &'input str,
    pub device_id: &'input str,
    pub sequence: i64,
    pub event_type: &'input str,
    pub payload_json: &'input str,
    pub occurred_at_utc: &'input str,
    pub previous_hash: Option<&'input [u8]>,
}

/// Computes the v1 hash for one append-only audit event. This function does
/// no normalization: the bytes supplied here are the authoritative stored
/// envelope and must be preserved exactly across sync transport.
pub fn audit_event_hash(input: AuditEventHashInput<'_>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(AUDIT_EVENT_HASH_PROTOCOL_V1);
    hash_audit_component(&mut hasher, input.event_id.as_bytes());
    hash_audit_component(&mut hasher, input.branch_id.as_bytes());
    hash_audit_component(&mut hasher, input.actor_id.as_bytes());
    hash_audit_component(&mut hasher, input.device_id.as_bytes());
    hash_audit_component(&mut hasher, &input.sequence.to_be_bytes());
    hash_audit_component(&mut hasher, input.event_type.as_bytes());
    hash_audit_component(&mut hasher, input.payload_json.as_bytes());
    hash_audit_component(&mut hasher, input.occurred_at_utc.as_bytes());

    match input.previous_hash {
        Some(previous_hash) => {
            hasher.update([1]);
            hash_audit_component(&mut hasher, previous_hash);
        }
        None => hasher.update([0]),
    }

    hasher.finalize().into()
}

fn hash_audit_component(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

/// A user-facing label with a stable Unicode-normalized comparison key.
///
/// SQLite's `NOCASE` is intentionally not used as a uniqueness mechanism: it
/// does not implement full Unicode casing. The label key is generated in Rust
/// so names such as Indian-language menu categories remain consistently stored
/// across supported platforms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayName {
    display: String,
    key: String,
}

impl DisplayName {
    pub fn organization(value: &str) -> Result<Self, DomainError> {
        Self::new(value, 160, "Organization name")
    }

    pub fn branch(value: &str) -> Result<Self, DomainError> {
        Self::new(value, 160, "Branch name")
    }

    pub fn category(value: &str) -> Result<Self, DomainError> {
        Self::new(value, 120, "Category name")
    }

    pub fn product(value: &str) -> Result<Self, DomainError> {
        Self::new(value, 160, "Product name")
    }

    /// A catalogue modifier is intentionally its own bounded label type. It
    /// is not a free-form kitchen instruction: the value participates in a
    /// trusted price calculation and is preserved in sales snapshots.
    pub fn modifier_option(value: &str) -> Result<Self, DomainError> {
        Self::new(value, 120, "Modifier option name")
    }

    pub fn staff(value: &str) -> Result<Self, DomainError> {
        Self::new(value, 120, "Staff name")
    }

    pub fn customer(value: &str) -> Result<Self, DomainError> {
        Self::new(value, 160, "Customer name")
    }

    fn new(
        value: &str,
        maximum_characters: usize,
        field: &'static str,
    ) -> Result<Self, DomainError> {
        let display = value.trim().nfc().collect::<String>();

        if display.is_empty()
            || display.chars().count() > maximum_characters
            || display.chars().any(char::is_control)
        {
            return Err(DomainError::InvalidDisplayName(field));
        }

        let lowercase = display
            .chars()
            .flat_map(char::to_lowercase)
            .collect::<String>();
        let key = lowercase.nfc().collect::<String>();

        Ok(Self { display, key })
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeZoneId(String);

impl TimeZoneId {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let value = value.trim();

        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'+' | b'-')
            })
        {
            return Err(DomainError::InvalidTimeZone(value.to_owned()));
        }

        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunitySetup {
    organization_name: DisplayName,
    branch_name: DisplayName,
    currency: CurrencyCode,
    time_zone: TimeZoneId,
}

impl CommunitySetup {
    pub fn new(
        organization_name: &str,
        branch_name: &str,
        currency: &str,
        time_zone: &str,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            organization_name: DisplayName::organization(organization_name)?,
            branch_name: DisplayName::branch(branch_name)?,
            currency: CurrencyCode::parse(currency)?,
            time_zone: TimeZoneId::parse(time_zone)?,
        })
    }

    pub fn organization_name(&self) -> &DisplayName {
        &self.organization_name
    }

    pub fn branch_name(&self) -> &DisplayName {
        &self.branch_name
    }

    pub fn currency(&self) -> &str {
        self.currency.as_str()
    }

    pub fn currency_code(&self) -> CurrencyCode {
        self.currency.clone()
    }

    pub fn time_zone(&self) -> &str {
        self.time_zone.as_str()
    }

    pub fn time_zone_id(&self) -> TimeZoneId {
        self.time_zone.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    organization_id: EntityId,
    branch_id: EntityId,
    display_name: DisplayName,
    currency: CurrencyCode,
    time_zone: TimeZoneId,
}

impl Branch {
    pub fn new(
        organization_id: EntityId,
        branch_id: EntityId,
        display_name: DisplayName,
        currency: CurrencyCode,
        time_zone: TimeZoneId,
    ) -> Self {
        Self {
            organization_id,
            branch_id,
            display_name,
            currency,
            time_zone,
        }
    }

    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn currency(&self) -> &str {
        self.currency.as_str()
    }

    pub fn time_zone(&self) -> &str {
        self.time_zone.as_str()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorRole {
    Owner,
    Manager,
    Cashier,
    Kitchen,
}

impl ActorRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Manager => "manager",
            Self::Cashier => "cashier",
            Self::Kitchen => "kitchen",
        }
    }
}

/// Attribution supplied by the trusted application session for each mutation.
/// The storage layer generates the event identity, time, sequence, and hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationContext {
    branch_id: EntityId,
    actor_id: EntityId,
    device_id: EntityId,
    correlation_id: EntityId,
    actor_role: ActorRole,
}

impl MutationContext {
    pub fn new(
        branch_id: EntityId,
        actor_id: EntityId,
        device_id: EntityId,
        correlation_id: EntityId,
        actor_role: ActorRole,
    ) -> Self {
        Self {
            branch_id,
            actor_id,
            device_id,
            correlation_id,
            actor_role,
        }
    }

    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    pub fn actor_id(&self) -> &EntityId {
        &self.actor_id
    }

    pub fn device_id(&self) -> &EntityId {
        &self.device_id
    }

    pub fn correlation_id(&self) -> &EntityId {
        &self.correlation_id
    }

    pub const fn actor_role(&self) -> ActorRole {
        self.actor_role
    }
}

/// Required human-readable rationale for a sensitive correction or archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationReason(String);

impl MutationReason {
    pub fn new(value: &str) -> Result<Self, DomainError> {
        let value = value.trim().nfc().collect::<String>();

        if value.is_empty() || value.chars().count() > 500 || value.chars().any(char::is_control) {
            return Err(DomainError::InvalidMutationReason);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateCategory {
    display_name: DisplayName,
    sort_order: i64,
}

impl CreateCategory {
    pub fn new(display_name: &str, sort_order: i64) -> Result<Self, DomainError> {
        Ok(Self {
            display_name: DisplayName::category(display_name)?,
            sort_order,
        })
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub const fn sort_order(&self) -> i64 {
        self.sort_order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Category {
    category_id: EntityId,
    branch_id: EntityId,
    display_name: DisplayName,
    sort_order: i64,
    revision: i64,
    archived: bool,
}

impl Category {
    pub fn new(
        category_id: EntityId,
        branch_id: EntityId,
        display_name: DisplayName,
        sort_order: i64,
        revision: i64,
        archived: bool,
    ) -> Self {
        Self {
            category_id,
            branch_id,
            display_name,
            sort_order,
            revision,
            archived,
        }
    }

    pub fn category_id(&self) -> &EntityId {
        &self.category_id
    }

    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub const fn sort_order(&self) -> i64 {
        self.sort_order
    }

    pub const fn revision(&self) -> i64 {
        self.revision
    }

    pub const fn archived(&self) -> bool {
        self.archived
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateProduct {
    display_name: DisplayName,
    category_id: Option<EntityId>,
    unit_price: Money,
    sku: Option<String>,
    barcode: Option<String>,
    sort_order: i64,
}

impl CreateProduct {
    pub fn new(
        display_name: &str,
        category_id: Option<EntityId>,
        unit_price: Money,
        sku: Option<&str>,
        barcode: Option<&str>,
        sort_order: i64,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            display_name: DisplayName::product(display_name)?,
            category_id,
            unit_price,
            sku: normalize_optional_code(sku, 64, "SKU")?,
            barcode: normalize_optional_code(barcode, 64, "Barcode")?,
            sort_order,
        })
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn category_id(&self) -> Option<&EntityId> {
        self.category_id.as_ref()
    }

    pub fn unit_price(&self) -> &Money {
        &self.unit_price
    }

    pub fn sku(&self) -> Option<&str> {
        self.sku.as_deref()
    }

    pub fn barcode(&self) -> Option<&str> {
        self.barcode.as_deref()
    }

    pub const fn sort_order(&self) -> i64 {
        self.sort_order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product {
    product_id: EntityId,
    branch_id: EntityId,
    category_id: Option<EntityId>,
    display_name: DisplayName,
    unit_price: Money,
    sku: Option<String>,
    barcode: Option<String>,
    is_available: bool,
    sort_order: i64,
    revision: i64,
    archived: bool,
}

/// A new, immutable catalogue modifier option. The option is attached to one
/// product and may later be archived, but its name and price delta are never
/// rewritten. A replacement must be a new option so historical orders retain
/// exactly what the guest selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateModifierOption {
    display_name: DisplayName,
    price_delta: Money,
}

impl CreateModifierOption {
    pub fn new(display_name: &str, price_delta: Money) -> Result<Self, DomainError> {
        if price_delta.minor_units() < 0 {
            return Err(DomainError::NegativeModifierPriceDelta);
        }

        Ok(Self {
            display_name: DisplayName::modifier_option(display_name)?,
            price_delta,
        })
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn price_delta(&self) -> &Money {
        &self.price_delta
    }
}

/// The current catalogue projection of an immutable product modifier option.
/// `archived` options are returned for historical order rendering but must
/// never be accepted for a new or revised sale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierOption {
    modifier_option_id: EntityId,
    branch_id: EntityId,
    product_id: EntityId,
    display_name: DisplayName,
    price_delta: Money,
    revision: i64,
    archived: bool,
}

impl ModifierOption {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        modifier_option_id: EntityId,
        branch_id: EntityId,
        product_id: EntityId,
        display_name: DisplayName,
        price_delta: Money,
        revision: i64,
        archived: bool,
    ) -> Self {
        Self {
            modifier_option_id,
            branch_id,
            product_id,
            display_name,
            price_delta,
            revision,
            archived,
        }
    }

    pub fn modifier_option_id(&self) -> &EntityId {
        &self.modifier_option_id
    }

    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    pub fn product_id(&self) -> &EntityId {
        &self.product_id
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn price_delta(&self) -> &Money {
        &self.price_delta
    }

    pub const fn revision(&self) -> i64 {
        self.revision
    }

    pub const fn archived(&self) -> bool {
        self.archived
    }
}

impl Product {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        product_id: EntityId,
        branch_id: EntityId,
        category_id: Option<EntityId>,
        display_name: DisplayName,
        unit_price: Money,
        sku: Option<String>,
        barcode: Option<String>,
        is_available: bool,
        sort_order: i64,
        revision: i64,
        archived: bool,
    ) -> Self {
        Self {
            product_id,
            branch_id,
            category_id,
            display_name,
            unit_price,
            sku,
            barcode,
            is_available,
            sort_order,
            revision,
            archived,
        }
    }

    pub fn product_id(&self) -> &EntityId {
        &self.product_id
    }

    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    pub fn category_id(&self) -> Option<&EntityId> {
        self.category_id.as_ref()
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn unit_price(&self) -> &Money {
        &self.unit_price
    }

    pub fn sku(&self) -> Option<&str> {
        self.sku.as_deref()
    }

    pub fn barcode(&self) -> Option<&str> {
        self.barcode.as_deref()
    }

    pub const fn is_available(&self) -> bool {
        self.is_available
    }

    pub const fn sort_order(&self) -> i64 {
        self.sort_order
    }

    pub const fn revision(&self) -> i64 {
        self.revision
    }

    pub const fn archived(&self) -> bool {
        self.archived
    }
}

/// How a local counter order will be fulfilled. More complex table and delivery
/// workflows can extend this closed initial set without changing invoice facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderFulfillment {
    DineIn,
    Takeaway,
}

impl OrderFulfillment {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "dine_in" => Ok(Self::DineIn),
            "takeaway" => Ok(Self::Takeaway),
            _ => Err(DomainError::InvalidOrderFulfillment(value.to_owned())),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DineIn => "dine_in",
            Self::Takeaway => "takeaway",
        }
    }
}

/// A payment method descriptor only. Card numbers, UPI handles, and processor
/// credentials are intentionally not part of a local payment record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentMethod {
    Cash,
    Card,
    Upi,
}

impl PaymentMethod {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "cash" => Ok(Self::Cash),
            "card" => Ok(Self::Card),
            "upi" => Ok(Self::Upi),
            _ => Err(DomainError::InvalidPaymentMethod(value.to_owned())),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Card => "card",
            Self::Upi => "upi",
        }
    }
}

/// An untrusted payment-tender allocation. Rust storage later verifies that
/// all allocations add up exactly to the trusted invoice total in its write
/// transaction; this type only rejects structurally impossible inputs early.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaymentAllocationInput {
    payment_method: PaymentMethod,
    amount_minor: i64,
}

impl PaymentAllocationInput {
    pub fn new(payment_method: PaymentMethod, amount_minor: i64) -> Result<Self, DomainError> {
        if amount_minor <= 0 {
            return Err(DomainError::InvalidPaymentAllocation);
        }
        Ok(Self {
            payment_method,
            amount_minor,
        })
    }

    pub const fn payment_method(self) -> PaymentMethod {
        self.payment_method
    }

    pub const fn amount_minor(self) -> i64 {
        self.amount_minor
    }
}

/// A product and positive whole-unit quantity selected for a local sale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaleLineInput {
    product_id: EntityId,
    quantity: i64,
    modifier_option_ids: Vec<EntityId>,
}

impl SaleLineInput {
    pub fn new(product_id: EntityId, quantity: i64) -> Result<Self, DomainError> {
        if quantity <= 0 {
            return Err(DomainError::InvalidSaleQuantity);
        }

        Ok(Self {
            product_id,
            quantity,
            modifier_option_ids: Vec::new(),
        })
    }

    /// Attaches a bounded, canonical set of catalogue option identities to a
    /// product line. The caller supplies identities only; encrypted storage
    /// loads option names and price deltas from the active catalogue inside
    /// the sale transaction.
    pub fn with_modifier_options(
        mut self,
        mut modifier_option_ids: Vec<EntityId>,
    ) -> Result<Self, DomainError> {
        if modifier_option_ids.len() > MAX_MODIFIER_OPTIONS_PER_SALE_LINE {
            return Err(DomainError::TooManyModifierSelections);
        }
        modifier_option_ids.sort();
        if modifier_option_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(DomainError::DuplicateModifierSelection);
        }
        self.modifier_option_ids = modifier_option_ids;
        Ok(self)
    }

    pub fn product_id(&self) -> &EntityId {
        &self.product_id
    }

    pub const fn quantity(&self) -> i64 {
        self.quantity
    }

    pub fn modifier_option_ids(&self) -> &[EntityId] {
        &self.modifier_option_ids
    }
}

/// A deliberately small bound protects every local checkout, draft, and
/// receipt snapshot from a pathological cart while still covering realistic
/// restaurant customisation.
pub const MAX_MODIFIER_OPTIONS_PER_SALE_LINE: usize = 20;

/// A complete, immediate local sale command. Rust derives prices and snapshot
/// data from the trusted product store; Flutter may never provide prices,
/// invoice numbers, or financial audit data for this command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteSale {
    fulfillment: OrderFulfillment,
    payment_method: PaymentMethod,
    payment_allocations: Option<Vec<PaymentAllocationInput>>,
    lines: Vec<SaleLineInput>,
    customer_id: Option<EntityId>,
    discount: Option<pricing::OrderDiscount>,
}

impl CompleteSale {
    pub fn new(
        fulfillment: OrderFulfillment,
        payment_method: PaymentMethod,
        lines: Vec<SaleLineInput>,
    ) -> Result<Self, DomainError> {
        if lines.is_empty() {
            return Err(DomainError::EmptySale);
        }

        let mut line_configurations = HashSet::with_capacity(lines.len());
        if lines.iter().any(|line| {
            !line_configurations.insert((
                line.product_id().clone(),
                line.modifier_option_ids().to_vec(),
            ))
        }) {
            return Err(DomainError::DuplicateSaleLineConfiguration);
        }

        Ok(Self {
            fulfillment,
            payment_method,
            payment_allocations: None,
            lines,
            customer_id: None,
            discount: None,
        })
    }

    /// Attaches an already selected local customer identity without allowing
    /// the counter to send contact details or create a customer implicitly.
    pub fn with_customer(mut self, customer_id: Option<EntityId>) -> Self {
        self.customer_id = customer_id;
        self
    }

    /// Attaches a validated order discount. Storage must revalidate that the
    /// active actor may approve discounts before the sale transaction commits.
    pub fn with_discount(mut self, discount: pricing::OrderDiscount) -> Self {
        self.discount = Some(discount);
        self
    }

    pub const fn fulfillment(&self) -> OrderFulfillment {
        self.fulfillment
    }

    pub const fn payment_method(&self) -> PaymentMethod {
        self.payment_method
    }

    /// Replaces the compatibility single-tender default with a non-empty set
    /// of positive allocations. Their exact total is deliberately checked by
    /// encrypted storage only after trusted product prices are loaded.
    pub fn with_payment_allocations(
        mut self,
        allocations: Vec<PaymentAllocationInput>,
    ) -> Result<Self, DomainError> {
        if allocations.is_empty() {
            return Err(DomainError::InvalidPaymentAllocation);
        }
        self.payment_allocations = Some(allocations);
        Ok(self)
    }

    pub fn payment_allocations(&self) -> Option<&[PaymentAllocationInput]> {
        self.payment_allocations.as_deref()
    }

    pub fn lines(&self) -> &[SaleLineInput] {
        &self.lines
    }

    pub fn customer_id(&self) -> Option<&EntityId> {
        self.customer_id.as_ref()
    }

    pub fn discount(&self) -> Option<&pricing::OrderDiscount> {
        self.discount.as_ref()
    }
}

fn normalize_optional_code(
    value: Option<&str>,
    maximum_characters: usize,
    field: &'static str,
) -> Result<Option<String>, DomainError> {
    value
        .map(|code| {
            let code = code.trim().nfc().collect::<String>();
            if code.is_empty()
                || code.chars().count() > maximum_characters
                || code.chars().any(char::is_control)
            {
                return Err(DomainError::InvalidCatalogCode(field));
            }

            Ok(code)
        })
        .transpose()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    AmountOverflow,
    CurrencyMismatch { left: String, right: String },
    DuplicateModifierSelection,
    DuplicateSaleLineConfiguration,
    EmptySale,
    InvalidCatalogCode(&'static str),
    InvalidCurrency(String),
    InvalidDisplayName(&'static str),
    InvalidIdentifier(String),
    InvalidOrderFulfillment(String),
    InvalidPaymentAllocation,
    InvalidPaymentMethod(String),
    InvalidMutationReason,
    InvalidSaleQuantity,
    InvalidTimeZone(String),
    NegativeModifierPriceDelta,
    TooManyModifierSelections,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmountOverflow => formatter.write_str("The amount exceeds the supported range."),
            Self::CurrencyMismatch { left, right } => {
                write!(formatter, "Cannot combine {left} and {right}.")
            }
            Self::DuplicateModifierSelection => formatter
                .write_str("A menu modifier may only be selected once on the same item."),
            Self::DuplicateSaleLineConfiguration => formatter.write_str(
                "Each product and modifier combination must appear once; combine quantities before saving.",
            ),
            Self::EmptySale => formatter.write_str("A sale must contain at least one menu item."),
            Self::InvalidCurrency(value) => {
                write!(
                    formatter,
                    "Currency code must be three uppercase letters: {value}"
                )
            }
            Self::InvalidCatalogCode(field) => {
                write!(formatter, "{field} must be non-empty, readable text.")
            }
            Self::InvalidDisplayName(field) => {
                write!(
                    formatter,
                    "{field} must be non-empty, readable text within its length limit."
                )
            }
            Self::InvalidIdentifier(value) => {
                write!(formatter, "Identifier must be a canonical UUIDv7: {value}")
            }
            Self::InvalidOrderFulfillment(value) => {
                write!(formatter, "Order fulfillment is invalid: {value}")
            }
            Self::InvalidPaymentAllocation => {
                formatter.write_str("Each payment allocation must have a positive amount.")
            }
            Self::InvalidPaymentMethod(value) => {
                write!(formatter, "Payment method is invalid: {value}")
            }
            Self::InvalidMutationReason => formatter
                .write_str("A correction reason must be readable text within 500 characters."),
            Self::InvalidSaleQuantity => {
                formatter.write_str("Sale quantities must be positive whole units.")
            }
            Self::InvalidTimeZone(value) => {
                write!(formatter, "Time zone identifier is invalid: {value}")
            }
            Self::NegativeModifierPriceDelta => {
                formatter.write_str("Modifier price adjustments cannot be negative.")
            }
            Self::TooManyModifierSelections => formatter.write_str(
                "A menu item cannot have more than 20 selected modifiers.",
            ),
        }
    }
}

impl Error for DomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_precise_in_minor_units() {
        let first = Money::new(19_999, "INR").expect("valid INR money");
        let second = Money::new(1, "INR").expect("valid INR money");

        assert_eq!(
            first
                .checked_add(&second)
                .expect("same currency")
                .minor_units(),
            20_000
        );
    }

    #[test]
    fn different_currencies_cannot_be_added() {
        let inr = Money::new(100, "INR").expect("valid INR money");
        let usd = Money::new(100, "USD").expect("valid USD money");

        assert!(matches!(
            inr.checked_add(&usd),
            Err(DomainError::CurrencyMismatch { .. })
        ));
    }

    #[test]
    fn currency_is_deliberately_strict() {
        assert!(Money::new(100, "inr").is_err());
        assert!(Money::new(100, "RUPEE").is_err());
    }

    #[test]
    fn financial_mutations_are_append_only() {
        for mutation in [
            FinancialMutationKind::InvoiceCreated,
            FinancialMutationKind::PaymentRecorded,
            FinancialMutationKind::InvoiceVoided,
            FinancialMutationKind::PaymentRefunded,
            FinancialMutationKind::StockAdjusted,
            FinancialMutationKind::ExpenseRecorded,
        ] {
            assert!(mutation.is_append_only());
        }
    }

    #[test]
    fn summation_detects_overflow() {
        assert_eq!(sum_minor_units([100, 200]).expect("safe sum"), 300);
        assert!(matches!(
            sum_minor_units([i64::MAX, 1]),
            Err(DomainError::AmountOverflow)
        ));
    }

    #[test]
    fn identifiers_are_canonical_uuidv7_values() {
        let identifier = EntityId::new_v7();
        let text = identifier.to_string();

        assert_eq!(
            EntityId::parse(&text).expect("generated UUIDv7"),
            identifier
        );
        assert!(EntityId::parse(&text.to_uppercase()).is_err());
        assert!(EntityId::parse("00000000-0000-4000-8000-000000000000").is_err());
    }

    #[test]
    fn audit_hash_binds_every_attribution_and_chain_component() {
        let event_id = EntityId::new_v7().to_string();
        let branch_id = EntityId::new_v7().to_string();
        let actor_id = EntityId::new_v7().to_string();
        let device_id = EntityId::new_v7().to_string();
        let common = || AuditEventHashInput {
            event_id: &event_id,
            branch_id: &branch_id,
            actor_id: &actor_id,
            device_id: &device_id,
            sequence: 1,
            event_type: "sales.order.finalized",
            payload_json: r#"{"order_id":"example"}"#,
            occurred_at_utc: "2026-07-17T00:00:00.000Z",
            previous_hash: None,
        };

        let original = audit_event_hash(common());
        assert_eq!(original, audit_event_hash(common()));

        let other_actor = EntityId::new_v7().to_string();
        assert_ne!(
            original,
            audit_event_hash(AuditEventHashInput {
                actor_id: &other_actor,
                ..common()
            })
        );
        assert_ne!(
            original,
            audit_event_hash(AuditEventHashInput {
                previous_hash: Some(&[7; 32]),
                ..common()
            })
        );
    }

    #[test]
    fn catalog_names_are_unicode_normalized_and_control_safe() {
        let category = CreateCategory::new("  Cafe\u{301}  ", 0).expect("valid category");

        assert_eq!(category.display_name().display(), "Café");
        assert_eq!(category.display_name().key(), "café");
        assert!(CreateCategory::new("Kitchen\nStation", 0).is_err());
    }

    #[test]
    fn product_codes_are_optional_but_not_blank() {
        let product = CreateProduct::new(
            "Masala chai",
            None,
            Money::new(2_500, "INR").expect("valid money"),
            Some(" TEA-001 "),
            None,
            0,
        )
        .expect("valid product");

        assert_eq!(product.sku(), Some("TEA-001"));
        assert!(
            CreateProduct::new(
                "Masala chai",
                None,
                Money::new(2_500, "INR").expect("valid money"),
                Some("   "),
                None,
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn complete_sale_requires_a_nonempty_unique_positive_cart() {
        let product = EntityId::new_v7();
        let valid_line = SaleLineInput::new(product.clone(), 2).expect("positive quantity");

        assert!(
            CompleteSale::new(OrderFulfillment::Takeaway, PaymentMethod::Cash, vec![]).is_err()
        );
        assert!(SaleLineInput::new(product.clone(), 0).is_err());
        assert!(
            CompleteSale::new(
                OrderFulfillment::DineIn,
                PaymentMethod::Upi,
                vec![valid_line.clone(), valid_line],
            )
            .is_err()
        );

        let sale = CompleteSale::new(
            OrderFulfillment::DineIn,
            PaymentMethod::Card,
            vec![SaleLineInput::new(product, 3).expect("positive quantity")],
        )
        .expect("valid sale");
        assert_eq!(sale.fulfillment(), OrderFulfillment::DineIn);
        assert_eq!(sale.payment_method(), PaymentMethod::Card);
        assert!(PaymentAllocationInput::new(PaymentMethod::Cash, 0).is_err());
        let split = sale
            .clone()
            .with_payment_allocations(vec![
                PaymentAllocationInput::new(PaymentMethod::Cash, 1).expect("cash"),
                PaymentAllocationInput::new(PaymentMethod::Upi, 2).expect("upi"),
            ])
            .expect("valid allocation structure");
        assert_eq!(split.payment_allocations().expect("allocations").len(), 2);
    }
}
