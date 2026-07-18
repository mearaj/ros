use std::io::Cursor;
use std::path::PathBuf;

use image::codecs::jpeg::JpegEncoder;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
#[cfg(test)]
use sha2::{Digest, Sha256};

const MAX_MENU_IMAGE_INPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_MENU_IMAGE_DECODE_WIDTH: u32 = 4_096;
const MAX_MENU_IMAGE_DECODE_HEIGHT: u32 = 4_096;
const MAX_MENU_IMAGE_DECODE_ALLOCATION: u64 = 32 * 1024 * 1024;
const MAX_MENU_IMAGE_ACCEPTED_BYTES: usize = 3 * 1024 * 1024;
const MAX_MENU_IMAGE_STORED_BYTES: usize = 65_536;
const GOTIGIN_CATALOG_SERVICE_ORIGIN: &str = "https://ros.gotigin.com";
const GOTIGIN_CATALOG_SERVICE_SCHEMA_VERSION: i64 = 1;

// This is intentionally selected by Cargo features, alongside the keyring
// namespace in `ros_storage`; it is not controlled by Flutter or a runtime
// environment variable. Development data can therefore never be mistaken for
// a release installation's database.
#[cfg(feature = "development-sqlcipher")]
const COMMUNITY_DATABASE_FILE_NAME: &str = "restaurant-os.development.db";
#[cfg(all(
    not(feature = "development-sqlcipher"),
    feature = "production-sqlcipher"
))]
const COMMUNITY_DATABASE_FILE_NAME: &str = "restaurant-os.db";

#[derive(Clone)]
pub struct CommunityCategoryView {
    pub category_id: String,
    pub display_name: String,
    pub revision: i64,
}

/// Untrusted catalogue selection supplied by Flutter. Rust verifies the
/// original bytes and every bounded provenance field before normalizing the
/// image or opening a storage transaction.
#[derive(Clone)]
pub struct GotiginCatalogMenuImageSelection {
    pub catalog_image_id: String,
    pub original_image_bytes: Vec<u8>,
    pub content_sha256: String,
    pub licence_label: String,
    pub licence_url: String,
    pub service_origin: String,
    pub service_schema_version: i64,
}

#[derive(Clone)]
pub struct CommunityProductView {
    pub product_id: String,
    pub category_id: Option<String>,
    pub display_name: String,
    pub unit_price_minor: i64,
    pub currency_code: String,
    pub revision: i64,
    pub is_available: bool,
    /// Provider-neutral tax treatment: `no_tax`, `exclusive`, or `inclusive`.
    pub tax_treatment: String,
    pub image_asset_key: Option<String>,
    pub image_bytes: Option<Vec<u8>>,
    /// Retained product-bound options. Archived values are included solely so
    /// restored drafts and receipts can render historical selections; the
    /// POS selector filters them out before it submits a new sale.
    pub modifier_options: Vec<CommunityModifierOptionView>,
}

#[derive(Clone)]
pub struct CommunityModifierOptionView {
    pub modifier_option_id: String,
    pub display_name: String,
    pub price_delta_minor: i64,
    pub currency_code: String,
    pub revision: i64,
    pub archived: bool,
}

/// Current, counter-safe customer projection. Earlier profile revisions and
/// anonymized identities never cross FFI, which keeps historical PII outside
/// the presentation layer by default.
#[derive(Clone)]
pub struct CommunityCustomerView {
    pub customer_id: String,
    pub display_name: String,
    pub phone_number: Option<String>,
    pub email_address: Option<String>,
    pub marketing_consent: bool,
    pub revision: i64,
}

#[derive(Clone)]
pub struct CommunityWorkspace {
    pub storage_status: String,
    pub setup_required: bool,
    pub branch_name: Option<String>,
    pub categories: Vec<CommunityCategoryView>,
    pub products: Vec<CommunityProductView>,
    pub customers: Vec<CommunityCustomerView>,
    pub open_drafts: Vec<CommunityDraftOrderView>,
    pub kitchen_tickets: Vec<CommunityKitchenTicketView>,
}

#[derive(Clone)]
pub struct CommunityDraftOrderView {
    pub draft_order_id: String,
    pub fulfillment: String,
    pub draft_state: String,
    pub table_name: Option<String>,
    /// An optional operational instruction. It is bounded and normalized in
    /// Rust storage and becomes immutable once sent to Kitchen Display.
    pub kitchen_note: Option<String>,
    pub revision: i64,
    pub subtotal_minor: i64,
    pub currency_code: String,
    pub line_count: i64,
    pub lines: Vec<CommunityCartLine>,
}

#[derive(Clone)]
pub struct CommunityKitchenTicketLineView {
    pub display_name: String,
    pub modifier_names: Vec<String>,
    pub quantity: i64,
}

#[derive(Clone)]
pub struct CommunityKitchenTicketView {
    pub ticket_id: String,
    pub table_label: Option<String>,
    pub state: String,
    pub revision: i64,
    /// Kitchen receives only the stop-work signal. The counter's free-text
    /// rationale remains in the protected audit ledger.
    pub cancellation_pending: bool,
    /// The immutable, counter-provided operational instruction for this
    /// ticket. Cancellation and management rationales never cross this
    /// boundary.
    pub kitchen_note: Option<String>,
    pub lines: Vec<CommunityKitchenTicketLineView>,
}

/// An untrusted selection from the Flutter cart. It intentionally contains no
/// price, tax, invoice, or timestamp fields: Rust derives those facts from the
/// encrypted catalog and its transaction clock.
#[derive(Clone)]
pub struct CommunityCartLine {
    pub product_id: String,
    pub quantity: i64,
    pub modifier_option_ids: Vec<String>,
}

/// An untrusted tender selection. Rust validates its shape and compares the
/// final allocation sum to the trusted invoice total inside SQLCipher.
#[derive(Clone)]
pub struct CommunityPaymentAllocation {
    pub payment_method: String,
    pub amount_minor: i64,
}

/// A deliberately small, user-safe checkout result. Detailed storage errors
/// and security-sensitive database state never cross the FFI boundary.
#[derive(Clone)]
pub struct CommunitySaleResult {
    pub storage_status: String,
    pub completed: bool,
    pub invoice_number: Option<String>,
    pub total_minor: i64,
    pub currency_code: Option<String>,
    pub payment_method: Option<String>,
}

#[derive(Clone)]
pub struct CommunityBackupResult {
    pub storage_status: String,
    pub created: bool,
    pub backup_file_name: Option<String>,
    pub sha256: Option<String>,
}

/// Owner-facing, privacy-allowlisted diagnostic event for local troubleshooting.
#[derive(Clone)]
pub struct CommunityDiagnosticEventView {
    pub occurred_at_utc: String,
    pub event_code: String,
    pub component: String,
    pub outcome: String,
    pub duration_ms: Option<i64>,
    pub detail_code: Option<String>,
}

#[derive(Clone)]
pub struct CommunityDiagnosticsWorkspace {
    pub storage_status: String,
    pub available: bool,
    pub events: Vec<CommunityDiagnosticEventView>,
}

#[derive(Clone)]
pub struct CommunityDiagnosticsPack {
    pub storage_status: String,
    pub available: bool,
    pub json_bytes: Vec<u8>,
    pub sha256: Option<String>,
    pub event_count: i64,
    pub byte_length: i64,
}

#[derive(Clone)]
pub struct CommunityDiagnosticsShareResult {
    pub storage_status: String,
    pub prepared: bool,
    pub uploaded: bool,
    pub sha256: Option<String>,
    pub event_count: i64,
    pub json_bytes: Vec<u8>,
}

/// A deliberately narrow, in-memory Community financial export. It contains
/// only verified branch-level payment/refund/expense aggregates and is passed
/// to Flutter solely so the owner can select an explicit save destination.
/// No local plaintext file is created by Rust.
#[derive(Clone)]
pub struct CommunityFinancialCsvExport {
    pub storage_status: String,
    pub available: bool,
    pub csv_bytes: Vec<u8>,
    pub record_count: i64,
    pub byte_length: i64,
}

/// A local-only sales summary for one UTC accounting day. Values are
/// calculated by Rust from finalized financial records; Flutter never
/// aggregates or invents totals.
#[derive(Clone)]
pub struct CommunitySalesSummary {
    pub storage_status: String,
    pub available: bool,
    pub accounting_date_utc: Option<String>,
    pub branch_time_zone: Option<String>,
    pub invoice_count: i64,
    pub total_minor: i64,
    pub cash_minor: i64,
    pub card_minor: i64,
    pub upi_minor: i64,
    pub refund_minor: i64,
    pub expense_minor: i64,
    pub discount_minor: i64,
    pub tax_minor: i64,
    pub currency_code: Option<String>,
    pub schema_version: i64,
    pub audit_event_count: i64,
    pub day_closed: bool,
    pub day_closed_at_utc: Option<String>,
    pub day_close_reason: Option<String>,
    pub recent_invoices: Vec<CommunityInvoiceView>,
    pub top_items: Vec<CommunityItemSalesView>,
}

/// Owner-only local audit view. It intentionally excludes audit payloads,
/// identifiers, hashes, and device metadata.
#[derive(Clone)]
pub struct CommunityAuditTimeline {
    pub storage_status: String,
    pub available: bool,
    pub events: Vec<CommunityAuditEventView>,
}

#[derive(Clone)]
pub struct CommunityAuditEventView {
    pub sequence: i64,
    pub event_type: String,
    pub occurred_at_utc: String,
}

/// Owner-only local sync-queue view. It intentionally excludes payloads,
/// hashes, device identifiers, actor identifiers, and correlation material.
#[derive(Clone)]
pub struct CommunitySyncQueue {
    pub storage_status: String,
    pub available: bool,
    pub pending_count: i64,
    pub operations: Vec<CommunitySyncQueueItemView>,
}

#[derive(Clone)]
pub struct CommunitySyncQueueItemView {
    pub sequence: i64,
    pub event_type: String,
    pub entity_type: String,
    pub created_at_utc: String,
}

#[derive(Clone)]
pub struct CommunityInvoiceView {
    pub invoice_id: String,
    pub invoice_number: i64,
    pub total_minor: i64,
    pub currency_code: String,
    pub finalized_at_utc: String,
    pub payment_method: String,
}

/// Receipt-safe detail projection loaded from immutable financial snapshots.
/// It deliberately contains no current catalogue, database, or credential
/// information.
#[derive(Clone)]
pub struct CommunityInvoiceDetail {
    pub storage_status: String,
    pub available: bool,
    pub invoice_id: Option<String>,
    pub invoice_number: Option<i64>,
    pub fulfillment: Option<String>,
    pub subtotal_minor: Option<i64>,
    pub discount_minor: Option<i64>,
    pub tax_minor: Option<i64>,
    pub total_minor: Option<i64>,
    pub refunded_minor: Option<i64>,
    pub currency_code: Option<String>,
    pub finalized_at_utc: Option<String>,
    pub lines: Vec<CommunityInvoiceLineView>,
    pub payments: Vec<CommunityInvoicePaymentView>,
}

#[derive(Clone)]
pub struct CommunityInvoiceLineView {
    pub display_name: String,
    pub modifier_names: Vec<String>,
    pub quantity: i64,
    pub unit_price_minor: i64,
    pub line_total_minor: i64,
}

#[derive(Clone)]
pub struct CommunityInvoicePaymentView {
    pub payment_method: String,
    pub amount_minor: i64,
}

#[derive(Clone)]
pub struct CommunityItemSalesView {
    pub display_name: String,
    pub quantity: i64,
    pub gross_total_minor: i64,
    pub currency_code: String,
}

/// A stock view derived only from immutable local ledger movements.
#[derive(Clone)]
pub struct CommunityInventoryItemView {
    pub product_id: String,
    pub display_name: String,
    pub tracked: bool,
    pub balance: i64,
    pub low_stock_threshold: Option<i64>,
    pub low_stock: bool,
}

#[derive(Clone)]
pub struct CommunityInventoryWorkspace {
    pub storage_status: String,
    pub available: bool,
    pub items: Vec<CommunityInventoryItemView>,
}

#[derive(Clone)]
pub struct CommunityExpenseView {
    pub expense_id: String,
    pub category: String,
    pub description: String,
    pub amount_minor: i64,
    pub currency_code: String,
    pub payment_method: String,
    pub incurred_at_utc: String,
}

#[derive(Clone)]
pub struct CommunityExpensesWorkspace {
    pub storage_status: String,
    pub available: bool,
    pub currency_code: Option<String>,
    pub total_minor: i64,
    pub expenses: Vec<CommunityExpenseView>,
}

#[derive(Clone)]
pub struct CommunityCashDrawerResult {
    pub storage_status: String,
    pub completed: bool,
    pub session_id: Option<String>,
    pub expected_cash_minor: i64,
    pub counted_cash_minor: i64,
    pub variance_minor: i64,
}

#[derive(Clone)]
pub struct CommunityOpenCashDrawer {
    pub session_id: String,
    pub opening_cash_minor: i64,
    pub currency_code: String,
}

/// A narrow local-auth projection for the Flutter lock screen. It contains no
/// PIN digest, session timestamp, or authentication-attempt history.
#[derive(Clone)]
pub struct CommunityStaffView {
    pub staff_id: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    pub pin_configured: bool,
}

#[derive(Clone)]
pub struct CommunityStaffSecurity {
    pub storage_status: String,
    pub available: bool,
    pub owner_pin_setup_required: bool,
    pub active_staff_id: Option<String>,
    pub active_staff_name: Option<String>,
    pub active_staff_role: Option<String>,
    pub staff: Vec<CommunityStaffView>,
}

#[derive(Clone)]
pub struct CommunityDraftOrderResult {
    pub storage_status: String,
    pub saved: bool,
    pub draft_order_id: Option<String>,
    pub revision: i64,
}

#[flutter_rust_bridge::frb(sync)]
pub fn local_core_status() -> String {
    let mode_status = match ros_storage::LOCAL_BUILD_MODE {
        "development" => "Development mode • isolated local test data",
        "release" => "Release mode • production local data boundary",
        _ => "Local build mode unavailable",
    };

    format!(
        "{} • {} • {} • contract v{}",
        mode_status,
        ros_core::initial_core_status().summary(),
        ros_storage::ENCRYPTED_STORAGE_ENGINE,
        ros_bridge::CLIENT_CORE_CONTRACT_VERSION,
    )
}

/// Prepares the encrypted local database without exposing its key to Flutter.
/// This is deliberately asynchronous at the bridge boundary: OS credential
/// stores and first-open migrations may block briefly and must not freeze the
/// cashier-facing Dart UI.
pub fn bootstrap_local_storage(application_support_directory: String) -> String {
    let started = std::time::Instant::now();
    let status = match open_community_database(&application_support_directory) {
        Ok(database) => match database.is_community_provisioned() {
            Ok(true) => "Encrypted local storage ready • restaurant setup complete".to_owned(),
            Ok(false) => "Encrypted local storage ready • restaurant setup required".to_owned(),
            Err(_) => {
                "Local storage needs attention • setup state could not be verified".to_owned()
            }
        },
        Err(error) => error,
    };
    let outcome = if status.to_lowercase().contains("attention") {
        ros_diagnostics::DiagnosticOutcome::Failed
    } else {
        ros_diagnostics::DiagnosticOutcome::Ok
    };
    let detail = if status.contains("setup complete") {
        Some("setup_complete")
    } else if status.contains("setup required") {
        Some("setup_required")
    } else if status.contains("secure device key") {
        Some("secret_missing")
    } else if status.contains("secure device storage") {
        Some("secure_store_unavailable")
    } else if status.contains("attention") {
        Some("open_failed")
    } else {
        None
    };
    record_diagnostic(
        &application_support_directory,
        "bootstrap_open",
        ros_diagnostics::DiagnosticComponent::Bootstrap,
        outcome,
        Some(started.elapsed().as_millis() as u64),
        detail,
    );
    status
}

/// Loads the Rust-owned local staff state. The lock UI may select a staff
/// record but never supplies a role or actor identity for a later mutation.
pub fn load_community_staff_security(
    application_support_directory: String,
) -> CommunityStaffSecurity {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_staff_security(status),
    };
    if !matches!(database.is_community_provisioned(), Ok(true)) {
        return CommunityStaffSecurity {
            storage_status: "Complete restaurant setup before configuring staff security"
                .to_owned(),
            available: false,
            owner_pin_setup_required: false,
            active_staff_id: None,
            active_staff_name: None,
            active_staff_role: None,
            staff: Vec::new(),
        };
    }
    let state = match database.local_staff_security_state() {
        Ok(state) => state,
        Err(_) => {
            return unavailable_staff_security(
                "Staff security needs attention • local state could not be read".to_owned(),
            )
        }
    };
    let staff = match database.list_local_staff() {
        Ok(staff) => staff,
        Err(_) => {
            return unavailable_staff_security(
                "Staff security needs attention • local accounts could not be read".to_owned(),
            )
        }
    };
    let active_staff = state.active_staff();
    CommunityStaffSecurity {
        storage_status: if state.owner_pin_setup_required() {
            "Owner PIN setup required before restaurant operations".to_owned()
        } else if let Some(staff) = active_staff {
            format!("Unlocked as {}", staff.display_name())
        } else {
            "Staff unlock required before restaurant operations".to_owned()
        },
        available: true,
        owner_pin_setup_required: state.owner_pin_setup_required(),
        active_staff_id: active_staff.map(|staff| staff.staff_id().to_string()),
        active_staff_name: active_staff.map(|staff| staff.display_name().to_owned()),
        active_staff_role: active_staff.map(|staff| staff.role().as_str().to_owned()),
        staff: staff
            .into_iter()
            .map(|staff| CommunityStaffView {
                staff_id: staff.staff_id().to_string(),
                display_name: staff.display_name().to_owned(),
                role: staff.role().as_str().to_owned(),
                active: staff.is_active(),
                pin_configured: staff.has_pin(),
            })
            .collect(),
    }
}

/// Sets the owner PIN once, then creates the first short-lived owner session
/// so onboarding can continue without a second plaintext PIN prompt.
pub fn configure_community_owner_pin(
    application_support_directory: String,
    pin: String,
) -> CommunityStaffSecurity {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "owner_pin_configure",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_staff_security(status);
        }
    };
    let owner_context = match database.community_owner_context() {
        Ok(context) => context,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "owner_pin_configure",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("setup_required"),
            );
            return unavailable_staff_security(
                "Staff security needs attention • restaurant setup is required".to_owned(),
            );
        }
    };
    if database.set_initial_owner_pin(&pin).is_err()
        || database
            .unlock_local_staff(owner_context.actor_id(), &pin)
            .is_err()
    {
        record_diagnostic(
            &application_support_directory,
            "owner_pin_configure",
            ros_diagnostics::DiagnosticComponent::Bridge,
            ros_diagnostics::DiagnosticOutcome::Denied,
            Some(started.elapsed().as_millis() as u64),
            Some("credential_rejected"),
        );
        return unavailable_staff_security(
            "Staff security needs attention • choose a six to twelve digit owner PIN".to_owned(),
        );
    }
    record_diagnostic(
        &application_support_directory,
        "owner_pin_configure",
        ros_diagnostics::DiagnosticComponent::Bridge,
        ros_diagnostics::DiagnosticOutcome::Ok,
        Some(started.elapsed().as_millis() as u64),
        None,
    );
    load_community_staff_security(application_support_directory)
}

pub fn unlock_community_staff(
    application_support_directory: String,
    staff_id: String,
    pin: String,
) -> CommunityStaffSecurity {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "staff_unlock",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_staff_security(status);
        }
    };
    let staff_id = match ros_core::EntityId::parse(&staff_id) {
        Ok(staff_id) => staff_id,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "staff_unlock",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("staff_unavailable"),
            );
            return unavailable_staff_security(
                "Staff security needs attention • the selected staff account is unavailable"
                    .to_owned(),
            );
        }
    };
    if database.unlock_local_staff(&staff_id, &pin).is_err() {
        record_diagnostic(
            &application_support_directory,
            "staff_unlock",
            ros_diagnostics::DiagnosticComponent::Bridge,
            ros_diagnostics::DiagnosticOutcome::Denied,
            Some(started.elapsed().as_millis() as u64),
            Some("credential_rejected"),
        );
        return unavailable_staff_security(
            "Staff security needs attention • PIN could not be verified".to_owned(),
        );
    }
    record_diagnostic(
        &application_support_directory,
        "staff_unlock",
        ros_diagnostics::DiagnosticComponent::Bridge,
        ros_diagnostics::DiagnosticOutcome::Ok,
        Some(started.elapsed().as_millis() as u64),
        None,
    );
    load_community_staff_security(application_support_directory)
}

pub fn lock_community_staff(application_support_directory: String) -> CommunityStaffSecurity {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "staff_lock",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_staff_security(status);
        }
    };
    if database.lock_local_staff().is_err() {
        record_diagnostic(
            &application_support_directory,
            "staff_lock",
            ros_diagnostics::DiagnosticComponent::Bridge,
            ros_diagnostics::DiagnosticOutcome::Failed,
            Some(started.elapsed().as_millis() as u64),
            Some("lock_failed"),
        );
        return unavailable_staff_security(
            "Staff security needs attention • the device could not be locked".to_owned(),
        );
    }
    record_diagnostic(
        &application_support_directory,
        "staff_lock",
        ros_diagnostics::DiagnosticComponent::Bridge,
        ros_diagnostics::DiagnosticOutcome::Ok,
        Some(started.elapsed().as_millis() as u64),
        None,
    );
    load_community_staff_security(application_support_directory)
}

/// Owner-only Community staff enrollment. A role is parsed in Rust and the
/// active local session—not Flutter—supplies the acting owner identity.
pub fn create_community_staff(
    application_support_directory: String,
    display_name: String,
    role: String,
    pin: String,
) -> CommunityStaffSecurity {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_staff_security(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • unlock as the owner before managing accounts".to_owned(),
            )
        }
    };
    let role = match role.as_str() {
        "manager" => ros_core::ActorRole::Manager,
        "cashier" => ros_core::ActorRole::Cashier,
        "kitchen" => ros_core::ActorRole::Kitchen,
        _ => {
            return unavailable_staff_security(
                "Staff needs attention • choose manager, cashier, or kitchen".to_owned(),
            )
        }
    };
    if database
        .create_local_staff(&display_name, role, &pin, &context)
        .is_err()
    {
        return unavailable_staff_security(
            "Staff needs attention • the account could not be created".to_owned(),
        );
    }
    load_community_staff_security(application_support_directory)
}

/// Owner-only, append-only change of a non-owner staff member's effective
/// role. Flutter supplies neither authority nor a mutable staff record.
pub fn change_community_staff_role(
    application_support_directory: String,
    staff_id: String,
    role: String,
    reason: String,
) -> CommunityStaffSecurity {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_staff_security(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • unlock as the owner before managing accounts".to_owned(),
            )
        }
    };
    let staff_id = match ros_core::EntityId::parse(&staff_id) {
        Ok(staff_id) => staff_id,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • the selected account is unavailable".to_owned(),
            )
        }
    };
    let role = match role.as_str() {
        "manager" => ros_core::ActorRole::Manager,
        "cashier" => ros_core::ActorRole::Cashier,
        "kitchen" => ros_core::ActorRole::Kitchen,
        _ => {
            return unavailable_staff_security(
                "Staff needs attention • choose manager, cashier, or kitchen".to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • explain the role change".to_owned(),
            )
        }
    };
    if database
        .change_local_staff_role(&staff_id, role, &reason, &context)
        .is_err()
    {
        return unavailable_staff_security(
            "Staff needs attention • the role could not be changed".to_owned(),
        );
    }
    load_community_staff_security(application_support_directory)
}

pub fn rotate_community_staff_pin(
    application_support_directory: String,
    staff_id: String,
    pin: String,
) -> CommunityStaffSecurity {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_staff_security(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • unlock as the owner before managing accounts".to_owned(),
            )
        }
    };
    let staff_id = match ros_core::EntityId::parse(&staff_id) {
        Ok(staff_id) => staff_id,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • the selected account is unavailable".to_owned(),
            )
        }
    };
    if database
        .rotate_local_staff_pin(&staff_id, &pin, &context)
        .is_err()
    {
        return unavailable_staff_security(
            "Staff needs attention • the PIN could not be updated".to_owned(),
        );
    }
    load_community_staff_security(application_support_directory)
}

pub fn revoke_community_staff(
    application_support_directory: String,
    staff_id: String,
    reason: String,
) -> CommunityStaffSecurity {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_staff_security(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • unlock as the owner before managing accounts".to_owned(),
            )
        }
    };
    let staff_id = match ros_core::EntityId::parse(&staff_id) {
        Ok(staff_id) => staff_id,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • the selected account is unavailable".to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return unavailable_staff_security(
                "Staff needs attention • a clear revocation reason is required".to_owned(),
            )
        }
    };
    if database
        .revoke_local_staff(&staff_id, &reason, &context)
        .is_err()
    {
        return unavailable_staff_security(
            "Staff needs attention • the account could not be revoked".to_owned(),
        );
    }
    load_community_staff_security(application_support_directory)
}

pub fn load_community_workspace(application_support_directory: String) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };

    match database.is_community_provisioned() {
        Ok(false) => CommunityWorkspace {
            storage_status: "Encrypted local storage ready • restaurant setup required".to_owned(),
            setup_required: true,
            branch_name: None,
            categories: Vec::new(),
            products: Vec::new(),
            customers: Vec::new(),
            open_drafts: Vec::new(),
            kitchen_tickets: Vec::new(),
        },
        Err(_) => unavailable_workspace(
            "Local storage needs attention • setup state could not be verified".to_owned(),
        ),
        Ok(true) => {
            // A provisioned workspace is sensitive operational data. Never
            // marshal it over FFI merely because the encrypted database can be
            // opened: Rust must first resolve an unexpired local staff session.
            let context = match database.community_active_staff_context() {
                Ok(context) => context,
                Err(_) => {
                    return locked_workspace(
                        "Staff unlock required before restaurant data can be loaded".to_owned(),
                    )
                }
            };
            let can_operate_counter = context.actor_role() != ros_core::ActorRole::Kitchen;
            let can_operate_kitchen = context.actor_role() != ros_core::ActorRole::Cashier;

            match database.community_branch() {
                Ok(branch) => {
                    let categories = match database.list_active_categories(branch.branch_id()) {
                        Ok(categories) => categories,
                        Err(_) => {
                            return unavailable_workspace(
                                "Local storage needs attention • categories could not be loaded"
                                    .to_owned(),
                            );
                        }
                    };
                    let products = match database.list_catalog_products(branch.branch_id()) {
                        Ok(products) => products,
                        Err(_) => {
                            return unavailable_workspace(
                                "Local storage needs attention • products could not be loaded"
                                    .to_owned(),
                            );
                        }
                    };
                    let tax_treatments = match database
                        .list_product_tax_treatments(branch.branch_id())
                    {
                        Ok(treatments) => treatments,
                        Err(_) => {
                            return unavailable_workspace(
                                    "Local storage needs attention • product tax treatments could not be loaded"
                                        .to_owned(),
                                );
                        }
                    };
                    let customers = match database.list_active_customers(branch.branch_id()) {
                        Ok(customers) => customers,
                        Err(_) => {
                            return unavailable_workspace(
                                "Local storage needs attention • customers could not be loaded"
                                    .to_owned(),
                            );
                        }
                    };
                    let product_images =
                        match database.list_catalog_product_images(branch.branch_id()) {
                            Ok(images) => images
                                .into_iter()
                                .map(|image| (image.product_id().to_string(), image))
                                .collect::<std::collections::HashMap<_, _>>(),
                            Err(_) => {
                                return unavailable_workspace(
                            "Local storage needs attention • menu images could not be loaded"
                                .to_owned(),
                        );
                            }
                        };
                    let product_modifier_options = match database
                    .list_catalog_product_modifier_options(branch.branch_id())
                {
                    Ok(options) => options.into_iter().fold(
                        std::collections::HashMap::<String, Vec<CommunityModifierOptionView>>::new(
                        ),
                        |mut options_by_product, option| {
                            options_by_product
                                .entry(option.product_id().to_string())
                                .or_default()
                                .push(CommunityModifierOptionView {
                                    modifier_option_id: option.modifier_option_id().to_string(),
                                    display_name: option.display_name().display().to_owned(),
                                    price_delta_minor: option.price_delta().minor_units(),
                                    currency_code: option.price_delta().currency().to_owned(),
                                    revision: option.revision(),
                                    archived: option.archived(),
                                });
                            options_by_product
                        },
                    ),
                    Err(_) => {
                        return unavailable_workspace(
                            "Local storage needs attention • menu modifiers could not be loaded"
                                .to_owned(),
                        )
                    }
                };
                    let mut product_images = product_images;
                    let mut product_modifier_options = product_modifier_options;
                    let open_drafts = match database.list_open_draft_orders(branch.branch_id()) {
                        Ok(drafts) => drafts
                            .into_iter()
                            .map(|draft| CommunityDraftOrderView {
                                draft_order_id: draft.draft().draft_order_id().to_string(),
                                fulfillment: draft.draft().fulfillment().as_str().to_owned(),
                                draft_state: draft.draft().state().to_owned(),
                                table_name: draft.draft().table_name().map(ToOwned::to_owned),
                                kitchen_note: draft.draft().kitchen_note().map(ToOwned::to_owned),
                                revision: draft.draft().revision(),
                                subtotal_minor: draft.draft().subtotal().minor_units(),
                                currency_code: draft.draft().subtotal().currency().to_owned(),
                                line_count: draft.draft().line_count(),
                                lines: draft
                                    .lines()
                                    .iter()
                                    .map(|line| CommunityCartLine {
                                        product_id: line.product_id().to_string(),
                                        quantity: line.quantity(),
                                        modifier_option_ids: line
                                            .modifier_option_ids()
                                            .iter()
                                            .map(ToString::to_string)
                                            .collect(),
                                    })
                                    .collect(),
                            })
                            .collect(),
                        Err(_) => {
                            return unavailable_workspace(
                                "Local storage needs attention • open orders could not be loaded"
                                    .to_owned(),
                            )
                        }
                    };
                    let kitchen_tickets = match database
                        .list_active_kitchen_tickets(branch.branch_id())
                    {
                        Ok(tickets) => {
                            tickets
                                .into_iter()
                                .map(|ticket| {
                                    let values: Vec<serde_json::Value> =
                                        serde_json::from_str(ticket.line_snapshot_json())
                                            .unwrap_or_default();
                                    CommunityKitchenTicketView {
                                        ticket_id: ticket.ticket_id().to_string(),
                                        table_label: ticket.table_label().map(ToOwned::to_owned),
                                        state: ticket.state().to_owned(),
                                        revision: ticket.revision(),
                                        cancellation_pending: ticket.cancellation_pending(),
                                        kitchen_note: ticket.kitchen_note().map(ToOwned::to_owned),
                                        lines: values
                                            .into_iter()
                                            .filter_map(|line| {
                                                Some(CommunityKitchenTicketLineView {
                                                    display_name: line
                                                        .get("product_name_snapshot")?
                                                        .as_str()?
                                                        .to_owned(),
                                                    modifier_names: line
                                                        .get("modifier_snapshot")
                                                        .and_then(serde_json::Value::as_array)
                                                        .map(|modifiers| {
                                                            modifiers
                                                                .iter()
                                                                .filter_map(|modifier| {
                                                                    modifier
                                                                .get("display_name_snapshot")
                                                                .and_then(serde_json::Value::as_str)
                                                                .map(ToOwned::to_owned)
                                                                })
                                                                .collect()
                                                        })
                                                        .unwrap_or_default(),
                                                    quantity: line.get("quantity")?.as_i64()?,
                                                })
                                            })
                                            .collect(),
                                    }
                                })
                                .collect()
                        }
                        Err(_) => return unavailable_workspace(
                            "Local storage needs attention • kitchen tickets could not be loaded"
                                .to_owned(),
                        ),
                    };

                    CommunityWorkspace {
                        storage_status: "Saved locally • encrypted database ready".to_owned(),
                        setup_required: false,
                        branch_name: can_operate_counter
                            .then(|| branch.display_name().display().to_owned()),
                        categories: if can_operate_counter {
                            categories
                                .into_iter()
                                .map(|category| CommunityCategoryView {
                                    category_id: category.category_id().to_string(),
                                    display_name: category.display_name().display().to_owned(),
                                    revision: category.revision(),
                                })
                                .collect()
                        } else {
                            Vec::new()
                        },
                        products: if can_operate_counter {
                            products
                                .into_iter()
                                .map(|product| {
                                    let product_id = product.product_id().to_string();
                                    let image = product_images.remove(&product_id);
                                    CommunityProductView {
                                        category_id: product.category_id().map(ToString::to_string),
                                        display_name: product.display_name().display().to_owned(),
                                        unit_price_minor: product.unit_price().minor_units(),
                                        currency_code: product.unit_price().currency().to_owned(),
                                        revision: product.revision(),
                                        is_available: product.is_available(),
                                        tax_treatment: tax_treatments
                                            .get(product.product_id())
                                            .cloned()
                                            .unwrap_or_else(|| "no_tax".to_owned()),
                                        image_asset_key: image.as_ref().and_then(|image| {
                                            image.asset_key().map(ToOwned::to_owned)
                                        }),
                                        image_bytes: image.as_ref().and_then(|image| {
                                            image.image_bytes().map(ToOwned::to_owned)
                                        }),
                                        modifier_options: product_modifier_options
                                            .remove(&product_id)
                                            .unwrap_or_default(),
                                        product_id,
                                    }
                                })
                                .collect()
                        } else {
                            Vec::new()
                        },
                        customers: if can_operate_counter {
                            customers
                                .into_iter()
                                .map(|customer| CommunityCustomerView {
                                    customer_id: customer.customer_id().to_string(),
                                    display_name: customer.display_name().to_owned(),
                                    phone_number: customer.phone_number().map(ToOwned::to_owned),
                                    email_address: customer.email_address().map(ToOwned::to_owned),
                                    marketing_consent: customer.marketing_consent(),
                                    revision: customer.revision(),
                                })
                                .collect()
                        } else {
                            Vec::new()
                        },
                        open_drafts: if can_operate_counter {
                            open_drafts
                        } else {
                            Vec::new()
                        },
                        kitchen_tickets: if can_operate_kitchen {
                            kitchen_tickets
                        } else {
                            Vec::new()
                        },
                    }
                }
                Err(_) => unavailable_workspace(
                    "Local storage needs attention • Community setup could not be loaded"
                        .to_owned(),
                ),
            }
        }
    }
}

pub fn load_community_sales_summary(
    application_support_directory: String,
    accounting_date_utc: Option<String>,
) -> CommunitySalesSummary {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_sales_summary(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_sales_summary(
                "Reports need attention • unlock an owner or manager session first".to_owned(),
            )
        }
    };
    if !matches!(
        context.actor_role(),
        ros_core::ActorRole::Owner | ros_core::ActorRole::Manager
    ) {
        return unavailable_sales_summary(
            "Reports need attention • this report is available only to owner or manager sessions"
                .to_owned(),
        );
    }
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return unavailable_sales_summary(
                "Reports need attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let integrity = match database.verify_local_integrity() {
        Ok(report) => {
            record_diagnostic(
                &application_support_directory,
                "storage_integrity",
                ros_diagnostics::DiagnosticComponent::Storage,
                ros_diagnostics::DiagnosticOutcome::Ok,
                None,
                None,
            );
            report
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "storage_integrity",
                ros_diagnostics::DiagnosticComponent::Storage,
                ros_diagnostics::DiagnosticOutcome::Failed,
                None,
                Some("integrity_failed"),
            );
            return unavailable_sales_summary(
                "Reports need attention • local integrity verification did not pass".to_owned(),
            );
        }
    };
    let report_day = match accounting_date_utc {
        Some(day) => day,
        None => match database.current_accounting_date_utc() {
            Ok(day) => day,
            Err(_) => {
                return unavailable_sales_summary(
                    "Reports need attention • the UTC accounting day could not be resolved"
                        .to_owned(),
                )
            }
        },
    };
    let recent_invoices = match database.list_recent_invoices(branch.branch_id(), 20, &report_day) {
        Ok(invoices) => invoices
            .into_iter()
            .map(|invoice| CommunityInvoiceView {
                invoice_id: invoice.invoice_id().to_string(),
                invoice_number: invoice.invoice_number(),
                total_minor: invoice.total_minor(),
                currency_code: invoice.currency_code().to_owned(),
                finalized_at_utc: invoice.finalized_at_utc().to_owned(),
                payment_method: invoice.payment_method().to_owned(),
            })
            .collect(),
        Err(_) => {
            return unavailable_sales_summary(
                "Reports need attention • local invoice records could not be read".to_owned(),
            )
        }
    };
    let top_items = match database.list_top_selling_items(branch.branch_id(), 20, &report_day) {
        Ok(items) => items
            .into_iter()
            .map(|item| CommunityItemSalesView {
                display_name: item.display_name().to_owned(),
                quantity: item.quantity(),
                gross_total_minor: item.gross_total_minor(),
                currency_code: item.currency_code().to_owned(),
            })
            .collect(),
        Err(_) => {
            return unavailable_sales_summary(
                "Reports need attention • local item sales could not be read".to_owned(),
            )
        }
    };
    match database.local_sales_summary(branch.branch_id(), &report_day) {
        Ok(summary) => {
            let day_close = match database.accounting_day_close(branch.branch_id(), &report_day) {
                Ok(value) => value,
                Err(_) => {
                    return unavailable_sales_summary(
                        "Reports need attention • accounting day close state could not be read"
                            .to_owned(),
                    )
                }
            };
            CommunitySalesSummary {
                storage_status: format!(
                    "Local report verified for {report_day} (UTC) from finalized invoices"
                ),
                available: true,
                accounting_date_utc: Some(report_day),
                branch_time_zone: Some(branch.time_zone().to_owned()),
                invoice_count: summary.invoice_count(),
                total_minor: summary.total_minor(),
                cash_minor: summary.cash_minor(),
                card_minor: summary.card_minor(),
                upi_minor: summary.upi_minor(),
                refund_minor: summary.refund_minor(),
                expense_minor: summary.expense_minor(),
                discount_minor: summary.discount_minor(),
                tax_minor: summary.tax_minor(),
                currency_code: Some(summary.currency_code().to_owned()),
                schema_version: integrity.schema_version(),
                audit_event_count: integrity.audit_event_count(),
                day_closed: day_close.is_some(),
                day_closed_at_utc: day_close
                    .as_ref()
                    .map(|close| close.closed_at_utc().to_owned()),
                day_close_reason: day_close.map(|close| close.reason().to_owned()),
                recent_invoices,
                top_items,
            }
        }
        Err(_) => unavailable_sales_summary(
            "Reports need attention • local financial records could not be read".to_owned(),
        ),
    }
}

/// Prepares a bounded RFC-4180 CSV only after encrypted storage has verified
/// the active Owner session, database integrity, schema, foreign keys, audit
/// chains, and financial-fact consistency. Flutter must display a native save
/// dialog before these in-memory bytes leave the device.
pub fn prepare_community_financial_csv(
    application_support_directory: String,
) -> CommunityFinancialCsvExport {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_financial_csv_export(status),
    };

    match database.export_verified_community_financial_csv() {
        Ok(export) => match export.byte_length() {
            Ok(byte_length) => CommunityFinancialCsvExport {
                storage_status: format!(
                    "Verified financial CSV prepared locally • {} aggregate records • {} bytes",
                    export.record_count(),
                    byte_length
                ),
                available: true,
                csv_bytes: export.csv_bytes().to_vec(),
                record_count: export.record_count(),
                byte_length,
            },
            Err(_) => unavailable_financial_csv_export(
                "Financial export needs attention • the verified report could not be bounded safely"
                    .to_owned(),
            ),
        },
        Err(ros_storage::StorageError::StaffSessionRequired) => {
            unavailable_financial_csv_export(
                "Financial export needs attention • unlock the active owner session first"
                    .to_owned(),
            )
        }
        Err(ros_storage::StorageError::PermissionDenied) => unavailable_financial_csv_export(
            "Financial export needs attention • only the active owner may export financial data"
                .to_owned(),
        ),
        Err(ros_storage::StorageError::FinancialExportTooLarge) => {
            unavailable_financial_csv_export(
                "Financial export needs attention • local history exceeds this device-safe export bound"
                    .to_owned(),
            )
        }
        Err(
            ros_storage::StorageError::AuditChainInvalid(_)
            | ros_storage::StorageError::FinancialExportIntegrityMismatch
            | ros_storage::StorageError::IntegrityCheckFailed(_)
            | ros_storage::StorageError::SchemaContractRejected(_),
        ) => unavailable_financial_csv_export(
            "Financial export needs attention • local integrity verification did not pass"
                .to_owned(),
        ),
        Err(ros_storage::StorageError::BranchNotFound) => unavailable_financial_csv_export(
            "Financial export needs attention • restaurant setup is required".to_owned(),
        ),
        Err(_) => unavailable_financial_csv_export(
            "Financial export needs attention • no file was created".to_owned(),
        ),
    }
}

/// Loads the recent owner-visible audit timeline only after full local
/// integrity verification. Flutter receives a safe projection rather than raw
/// payloads, hash-chain material, credentials, or staff/device identifiers.
pub fn load_community_audit_timeline(
    application_support_directory: String,
) -> CommunityAuditTimeline {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_audit_timeline(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_audit_timeline(
                "Audit history needs attention • unlock the owner session first".to_owned(),
            )
        }
    };
    if database.verify_local_integrity().is_err() {
        return unavailable_audit_timeline(
            "Audit history needs attention • local integrity verification did not pass".to_owned(),
        );
    }
    match database.list_recent_audit_events(100, &context) {
        Ok(events) => CommunityAuditTimeline {
            storage_status: "Recent local audit history verified".to_owned(),
            available: true,
            events: events
                .into_iter()
                .map(|event| CommunityAuditEventView {
                    sequence: event.sequence(),
                    event_type: event.event_type().to_owned(),
                    occurred_at_utc: event.occurred_at_utc().to_owned(),
                })
                .collect(),
        },
        Err(_) => unavailable_audit_timeline(
            "Audit history needs attention • it is available only to the owner".to_owned(),
        ),
    }
}

/// Loads the local Professional sync outbox as a privacy-safe owner view.
/// This does not contact the cloud; it only surfaces how many immutable local
/// operations are still waiting for a future acknowledgement.
pub fn load_community_sync_queue(application_support_directory: String) -> CommunitySyncQueue {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_sync_queue(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_sync_queue(
                "Sync queue needs attention • unlock the owner session first".to_owned(),
            )
        }
    };
    if !matches!(context.actor_role(), ros_core::ActorRole::Owner) {
        return unavailable_sync_queue(
            "Sync queue needs attention • it is available only to the owner".to_owned(),
        );
    }
    if database.verify_local_integrity().is_err() {
        return unavailable_sync_queue(
            "Sync queue needs attention • local integrity verification did not pass".to_owned(),
        );
    }
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return unavailable_sync_queue(
                "Sync queue needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    match database.list_pending_sync_operations(branch.branch_id()) {
        Ok(operations) => {
            let pending_count = i64::try_from(operations.len()).unwrap_or(i64::MAX);
            CommunitySyncQueue {
                storage_status: if pending_count == 0 {
                    "Local sync queue is empty • nothing waiting to acknowledge".to_owned()
                } else {
                    format!(
                        "Local sync queue verified • {pending_count} operation{} waiting for acknowledgement",
                        if pending_count == 1 { "" } else { "s" }
                    )
                },
                available: true,
                pending_count,
                operations: operations
                    .into_iter()
                    .take(100)
                    .map(|operation| CommunitySyncQueueItemView {
                        sequence: operation.sequence(),
                        event_type: operation.event_type().to_owned(),
                        entity_type: operation.entity_type().to_owned(),
                        created_at_utc: operation.created_at_utc().to_owned(),
                    })
                    .collect(),
            }
        }
        Err(_) => unavailable_sync_queue(
            "Sync queue needs attention • pending operations could not be read".to_owned(),
        ),
    }
}

/// Reprints one historical receipt from immutable order, invoice, and payment
/// snapshots. Counter-capable staff can read only the requested receipt in
/// their active branch; the Flutter caller never queries SQLite directly.
pub fn load_community_invoice_detail(
    application_support_directory: String,
    invoice_id: String,
) -> CommunityInvoiceDetail {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_invoice_detail(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_invoice_detail(
                "Receipt needs attention • unlock an authorized staff session first".to_owned(),
            )
        }
    };
    let invoice_id = match ros_core::EntityId::parse(&invoice_id) {
        Ok(invoice_id) => invoice_id,
        Err(_) => {
            return unavailable_invoice_detail(
                "Receipt needs attention • the invoice could not be identified".to_owned(),
            )
        }
    };
    match database.load_invoice_detail(&invoice_id, &context) {
        Ok(detail) => CommunityInvoiceDetail {
            storage_status: "Receipt loaded from immutable local records".to_owned(),
            available: true,
            invoice_id: Some(detail.invoice_id().to_string()),
            invoice_number: Some(detail.invoice_number()),
            fulfillment: Some(detail.fulfillment().to_owned()),
            subtotal_minor: Some(detail.subtotal_minor()),
            discount_minor: Some(detail.discount_minor()),
            tax_minor: Some(detail.tax_minor()),
            total_minor: Some(detail.total_minor()),
            refunded_minor: Some(detail.refunded_minor()),
            currency_code: Some(detail.currency_code().to_owned()),
            finalized_at_utc: Some(detail.finalized_at_utc().to_owned()),
            lines: detail
                .lines()
                .iter()
                .map(|line| CommunityInvoiceLineView {
                    display_name: line.display_name().to_owned(),
                    modifier_names: line.modifier_names().to_vec(),
                    quantity: line.quantity(),
                    unit_price_minor: line.unit_price_minor(),
                    line_total_minor: line.line_total_minor(),
                })
                .collect(),
            payments: detail
                .payments()
                .iter()
                .map(|payment| CommunityInvoicePaymentView {
                    payment_method: payment.payment_method().to_owned(),
                    amount_minor: payment.amount_minor(),
                })
                .collect(),
        },
        Err(_) => unavailable_invoice_detail(
            "Receipt needs attention • it could not be loaded from local records".to_owned(),
        ),
    }
}

/// Loads branch inventory from the encrypted, append-only movement ledger.
/// An item is tracked only after its first explicit stock movement.
pub fn load_community_inventory(
    application_support_directory: String,
) -> CommunityInventoryWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_inventory(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • unlock an owner or manager session first".to_owned(),
            )
        }
    };
    if !matches!(
        context.actor_role(),
        ros_core::ActorRole::Owner | ros_core::ActorRole::Manager
    ) {
        return unavailable_inventory(
            "Inventory needs attention • this ledger is available only to owner or manager sessions"
                .to_owned(),
        );
    }
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let products = match database.list_catalog_products(branch.branch_id()) {
        Ok(products) => products,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • menu items could not be loaded".to_owned(),
            )
        }
    };
    let mut items = Vec::with_capacity(products.len());
    for product in products {
        let tracked = match database.is_inventory_tracked(branch.branch_id(), product.product_id())
        {
            Ok(tracked) => tracked,
            Err(_) => {
                return unavailable_inventory(
                    "Inventory needs attention • stock ledger could not be read".to_owned(),
                )
            }
        };
        let balance = match database.inventory_balance(branch.branch_id(), product.product_id()) {
            Ok(balance) => balance,
            Err(_) => {
                return unavailable_inventory(
                    "Inventory needs attention • stock balance could not be read".to_owned(),
                )
            }
        };
        let low_stock_threshold = match database
            .inventory_low_stock_threshold(branch.branch_id(), product.product_id())
        {
            Ok(threshold) => threshold,
            Err(_) => {
                return unavailable_inventory(
                    "Inventory needs attention • low-stock policy could not be read".to_owned(),
                )
            }
        };
        items.push(CommunityInventoryItemView {
            product_id: product.product_id().to_string(),
            display_name: product.display_name().display().to_owned(),
            tracked,
            balance,
            low_stock: tracked && low_stock_threshold.is_some_and(|threshold| balance <= threshold),
            low_stock_threshold,
        });
    }
    CommunityInventoryWorkspace {
        storage_status: "Local stock balances verified from immutable movements".to_owned(),
        available: true,
        items,
    }
}

/// Records one validated Community inventory command. Flutter supplies only
/// identity, movement, quantity, and rationale; Rust owns authority and data
/// validation before touching encrypted storage.
pub fn record_community_inventory_movement(
    application_support_directory: String,
    product_id: String,
    movement_type: String,
    quantity: i64,
    reason: Option<String>,
) -> CommunityInventoryWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_inventory(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • choose an active menu item".to_owned(),
            )
        }
    };
    let result = match movement_type.as_str() {
        "opening" => database.record_inventory_opening(&product_id, quantity, &context),
        "purchase" => database.record_inventory_purchase(&product_id, quantity, &context),
        "waste" | "adjustment" => {
            let reason = match reason
                .as_deref()
                .and_then(|value| ros_core::MutationReason::new(value).ok())
            {
                Some(reason) => reason,
                None => {
                    return unavailable_inventory(
                        "Inventory needs attention • add a clear reason".to_owned(),
                    )
                }
            };
            if movement_type == "waste" {
                database.record_inventory_waste(&product_id, quantity, &reason, &context)
            } else {
                database.record_inventory_adjustment(&product_id, quantity, &reason, &context)
            }
        }
        _ => {
            return unavailable_inventory(
                "Inventory needs attention • choose a supported movement".to_owned(),
            )
        }
    };
    match result {
        Ok(()) => {
            let mut workspace = load_community_inventory(application_support_directory);
            if workspace.available {
                workspace.storage_status =
                    "Inventory movement recorded locally • history retained".to_owned();
            }
            workspace
        }
        Err(_) => unavailable_inventory(
            "Inventory needs attention • the movement was not recorded".to_owned(),
        ),
    }
}

/// Appends an owner/manager-selected replenishment threshold for a tracked
/// product. A threshold affects only the low-stock indicator, never quantity.
pub fn set_community_inventory_low_stock_threshold(
    application_support_directory: String,
    product_id: String,
    threshold_quantity: i64,
    reason: String,
) -> CommunityInventoryWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_inventory(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • unlock an owner or manager session first".to_owned(),
            )
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • choose a tracked menu item".to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • explain the low-stock policy change".to_owned(),
            )
        }
    };
    match database.set_inventory_low_stock_threshold(
        &product_id,
        threshold_quantity,
        &reason,
        &context,
    ) {
        Ok(()) => {
            let mut workspace = load_community_inventory(application_support_directory);
            if workspace.available {
                workspace.storage_status =
                    "Low-stock threshold saved locally • history retained".to_owned();
            }
            workspace
        }
        Err(_) => unavailable_inventory(
            "Inventory needs attention • the low-stock threshold was not saved".to_owned(),
        ),
    }
}

/// Retains an explicit clear event for the current low-stock policy. This
/// removes the alert without mutating or deleting any previous threshold.
pub fn clear_community_inventory_low_stock_threshold(
    application_support_directory: String,
    product_id: String,
    reason: String,
) -> CommunityInventoryWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_inventory(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • unlock an owner or manager session first".to_owned(),
            )
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • choose a tracked menu item".to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return unavailable_inventory(
                "Inventory needs attention • explain why the alert is being cleared".to_owned(),
            )
        }
    };
    match database.clear_inventory_low_stock_threshold(&product_id, &reason, &context) {
        Ok(()) => {
            let mut workspace = load_community_inventory(application_support_directory);
            if workspace.available {
                workspace.storage_status =
                    "Low-stock threshold cleared locally • history retained".to_owned();
            }
            workspace
        }
        Err(_) => unavailable_inventory(
            "Inventory needs attention • the low-stock threshold was not cleared".to_owned(),
        ),
    }
}

pub fn load_community_expenses(
    application_support_directory: String,
) -> CommunityExpensesWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_expenses(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_expenses(
                "Expenses need attention • unlock an owner or manager session first".to_owned(),
            )
        }
    };
    if !matches!(
        context.actor_role(),
        ros_core::ActorRole::Owner | ros_core::ActorRole::Manager
    ) {
        return unavailable_expenses(
            "Expenses need attention • this ledger is available only to owner or manager sessions"
                .to_owned(),
        );
    }
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return unavailable_expenses(
                "Expenses need attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let expenses = match database.list_recent_expenses(branch.branch_id(), 50) {
        Ok(expenses) => expenses,
        Err(_) => {
            return unavailable_expenses(
                "Expenses need attention • local records could not be read".to_owned(),
            )
        }
    };
    let total_minor = expenses.iter().try_fold(0_i64, |total, expense| {
        total.checked_add(expense.amount_minor())
    });
    let total_minor = match total_minor {
        Some(total_minor) => total_minor,
        None => {
            return unavailable_expenses(
                "Expenses need attention • total was outside the supported range".to_owned(),
            )
        }
    };
    CommunityExpensesWorkspace {
        storage_status: "Local expenses verified from immutable records".to_owned(),
        available: true,
        currency_code: Some(branch.currency().to_owned()),
        total_minor,
        expenses: expenses
            .into_iter()
            .map(|expense| CommunityExpenseView {
                expense_id: expense.expense_id().to_string(),
                category: expense.category().to_owned(),
                description: expense.description().to_owned(),
                amount_minor: expense.amount_minor(),
                currency_code: expense.currency_code().to_owned(),
                payment_method: expense.payment_method().to_owned(),
                incurred_at_utc: expense.incurred_at_utc().to_owned(),
            })
            .collect(),
    }
}

pub fn record_community_expense(
    application_support_directory: String,
    category: String,
    description: String,
    amount_minor: i64,
    payment_method: String,
) -> CommunityExpensesWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_expenses(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_expenses(
                "Expenses need attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return unavailable_expenses(
                "Expenses need attention • branch could not be loaded".to_owned(),
            )
        }
    };
    let description = match ros_core::MutationReason::new(&description) {
        Ok(description) => description,
        Err(_) => {
            return unavailable_expenses(
                "Expenses need attention • add a clear description".to_owned(),
            )
        }
    };
    let amount = match ros_core::Money::new(amount_minor, branch.currency()) {
        Ok(amount) if amount.minor_units() > 0 => amount,
        _ => {
            return unavailable_expenses(
                "Expenses need attention • enter a positive amount".to_owned(),
            )
        }
    };
    let payment_method = match ros_core::PaymentMethod::parse(&payment_method) {
        Ok(payment_method) => payment_method,
        Err(_) => {
            return unavailable_expenses(
                "Expenses need attention • choose cash, card, or UPI".to_owned(),
            )
        }
    };
    match database.record_expense(&category, &description, &amount, payment_method, &context) {
        Ok(_) => {
            let mut workspace = load_community_expenses(application_support_directory);
            if workspace.available {
                workspace.storage_status = "Expense recorded locally • history retained".to_owned();
            }
            workspace
        }
        Err(_) => unavailable_expenses(
            "Expenses need attention • the expense was not recorded".to_owned(),
        ),
    }
}

pub fn open_community_cash_drawer(
    application_support_directory: String,
    opening_cash_minor: i64,
) -> CommunityCashDrawerResult {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_cash_drawer(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_cash_drawer(
                "Cash drawer needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    match database.open_cash_drawer(opening_cash_minor, &context) {
        Ok(session_id) => CommunityCashDrawerResult {
            storage_status: "Cash drawer opened locally • opening float preserved".to_owned(),
            completed: true,
            session_id: Some(session_id.to_string()),
            expected_cash_minor: opening_cash_minor,
            counted_cash_minor: 0,
            variance_minor: 0,
        },
        Err(_) => unavailable_cash_drawer(
            "Cash drawer needs attention • it may already be open".to_owned(),
        ),
    }
}

pub fn load_community_open_cash_drawer(
    application_support_directory: String,
) -> Option<CommunityOpenCashDrawer> {
    let database = open_community_database(&application_support_directory).ok()?;
    let context = database.community_active_staff_context().ok()?;
    if matches!(context.actor_role(), ros_core::ActorRole::Kitchen) {
        return None;
    }
    let branch = database.community_branch().ok()?;
    let session = database
        .current_open_cash_drawer(branch.branch_id())
        .ok()??;
    Some(CommunityOpenCashDrawer {
        session_id: session.session_id.to_string(),
        opening_cash_minor: session.opening_cash_minor,
        currency_code: session.currency_code,
    })
}

pub fn close_community_cash_drawer(
    application_support_directory: String,
    session_id: String,
    counted_cash_minor: i64,
) -> CommunityCashDrawerResult {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_cash_drawer(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_cash_drawer(
                "Cash drawer needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let session_id = match ros_core::EntityId::parse(&session_id) {
        Ok(value) => value,
        Err(_) => {
            return unavailable_cash_drawer(
                "Cash drawer needs attention • session could not be identified".to_owned(),
            )
        }
    };
    match database.close_cash_drawer(&session_id, counted_cash_minor, &context) {
        Ok(closure) => CommunityCashDrawerResult {
            storage_status: "Cash drawer closed locally • variance preserved".to_owned(),
            completed: true,
            session_id: Some(closure.session_id.to_string()),
            expected_cash_minor: closure.expected_cash_minor,
            counted_cash_minor: closure.counted_cash_minor,
            variance_minor: closure.variance_minor,
        },
        Err(_) => unavailable_cash_drawer(
            "Cash drawer needs attention • it could not be closed".to_owned(),
        ),
    }
}

/// Records an owner/manager-authorized refund as a new immutable fact. The
/// amount may be a full remaining balance or a controlled partial refund; Rust
/// revalidates the remaining invoice total and original tender allocations.
pub fn refund_community_invoice(
    application_support_directory: String,
    invoice_id: String,
    amount_minor: i64,
    reason: String,
    approver_staff_id: String,
    approver_pin: String,
    accounting_date_utc: Option<String>,
) -> CommunitySalesSummary {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_refund",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_sales_summary(status);
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_refund",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("session_required"),
            );
            return unavailable_sales_summary(
                "Refund needs attention • restaurant setup is required".to_owned(),
            );
        }
    };
    let invoice_id = match ros_core::EntityId::parse(&invoice_id) {
        Ok(invoice_id) => invoice_id,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_refund",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("target_unavailable"),
            );
            return unavailable_sales_summary(
                "Refund needs attention • invoice could not be identified".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_refund",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("reason_required"),
            );
            return unavailable_sales_summary(
                "Refund needs attention • add a clear reason for this correction".to_owned(),
            );
        }
    };
    let approver_actor_id = match ros_core::EntityId::parse(&approver_staff_id) {
        Ok(approver_actor_id) => approver_actor_id,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_refund",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("approver_required"),
            );
            return unavailable_sales_summary(
                "Refund needs attention • a second manager or owner must approve".to_owned(),
            );
        }
    };
    let approval = ros_storage::DualPersonApproval {
        approver_actor_id,
        approver_pin: &approver_pin,
    };
    match database.refund_invoice(&invoice_id, amount_minor, &reason, &context, &approval) {
        Ok(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_refund",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                Some(started.elapsed().as_millis() as u64),
                None,
            );
            let mut summary =
                load_community_sales_summary(application_support_directory, accounting_date_utc);
            if summary.available {
                summary.storage_status =
                    "Refund recorded locally • original invoice preserved".to_owned();
            }
            summary
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_refund",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("refund_failed"),
            );
            unavailable_sales_summary(
                "Refund needs attention • this invoice could not be refunded".to_owned(),
            )
        }
    }
}

pub fn create_community_local_backup(
    application_support_directory: String,
) -> CommunityBackupResult {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "backup_create",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_backup(status);
        }
    };
    // A backup contains the complete encrypted restaurant database. The UI
    // also hides this action from other roles, but the Rust bridge must enforce
    // the same owner-only boundary so a caller cannot bypass the presentation
    // layer.
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "backup_create",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("session_required"),
            );
            return unavailable_backup(
                "Backup needs attention • unlock the owner session first".to_owned(),
            );
        }
    };
    let file_name = format!("restaurant-os-backup-{}.db", ros_core::EntityId::new_v7());
    let destination = PathBuf::from(&application_support_directory)
        .join("verified-backups")
        .join(&file_name);
    match database.create_verified_local_backup(&destination, &context) {
        Ok(backup) => {
            record_diagnostic(
                &application_support_directory,
                "backup_create",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                Some(started.elapsed().as_millis() as u64),
                None,
            );
            CommunityBackupResult {
                storage_status: format!(
                    "Verified local backup created • schema v{} • {} bytes",
                    backup.schema_version(),
                    backup.byte_length()
                ),
                created: true,
                backup_file_name: Some(file_name),
                sha256: Some(backup.sha256().to_owned()),
            }
        }
        Err(ros_storage::StorageError::PermissionDenied) => {
            record_diagnostic(
                &application_support_directory,
                "backup_create",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("permission_denied"),
            );
            unavailable_backup(
                "Backup needs attention • only the active owner may create an encrypted backup"
                    .to_owned(),
            )
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "backup_create",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("create_failed"),
            );
            unavailable_backup("Backup needs attention • no existing data was changed".to_owned())
        }
    }
}

pub fn verify_community_local_backup(
    application_support_directory: String,
    backup_file_name: String,
) -> CommunityBackupResult {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "backup_verify",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_backup(status);
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "backup_verify",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("session_required"),
            );
            return unavailable_backup(
                "Backup needs attention • unlock the owner session first".to_owned(),
            );
        }
    };
    if !matches!(context.actor_role(), ros_core::ActorRole::Owner) {
        record_diagnostic(
            &application_support_directory,
            "backup_verify",
            ros_diagnostics::DiagnosticComponent::Bridge,
            ros_diagnostics::DiagnosticOutcome::Denied,
            Some(started.elapsed().as_millis() as u64),
            Some("permission_denied"),
        );
        return unavailable_backup(
            "Backup needs attention • only the active owner may verify an encrypted backup"
                .to_owned(),
        );
    }
    let path = PathBuf::from(&application_support_directory)
        .join("verified-backups")
        .join(&backup_file_name);
    match database.verify_local_backup(&path) {
        Ok(backup) => {
            record_diagnostic(
                &application_support_directory,
                "backup_verify",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                Some(started.elapsed().as_millis() as u64),
                None,
            );
            CommunityBackupResult {
                storage_status: format!(
                    "Verified local backup • schema v{} • {} bytes",
                    backup.schema_version(),
                    backup.byte_length()
                ),
                created: false,
                backup_file_name: Some(backup_file_name),
                sha256: Some(backup.sha256().to_owned()),
            }
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "backup_verify",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("verify_failed"),
            );
            unavailable_backup(
                "Backup needs attention • the selected snapshot could not be verified".to_owned(),
            )
        }
    }
}

pub fn restore_community_local_backup(
    application_support_directory: String,
    backup_file_name: String,
) -> CommunityBackupResult {
    let started = std::time::Instant::now();
    let support = PathBuf::from(&application_support_directory);
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "backup_restore",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_backup(status);
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "backup_restore",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("session_required"),
            );
            return unavailable_backup(
                "Restore needs attention • unlock the owner session first".to_owned(),
            );
        }
    };
    let source = support.join("verified-backups").join(&backup_file_name);
    let destination = support.join("restaurant-os.restored.db");
    match database.restore_verified_local_backup(&source, &destination, &context, false) {
        Ok(backup) => {
            record_diagnostic(
                &application_support_directory,
                "backup_restore",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                Some(started.elapsed().as_millis() as u64),
                None,
            );
            CommunityBackupResult {
                storage_status: format!(
                    "Verified backup restored beside live data • schema v{} • {} bytes • live database unchanged",
                    backup.schema_version(),
                    backup.byte_length()
                ),
                created: true,
                backup_file_name: Some("restaurant-os.restored.db".to_owned()),
                sha256: Some(backup.sha256().to_owned()),
            }
        }
        Err(ros_storage::StorageError::PermissionDenied) => {
            record_diagnostic(
                &application_support_directory,
                "backup_restore",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("permission_denied"),
            );
            unavailable_backup(
                "Restore needs attention • only the active owner may restore an encrypted backup"
                    .to_owned(),
            )
        }
        Err(ros_storage::StorageError::InvalidPersistedData(_)) => {
            record_diagnostic(
                &application_support_directory,
                "backup_restore",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("destination_rejected"),
            );
            unavailable_backup(
                "Restore needs attention • destination already exists or snapshot was rejected"
                    .to_owned(),
            )
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "backup_restore",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("restore_failed"),
            );
            unavailable_backup(
                "Restore needs attention • the selected snapshot could not be restored".to_owned(),
            )
        }
    }
}

pub fn void_community_invoice(
    application_support_directory: String,
    invoice_id: String,
    reason: String,
    approver_staff_id: String,
    approver_pin: String,
    accounting_date_utc: Option<String>,
) -> CommunitySalesSummary {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_void",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_sales_summary(status);
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_void",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("session_required"),
            );
            return unavailable_sales_summary(
                "Void needs attention • unlock an owner or manager session first".to_owned(),
            );
        }
    };
    let invoice_id = match ros_core::EntityId::parse(&invoice_id) {
        Ok(invoice_id) => invoice_id,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_void",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("target_unavailable"),
            );
            return unavailable_sales_summary(
                "Void needs attention • invoice could not be identified".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_void",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("reason_required"),
            );
            return unavailable_sales_summary(
                "Void needs attention • add a clear reason for this correction".to_owned(),
            );
        }
    };
    let approver_actor_id = match ros_core::EntityId::parse(&approver_staff_id) {
        Ok(approver_actor_id) => approver_actor_id,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_void",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("approver_required"),
            );
            return unavailable_sales_summary(
                "Void needs attention • a second manager or owner must approve".to_owned(),
            );
        }
    };
    let approval = ros_storage::DualPersonApproval {
        approver_actor_id,
        approver_pin: &approver_pin,
    };
    match database.void_invoice(&invoice_id, &reason, &context, &approval) {
        Ok(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_void",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                Some(started.elapsed().as_millis() as u64),
                None,
            );
            let mut summary =
                load_community_sales_summary(application_support_directory, accounting_date_utc);
            if summary.available {
                summary.storage_status =
                    "Invoice voided locally • original invoice preserved".to_owned();
            }
            summary
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "invoice_void",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("void_failed"),
            );
            unavailable_sales_summary(
                "Void needs attention • this invoice could not be voided".to_owned(),
            )
        }
    }
}

/// Closes one UTC accounting day with an immutable snapshot. Reopen is not
/// offered; founder policy is required before any dual-approval reopen path.
pub fn close_community_accounting_day(
    application_support_directory: String,
    accounting_date_utc: String,
    reason: String,
) -> CommunitySalesSummary {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "accounting_day_close",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_sales_summary(status);
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "accounting_day_close",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("session_required"),
            );
            return unavailable_sales_summary(
                "Day close needs attention • unlock an owner or manager session first".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "accounting_day_close",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("reason_required"),
            );
            return unavailable_sales_summary(
                "Day close needs attention • add a clear reason for this close".to_owned(),
            );
        }
    };
    match database.close_accounting_day(&accounting_date_utc, &reason, &context) {
        Ok(_) => {
            record_diagnostic(
                &application_support_directory,
                "accounting_day_close",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                Some(started.elapsed().as_millis() as u64),
                None,
            );
            let mut summary = load_community_sales_summary(
                application_support_directory,
                Some(accounting_date_utc),
            );
            if summary.available {
                summary.storage_status =
                    "Accounting day closed locally • reopen is not supported".to_owned();
            }
            summary
        }
        Err(ros_storage::StorageError::CatalogConflict) => {
            record_diagnostic(
                &application_support_directory,
                "accounting_day_close",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("already_closed"),
            );
            unavailable_sales_summary(
                "Day close needs attention • this UTC day is already closed".to_owned(),
            )
        }
        Err(ros_storage::StorageError::PermissionDenied) => {
            record_diagnostic(
                &application_support_directory,
                "accounting_day_close",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                Some(started.elapsed().as_millis() as u64),
                Some("permission_denied"),
            );
            unavailable_sales_summary(
                "Day close needs attention • only owner or manager sessions may close the day"
                    .to_owned(),
            )
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "accounting_day_close",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("close_failed"),
            );
            unavailable_sales_summary(
                "Day close needs attention • the accounting day could not be closed".to_owned(),
            )
        }
    }
}

/// Records an allow-listed UI breadcrumb. Unknown codes are rejected in Rust
/// and never written.
pub fn record_community_diagnostic_breadcrumb(
    application_support_directory: String,
    event_code: String,
    detail_code: Option<String>,
) -> bool {
    record_diagnostic(
        &application_support_directory,
        &event_code,
        ros_diagnostics::DiagnosticComponent::Ui,
        ros_diagnostics::DiagnosticOutcome::Ok,
        None,
        detail_code.as_deref(),
    )
}

/// Owner-only recent diagnostics projection. Payloads stay allow-listed codes.
pub fn load_community_diagnostics(
    application_support_directory: String,
) -> CommunityDiagnosticsWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_diagnostics(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_diagnostics(
                "Diagnostics need attention • unlock the owner session first".to_owned(),
            )
        }
    };
    if !matches!(context.actor_role(), ros_core::ActorRole::Owner) {
        return unavailable_diagnostics(
            "Diagnostics need attention • only the active owner may view local diagnostics"
                .to_owned(),
        );
    }
    let sink = match ros_diagnostics::LocalDiagnosticsSink::open(&application_support_directory) {
        Ok(sink) => sink,
        Err(_) => {
            return unavailable_diagnostics(
                "Diagnostics need attention • local diagnostic storage is unavailable".to_owned(),
            )
        }
    };
    match sink.recent_events(100) {
        Ok(events) => CommunityDiagnosticsWorkspace {
            storage_status: format!(
                "Local diagnostics ready • {} recent allow-listed event{}",
                events.len(),
                if events.len() == 1 { "" } else { "s" }
            ),
            available: true,
            events: events
                .into_iter()
                .map(|event| CommunityDiagnosticEventView {
                    occurred_at_utc: event.occurred_at_utc,
                    event_code: event.event_code,
                    component: diagnostic_component_label(event.component).to_owned(),
                    outcome: diagnostic_outcome_label(event.outcome).to_owned(),
                    duration_ms: event.duration_ms.map(|value| value as i64),
                    detail_code: event.detail_code,
                })
                .collect(),
        },
        Err(_) => unavailable_diagnostics(
            "Diagnostics need attention • local events could not be read".to_owned(),
        ),
    }
}

/// Owner-only redacted diagnostics pack for native save or voluntary share.
pub fn export_community_diagnostics_pack(
    application_support_directory: String,
) -> CommunityDiagnosticsPack {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_diagnostics_pack(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_diagnostics_pack(
                "Diagnostics need attention • unlock the owner session first".to_owned(),
            )
        }
    };
    if !matches!(context.actor_role(), ros_core::ActorRole::Owner) {
        return unavailable_diagnostics_pack(
            "Diagnostics need attention • only the active owner may export diagnostics".to_owned(),
        );
    }
    let sink = match ros_diagnostics::LocalDiagnosticsSink::open(&application_support_directory) {
        Ok(sink) => sink,
        Err(_) => {
            return unavailable_diagnostics_pack(
                "Diagnostics need attention • local diagnostic storage is unavailable".to_owned(),
            )
        }
    };
    match sink.export_pack() {
        Ok(pack) => {
            let _ = record_diagnostic(
                &application_support_directory,
                "diagnostics_export",
                ros_diagnostics::DiagnosticComponent::Share,
                ros_diagnostics::DiagnosticOutcome::Ok,
                None,
                None,
            );
            CommunityDiagnosticsPack {
                storage_status: format!(
                    "Redacted diagnostics pack ready • {} events • {} bytes",
                    pack.event_count,
                    pack.json_bytes.len()
                ),
                available: true,
                byte_length: pack.json_bytes.len() as i64,
                event_count: pack.event_count as i64,
                sha256: Some(pack.sha256),
                json_bytes: pack.json_bytes,
            }
        }
        Err(_) => unavailable_diagnostics_pack(
            "Diagnostics need attention • the redacted pack could not be prepared".to_owned(),
        ),
    }
}

/// Prepares an Owner-consented share pack. Upload is performed by Flutter only
/// after consent; Rust records the attempt outcome category supplied by Flutter.
pub fn prepare_community_diagnostics_share(
    application_support_directory: String,
    purpose: String,
) -> CommunityDiagnosticsShareResult {
    if purpose != "support_issue" && purpose != "product_improvement" {
        return CommunityDiagnosticsShareResult {
            storage_status: "Share needs attention • choose a valid share purpose".to_owned(),
            prepared: false,
            uploaded: false,
            sha256: None,
            event_count: 0,
            json_bytes: Vec::new(),
        };
    }
    let pack = export_community_diagnostics_pack(application_support_directory.clone());
    if !pack.available {
        return CommunityDiagnosticsShareResult {
            storage_status: pack.storage_status,
            prepared: false,
            uploaded: false,
            sha256: None,
            event_count: 0,
            json_bytes: Vec::new(),
        };
    }
    let _ = record_diagnostic(
        &application_support_directory,
        "diagnostics_share_attempted",
        ros_diagnostics::DiagnosticComponent::Share,
        ros_diagnostics::DiagnosticOutcome::Ok,
        None,
        Some(purpose.as_str()),
    );
    CommunityDiagnosticsShareResult {
        storage_status: format!(
            "Owner-consented pack prepared • purpose {purpose} • cloud upload happens only if you continue"
        ),
        prepared: true,
        uploaded: false,
        sha256: pack.sha256,
        event_count: pack.event_count,
        json_bytes: pack.json_bytes,
    }
}

/// Records whether a voluntary Flutter-side upload succeeded. Never uploads itself.
pub fn record_community_diagnostics_share_outcome(
    application_support_directory: String,
    uploaded: bool,
) -> bool {
    record_diagnostic(
        &application_support_directory,
        "diagnostics_share_attempted",
        ros_diagnostics::DiagnosticComponent::Share,
        if uploaded {
            ros_diagnostics::DiagnosticOutcome::Ok
        } else {
            ros_diagnostics::DiagnosticOutcome::Unavailable
        },
        None,
        Some(if uploaded {
            "upload_ok"
        } else {
            "upload_unavailable"
        }),
    )
}

/// Owner-only clear of local diagnostic files.
pub fn clear_community_diagnostics(application_support_directory: String) -> bool {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(_) => return false,
    };
    let Ok(context) = database.community_active_staff_context() else {
        return false;
    };
    if !matches!(context.actor_role(), ros_core::ActorRole::Owner) {
        return false;
    }
    let Ok(sink) = ros_diagnostics::LocalDiagnosticsSink::open(&application_support_directory)
    else {
        return false;
    };
    if sink.clear().is_err() {
        return false;
    }
    record_diagnostic(
        &application_support_directory,
        "diagnostics_cleared",
        ros_diagnostics::DiagnosticComponent::Share,
        ros_diagnostics::DiagnosticOutcome::Ok,
        None,
        None,
    )
}

pub fn complete_community_setup(
    application_support_directory: String,
    organization_name: String,
    branch_name: String,
    currency_code: String,
    time_zone: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let setup = match ros_core::CommunitySetup::new(
        &organization_name,
        &branch_name,
        &currency_code,
        &time_zone,
    ) {
        Ok(setup) => setup,
        Err(error) => {
            return workspace_with_error(
                &application_support_directory,
                format!("Setup needs attention • {error}"),
            );
        }
    };

    match database.provision_community(&setup) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Restaurant setup saved locally".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Setup needs attention • local restaurant setup could not be saved".to_owned(),
        ),
    }
}

pub fn create_community_category(
    application_support_directory: String,
    display_name: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Category needs attention • Community setup is required".to_owned(),
            );
        }
    };
    let command = match ros_core::CreateCategory::new(&display_name, 0) {
        Ok(command) => command,
        Err(error) => {
            return workspace_with_error(
                &application_support_directory,
                format!("Category needs attention • {error}"),
            );
        }
    };

    match database.create_category(&command, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Category saved locally".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Category needs attention • local changes could not be saved".to_owned(),
        ),
    }
}

/// Enrolls a customer as an active local profile. The active staff session
/// supplies the actor identity and role; the client cannot impersonate one.
pub fn create_community_customer(
    application_support_directory: String,
    display_name: String,
    phone_number: Option<String>,
    email_address: Option<String>,
    marketing_consent: bool,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Customer needs attention • unlock a counter staff session first".to_owned(),
            );
        }
    };
    match database.create_customer(
        &display_name,
        phone_number.as_deref(),
        email_address.as_deref(),
        marketing_consent,
        &context,
    ) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Customer saved locally • encrypted and audited".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Customer needs attention • check the name and contact details".to_owned(),
        ),
    }
}

/// Appends a corrected profile revision. The correction rationale is retained
/// in the local audit trail while prior profile facts remain immutable.
pub fn revise_community_customer(
    application_support_directory: String,
    customer_id: String,
    display_name: String,
    phone_number: Option<String>,
    email_address: Option<String>,
    marketing_consent: bool,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Customer needs attention • unlock as owner or manager to correct a profile"
                    .to_owned(),
            );
        }
    };
    let customer_id = match ros_core::EntityId::parse(&customer_id) {
        Ok(customer_id) => customer_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Customer needs attention • the selected customer is unavailable".to_owned(),
            );
        }
    };
    match database.revise_customer(
        &customer_id,
        ros_storage::CustomerProfileInput {
            display_name: &display_name,
            phone_number: phone_number.as_deref(),
            email_address: email_address.as_deref(),
            marketing_consent,
        },
        &reason,
        &context,
    ) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Customer correction saved locally • prior profile retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Customer needs attention • only an owner or manager can correct an active profile"
                .to_owned(),
        ),
    }
}

/// Appends an anonymized current profile. Existing invoices remain intact but
/// the record cannot be selected for a new sale.
pub fn anonymize_community_customer(
    application_support_directory: String,
    customer_id: String,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Customer needs attention • unlock as owner or manager to anonymize a profile"
                    .to_owned(),
            );
        }
    };
    let customer_id = match ros_core::EntityId::parse(&customer_id) {
        Ok(customer_id) => customer_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Customer needs attention • the selected customer is unavailable".to_owned(),
            );
        }
    };
    match database.anonymize_customer(&customer_id, &reason, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Customer anonymized locally • financial history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Customer needs attention • only an owner or manager can anonymize an active profile"
                .to_owned(),
        ),
    }
}

pub fn create_community_product(
    application_support_directory: String,
    display_name: String,
    category_id: String,
    unit_price_minor: i64,
    built_in_image_key: Option<String>,
    restaurant_image_bytes: Option<Vec<u8>>,
    gotigin_catalog_image: Option<GotiginCatalogMenuImageSelection>,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • Community setup is required".to_owned(),
            );
        }
    };
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • Community setup could not be loaded".to_owned(),
            );
        }
    };
    let category_id = match ros_core::EntityId::parse(&category_id) {
        Ok(category_id) => category_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • choose an active category".to_owned(),
            );
        }
    };
    if unit_price_minor < 0 {
        return workspace_with_error(
            &application_support_directory,
            "Menu item needs attention • price cannot be negative".to_owned(),
        );
    }
    let price = match ros_core::Money::new(unit_price_minor, branch.currency()) {
        Ok(price) => price,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • price could not be validated".to_owned(),
            );
        }
    };
    let command = match ros_core::CreateProduct::new(
        &display_name,
        Some(category_id),
        price,
        None,
        None,
        0,
    ) {
        Ok(command) => command,
        Err(error) => {
            return workspace_with_error(
                &application_support_directory,
                format!("Menu item needs attention • {error}"),
            );
        }
    };
    let image = match selected_menu_item_image(
        built_in_image_key,
        restaurant_image_bytes,
        gotigin_catalog_image,
    ) {
        Ok(image) => image,
        Err(message) => {
            return workspace_with_error(
                &application_support_directory,
                format!("Menu image needs attention • {message}"),
            );
        }
    };

    match database.create_product_with_image(&command, image.as_ref(), &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            if image.is_some() {
                "Menu item and image saved locally".to_owned()
            } else {
                "Menu item saved locally".to_owned()
            },
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Menu item needs attention • local changes could not be saved".to_owned(),
        ),
    }
}

/// Adds a product-bound, append-only modifier option. Flutter supplies only a
/// display label and minor-unit delta; Rust resolves the branch currency and
/// records the immutable catalogue/audit/outbox facts atomically.
pub fn create_community_product_modifier_option(
    application_support_directory: String,
    product_id: String,
    display_name: String,
    price_delta_minor: i64,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu modifier needs attention • unlock as owner or manager first".to_owned(),
            )
        }
    };
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu modifier needs attention • Community setup could not be loaded".to_owned(),
            )
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu modifier needs attention • choose an active menu item".to_owned(),
            )
        }
    };
    let delta = match ros_core::Money::new(price_delta_minor, branch.currency()) {
        Ok(delta) => delta,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu modifier needs attention • enter a valid non-negative price adjustment"
                    .to_owned(),
            )
        }
    };
    let command = match ros_core::CreateModifierOption::new(&display_name, delta) {
        Ok(command) => command,
        Err(_) => return workspace_with_error(
            &application_support_directory,
            "Menu modifier needs attention • enter a readable name and a non-negative adjustment"
                .to_owned(),
        ),
    };
    match database.create_product_modifier_option(&product_id, &command, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Menu modifier saved locally • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Menu modifier needs attention • it could not be saved".to_owned(),
        ),
    }
}

/// Archives one immutable option without deleting it. Earlier order, kitchen,
/// invoice, and audit snapshots continue to render the original selection.
pub fn archive_community_product_modifier_option(
    application_support_directory: String,
    modifier_option_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu modifier needs attention • unlock as owner or manager first".to_owned(),
            )
        }
    };
    let modifier_option_id = match ros_core::EntityId::parse(&modifier_option_id) {
        Ok(modifier_option_id) => modifier_option_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu modifier needs attention • the selected modifier is unavailable".to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu modifier needs attention • explain why it is being archived".to_owned(),
            )
        }
    };
    match database.archive_product_modifier_option(
        &modifier_option_id,
        expected_revision,
        &reason,
        &context,
    ) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Menu modifier removed from new orders • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Menu modifier needs attention • it could not be archived".to_owned(),
        ),
    }
}

#[derive(Clone)]
pub struct CommunityTaxRateView {
    pub tax_rate_id: String,
    pub display_name: String,
    pub basis_points: i64,
    pub revision: i64,
    pub archived: bool,
}

#[derive(Clone)]
pub struct CommunityTaxRateWorkspace {
    pub storage_status: String,
    pub available: bool,
    pub rates: Vec<CommunityTaxRateView>,
}

pub fn list_community_tax_rates(
    application_support_directory: String,
) -> CommunityTaxRateWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            return CommunityTaxRateWorkspace {
                storage_status: status,
                available: false,
                rates: Vec::new(),
            }
        }
    };
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return CommunityTaxRateWorkspace {
                storage_status: "Tax rates need attention • restaurant setup is required"
                    .to_owned(),
                available: false,
                rates: Vec::new(),
            }
        }
    };
    match database.list_branch_tax_rates(branch.branch_id()) {
        Ok(rates) => CommunityTaxRateWorkspace {
            storage_status: "Branch tax rates loaded".to_owned(),
            available: true,
            rates: rates
                .into_iter()
                .map(|rate| CommunityTaxRateView {
                    tax_rate_id: rate.tax_rate_id().to_string(),
                    display_name: rate.display_name().to_owned(),
                    basis_points: i64::from(rate.basis_points()),
                    revision: rate.revision(),
                    archived: rate.archived(),
                })
                .collect(),
        },
        Err(_) => CommunityTaxRateWorkspace {
            storage_status: "Tax rates need attention • they could not be read".to_owned(),
            available: false,
            rates: Vec::new(),
        },
    }
}

pub fn create_community_tax_rate(
    application_support_directory: String,
    display_name: String,
    basis_points: i64,
) -> CommunityTaxRateWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            return CommunityTaxRateWorkspace {
                storage_status: status,
                available: false,
                rates: Vec::new(),
            }
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return CommunityTaxRateWorkspace {
                storage_status: "Tax rate needs attention • unlock as owner or manager first"
                    .to_owned(),
                available: false,
                rates: Vec::new(),
            }
        }
    };
    let basis_points = match u32::try_from(basis_points) {
        Ok(basis_points) => basis_points,
        Err(_) => {
            return CommunityTaxRateWorkspace {
                storage_status: "Tax rate needs attention • basis points are invalid".to_owned(),
                available: false,
                rates: Vec::new(),
            }
        }
    };
    match database.create_branch_tax_rate(&display_name, basis_points, &context) {
        Ok(_) => {
            let mut rates = list_community_tax_rates(application_support_directory);
            if rates.available {
                rates.storage_status = "Tax rate saved locally".to_owned();
            }
            rates
        }
        Err(_) => CommunityTaxRateWorkspace {
            storage_status: "Tax rate needs attention • it could not be saved".to_owned(),
            available: false,
            rates: Vec::new(),
        },
    }
}

pub fn archive_community_tax_rate(
    application_support_directory: String,
    tax_rate_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityTaxRateWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            return CommunityTaxRateWorkspace {
                storage_status: status,
                available: false,
                rates: Vec::new(),
            }
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return CommunityTaxRateWorkspace {
                storage_status: "Tax rate needs attention • unlock as owner or manager first"
                    .to_owned(),
                available: false,
                rates: Vec::new(),
            }
        }
    };
    let tax_rate_id = match ros_core::EntityId::parse(&tax_rate_id) {
        Ok(tax_rate_id) => tax_rate_id,
        Err(_) => {
            return CommunityTaxRateWorkspace {
                storage_status: "Tax rate needs attention • rate could not be identified"
                    .to_owned(),
                available: false,
                rates: Vec::new(),
            }
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return CommunityTaxRateWorkspace {
                storage_status: "Tax rate needs attention • explain why this rate is archived"
                    .to_owned(),
                available: false,
                rates: Vec::new(),
            }
        }
    };
    match database.archive_branch_tax_rate(&tax_rate_id, expected_revision, &reason, &context) {
        Ok(_) => {
            let mut rates = list_community_tax_rates(application_support_directory);
            if rates.available {
                rates.storage_status = "Tax rate archived locally • history retained".to_owned();
            }
            rates
        }
        Err(_) => CommunityTaxRateWorkspace {
            storage_status: "Tax rate needs attention • it could not be archived".to_owned(),
            available: false,
            rates: Vec::new(),
        },
    }
}

pub fn set_community_product_tax_treatment(
    application_support_directory: String,
    product_id: String,
    tax_treatment: String,
    expected_revision: i64,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Tax treatment needs attention • unlock as owner or manager first".to_owned(),
            )
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Tax treatment needs attention • the menu item is unavailable".to_owned(),
            )
        }
    };
    match database.set_product_tax_treatment(
        &product_id,
        &tax_treatment,
        expected_revision,
        &context,
    ) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Menu item tax treatment updated locally".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Tax treatment needs attention • it could not be updated".to_owned(),
        ),
    }
}

/// Prepares a selected image before it is accepted by the Flutter form. This
/// keeps the expensive decoding and re-encoding in Rust, removes metadata,
/// and enforces the 3 MiB post-compression acceptance ceiling. An empty return
/// is intentionally a safe, detail-free rejection for untrusted input.
pub fn prepare_community_menu_image(image_bytes: Vec<u8>) -> Vec<u8> {
    normalize_restaurant_menu_image(image_bytes)
        .ok()
        .and_then(|image| match image {
            ros_storage::ProductImageContent::RestaurantUpload { jpeg_bytes, .. } => {
                Some(jpeg_bytes)
            }
            ros_storage::ProductImageContent::BuiltIn { .. }
            | ros_storage::ProductImageContent::GotiginCatalog { .. } => None,
        })
        .filter(|bytes| bytes.len() <= MAX_MENU_IMAGE_ACCEPTED_BYTES)
        .unwrap_or_default()
}

/// Applies a reasoned selling-price change. The encrypted storage core uses a
/// revision check and records before/after values, while past invoices retain
/// their already-snapshotted line price.
pub fn update_community_product_price(
    application_support_directory: String,
    product_id: String,
    expected_revision: i64,
    unit_price_minor: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Price needs attention • Community setup is required".to_owned(),
            );
        }
    };
    let branch = match database.community_branch() {
        Ok(branch) => branch,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Price needs attention • Community setup could not be loaded".to_owned(),
            );
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Price needs attention • choose an active menu item".to_owned(),
            );
        }
    };
    let price = match ros_core::Money::new(unit_price_minor, branch.currency()) {
        Ok(price) => price,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Price needs attention • enter a valid non-negative amount".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Price needs attention • explain why the price is changing".to_owned(),
            );
        }
    };

    match database.update_product_price(&product_id, expected_revision, &price, &reason, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Menu price updated locally • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Price needs attention • it could not be updated".to_owned(),
        ),
    }
}

/// Temporarily removes an active menu item from checkout, or resumes it,
/// without mutating its financial history. The storage core requires a
/// management role, a current revision, and a human-readable reason.
pub fn set_community_product_availability(
    application_support_directory: String,
    product_id: String,
    expected_revision: i64,
    is_available: bool,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • Community setup is required".to_owned(),
            );
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • choose an active menu item".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • explain this availability change".to_owned(),
            );
        }
    };

    match database.set_product_availability(
        &product_id,
        expected_revision,
        is_available,
        &reason,
        &context,
    ) {
        Ok(_) if is_available => workspace_with_status(
            &application_support_directory,
            "Menu item resumed locally • history retained".to_owned(),
        ),
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Menu item marked sold out locally • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Menu item needs attention • availability could not be updated".to_owned(),
        ),
    }
}

/// Removes an item from the active menu without deleting it. The product,
/// selected image versions, sales references, and audit record stay intact for
/// reconciliation and future sync.
pub fn archive_community_product(
    application_support_directory: String,
    product_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • Community setup is required".to_owned(),
            );
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • choose an active menu item".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Menu item needs attention • explain why it is being removed".to_owned(),
            );
        }
    };

    match database.archive_product(&product_id, expected_revision, &reason, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Menu item removed from the active menu • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Menu item needs attention • it could not be removed from the active menu".to_owned(),
        ),
    }
}

/// Archives a category after its active products have been removed. History is
/// retained for audit and future sync.
pub fn archive_community_category(
    application_support_directory: String,
    category_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Category needs attention • unlock as owner or manager first".to_owned(),
            );
        }
    };
    let category_id = match ros_core::EntityId::parse(&category_id) {
        Ok(category_id) => category_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Category needs attention • choose an active category".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Category needs attention • explain why it is being archived".to_owned(),
            );
        }
    };
    match database.archive_category(&category_id, expected_revision, &reason, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Category archived locally • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Category needs attention • remove active products first or retry later".to_owned(),
        ),
    }
}

/// Replaces the active menu image for a product by appending a new immutable
/// image version. Past sales retain their snapshotted catalogue facts.
pub fn replace_community_product_image(
    application_support_directory: String,
    product_id: String,
    restaurant_image_bytes: Option<Vec<u8>>,
    built_in_image_key: Option<String>,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Image needs attention • unlock as owner or manager first".to_owned(),
            );
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Image needs attention • choose an active menu item".to_owned(),
            );
        }
    };
    let image = match selected_menu_item_image(built_in_image_key, restaurant_image_bytes, None) {
        Ok(Some(image)) => image,
        Ok(None) => {
            return workspace_with_error(
                &application_support_directory,
                "Image needs attention • choose a menu image".to_owned(),
            );
        }
        Err(message) => {
            return workspace_with_error(
                &application_support_directory,
                format!("Image needs attention • {message}"),
            );
        }
    };
    match database.replace_product_image(&product_id, &image, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Menu image updated locally • previous versions retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Image needs attention • it could not be updated".to_owned(),
        ),
    }
}

/// Permanently deletes an unused menu item only when the storage policy proves
/// that it has no retained financial, image, or synchronization history.
pub fn delete_unused_community_product(
    application_support_directory: String,
    product_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Deletion needs attention • Community setup is required".to_owned(),
            );
        }
    };
    let product_id = match ros_core::EntityId::parse(&product_id) {
        Ok(product_id) => product_id,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Deletion needs attention • choose an active menu item".to_owned(),
            );
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(reason) => reason,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Deletion needs attention • explain why the unused item is being deleted"
                    .to_owned(),
            );
        }
    };

    match database.delete_unused_product(&product_id, expected_revision, &reason, &context) {
        Ok(()) => workspace_with_status(
            &application_support_directory,
            "Unused menu item permanently deleted • audit record retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Deletion needs attention • this item has retained history and must be archived"
                .to_owned(),
        ),
    }
}

/// Saves an open order as a versioned local draft. The draft contains trusted
/// catalog snapshots and no payment or invoice facts.
pub fn save_community_draft_order(
    application_support_directory: String,
    draft_order_id: Option<String>,
    expected_revision: Option<i64>,
    cart_lines: Vec<CommunityCartLine>,
    fulfillment: String,
    table_name: Option<String>,
    kitchen_note: Option<String>,
) -> CommunityDraftOrderResult {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_draft(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_draft(
                "Order needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let fulfillment = match ros_core::OrderFulfillment::parse(&fulfillment) {
        Ok(value) => value,
        Err(_) => {
            return unavailable_draft(
                "Order needs attention • choose dine-in or takeaway".to_owned(),
            )
        }
    };
    let lines = cart_lines
        .into_iter()
        .map(|line| {
            let product_id = ros_core::EntityId::parse(&line.product_id).map_err(|_| ())?;
            let modifier_option_ids = line
                .modifier_option_ids
                .into_iter()
                .map(|modifier_option_id| {
                    ros_core::EntityId::parse(&modifier_option_id).map_err(|_| ())
                })
                .collect::<Result<Vec<_>, _>>()?;
            ros_core::SaleLineInput::new(product_id, line.quantity)
                .and_then(|line| line.with_modifier_options(modifier_option_ids))
                .map_err(|_| ())
        })
        .collect::<Result<Vec<_>, _>>();
    let lines = match lines {
        Ok(lines) if !lines.is_empty() => lines,
        _ => {
            return unavailable_draft(
                "Order needs attention • add at least one valid menu item".to_owned(),
            )
        }
    };
    let draft_order_id = match draft_order_id {
        Some(value) => match ros_core::EntityId::parse(&value) {
            Ok(value) => Some(value),
            Err(_) => {
                return unavailable_draft(
                    "Order needs attention • saved order could not be identified".to_owned(),
                )
            }
        },
        None => None,
    };
    match database.save_draft_order_with_kitchen_note(
        ros_storage::DraftOrderSaveRequest::with_kitchen_note(
            draft_order_id.as_ref(),
            expected_revision,
            fulfillment,
            table_name.as_deref(),
            kitchen_note.as_deref(),
            &lines,
        ),
        &context,
    ) {
        Ok(draft) => CommunityDraftOrderResult {
            storage_status: if draft.revision() == 1 {
                "Open order saved locally • audited revision 1"
            } else {
                "Open order updated locally • new audited revision saved"
            }
            .to_owned(),
            saved: true,
            draft_order_id: Some(draft.draft_order_id().to_string()),
            revision: draft.revision(),
        },
        Err(_) => unavailable_draft(
            "Order needs attention • it could not be saved. Your cart is still here.".to_owned(),
        ),
    }
}

pub fn cancel_community_open_draft_order(
    application_support_directory: String,
    draft_order_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Order cancellation needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let draft_order_id = match ros_core::EntityId::parse(&draft_order_id) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Order cancellation needs attention • open order could not be identified"
                    .to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Order cancellation needs attention • add a clear reason".to_owned(),
            )
        }
    };
    match database.cancel_open_draft_order(&draft_order_id, expected_revision, &reason, &context) {
        Ok(()) => workspace_with_status(
            &application_support_directory,
            "Open order cancelled locally • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Order cancellation needs attention • only an unchanged unsent order can be cancelled"
                .to_owned(),
        ),
    }
}

/// Sends an immutable stop-work notice to kitchen and closes the selected
/// sent draft. Only an active owner or manager session can perform this
/// action; kitchen receives the signal but not the counter rationale.
pub fn cancel_community_sent_draft_order(
    application_support_directory: String,
    draft_order_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context =
        match database.community_active_staff_context() {
            Ok(context) => context,
            Err(_) => return workspace_with_error(
                &application_support_directory,
                "Kitchen cancellation needs attention • unlock an owner or manager session first"
                    .to_owned(),
            ),
        };
    let draft_order_id = match ros_core::EntityId::parse(&draft_order_id) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen cancellation needs attention • sent order could not be identified"
                    .to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen cancellation needs attention • add a clear reason".to_owned(),
            )
        }
    };
    match database.cancel_sent_draft_order(&draft_order_id, expected_revision, &reason, &context) {
        Ok(()) => workspace_with_status(
            &application_support_directory,
            "Kitchen cancellation sent locally • kitchen acknowledgement is required".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Kitchen cancellation needs attention • only an unchanged sent order can be cancelled"
                .to_owned(),
        ),
    }
}

/// Preserves the original kitchen ticket, sends its cancellation notice, and
/// creates a separate editable draft revision for a counter correction.
pub fn reopen_community_sent_draft_order(
    application_support_directory: String,
    draft_order_id: String,
    expected_revision: i64,
    reason: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Order revision needs attention • unlock an owner or manager session first"
                    .to_owned(),
            )
        }
    };
    let draft_order_id = match ros_core::EntityId::parse(&draft_order_id) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Order revision needs attention • sent order could not be identified".to_owned(),
            )
        }
    };
    let reason = match ros_core::MutationReason::new(&reason) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Order revision needs attention • add a clear reason".to_owned(),
            )
        }
    };
    match database.reopen_sent_draft_order(&draft_order_id, expected_revision, &reason, &context) {
        Ok(draft) => workspace_with_status(
            &application_support_directory,
            format!(
                "Order reopened as revision {} • original kitchen ticket cancelled",
                draft.revision()
            ),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Order revision needs attention • only an unchanged sent order can be reopened"
                .to_owned(),
        ),
    }
}

/// Records that an authorized kitchen session has seen a counter cancellation
/// notice. This removes it from the active kitchen queue while retaining both
/// the request and acknowledgement as immutable audit facts.
pub fn acknowledge_community_kitchen_ticket_cancellation(
    application_support_directory: String,
    ticket_id: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen cancellation needs attention • unlock a kitchen, manager, or owner session first"
                    .to_owned(),
            )
        }
    };
    let ticket_id = match ros_core::EntityId::parse(&ticket_id) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen cancellation needs attention • ticket could not be identified".to_owned(),
            )
        }
    };
    match database.acknowledge_kitchen_ticket_cancellation(&ticket_id, &context) {
        Ok(()) => workspace_with_status(
            &application_support_directory,
            "Kitchen cancellation acknowledged locally • history retained".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Kitchen cancellation needs attention • this stop-work notice could not be acknowledged"
                .to_owned(),
        ),
    }
}

pub fn send_community_draft_to_kitchen(
    application_support_directory: String,
    draft_order_id: String,
    expected_revision: i64,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let draft_order_id = match ros_core::EntityId::parse(&draft_order_id) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen needs attention • open order could not be identified".to_owned(),
            )
        }
    };
    match database.send_draft_to_kitchen(&draft_order_id, expected_revision, &context) {
        Ok(_) => workspace_with_status(
            &application_support_directory,
            "Kitchen ticket sent locally • ready for kitchen display".to_owned(),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Kitchen needs attention • open order could not be sent".to_owned(),
        ),
    }
}

pub fn advance_community_kitchen_ticket(
    application_support_directory: String,
    ticket_id: String,
    expected_revision: i64,
    next_state: String,
) -> CommunityWorkspace {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_workspace(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen needs attention • restaurant setup is required".to_owned(),
            )
        }
    };
    let ticket_id = match ros_core::EntityId::parse(&ticket_id) {
        Ok(value) => value,
        Err(_) => {
            return workspace_with_error(
                &application_support_directory,
                "Kitchen needs attention • ticket could not be identified".to_owned(),
            )
        }
    };
    match database.advance_kitchen_ticket(&ticket_id, expected_revision, &next_state, &context) {
        Ok(ticket) => workspace_with_status(
            &application_support_directory,
            format!("Kitchen ticket marked {} locally", ticket.state()),
        ),
        Err(_) => workspace_with_error(
            &application_support_directory,
            "Kitchen needs attention • ticket could not be updated".to_owned(),
        ),
    }
}

#[derive(Clone)]
pub struct CommunitySalePricingPreview {
    pub storage_status: String,
    pub available: bool,
    pub subtotal_minor: i64,
    pub discount_minor: i64,
    pub tax_minor: i64,
    pub payable_minor: i64,
    pub currency_code: Option<String>,
}

fn unavailable_sale_pricing_preview(storage_status: String) -> CommunitySalePricingPreview {
    CommunitySalePricingPreview {
        storage_status,
        available: false,
        subtotal_minor: 0,
        discount_minor: 0,
        tax_minor: 0,
        payable_minor: 0,
        currency_code: None,
    }
}

fn parse_order_discount(
    discount_fixed_minor: Option<i64>,
    discount_percentage_basis_points: Option<i64>,
    discount_percentage_cap_minor: Option<i64>,
    discount_reason: Option<String>,
) -> Result<Option<ros_core::pricing::OrderDiscount>, String> {
    match (
        discount_fixed_minor,
        discount_percentage_basis_points,
        discount_reason,
    ) {
        (None, None, None) => Ok(None),
        (Some(amount_minor), None, Some(reason)) => {
            ros_core::pricing::OrderDiscount::fixed(amount_minor, &reason)
                .map(Some)
                .map_err(|_| "discount amount or reason is invalid".to_owned())
        }
        (None, Some(basis_points), Some(reason)) => {
            let basis_points = u32::try_from(basis_points)
                .map_err(|_| "discount percentage is invalid".to_owned())?;
            let cap = match discount_percentage_cap_minor {
                None => None,
                Some(cap) if cap >= 0 => Some(cap),
                Some(_) => return Err("discount percentage cap is invalid".to_owned()),
            };
            ros_core::pricing::OrderDiscount::percentage(basis_points, cap, &reason)
                .map(Some)
                .map_err(|_| "discount percentage or reason is invalid".to_owned())
        }
        (Some(_), Some(_), _) => {
            Err("provide either a fixed discount or a percentage discount, not both".to_owned())
        }
        _ => Err("discount amount/percentage and reason are both required".to_owned()),
    }
}

/// Calculates the trusted payable total for the current cart without recording
/// a sale. Split tender and checkout previews must use this path.
#[allow(clippy::too_many_arguments)]
pub fn preview_community_sale_pricing(
    application_support_directory: String,
    cart_lines: Vec<CommunityCartLine>,
    discount_fixed_minor: Option<i64>,
    discount_percentage_basis_points: Option<i64>,
    discount_percentage_cap_minor: Option<i64>,
    discount_reason: Option<String>,
) -> CommunitySalePricingPreview {
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => return unavailable_sale_pricing_preview(status),
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_sale_pricing_preview(
                "Pricing needs attention • unlock an authorized staff session first".to_owned(),
            );
        }
    };
    let lines = cart_lines
        .into_iter()
        .map(|line| {
            let product_id = ros_core::EntityId::parse(&line.product_id)
                .map_err(|_| "invalid product selection")?;
            let modifier_option_ids = line
                .modifier_option_ids
                .into_iter()
                .map(|modifier_option_id| {
                    ros_core::EntityId::parse(&modifier_option_id)
                        .map_err(|_| "invalid modifier selection")
                })
                .collect::<Result<Vec<_>, _>>()?;
            ros_core::SaleLineInput::new(product_id, line.quantity)
                .and_then(|line| line.with_modifier_options(modifier_option_ids))
                .map_err(|_| "invalid cart line")
        })
        .collect::<Result<Vec<_>, _>>();
    let lines = match lines {
        Ok(lines) => lines,
        Err(_) => {
            return unavailable_sale_pricing_preview(
                "Pricing needs attention • cart details are invalid".to_owned(),
            );
        }
    };
    let discount = match parse_order_discount(
        discount_fixed_minor,
        discount_percentage_basis_points,
        discount_percentage_cap_minor,
        discount_reason,
    ) {
        Ok(discount) => discount,
        Err(message) => {
            return unavailable_sale_pricing_preview(format!(
                "Pricing needs attention • {message}"
            ));
        }
    };
    match database.preview_sale_pricing(&lines, discount, &context) {
        Ok(preview) => {
            record_diagnostic(
                &application_support_directory,
                "sale_preview",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                None,
                None,
            );
            CommunitySalePricingPreview {
                storage_status: "Payable total calculated from trusted local catalogue".to_owned(),
                available: true,
                subtotal_minor: preview.subtotal_minor(),
                discount_minor: preview.discount_minor(),
                tax_minor: preview.tax_minor(),
                payable_minor: preview.payable_minor(),
                currency_code: Some(preview.currency_code().to_owned()),
            }
        }
        Err(ros_storage::StorageError::PermissionDenied) => {
            record_diagnostic(
                &application_support_directory,
                "sale_preview",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Denied,
                None,
                Some("permission_denied"),
            );
            unavailable_sale_pricing_preview(
                "Pricing needs attention • discounts require an owner or manager session"
                    .to_owned(),
            )
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "sale_preview",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                None,
                Some("preview_failed"),
            );
            unavailable_sale_pricing_preview(
                "Pricing needs attention • payable total could not be calculated".to_owned(),
            )
        }
    }
}

/// Records a complete immediate counter sale. This Stage 1 vertical slice does
/// not create persistent drafts: a successful response means the order,
/// invoice, payment, audit envelopes, and future-sync outbox entries all
/// committed together in the encrypted local database.
#[allow(clippy::too_many_arguments)] // Generated Flutter bridge uses named parameters.
pub fn complete_community_sale(
    application_support_directory: String,
    cart_lines: Vec<CommunityCartLine>,
    fulfillment: String,
    payment_method: String,
    payment_allocations: Option<Vec<CommunityPaymentAllocation>>,
    customer_id: Option<String>,
    draft_order_id: Option<String>,
    expected_draft_revision: Option<i64>,
    discount_fixed_minor: Option<i64>,
    discount_percentage_basis_points: Option<i64>,
    discount_percentage_cap_minor: Option<i64>,
    discount_reason: Option<String>,
) -> CommunitySaleResult {
    let started = std::time::Instant::now();
    let database = match open_community_database(&application_support_directory) {
        Ok(database) => database,
        Err(status) => {
            record_diagnostic(
                &application_support_directory,
                "sale_complete",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("storage_unavailable"),
            );
            return unavailable_sale(status);
        }
    };
    let context = match database.community_active_staff_context() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_sale(
                "Counter sale needs attention • restaurant setup is required".to_owned(),
            );
        }
    };
    let fulfillment = match ros_core::OrderFulfillment::parse(&fulfillment) {
        Ok(fulfillment) => fulfillment,
        Err(_) => {
            return unavailable_sale(
                "Counter sale needs attention • choose a valid fulfillment method".to_owned(),
            );
        }
    };
    let payment_method = match ros_core::PaymentMethod::parse(&payment_method) {
        Ok(payment_method) => payment_method,
        Err(_) => {
            return unavailable_sale(
                "Counter sale needs attention • choose a valid payment method".to_owned(),
            );
        }
    };
    let lines = cart_lines
        .into_iter()
        .map(|line| {
            let product_id = ros_core::EntityId::parse(&line.product_id)
                .map_err(|_| "invalid product selection")?;
            let modifier_option_ids = line
                .modifier_option_ids
                .into_iter()
                .map(|modifier_option_id| {
                    ros_core::EntityId::parse(&modifier_option_id)
                        .map_err(|_| "invalid modifier selection")
                })
                .collect::<Result<Vec<_>, _>>()?;
            ros_core::SaleLineInput::new(product_id, line.quantity)
                .and_then(|line| line.with_modifier_options(modifier_option_ids))
                .map_err(|_| "invalid cart line")
        })
        .collect::<Result<Vec<_>, _>>();
    let lines = match lines {
        Ok(lines) => lines,
        Err(_) => {
            return unavailable_sale(
                "Counter sale needs attention • cart details are invalid".to_owned(),
            );
        }
    };
    let customer_id = match customer_id {
        Some(customer_id) => match ros_core::EntityId::parse(&customer_id) {
            Ok(customer_id) => Some(customer_id),
            Err(_) => {
                return unavailable_sale(
                    "Counter sale needs attention • customer selection is unavailable".to_owned(),
                );
            }
        },
        None => None,
    };
    let command = match ros_core::CompleteSale::new(fulfillment, payment_method, lines) {
        Ok(command) => command,
        Err(_) => {
            return unavailable_sale(
                "Counter sale needs attention • add at least one distinct menu item".to_owned(),
            );
        }
    }
    .with_customer(customer_id);
    let command = match parse_order_discount(
        discount_fixed_minor,
        discount_percentage_basis_points,
        discount_percentage_cap_minor,
        discount_reason,
    ) {
        Ok(None) => command,
        Ok(Some(discount)) => command.with_discount(discount),
        Err(message) => {
            return unavailable_sale(format!("Counter sale needs attention • {message}"));
        }
    };
    let command = match payment_allocations {
        Some(allocations) => {
            let allocations = allocations
                .into_iter()
                .map(|allocation| {
                    let payment_method = ros_core::PaymentMethod::parse(&allocation.payment_method)
                        .map_err(|_| ())?;
                    ros_core::PaymentAllocationInput::new(payment_method, allocation.amount_minor)
                        .map_err(|_| ())
                })
                .collect::<Result<Vec<_>, _>>();
            match allocations.and_then(|allocations| {
                command
                    .with_payment_allocations(allocations)
                    .map_err(|_| ())
            }) {
                Ok(command) => command,
                Err(_) => {
                    return unavailable_sale(
                        "Counter sale needs attention • payment allocations are invalid".to_owned(),
                    )
                }
            }
        }
        None => command,
    };

    let completed = match (draft_order_id, expected_draft_revision) {
        (None, None) => database.complete_sale(&command, &context),
        (Some(draft_order_id), Some(expected_revision)) => {
            match ros_core::EntityId::parse(&draft_order_id) {
                Ok(draft_order_id) => database.complete_draft_sale(
                    &command,
                    &draft_order_id,
                    expected_revision,
                    &context,
                ),
                Err(_) => {
                    return unavailable_sale(
                        "Sale needs attention • saved order could not be identified".to_owned(),
                    )
                }
            }
        }
        _ => {
            return unavailable_sale(
                "Sale needs attention • saved order revision is missing".to_owned(),
            )
        }
    };
    match completed {
        Ok(sale) => {
            record_diagnostic(
                &application_support_directory,
                "sale_complete",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Ok,
                Some(started.elapsed().as_millis() as u64),
                None,
            );
            CommunitySaleResult {
                storage_status: "Sale saved locally • encrypted, audited, and ready for future sync"
                    .to_owned(),
                completed: true,
                invoice_number: Some(format!("INV-{:06}", sale.invoice_number())),
                total_minor: sale.total().minor_units(),
                currency_code: Some(sale.total().currency().to_owned()),
                payment_method: Some(sale.payment_method().as_str().to_owned()),
            }
        }
        Err(_) => {
            record_diagnostic(
                &application_support_directory,
                "sale_complete",
                ros_diagnostics::DiagnosticComponent::Bridge,
                ros_diagnostics::DiagnosticOutcome::Failed,
                Some(started.elapsed().as_millis() as u64),
                Some("sale_failed"),
            );
            unavailable_sale(
                "Counter sale needs attention • local sale could not be recorded".to_owned(),
            )
        }
    }
}

/// Converts an untrusted client selection into the narrow media contract that
/// encrypted storage accepts. The Flutter client may choose a file, but it
/// never decides its persisted format, dimensions, or byte budget.
fn selected_menu_item_image(
    built_in_image_key: Option<String>,
    restaurant_image_bytes: Option<Vec<u8>>,
    gotigin_catalog_image: Option<GotiginCatalogMenuImageSelection>,
) -> Result<Option<ros_storage::ProductImageContent>, &'static str> {
    match (
        built_in_image_key,
        restaurant_image_bytes,
        gotigin_catalog_image,
    ) {
        (None, None, None) => Ok(None),
        (Some(asset_key), None, None) => {
            Ok(Some(ros_storage::ProductImageContent::built_in(asset_key)))
        }
        (None, Some(bytes), None) => normalize_restaurant_menu_image(bytes).map(Some),
        (None, None, Some(selection)) => normalize_gotigin_catalog_menu_image(selection).map(Some),
        _ => Err("choose exactly one app, Gotigin catalogue, or restaurant-owned image"),
    }
}

/// Re-verifies the catalogue trust boundary in Rust. Dart's early checks make
/// the chooser responsive, but neither its digest result nor its provenance
/// fields are authoritative at the persistence boundary.
fn normalize_gotigin_catalog_menu_image(
    selection: GotiginCatalogMenuImageSelection,
) -> Result<ros_storage::ProductImageContent, &'static str> {
    if selection.service_origin != GOTIGIN_CATALOG_SERVICE_ORIGIN
        || selection.service_schema_version != GOTIGIN_CATALOG_SERVICE_SCHEMA_VERSION
    {
        return Err("the Gotigin catalogue contract is unsupported");
    }
    let provenance = ros_storage::ProductImageCatalogProvenance::from_verified_original(
        selection.catalog_image_id,
        &selection.original_image_bytes,
        &selection.content_sha256,
        selection.licence_label,
        selection.licence_url,
        selection.service_origin,
        selection.service_schema_version,
    )
    .map_err(|_| "the Gotigin catalogue provenance is invalid")?;

    match normalize_restaurant_menu_image(selection.original_image_bytes)? {
        ros_storage::ProductImageContent::RestaurantUpload {
            jpeg_bytes,
            pixel_width,
            pixel_height,
        } => Ok(ros_storage::ProductImageContent::gotigin_catalog(
            jpeg_bytes,
            pixel_width,
            pixel_height,
            provenance,
        )),
        ros_storage::ProductImageContent::BuiltIn { .. }
        | ros_storage::ProductImageContent::GotiginCatalog { .. } => {
            Err("the Gotigin catalogue image could not be normalized")
        }
    }
}

/// Decodes a JPEG, PNG, or WebP file under bounded limits, strips the source
/// metadata by re-encoding it, and returns a counter-sized JPEG thumbnail.
/// The encoded bytes go into SQLCipher rather than an app-readable file.
fn normalize_restaurant_menu_image(
    source_bytes: Vec<u8>,
) -> Result<ros_storage::ProductImageContent, &'static str> {
    if source_bytes.is_empty() || source_bytes.len() > MAX_MENU_IMAGE_INPUT_BYTES {
        return Err("choose an image smaller than 32 MiB");
    }

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_MENU_IMAGE_DECODE_WIDTH);
    limits.max_image_height = Some(MAX_MENU_IMAGE_DECODE_HEIGHT);
    limits.max_alloc = Some(MAX_MENU_IMAGE_DECODE_ALLOCATION);

    let mut reader = ImageReader::new(Cursor::new(source_bytes));
    reader.limits(limits);
    let reader = reader
        .with_guessed_format()
        .map_err(|_| "choose a JPEG, PNG, or WebP image")?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)
    ) {
        return Err("choose a JPEG, PNG, or WebP image");
    }

    let mut decoder = reader
        .into_decoder()
        .map_err(|_| "use a valid JPEG, PNG, or WebP image no larger than 4096 pixels")?;
    if decoder.total_bytes() > MAX_MENU_IMAGE_DECODE_ALLOCATION {
        return Err("use an image that needs less memory to prepare for the menu");
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut decoded = DynamicImage::from_decoder(decoder)
        .map_err(|_| "use a valid JPEG, PNG, or WebP image no larger than 4096 pixels")?;
    decoded.apply_orientation(orientation);

    for (max_width, max_height, quality) in [
        (320_u32, 240_u32, 78_u8),
        (320, 240, 70),
        (288, 216, 68),
        (256, 192, 64),
        (224, 168, 60),
    ] {
        let thumbnail = if decoded.width() <= max_width && decoded.height() <= max_height {
            decoded.to_rgb8()
        } else {
            decoded.thumbnail(max_width, max_height).to_rgb8()
        };
        let mut encoded = Vec::new();
        JpegEncoder::new_with_quality(&mut encoded, quality)
            .encode_image(&thumbnail)
            .map_err(|_| "the selected image could not be prepared for the menu")?;
        if encoded.len() <= MAX_MENU_IMAGE_STORED_BYTES {
            return Ok(ros_storage::ProductImageContent::restaurant_upload(
                encoded,
                i64::from(thumbnail.width()),
                i64::from(thumbnail.height()),
            ));
        }
    }

    Err("the selected image has too much detail for a small menu tile; choose a simpler photo")
}

fn workspace_with_status(
    application_support_directory: &str,
    success_status: String,
) -> CommunityWorkspace {
    let mut workspace = load_community_workspace(application_support_directory.to_owned());
    if !workspace.setup_required {
        workspace.storage_status = success_status;
    }
    workspace
}

fn workspace_with_error(
    application_support_directory: &str,
    error_status: String,
) -> CommunityWorkspace {
    let mut workspace = load_community_workspace(application_support_directory.to_owned());
    if !workspace
        .storage_status
        .to_lowercase()
        .contains("attention")
    {
        workspace.storage_status = error_status;
    }
    workspace
}

fn open_community_database(
    application_support_directory: &str,
) -> Result<ros_storage::LocalDatabase, String> {
    let support_directory = PathBuf::from(application_support_directory);
    let database_path = support_directory.join(COMMUNITY_DATABASE_FILE_NAME);

    std::fs::create_dir_all(&support_directory).map_err(|_| {
        "Local storage needs attention • application data directory is unavailable".to_owned()
    })?;
    ros_storage::open_or_create_platform_database(&database_path)
        .map_err(user_safe_bootstrap_status)
}

/// Deliberately collapses internal storage, SQLCipher, and operating-system
/// error detail at the Flutter boundary. A caller gets an actionable category
/// without learning schema, key, path, or native-library details.
fn user_safe_bootstrap_status(error: ros_storage::DatabaseBootstrapError) -> String {
    match error {
        ros_storage::DatabaseBootstrapError::DatabaseKeyMissing => {
            "Local storage needs attention • encrypted restaurant data needs its secure device key"
                .to_owned()
        }
        ros_storage::DatabaseBootstrapError::ExistingKeyWithoutDatabase => {
            "Local storage needs attention • local data recovery is required before setup"
                .to_owned()
        }
        ros_storage::DatabaseBootstrapError::KeyStore(_) => {
            "Local storage needs attention • secure device storage is unavailable or locked"
                .to_owned()
        }
        ros_storage::DatabaseBootstrapError::Lock(_) => {
            "Local storage needs attention • another local storage operation is in progress"
                .to_owned()
        }
        ros_storage::DatabaseBootstrapError::Storage(_) => {
            "Local storage needs attention • encrypted local storage could not be opened"
                .to_owned()
        }
        ros_storage::DatabaseBootstrapError::UnsupportedSecureStoragePlatform => {
            "Local storage needs attention • this platform is not yet supported for secure local storage"
                .to_owned()
        }
    }
}

fn unavailable_staff_security(storage_status: String) -> CommunityStaffSecurity {
    CommunityStaffSecurity {
        storage_status,
        available: false,
        owner_pin_setup_required: false,
        active_staff_id: None,
        active_staff_name: None,
        active_staff_role: None,
        staff: Vec::new(),
    }
}

fn unavailable_workspace(storage_status: String) -> CommunityWorkspace {
    CommunityWorkspace {
        storage_status,
        setup_required: true,
        branch_name: None,
        categories: Vec::new(),
        products: Vec::new(),
        customers: Vec::new(),
        open_drafts: Vec::new(),
        kitchen_tickets: Vec::new(),
    }
}

/// A provisioned but locked installation is deliberately different from an
/// unprovisioned one. Keeping `setup_required` false makes Flutter render the
/// PIN gate rather than accidentally treating a locked restaurant as a fresh
/// setup and exposing operational navigation with an empty projection.
fn locked_workspace(storage_status: String) -> CommunityWorkspace {
    CommunityWorkspace {
        storage_status,
        setup_required: false,
        branch_name: None,
        categories: Vec::new(),
        products: Vec::new(),
        customers: Vec::new(),
        open_drafts: Vec::new(),
        kitchen_tickets: Vec::new(),
    }
}

fn unavailable_sale(storage_status: String) -> CommunitySaleResult {
    CommunitySaleResult {
        storage_status,
        completed: false,
        invoice_number: None,
        total_minor: 0,
        currency_code: None,
        payment_method: None,
    }
}

fn unavailable_sales_summary(storage_status: String) -> CommunitySalesSummary {
    CommunitySalesSummary {
        storage_status,
        available: false,
        accounting_date_utc: None,
        branch_time_zone: None,
        invoice_count: 0,
        total_minor: 0,
        cash_minor: 0,
        card_minor: 0,
        upi_minor: 0,
        refund_minor: 0,
        expense_minor: 0,
        discount_minor: 0,
        tax_minor: 0,
        currency_code: None,
        schema_version: 0,
        audit_event_count: 0,
        day_closed: false,
        day_closed_at_utc: None,
        day_close_reason: None,
        recent_invoices: Vec::new(),
        top_items: Vec::new(),
    }
}

fn unavailable_audit_timeline(storage_status: String) -> CommunityAuditTimeline {
    CommunityAuditTimeline {
        storage_status,
        available: false,
        events: Vec::new(),
    }
}

fn unavailable_sync_queue(storage_status: String) -> CommunitySyncQueue {
    CommunitySyncQueue {
        storage_status,
        available: false,
        pending_count: 0,
        operations: Vec::new(),
    }
}

fn unavailable_invoice_detail(storage_status: String) -> CommunityInvoiceDetail {
    CommunityInvoiceDetail {
        storage_status,
        available: false,
        invoice_id: None,
        invoice_number: None,
        fulfillment: None,
        subtotal_minor: None,
        discount_minor: None,
        tax_minor: None,
        total_minor: None,
        refunded_minor: None,
        currency_code: None,
        finalized_at_utc: None,
        lines: Vec::new(),
        payments: Vec::new(),
    }
}

fn unavailable_inventory(storage_status: String) -> CommunityInventoryWorkspace {
    CommunityInventoryWorkspace {
        storage_status,
        available: false,
        items: Vec::new(),
    }
}

fn unavailable_expenses(storage_status: String) -> CommunityExpensesWorkspace {
    CommunityExpensesWorkspace {
        storage_status,
        available: false,
        currency_code: None,
        total_minor: 0,
        expenses: Vec::new(),
    }
}

fn unavailable_cash_drawer(storage_status: String) -> CommunityCashDrawerResult {
    CommunityCashDrawerResult {
        storage_status,
        completed: false,
        session_id: None,
        expected_cash_minor: 0,
        counted_cash_minor: 0,
        variance_minor: 0,
    }
}

fn unavailable_backup(storage_status: String) -> CommunityBackupResult {
    CommunityBackupResult {
        storage_status,
        created: false,
        backup_file_name: None,
        sha256: None,
    }
}

fn unavailable_diagnostics(storage_status: String) -> CommunityDiagnosticsWorkspace {
    CommunityDiagnosticsWorkspace {
        storage_status,
        available: false,
        events: Vec::new(),
    }
}

fn unavailable_diagnostics_pack(storage_status: String) -> CommunityDiagnosticsPack {
    CommunityDiagnosticsPack {
        storage_status,
        available: false,
        json_bytes: Vec::new(),
        sha256: None,
        event_count: 0,
        byte_length: 0,
    }
}

fn diagnostic_platform() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "unknown"
    }
}

fn diagnostic_app_channel() -> &'static str {
    if cfg!(feature = "development-sqlcipher") {
        "development"
    } else {
        "production"
    }
}

fn diagnostic_component_label(component: ros_diagnostics::DiagnosticComponent) -> &'static str {
    match component {
        ros_diagnostics::DiagnosticComponent::Bootstrap => "bootstrap",
        ros_diagnostics::DiagnosticComponent::Storage => "storage",
        ros_diagnostics::DiagnosticComponent::Bridge => "bridge",
        ros_diagnostics::DiagnosticComponent::Ui => "ui",
        ros_diagnostics::DiagnosticComponent::Share => "share",
    }
}

fn diagnostic_outcome_label(outcome: ros_diagnostics::DiagnosticOutcome) -> &'static str {
    match outcome {
        ros_diagnostics::DiagnosticOutcome::Ok => "ok",
        ros_diagnostics::DiagnosticOutcome::Denied => "denied",
        ros_diagnostics::DiagnosticOutcome::Failed => "failed",
        ros_diagnostics::DiagnosticOutcome::Busy => "busy",
        ros_diagnostics::DiagnosticOutcome::Throttled => "throttled",
        ros_diagnostics::DiagnosticOutcome::Unavailable => "unavailable",
    }
}

fn record_diagnostic(
    application_support_directory: &str,
    event_code: &str,
    component: ros_diagnostics::DiagnosticComponent,
    outcome: ros_diagnostics::DiagnosticOutcome,
    duration_ms: Option<u64>,
    detail_code: Option<&str>,
) -> bool {
    let Ok(sink) = ros_diagnostics::LocalDiagnosticsSink::open(application_support_directory) else {
        return false;
    };
    sink.record(ros_diagnostics::DiagnosticRecordInput {
        event_code,
        component,
        outcome,
        duration_ms,
        platform: diagnostic_platform(),
        app_channel: diagnostic_app_channel(),
        detail_code,
    })
    .is_ok()
}

fn unavailable_financial_csv_export(storage_status: String) -> CommunityFinancialCsvExport {
    CommunityFinancialCsvExport {
        storage_status,
        available: false,
        csv_bytes: Vec::new(),
        record_count: 0,
        byte_length: 0,
    }
}

fn unavailable_draft(storage_status: String) -> CommunityDraftOrderResult {
    CommunityDraftOrderResult {
        storage_status,
        saved: false,
        draft_order_id: None,
        revision: 0,
    }
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue_source_image() -> Vec<u8> {
        let source = image::RgbImage::from_pixel(96, 64, image::Rgb([180, 90, 20]));
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 92)
            .encode_image(&source)
            .expect("source JPEG");
        bytes
    }

    fn catalogue_selection(bytes: Vec<u8>) -> GotiginCatalogMenuImageSelection {
        GotiginCatalogMenuImageSelection {
            catalog_image_id: "dish.indian.biryani.1".to_owned(),
            content_sha256: format!("{:x}", Sha256::digest(&bytes)),
            original_image_bytes: bytes,
            licence_label: "Pexels License".to_owned(),
            licence_url: "https://www.pexels.com/legal-pages/license/".to_owned(),
            service_origin: GOTIGIN_CATALOG_SERVICE_ORIGIN.to_owned(),
            service_schema_version: GOTIGIN_CATALOG_SERVICE_SCHEMA_VERSION,
        }
    }

    #[test]
    fn catalogue_selection_is_reverified_and_retains_source_provenance() {
        let source = catalogue_source_image();
        let source_digest = Sha256::digest(&source).to_vec();
        let normalized = normalize_gotigin_catalog_menu_image(catalogue_selection(source))
            .expect("verified catalogue image");

        match normalized {
            ros_storage::ProductImageContent::GotiginCatalog {
                jpeg_bytes,
                pixel_width,
                pixel_height,
                provenance,
            } => {
                assert!(jpeg_bytes.len() <= MAX_MENU_IMAGE_STORED_BYTES);
                assert!((1..=320).contains(&pixel_width));
                assert!((1..=240).contains(&pixel_height));
                assert_eq!(provenance.catalog_image_id(), "dish.indian.biryani.1");
                assert_eq!(provenance.original_content_sha256(), source_digest);
                assert_eq!(provenance.service_origin(), GOTIGIN_CATALOG_SERVICE_ORIGIN);
            }
            _ => panic!("catalogue selection must retain its source kind"),
        }
    }

    #[test]
    fn catalogue_selection_fails_closed_on_tampering_or_contract_drift() {
        let mut tampered = catalogue_selection(catalogue_source_image());
        tampered.original_image_bytes.push(0);
        assert!(normalize_gotigin_catalog_menu_image(tampered).is_err());

        let mut wrong_origin = catalogue_selection(catalogue_source_image());
        wrong_origin.service_origin = "https://example.com".to_owned();
        assert!(normalize_gotigin_catalog_menu_image(wrong_origin).is_err());

        let mut bad_licence = catalogue_selection(catalogue_source_image());
        bad_licence.licence_url = "http://example.com/licence".to_owned();
        assert!(normalize_gotigin_catalog_menu_image(bad_licence).is_err());
    }

    #[test]
    fn restaurant_upload_is_reencoded_as_a_small_menu_jpeg() {
        let source = image::RgbImage::from_pixel(640, 480, image::Rgb([180, 90, 20]));
        let mut source_bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut source_bytes, 92)
            .encode_image(&source)
            .expect("source JPEG");

        let normalized =
            normalize_restaurant_menu_image(source_bytes).expect("normalised menu image");
        match normalized {
            ros_storage::ProductImageContent::RestaurantUpload {
                jpeg_bytes,
                pixel_width,
                pixel_height,
            } => {
                assert!(jpeg_bytes.len() <= MAX_MENU_IMAGE_STORED_BYTES);
                assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8]));
                assert!(jpeg_bytes.ends_with(&[0xFF, 0xD9]));
                assert!((1..=320).contains(&pixel_width));
                assert!((1..=240).contains(&pixel_height));
            }
            ros_storage::ProductImageContent::BuiltIn { .. } => {
                panic!("a restaurant upload must remain an upload")
            }
            ros_storage::ProductImageContent::GotiginCatalog { .. } => {
                panic!("a restaurant upload must not become a catalogue image")
            }
        }
    }

    #[test]
    fn restaurant_upload_applies_exif_orientation_before_thumbnailing() {
        let source = image::RgbImage::from_pixel(80, 40, image::Rgb([40, 120, 200]));
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 92)
            .encode_image(&source)
            .expect("source JPEG");

        // Insert an Exif orientation-6 (rotate 90° clockwise) APP1 segment
        // immediately after the JPEG SOI marker. The stored thumbnail must
        // reflect the visual orientation rather than the raw sensor pixels.
        let mut oriented_jpeg = jpeg[..2].to_vec();
        oriented_jpeg.extend_from_slice(&[
            0xFF, 0xE1, 0x00, 0x22, b'E', b'x', b'i', b'f', 0x00, 0x00, 0x49, 0x49, 0x2A, 0x00,
            0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);
        oriented_jpeg.extend_from_slice(&jpeg[2..]);

        let normalized =
            normalize_restaurant_menu_image(oriented_jpeg).expect("oriented menu image");
        match normalized {
            ros_storage::ProductImageContent::RestaurantUpload {
                pixel_width,
                pixel_height,
                ..
            } => {
                assert_eq!((pixel_width, pixel_height), (40, 80));
            }
            ros_storage::ProductImageContent::BuiltIn { .. } => {
                panic!("a restaurant upload must remain an upload")
            }
            ros_storage::ProductImageContent::GotiginCatalog { .. } => {
                panic!("a restaurant upload must not become a catalogue image")
            }
        }
    }

    #[test]
    fn malformed_or_oversized_upload_is_rejected_before_storage() {
        assert!(normalize_restaurant_menu_image(vec![1, 2, 3]).is_err());
        assert!(normalize_restaurant_menu_image(vec![0; MAX_MENU_IMAGE_INPUT_BYTES + 1]).is_err());
    }

    #[test]
    fn unsupported_or_unknown_upload_formats_are_rejected_before_decoding() {
        let unsupported_or_unknown_headers = [
            b"GIF89a\x01\x00\x01\x00\x80\x00\x00".as_slice(),
            b"BM\x00\x00\x00\x00\x00\x00\x00\x00".as_slice(),
            b"II*\x00\x08\x00\x00\x00".as_slice(),
            b"not an image format".as_slice(),
        ];

        for source in unsupported_or_unknown_headers {
            assert_eq!(
                normalize_restaurant_menu_image(source.to_vec()).unwrap_err(),
                "choose a JPEG, PNG, or WebP image"
            );
        }
    }

    #[test]
    fn form_preparation_compresses_before_enforcing_the_three_mib_limit() {
        let source = image::RgbImage::from_pixel(640, 480, image::Rgb([20, 130, 180]));
        let mut source_bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut source_bytes, 92)
            .encode_image(&source)
            .expect("source JPEG");

        let prepared = prepare_community_menu_image(source_bytes);

        assert!(!prepared.is_empty());
        assert!(prepared.len() <= MAX_MENU_IMAGE_ACCEPTED_BYTES);
        assert!(prepared.len() <= MAX_MENU_IMAGE_STORED_BYTES);
        assert!(prepare_community_menu_image(vec![1, 2, 3]).is_empty());
    }
}
