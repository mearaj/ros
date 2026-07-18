//! Encrypted local database access for Restaurant Operating System.
//!
//! Flutter never opens SQLite directly. This crate configures and owns each
//! SQLCipher connection so a caller cannot accidentally create a plaintext
//! database or weaken transaction durability.

#![forbid(unsafe_code)]

mod receipts;
mod recovery;

pub use receipts::{encode_escpos_receipt, encode_simple_pdf_receipt};
pub use recovery::{PortableRecoveryEnvelope, RECOVERY_ENVELOPE_VERSION};

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use argon2::password_hash::SaltString;
use argon2::{
    Algorithm, Argon2, Params as Argon2Params, PasswordHash, PasswordHasher, PasswordVerifier,
    Version,
};
use rand_core::OsRng;
use ros_core::pricing::{OrderDiscount, PricingBreakdown, PricingLineInput, TaxRate, TaxTreatment};
use ros_core::{
    ActorRole, Branch, Category, CommunitySetup, CompleteSale, CreateCategory,
    CreateModifierOption, CreateProduct, CurrencyCode, DisplayName, EntityId, ModifierOption,
    Money, MutationContext, MutationReason, OrderFulfillment, PaymentMethod, Product,
    SaleLineInput, TimeZoneId,
};
use rusqlite::config::DbConfig;
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::Zeroizing;

const SCHEMA_VERSION: i64 = 34;
const LOCAL_SCHEMA_V1: &str =
    include_str!("../../../database/local-migrations/0001_foundation.sql");
const LOCAL_SCHEMA_V1_CHECKSUM: &str =
    "sha256:a9645cbf79e62f35c44f039e390a0613be1be4b2a9cfc3f23feb91d0e7eca7fd";
const LOCAL_SCHEMA_V2: &str =
    include_str!("../../../database/local-migrations/0002_tenant_catalog.sql");
const LOCAL_SCHEMA_V2_CHECKSUM: &str =
    "sha256:d563729fdb24e045260f38643c076a82fc22aee231eccd979403f3132dd7a5a2";
const LOCAL_SCHEMA_V3: &str =
    include_str!("../../../database/local-migrations/0003_local_installation_identity.sql");
const LOCAL_SCHEMA_V3_CHECKSUM: &str =
    "sha256:83bd73e43aeba4fd2fe22bfbff8484c05a4aee91206f54c1637a2f9674d3a6a8";
const LOCAL_SCHEMA_V4: &str =
    include_str!("../../../database/local-migrations/0004_sales_invoices_and_outbox.sql");
const LOCAL_SCHEMA_V4_CHECKSUM: &str =
    "sha256:c3365bd1ef7d37f409ecc00bc17e3c0f22c6a21a07bae7c9e0d2e11438456326";
const LOCAL_SCHEMA_V5: &str =
    include_str!("../../../database/local-migrations/0005_financial_integrity_hardening.sql");
const LOCAL_SCHEMA_V5_CHECKSUM: &str =
    "sha256:fa2e54732f3984c2ae6634cdf9755182760c73ad2c668d3418755356b16f53e5";
const LOCAL_SCHEMA_V6: &str =
    include_str!("../../../database/local-migrations/0006_menu_item_images.sql");
const LOCAL_SCHEMA_V6_CHECKSUM: &str =
    "sha256:9dabf4d881174672eef8da6edb3abfdd7c076c719e925cbeb8fbdafe18316b1b";
const LOCAL_SCHEMA_V7: &str = include_str!(
    "../../../database/local-migrations/0007_catalog_price_updates_and_unused_product_deletion.sql"
);
const LOCAL_SCHEMA_V7_CHECKSUM: &str =
    "sha256:0878a248831938a5135d8ccb59f20032ad484d626b760953c384e20d7f650555";
const LOCAL_SCHEMA_V8: &str =
    include_str!("../../../database/local-migrations/0008_restaurant_tables_and_draft_orders.sql");
const LOCAL_SCHEMA_V8_CHECKSUM: &str =
    "sha256:0b4398d078192f0d2849d02c2799c7b60ccf276fedc08df861b8bf10e47d7898";
const LOCAL_SCHEMA_V9: &str =
    include_str!("../../../database/local-migrations/0009_draft_order_settlements.sql");
const LOCAL_SCHEMA_V9_CHECKSUM: &str =
    "sha256:ab561b7799609498dc23af4b0f4129272a7c7a698e936cde91010b8a3e9db466";
const LOCAL_SCHEMA_V10: &str =
    include_str!("../../../database/local-migrations/0010_kitchen_tickets.sql");
const LOCAL_SCHEMA_V10_CHECKSUM: &str =
    "sha256:1fff95a63f31868a598257e29f137b6bb9f709c4eb8369967ae5dcceb39f0d04";
const LOCAL_SCHEMA_V11: &str =
    include_str!("../../../database/local-migrations/0011_invoice_refunds.sql");
const LOCAL_SCHEMA_V11_CHECKSUM: &str =
    "sha256:bbb76c702407528f42d6a833aca04c53ac4e54c53ff8e5be8fbe0e83665bfffd";
const LOCAL_SCHEMA_V12: &str =
    include_str!("../../../database/local-migrations/0012_inventory_movements.sql");
const LOCAL_SCHEMA_V12_CHECKSUM: &str =
    "sha256:097a0256f7f514b21105f52c5e4157a27d4e6f3f9bccc2f53e8416ee4fa91705";
const LOCAL_SCHEMA_V13: &str =
    include_str!("../../../database/local-migrations/0013_inventory_opening_once.sql");
const LOCAL_SCHEMA_V13_CHECKSUM: &str =
    "sha256:eb6fa95f43cf83c5900a627fe26d40e30ed73cb80ccaa1ff5c02cf8d4de6fee6";
const LOCAL_SCHEMA_V14: &str = include_str!("../../../database/local-migrations/0014_expenses.sql");
const LOCAL_SCHEMA_V14_CHECKSUM: &str =
    "sha256:486b4cc14a268b1ee7ba6a0dfbe70895ef53427f503891a71770271b44e6b6bb";
const LOCAL_SCHEMA_V15: &str =
    include_str!("../../../database/local-migrations/0015_cash_drawer_sessions.sql");
const LOCAL_SCHEMA_V15_CHECKSUM: &str =
    "sha256:88cb19bbbe6f25f5d8296be0300cc6e8f7fd6be4c869c3fa26eb167b064252a1";
const LOCAL_SCHEMA_V16: &str =
    include_str!("../../../database/local-migrations/0016_local_staff_security.sql");
const LOCAL_SCHEMA_V16_CHECKSUM: &str =
    "sha256:764dd452a17fdbf4869cbfa0cb13161f3459550fcd39794f1129da30cad5b568";
const LOCAL_SCHEMA_V17: &str =
    include_str!("../../../database/local-migrations/0017_customers_and_privacy.sql");
const LOCAL_SCHEMA_V17_CHECKSUM: &str =
    "sha256:a7187949d32d1f93df34fdeab400347c6ba7ea111e0936505f97acc88ee665d3";
const LOCAL_SCHEMA_V18: &str =
    include_str!("../../../database/local-migrations/0018_split_payment_refund_integrity.sql");
const LOCAL_SCHEMA_V18_CHECKSUM: &str =
    "sha256:cafc51560dcde01f073beab9c1c993f3f6854f375fa66ce3b3ddfb245c1e6a5f";
const LOCAL_SCHEMA_V19: &str =
    include_str!("../../../database/local-migrations/0019_staff_role_history.sql");
const LOCAL_SCHEMA_V19_CHECKSUM: &str =
    "sha256:604396cca8f5459d92ef7906c76c2b4cb509ed4896d840e3d34e5c67d6b82e6c";
const LOCAL_SCHEMA_V20: &str =
    include_str!("../../../database/local-migrations/0020_inventory_low_stock_thresholds.sql");
const LOCAL_SCHEMA_V20_CHECKSUM: &str =
    "sha256:b5341c7378014c68f11d9d4b21dd3b067662906b4809899e902c37d0bd4f2031";
const LOCAL_SCHEMA_V21: &str = include_str!(
    "../../../database/local-migrations/0021_inventory_low_stock_threshold_clears.sql"
);
const LOCAL_SCHEMA_V21_CHECKSUM: &str =
    "sha256:2539d22e855014c569a767902ad602dce71d1c200027d9a33b61898a88bfefe9";
const LOCAL_SCHEMA_V22: &str =
    include_str!("../../../database/local-migrations/0022_kitchen_ticket_cancellations.sql");
const LOCAL_SCHEMA_V22_CHECKSUM: &str =
    "sha256:8648e2c1ad6d45abca1104c34f3be6ccd54788d382a2ad3dc88ec60db93d4d48";
const LOCAL_SCHEMA_V23: &str = include_str!(
    "../../../database/local-migrations/0023_kitchen_cancellation_relationship_integrity.sql"
);
const LOCAL_SCHEMA_V23_CHECKSUM: &str =
    "sha256:c628d54e48b72494484f046c639a60dea791b08c8c6229a1c94083606ccdf61c";
const LOCAL_SCHEMA_V24: &str =
    include_str!("../../../database/local-migrations/0024_kitchen_notes.sql");
const LOCAL_SCHEMA_V24_CHECKSUM: &str =
    "sha256:dbdac654cdb6338730ace987b807e1c860ea5de8a4e5ba4f4580498beaac0ed3";
const LOCAL_SCHEMA_V25: &str =
    include_str!("../../../database/local-migrations/0025_product_modifier_options.sql");
const LOCAL_SCHEMA_V25_CHECKSUM: &str =
    "sha256:0ac7fd3b459f0c121cf67fc4c0ca085bffd68c7303778b0af7b28b59eb68f1fe";
const LOCAL_SCHEMA_V26: &str =
    include_str!("../../../database/local-migrations/0026_local_authorization_hardening.sql");
const LOCAL_SCHEMA_V26_CHECKSUM: &str =
    "sha256:c60b1de1f5eb33bf3958030895f77961c2bc2ffa24b0186fa3bd7ce4e1f9fde7";
const LOCAL_SCHEMA_V27: &str =
    include_str!("../../../database/local-migrations/0027_product_image_catalog_provenance.sql");
const LOCAL_SCHEMA_V27_CHECKSUM: &str =
    "sha256:6c5db2b9332f5ed92d9759c66ee5c4369ea040be8919fb8c051730ec190287dd";
const LOCAL_SCHEMA_V28: &str =
    include_str!("../../../database/local-migrations/0028_pricing_adjustments.sql");
const LOCAL_SCHEMA_V28_CHECKSUM: &str =
    "sha256:c66a9206a52e653e4169e5457e3519ac66653d94eee3b9433979428908cfc44e";
const LOCAL_SCHEMA_V29: &str =
    include_str!("../../../database/local-migrations/0029_invoice_voids.sql");
const LOCAL_SCHEMA_V29_CHECKSUM: &str =
    "sha256:0f978deae1844755b31aa26688b4955bcc4c497be9621b0dc83de3f486506a59";
const LOCAL_SCHEMA_V30: &str =
    include_str!("../../../database/local-migrations/0030_accounting_day_closes.sql");
const LOCAL_SCHEMA_V30_CHECKSUM: &str =
    "sha256:d30877340ee2e813d7285d0515c5e5d83ae43a6cf8f70ee1bd3be7ddd7a6a0ea";
const LOCAL_SCHEMA_V31: &str =
    include_str!("../../../database/local-migrations/0031_correction_approvals.sql");
const LOCAL_SCHEMA_V31_CHECKSUM: &str =
    "sha256:92155c0a30e40e921f27c444d0f750f3d570d8f21d9ea72ed349021fdb064db9";
const LOCAL_SCHEMA_V32: &str =
    include_str!("../../../database/local-migrations/0032_suppliers_and_purchases.sql");
const LOCAL_SCHEMA_V32_CHECKSUM: &str =
    "sha256:6d81340f0e7f6b4b33962740fccd9ac818c4da515498e530bc65bc2cbac44ed2";
const LOCAL_SCHEMA_V33: &str =
    include_str!("../../../database/local-migrations/0033_product_recipes.sql");
const LOCAL_SCHEMA_V33_CHECKSUM: &str =
    "sha256:c8eb0380428f7b3ff74ac407b2106379c5d2e7da5d96dbf98c0508d30c3f36ce";
const LOCAL_SCHEMA_V34: &str =
    include_str!("../../../database/local-migrations/0034_owner_recovery_verifier.sql");
const LOCAL_SCHEMA_V34_CHECKSUM: &str =
    "sha256:64390b038277176d8f77bc9955f5f1db4af385d640134488bfe3665a3ac2aa38";
const LOCAL_STAFF_SESSION_MINUTES: i64 = 15;
const LOCAL_STAFF_PIN_MAXIMUM_FAILED_ATTEMPTS: i64 = 5;
const LOCAL_STAFF_PIN_THROTTLE_MINUTES: i64 = 15;
const MAX_MENU_IMAGE_BYTES: usize = 65_536;
const MAX_MENU_IMAGE_WIDTH: i64 = 320;
const MAX_MENU_IMAGE_HEIGHT: i64 = 240;
const GOTIGIN_CATALOG_SERVICE_ORIGIN: &str = "https://ros.gotigin.com";
const GOTIGIN_CATALOG_SERVICE_SCHEMA_VERSION: i64 = 1;
const MAX_CATALOG_IMAGE_ID_BYTES: usize = 128;
const MAX_CATALOG_LICENCE_LABEL_BYTES: usize = 160;
const MAX_CATALOG_LICENCE_URL_BYTES: usize = 2_048;
// Community reporting remains local-first, so a report must be bounded before
// it crosses the Rust/Flutter boundary. The export aggregates immutable facts
// by UTC accounting day and tender; this supports a decade of normal history
// without turning a single owner action into an unbounded allocation.
const MAX_FINANCIAL_CSV_RECORDS: i64 = 50_000;
const MAX_FINANCIAL_CSV_BYTES: usize = 4 * 1024 * 1024;
// A backup file is never opened at its user-selected destination.  Reserve a
// private, unpredictable sibling first, then atomically publish it without
// replacing an existing path.
const BACKUP_TEMPORARY_RESERVATION_ATTEMPTS: usize = 32;
const BUILT_IN_MENU_IMAGE_KEYS: [&str; 20] = [
    "biryani",
    "curry",
    "dosa",
    "idli",
    "snacks",
    "pizza",
    "burger",
    "pasta",
    "noodles",
    "rice",
    "sandwich",
    "salad",
    "soup",
    "coffee",
    "chai",
    "juice",
    "mocktail",
    "dessert",
    "ice_cream",
    "bakery",
];
pub const ENCRYPTED_STORAGE_ENGINE: &str = "SQLCipher-backed SQLite";

/// The build mode is selected by the Rust feature graph, never by Flutter.
///
/// In particular, a developer cannot point a debug UI at a production
/// database merely by changing a Dart define. The mode also selects a
/// separate platform-keyring namespace below.
#[cfg(feature = "development-bundled-sqlcipher")]
pub const LOCAL_BUILD_MODE: &str = "development";
#[cfg(all(
    not(feature = "development-bundled-sqlcipher"),
    feature = "production-sqlcipher"
))]
pub const LOCAL_BUILD_MODE: &str = "release";

// Development keys are deliberately namespaced away from release keys. This
// prevents a debug build and a release build from ever opening the same local
// ciphertext with the same OS-managed credential.
#[cfg(feature = "development-bundled-sqlcipher")]
const DATABASE_KEYRING_SERVICE: &str = "com.gotigin.ros.development.sqlcipher.v1";
#[cfg(all(
    not(feature = "development-bundled-sqlcipher"),
    feature = "production-sqlcipher"
))]
const DATABASE_KEYRING_SERVICE: &str = "com.gotigin.ros.sqlcipher.v1";
// Pre-rename service IDs. Load migrates once into DATABASE_KEYRING_SERVICE so
// existing desktop installs keep opening their encrypted database.
#[cfg(feature = "development-bundled-sqlcipher")]
const LEGACY_DATABASE_KEYRING_SERVICE: &str = "com.gotigin.restaurantos.development.sqlcipher.v1";
#[cfg(all(
    not(feature = "development-bundled-sqlcipher"),
    feature = "production-sqlcipher"
))]
const LEGACY_DATABASE_KEYRING_SERVICE: &str = "com.gotigin.restaurantos.sqlcipher.v1";
const COMMUNITY_DATABASE_KEYRING_ACCOUNT: &str = "community-default";
#[cfg(feature = "production-sqlcipher")]
const PRODUCTION_SQLCIPHER_VERSION_PREFIX: &str = "4.17.";
static KEY_STORE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(all(
    feature = "development-bundled-sqlcipher",
    feature = "production-sqlcipher"
))]
compile_error!(
    "Choose exactly one SQLCipher linkage path: development-bundled-sqlcipher or production-sqlcipher."
);

#[cfg(not(any(
    feature = "development-bundled-sqlcipher",
    feature = "production-sqlcipher"
)))]
compile_error!("A SQLCipher linkage path is required for ros_storage.");

#[cfg(all(not(debug_assertions), feature = "development-bundled-sqlcipher"))]
compile_error!(
    "Release/profile builds cannot use the development SQLCipher bundle. Build with the reviewed production-sqlcipher feature graph."
);

struct LocalMigration {
    version: i64,
    sql: &'static str,
    checksum: &'static str,
}

const LOCAL_MIGRATIONS: [LocalMigration; 34] = [
    LocalMigration {
        version: 1,
        sql: LOCAL_SCHEMA_V1,
        checksum: LOCAL_SCHEMA_V1_CHECKSUM,
    },
    LocalMigration {
        version: 2,
        sql: LOCAL_SCHEMA_V2,
        checksum: LOCAL_SCHEMA_V2_CHECKSUM,
    },
    LocalMigration {
        version: 3,
        sql: LOCAL_SCHEMA_V3,
        checksum: LOCAL_SCHEMA_V3_CHECKSUM,
    },
    LocalMigration {
        version: 4,
        sql: LOCAL_SCHEMA_V4,
        checksum: LOCAL_SCHEMA_V4_CHECKSUM,
    },
    LocalMigration {
        version: 5,
        sql: LOCAL_SCHEMA_V5,
        checksum: LOCAL_SCHEMA_V5_CHECKSUM,
    },
    LocalMigration {
        version: 6,
        sql: LOCAL_SCHEMA_V6,
        checksum: LOCAL_SCHEMA_V6_CHECKSUM,
    },
    LocalMigration {
        version: 7,
        sql: LOCAL_SCHEMA_V7,
        checksum: LOCAL_SCHEMA_V7_CHECKSUM,
    },
    LocalMigration {
        version: 8,
        sql: LOCAL_SCHEMA_V8,
        checksum: LOCAL_SCHEMA_V8_CHECKSUM,
    },
    LocalMigration {
        version: 9,
        sql: LOCAL_SCHEMA_V9,
        checksum: LOCAL_SCHEMA_V9_CHECKSUM,
    },
    LocalMigration {
        version: 10,
        sql: LOCAL_SCHEMA_V10,
        checksum: LOCAL_SCHEMA_V10_CHECKSUM,
    },
    LocalMigration {
        version: 11,
        sql: LOCAL_SCHEMA_V11,
        checksum: LOCAL_SCHEMA_V11_CHECKSUM,
    },
    LocalMigration {
        version: 12,
        sql: LOCAL_SCHEMA_V12,
        checksum: LOCAL_SCHEMA_V12_CHECKSUM,
    },
    LocalMigration {
        version: 13,
        sql: LOCAL_SCHEMA_V13,
        checksum: LOCAL_SCHEMA_V13_CHECKSUM,
    },
    LocalMigration {
        version: 14,
        sql: LOCAL_SCHEMA_V14,
        checksum: LOCAL_SCHEMA_V14_CHECKSUM,
    },
    LocalMigration {
        version: 15,
        sql: LOCAL_SCHEMA_V15,
        checksum: LOCAL_SCHEMA_V15_CHECKSUM,
    },
    LocalMigration {
        version: 16,
        sql: LOCAL_SCHEMA_V16,
        checksum: LOCAL_SCHEMA_V16_CHECKSUM,
    },
    LocalMigration {
        version: 17,
        sql: LOCAL_SCHEMA_V17,
        checksum: LOCAL_SCHEMA_V17_CHECKSUM,
    },
    LocalMigration {
        version: 18,
        sql: LOCAL_SCHEMA_V18,
        checksum: LOCAL_SCHEMA_V18_CHECKSUM,
    },
    LocalMigration {
        version: 19,
        sql: LOCAL_SCHEMA_V19,
        checksum: LOCAL_SCHEMA_V19_CHECKSUM,
    },
    LocalMigration {
        version: 20,
        sql: LOCAL_SCHEMA_V20,
        checksum: LOCAL_SCHEMA_V20_CHECKSUM,
    },
    LocalMigration {
        version: 21,
        sql: LOCAL_SCHEMA_V21,
        checksum: LOCAL_SCHEMA_V21_CHECKSUM,
    },
    LocalMigration {
        version: 22,
        sql: LOCAL_SCHEMA_V22,
        checksum: LOCAL_SCHEMA_V22_CHECKSUM,
    },
    LocalMigration {
        version: 23,
        sql: LOCAL_SCHEMA_V23,
        checksum: LOCAL_SCHEMA_V23_CHECKSUM,
    },
    LocalMigration {
        version: 24,
        sql: LOCAL_SCHEMA_V24,
        checksum: LOCAL_SCHEMA_V24_CHECKSUM,
    },
    LocalMigration {
        version: 25,
        sql: LOCAL_SCHEMA_V25,
        checksum: LOCAL_SCHEMA_V25_CHECKSUM,
    },
    LocalMigration {
        version: 26,
        sql: LOCAL_SCHEMA_V26,
        checksum: LOCAL_SCHEMA_V26_CHECKSUM,
    },
    LocalMigration {
        version: 27,
        sql: LOCAL_SCHEMA_V27,
        checksum: LOCAL_SCHEMA_V27_CHECKSUM,
    },
    LocalMigration {
        version: 28,
        sql: LOCAL_SCHEMA_V28,
        checksum: LOCAL_SCHEMA_V28_CHECKSUM,
    },
    LocalMigration {
        version: 29,
        sql: LOCAL_SCHEMA_V29,
        checksum: LOCAL_SCHEMA_V29_CHECKSUM,
    },
    LocalMigration {
        version: 30,
        sql: LOCAL_SCHEMA_V30,
        checksum: LOCAL_SCHEMA_V30_CHECKSUM,
    },
    LocalMigration {
        version: 31,
        sql: LOCAL_SCHEMA_V31,
        checksum: LOCAL_SCHEMA_V31_CHECKSUM,
    },
    LocalMigration {
        version: 32,
        sql: LOCAL_SCHEMA_V32,
        checksum: LOCAL_SCHEMA_V32_CHECKSUM,
    },
    LocalMigration {
        version: 33,
        sql: LOCAL_SCHEMA_V33,
        checksum: LOCAL_SCHEMA_V33_CHECKSUM,
    },
    LocalMigration {
        version: 34,
        sql: LOCAL_SCHEMA_V34,
        checksum: LOCAL_SCHEMA_V34_CHECKSUM,
    },
];

pub(crate) struct DatabaseKey(Zeroizing<[u8; 32]>);

impl DatabaseKey {
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn same_as(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }

    #[cfg(feature = "platform-keyring")]
    fn generate() -> Result<Self, KeyStoreError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| KeyStoreError::RandomnessUnavailable)?;
        Ok(Self::from_bytes(bytes))
    }

    #[cfg(feature = "platform-keyring")]
    fn from_stored_secret(secret: Vec<u8>) -> Result<Self, KeyStoreError> {
        let secret = Zeroizing::new(secret);
        let bytes: [u8; 32] = secret
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::CorruptKey)?;

        Ok(Self::from_bytes(bytes))
    }
}

#[derive(Clone, Copy)]
struct DatabaseKeySlot {
    service: &'static str,
    account: &'static str,
}

impl DatabaseKeySlot {
    const fn community_default() -> Self {
        Self {
            service: DATABASE_KEYRING_SERVICE,
            account: COMMUNITY_DATABASE_KEYRING_ACCOUNT,
        }
    }
}

trait DatabaseKeyStore {
    fn load(&self, slot: DatabaseKeySlot) -> Result<Option<DatabaseKey>, KeyStoreError>;
    fn store_new(&self, slot: DatabaseKeySlot, key: &DatabaseKey) -> Result<(), KeyStoreError>;
}

#[derive(Debug)]
pub enum KeyStoreError {
    CorruptKey,
    RandomnessUnavailable,
    SecureStorageUnavailable,
    WriteVerificationFailed,
}

impl fmt::Display for KeyStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorruptKey => formatter.write_str("The stored database key is invalid."),
            Self::RandomnessUnavailable => {
                formatter.write_str("The operating system could not generate secure random data.")
            }
            Self::SecureStorageUnavailable => {
                formatter.write_str("Secure operating-system storage is unavailable or locked.")
            }
            Self::WriteVerificationFailed => {
                formatter.write_str("The database key could not be verified after secure storage.")
            }
        }
    }
}

impl Error for KeyStoreError {}

#[derive(Debug)]
pub enum DatabaseBootstrapError {
    DatabaseKeyMissing,
    ExistingKeyWithoutDatabase,
    KeyStore(KeyStoreError),
    Lock(std::io::Error),
    Storage(StorageError),
    UnsupportedSecureStoragePlatform,
}

impl fmt::Display for DatabaseBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatabaseKeyMissing => formatter.write_str(
                "The encrypted restaurant database exists but its secure key is unavailable.",
            ),
            Self::ExistingKeyWithoutDatabase => formatter.write_str(
                "A secure database key exists without its database; recovery is required before setup.",
            ),
            Self::KeyStore(error) => write!(formatter, "Secure storage failed: {error}"),
            Self::Lock(_) => {
                formatter.write_str("The local database setup lock could not be acquired.")
            }
            Self::Storage(error) => write!(formatter, "Local database setup failed: {error}"),
            Self::UnsupportedSecureStoragePlatform => formatter.write_str(
                "This build does not yet support secure local storage on this platform.",
            ),
        }
    }
}

impl Error for DatabaseBootstrapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::KeyStore(error) => Some(error),
            Self::Lock(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::DatabaseKeyMissing
            | Self::ExistingKeyWithoutDatabase
            | Self::UnsupportedSecureStoragePlatform => None,
        }
    }
}

#[cfg(all(
    feature = "platform-keyring",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
struct PlatformDatabaseKeyStore;

#[cfg(all(
    feature = "platform-keyring",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
impl DatabaseKeyStore for PlatformDatabaseKeyStore {
    fn load(&self, slot: DatabaseKeySlot) -> Result<Option<DatabaseKey>, KeyStoreError> {
        let _guard = key_store_lock();
        match load_keyring_secret(slot.service, slot.account)? {
            Some(key) => Ok(Some(key)),
            None => migrate_legacy_keyring_secret(slot),
        }
    }

    fn store_new(&self, slot: DatabaseKeySlot, key: &DatabaseKey) -> Result<(), KeyStoreError> {
        let _guard = key_store_lock();
        store_keyring_secret(slot.service, slot.account, key)
    }
}

#[cfg(all(
    feature = "platform-keyring",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn load_keyring_secret(service: &str, account: &str) -> Result<Option<DatabaseKey>, KeyStoreError> {
    let entry = keyring::Entry::new(service, account)
        .map_err(|_| KeyStoreError::SecureStorageUnavailable)?;

    match entry.get_secret() {
        Ok(secret) => DatabaseKey::from_stored_secret(secret).map(Some),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(KeyStoreError::SecureStorageUnavailable),
    }
}

#[cfg(all(
    feature = "platform-keyring",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn store_keyring_secret(
    service: &str,
    account: &str,
    key: &DatabaseKey,
) -> Result<(), KeyStoreError> {
    let entry = keyring::Entry::new(service, account)
        .map_err(|_| KeyStoreError::SecureStorageUnavailable)?;
    entry
        .set_secret(key.as_bytes())
        .map_err(|_| KeyStoreError::SecureStorageUnavailable)
}

#[cfg(all(
    feature = "platform-keyring",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn migrate_legacy_keyring_secret(
    slot: DatabaseKeySlot,
) -> Result<Option<DatabaseKey>, KeyStoreError> {
    if slot.service == LEGACY_DATABASE_KEYRING_SERVICE {
        return Ok(None);
    }

    let Some(key) = load_keyring_secret(LEGACY_DATABASE_KEYRING_SERVICE, slot.account)? else {
        return Ok(None);
    };

    store_keyring_secret(slot.service, slot.account, &key)?;
    let verified = load_keyring_secret(slot.service, slot.account)?
        .ok_or(KeyStoreError::WriteVerificationFailed)?;
    if !verified.same_as(&key) {
        return Err(KeyStoreError::WriteVerificationFailed);
    }

    // Best-effort cleanup of the pre-rename service ID. Failure here must not
    // block opening an already-verified current entry.
    if let Ok(legacy_entry) = keyring::Entry::new(LEGACY_DATABASE_KEYRING_SERVICE, slot.account) {
        let _ = legacy_entry.delete_credential();
    }

    Ok(Some(key))
}

fn key_store_lock() -> std::sync::MutexGuard<'static, ()> {
    KEY_STORE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned_lock| poisoned_lock.into_inner())
}

pub struct LocalDatabase {
    connection: Connection,
    key: DatabaseKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLocalBackup {
    sha256: String,
    schema_version: i64,
    byte_length: u64,
}

/// Second-person credentials for correction-approval.v1 (ADR 0006).
pub struct DualPersonApproval<'a> {
    pub approver_actor_id: EntityId,
    pub approver_pin: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Supplier {
    supplier_id: EntityId,
    display_name: String,
    revision: i64,
}

impl Supplier {
    pub fn supplier_id(&self) -> &EntityId {
        &self.supplier_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn revision(&self) -> i64 {
        self.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurchaseLineInput {
    pub product_id: EntityId,
    pub quantity: i64,
    pub unit_cost_minor: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeLineInput {
    pub ingredient_product_id: EntityId,
    pub quantity_per_unit: i64,
}

/// An exclusively-created backup staging file. The open file handle remains
/// live until publication so Unix and Windows file identities can be checked
/// before a path is linked into the caller-selected destination.
struct ReservedLocalBackupDestination {
    path: PathBuf,
    file: File,
}

impl ReservedLocalBackupDestination {
    fn reserve(destination: &Path) -> Result<Self, StorageError> {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let destination_file_name = destination
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                StorageError::InvalidPersistedData("backup destination must name a file".to_owned())
            })?;

        fs::create_dir_all(parent).map_err(StorageError::Io)?;
        for _ in 0..BACKUP_TEMPORARY_RESERVATION_ATTEMPTS {
            let path = backup_temporary_path(parent, destination_file_name);
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok(Self { path, file }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(StorageError::Io(error)),
            }
        }

        Err(StorageError::InvalidPersistedData(
            "could not reserve a unique backup destination".to_owned(),
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn sync_and_hash(&self) -> Result<(u64, String), StorageError> {
        self.ensure_path_still_references_reservation()?;
        self.file.sync_all().map_err(StorageError::Io)?;
        self.ensure_path_still_references_reservation()?;
        sha256_file(&self.file)
    }

    /// Publishes the backup with `link(2)`/`CreateHardLinkW` semantics rather
    /// than `rename`, because the standard rename operation may replace an
    /// existing destination. The temporary path lives beside the destination,
    /// so a hard link never crosses filesystems.
    fn finalize_without_replacement(&self, destination: &Path) -> Result<(), StorageError> {
        self.ensure_path_still_references_reservation()?;
        fs::hard_link(&self.path, destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                StorageError::InvalidPersistedData("backup destination already exists".to_owned())
            } else {
                StorageError::Io(error)
            }
        })?;

        // A hostile writer might replace a directory entry immediately after
        // the hard link call. Verify that the published path still references
        // the file we reserved before deleting any staging artifact.
        if !path_references_file(&self.file, destination)? {
            return Err(StorageError::InvalidPersistedData(
                "backup destination changed while it was being published".to_owned(),
            ));
        }

        self.remove_staging_artifacts()
    }

    fn ensure_path_still_references_reservation(&self) -> Result<(), StorageError> {
        if path_references_file(&self.file, &self.path)? {
            Ok(())
        } else {
            Err(StorageError::InvalidPersistedData(
                "backup temporary destination changed during creation".to_owned(),
            ))
        }
    }

    fn remove_staging_artifacts(&self) -> Result<(), StorageError> {
        self.ensure_path_still_references_reservation()?;
        fs::remove_file(&self.path).map_err(StorageError::Io)?;
        remove_backup_sidecars(&self.path);
        Ok(())
    }
}

impl Drop for ReservedLocalBackupDestination {
    fn drop(&mut self) {
        // Never remove a path that was swapped after reservation. The live
        // handle lets Unix and Windows builds distinguish our file from a
        // replacement, while the fallback remains fail-closed for symlinks.
        if path_references_file(&self.file, &self.path).unwrap_or(false) {
            let _ = fs::remove_file(&self.path);
            remove_backup_sidecars(&self.path);
        }
    }
}

fn backup_temporary_path(parent: &Path, destination_file_name: &OsStr) -> PathBuf {
    let mut temporary_file_name = OsString::from(".");
    temporary_file_name.push(destination_file_name);
    temporary_file_name.push(".ros-backup-");
    temporary_file_name.push(EntityId::new_v7().to_string());
    temporary_file_name.push(".partial");
    parent.join(temporary_file_name)
}

fn backup_sidecar_paths(path: &Path) -> [PathBuf; 3] {
    ["-journal", "-shm", "-wal"].map(|suffix| {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        PathBuf::from(sidecar)
    })
}

fn remove_backup_sidecars(path: &Path) {
    for sidecar in backup_sidecar_paths(path) {
        let _ = fs::remove_file(sidecar);
    }
}

fn ensure_backup_sidecars_are_absent(path: &Path) -> Result<(), StorageError> {
    for sidecar in backup_sidecar_paths(path) {
        match fs::symlink_metadata(&sidecar) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(StorageError::InvalidPersistedData(
                    "backup staging sidecar remained after the database closed".to_owned(),
                ));
            }
            Err(error) => return Err(StorageError::Io(error)),
        }
    }
    Ok(())
}

fn path_references_file(file: &File, path: &Path) -> Result<bool, StorageError> {
    let path_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(StorageError::Io(error)),
    };
    if !path_metadata.file_type().is_file() {
        return Ok(false);
    }

    let file_metadata = file.metadata().map_err(StorageError::Io)?;
    Ok(same_file_identity(&file_metadata, &path_metadata))
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    // `MetadataExt::volume_serial_number` / `file_index` remain unstable on
    // stable Rust (`windows_by_handle`). Keep a conservative comparison; the
    // SQLite no-follow open still rejects symlinks for the backup path.
    left.len() == right.len()
        && left.file_type().is_file()
        && right.file_type().is_file()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(not(any(unix, windows)))]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    // There is no stable file-identity API for this target in `std`. The
    // SQLite no-follow open still rejects symlinks, and this conservative
    // comparison detects ordinary replacement without claiming descriptor-
    // based identity guarantees unavailable on the platform.
    left.len() == right.len()
        && left.file_type().is_file()
        && right.file_type().is_file()
        && left.modified().ok() == right.modified().ok()
}

fn sha256_file(file: &File) -> Result<(u64, String), StorageError> {
    let mut reader = file.try_clone().map_err(StorageError::Io)?;
    reader.seek(SeekFrom::Start(0)).map_err(StorageError::Io)?;

    let mut hasher = Sha256::new();
    let mut byte_length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(StorageError::Io)?;
        if read == 0 {
            break;
        }
        byte_length = byte_length
            .checked_add(u64::try_from(read).map_err(|_| StorageError::FinancialAmountOverflow)?)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        hasher.update(&buffer[..read]);
    }

    Ok((byte_length, lowercase_hex(&hasher.finalize())))
}

/// Immutable facts produced by a successfully committed local sale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedSale {
    order_id: EntityId,
    invoice_id: EntityId,
    invoice_number: i64,
    total: Money,
    payment_method: PaymentMethod,
}

/// The current, mutable operational view of an order. Its individual saved
/// revisions are immutable snapshots in storage; this summary contains no
/// client-provided prices or financial-finalization facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftOrder {
    draft_order_id: EntityId,
    fulfillment: OrderFulfillment,
    state: String,
    table_name: Option<String>,
    kitchen_note: Option<String>,
    revision: i64,
    subtotal: Money,
    line_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenDraftOrder {
    draft: DraftOrder,
    lines: Vec<SaleLineInput>,
}

/// Counter-owned input for one append-only draft revision. Prices and totals
/// are deliberately absent: storage reloads those trusted facts itself.
///
/// The kitchen-note override has three states internally: preserve the
/// previous instruction for legacy callers, explicitly clear it, or set a new
/// instruction. Constructors keep that distinction out of application code.
pub struct DraftOrderSaveRequest<'a> {
    draft_order_id: Option<&'a EntityId>,
    expected_revision: Option<i64>,
    fulfillment: OrderFulfillment,
    table_name: Option<&'a str>,
    kitchen_note_override: Option<Option<&'a str>>,
    lines: &'a [SaleLineInput],
}

impl<'a> DraftOrderSaveRequest<'a> {
    /// A compatibility request that preserves an existing instruction when a
    /// draft is updated. New drafts start without one.
    pub fn preserving(
        draft_order_id: Option<&'a EntityId>,
        expected_revision: Option<i64>,
        fulfillment: OrderFulfillment,
        table_name: Option<&'a str>,
        lines: &'a [SaleLineInput],
    ) -> Self {
        Self {
            draft_order_id,
            expected_revision,
            fulfillment,
            table_name,
            kitchen_note_override: None,
            lines,
        }
    }

    /// An explicit instruction value for this revision. `None` means clear
    /// the prior value; blank text is normalized to absent by storage.
    pub fn with_kitchen_note(
        draft_order_id: Option<&'a EntityId>,
        expected_revision: Option<i64>,
        fulfillment: OrderFulfillment,
        table_name: Option<&'a str>,
        kitchen_note: Option<&'a str>,
        lines: &'a [SaleLineInput],
    ) -> Self {
        Self {
            draft_order_id,
            expected_revision,
            fulfillment,
            table_name,
            kitchen_note_override: Some(kitchen_note),
            lines,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KitchenTicket {
    ticket_id: EntityId,
    draft_order_id: EntityId,
    draft_revision: i64,
    state: String,
    table_label: Option<String>,
    line_snapshot_json: String,
    kitchen_note: Option<String>,
    revision: i64,
    cancellation_pending: bool,
}

/// The exact open draft revision a new Kitchen ticket may snapshot. Keeping it
/// named prevents a positional SQL tuple from accidentally swapping a table,
/// line snapshot, or instruction before the immutable ticket is written.
struct DraftKitchenSendSource {
    revision: i64,
    state: String,
    table_label: Option<String>,
    line_snapshot_json: String,
    kitchen_note: Option<String>,
}

/// Mutable operational fields plus immutable snapshot material used when a
/// Kitchen ticket advances. State may change; the ticket's order data and
/// kitchen instruction remain copied exactly from its original revision.
struct MutableKitchenTicketState {
    state: String,
    revision: i64,
    draft_order_id: EntityId,
    draft_revision: i64,
    table_label: Option<String>,
    line_snapshot_json: String,
    kitchen_note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSalesSummary {
    invoice_count: i64,
    total_minor: i64,
    cash_minor: i64,
    card_minor: i64,
    upi_minor: i64,
    refund_minor: i64,
    expense_minor: i64,
    discount_minor: i64,
    tax_minor: i64,
    currency_code: String,
}

/// Immutable snapshot recorded when an owner/manager closes a UTC accounting
/// day. Totals are frozen at close time; reopen is intentionally unsupported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAccountingDayClose {
    close_id: EntityId,
    accounting_date_utc: String,
    invoice_count: i64,
    total_minor: i64,
    cash_minor: i64,
    card_minor: i64,
    upi_minor: i64,
    refund_minor: i64,
    expense_minor: i64,
    discount_minor: i64,
    tax_minor: i64,
    currency_code: String,
    reason: String,
    closed_at_utc: String,
}

/// Gross item-sales projection from immutable invoice line snapshots. Refunds
/// are intentionally not allocated per item because an invoice-level refund
/// cannot be truthfully attributed to one line without an explicit policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalItemSalesSummary {
    display_name: String,
    quantity: i64,
    gross_total_minor: i64,
    currency_code: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalIntegrityReport {
    schema_version: i64,
    audit_event_count: i64,
}

/// A bounded CSV prepared from branch-scoped immutable financial facts. The
/// bytes stay in memory until the Flutter layer presents an explicit native
/// save destination; storage never writes an unencrypted report by itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFinancialCsvExport {
    csv_bytes: Vec<u8>,
    record_count: i64,
}

/// A deliberately narrow, owner-readable audit projection. Audit payloads can
/// contain operational context, so they remain internal to the Rust storage
/// boundary and are not exposed to the Flutter report surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAuditEventSummary {
    sequence: i64,
    event_type: String,
    occurred_at_utc: String,
}

/// An immutable, self-verifying event envelope for future Professional sync.
/// The cloud must accept an operation idempotently and validate this audit
/// chain before it acknowledges the operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSyncOperation {
    operation_id: EntityId,
    audit_event_id: EntityId,
    branch_id: EntityId,
    actor_id: EntityId,
    device_id: EntityId,
    sequence: i64,
    event_type: String,
    payload_json: String,
    occurred_at_utc: String,
    previous_hash: Option<Vec<u8>>,
    event_hash: Vec<u8>,
    entity_type: String,
    entity_id: String,
    correlation_id: String,
    created_at_utc: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBranchTaxRate {
    tax_rate_id: EntityId,
    display_name: String,
    basis_points: u32,
    revision: i64,
    archived: bool,
}

impl LocalBranchTaxRate {
    pub fn tax_rate_id(&self) -> &EntityId {
        &self.tax_rate_id
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub const fn basis_points(&self) -> u32 {
        self.basis_points
    }
    pub const fn revision(&self) -> i64 {
        self.revision
    }
    pub const fn archived(&self) -> bool {
        self.archived
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInvoiceSummary {
    invoice_id: EntityId,
    invoice_number: i64,
    total_minor: i64,
    currency_code: String,
    finalized_at_utc: String,
    payment_method: String,
}

/// A receipt-safe projection read solely from immutable order, invoice, and
/// payment records. It is deliberately separate from current catalog values,
/// so a later menu edit can never rewrite a historical receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInvoiceDetail {
    invoice_id: EntityId,
    invoice_number: i64,
    fulfillment: String,
    subtotal_minor: i64,
    discount_minor: i64,
    tax_minor: i64,
    total_minor: i64,
    refunded_minor: i64,
    currency_code: String,
    finalized_at_utc: String,
    lines: Vec<LocalInvoiceLine>,
    payments: Vec<LocalInvoicePayment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInvoiceLine {
    display_name: String,
    modifier_names: Vec<String>,
    quantity: i64,
    unit_price_minor: i64,
    line_total_minor: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInvoicePayment {
    payment_method: String,
    amount_minor: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalExpenseSummary {
    expense_id: EntityId,
    category: String,
    description: String,
    amount_minor: i64,
    currency_code: String,
    payment_method: String,
    incurred_at_utc: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CashDrawerClosure {
    pub session_id: EntityId,
    pub expected_cash_minor: i64,
    pub counted_cash_minor: i64,
    pub variance_minor: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCashDrawerSession {
    pub session_id: EntityId,
    pub opening_cash_minor: i64,
    pub currency_code: String,
}

/// A local staff projection deliberately excludes the credential hash and
/// authentication-attempt history. Flutter gets only what it needs to render
/// an unlock chooser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalStaffAccount {
    staff_id: EntityId,
    display_name: String,
    role: ActorRole,
    is_active: bool,
    has_pin: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalStaffSecurityState {
    owner_pin_setup_required: bool,
    active_staff: Option<LocalStaffAccount>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCustomer {
    customer_id: EntityId,
    display_name: String,
    phone_number: Option<String>,
    email_address: Option<String>,
    marketing_consent: bool,
    revision: i64,
}

/// Untrusted customer profile values entering the storage boundary. Storage
/// normalizes and validates every field before it becomes an immutable
/// revision; this type only avoids a brittle long positional argument list.
pub struct CustomerProfileInput<'a> {
    pub display_name: &'a str,
    pub phone_number: Option<&'a str>,
    pub email_address: Option<&'a str>,
    pub marketing_consent: bool,
}

impl LocalSalesSummary {
    pub const fn invoice_count(&self) -> i64 {
        self.invoice_count
    }
    pub const fn total_minor(&self) -> i64 {
        self.total_minor
    }
    pub const fn cash_minor(&self) -> i64 {
        self.cash_minor
    }
    pub const fn card_minor(&self) -> i64 {
        self.card_minor
    }
    pub const fn upi_minor(&self) -> i64 {
        self.upi_minor
    }
    pub const fn refund_minor(&self) -> i64 {
        self.refund_minor
    }
    pub const fn expense_minor(&self) -> i64 {
        self.expense_minor
    }
    pub const fn discount_minor(&self) -> i64 {
        self.discount_minor
    }
    pub const fn tax_minor(&self) -> i64 {
        self.tax_minor
    }
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }
}

impl LocalAccountingDayClose {
    pub fn close_id(&self) -> &EntityId {
        &self.close_id
    }
    pub fn accounting_date_utc(&self) -> &str {
        &self.accounting_date_utc
    }
    pub const fn invoice_count(&self) -> i64 {
        self.invoice_count
    }
    pub const fn total_minor(&self) -> i64 {
        self.total_minor
    }
    pub const fn cash_minor(&self) -> i64 {
        self.cash_minor
    }
    pub const fn card_minor(&self) -> i64 {
        self.card_minor
    }
    pub const fn upi_minor(&self) -> i64 {
        self.upi_minor
    }
    pub const fn refund_minor(&self) -> i64 {
        self.refund_minor
    }
    pub const fn expense_minor(&self) -> i64 {
        self.expense_minor
    }
    pub const fn discount_minor(&self) -> i64 {
        self.discount_minor
    }
    pub const fn tax_minor(&self) -> i64 {
        self.tax_minor
    }
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn closed_at_utc(&self) -> &str {
        &self.closed_at_utc
    }
}

impl LocalItemSalesSummary {
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub const fn quantity(&self) -> i64 {
        self.quantity
    }
    pub const fn gross_total_minor(&self) -> i64 {
        self.gross_total_minor
    }
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }
}

impl LocalIntegrityReport {
    pub const fn schema_version(&self) -> i64 {
        self.schema_version
    }

    pub const fn audit_event_count(&self) -> i64 {
        self.audit_event_count
    }
}

impl VerifiedFinancialCsvExport {
    pub fn csv_bytes(&self) -> &[u8] {
        &self.csv_bytes
    }

    pub const fn record_count(&self) -> i64 {
        self.record_count
    }

    pub fn byte_length(&self) -> Result<i64, StorageError> {
        i64::try_from(self.csv_bytes.len()).map_err(|_| StorageError::FinancialAmountOverflow)
    }
}

impl LocalAuditEventSummary {
    pub const fn sequence(&self) -> i64 {
        self.sequence
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn occurred_at_utc(&self) -> &str {
        &self.occurred_at_utc
    }
}

impl VerifiedLocalBackup {
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub const fn schema_version(&self) -> i64 {
        self.schema_version
    }
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

impl PendingSyncOperation {
    pub fn operation_id(&self) -> &EntityId {
        &self.operation_id
    }
    pub fn audit_event_id(&self) -> &EntityId {
        &self.audit_event_id
    }
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }
    /// The immutable actor recorded in the source audit event. Professional
    /// sync transports this alongside the envelope so a server can recompute
    /// and validate the audit-chain hash without trusting client-supplied
    /// attribution.
    pub fn actor_id(&self) -> &EntityId {
        &self.actor_id
    }
    pub fn device_id(&self) -> &EntityId {
        &self.device_id
    }
    pub const fn sequence(&self) -> i64 {
        self.sequence
    }
    pub fn event_type(&self) -> &str {
        &self.event_type
    }
    pub fn payload_json(&self) -> &str {
        &self.payload_json
    }
    pub fn occurred_at_utc(&self) -> &str {
        &self.occurred_at_utc
    }
    pub fn previous_hash(&self) -> Option<&[u8]> {
        self.previous_hash.as_deref()
    }
    pub fn event_hash(&self) -> &[u8] {
        &self.event_hash
    }
    pub fn entity_type(&self) -> &str {
        &self.entity_type
    }
    pub fn entity_id(&self) -> &str {
        &self.entity_id
    }
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
    pub fn created_at_utc(&self) -> &str {
        &self.created_at_utc
    }
}

impl LocalInvoiceSummary {
    pub fn invoice_id(&self) -> &EntityId {
        &self.invoice_id
    }
    pub const fn invoice_number(&self) -> i64 {
        self.invoice_number
    }
    pub const fn total_minor(&self) -> i64 {
        self.total_minor
    }
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }
    pub fn finalized_at_utc(&self) -> &str {
        &self.finalized_at_utc
    }
    pub fn payment_method(&self) -> &str {
        &self.payment_method
    }
}

/// Read-only payable preview for the counter. Used so split tender and cart
/// totals can match the same trusted pricing path as `complete_sale`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SalePricingPreview {
    subtotal_minor: i64,
    discount_minor: i64,
    tax_minor: i64,
    payable_minor: i64,
    currency_code: String,
}

impl SalePricingPreview {
    pub const fn subtotal_minor(&self) -> i64 {
        self.subtotal_minor
    }
    pub const fn discount_minor(&self) -> i64 {
        self.discount_minor
    }
    pub const fn tax_minor(&self) -> i64 {
        self.tax_minor
    }
    pub const fn payable_minor(&self) -> i64 {
        self.payable_minor
    }
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }
}

impl LocalInvoiceDetail {
    pub fn invoice_id(&self) -> &EntityId {
        &self.invoice_id
    }
    pub const fn invoice_number(&self) -> i64 {
        self.invoice_number
    }
    pub fn fulfillment(&self) -> &str {
        &self.fulfillment
    }
    pub const fn subtotal_minor(&self) -> i64 {
        self.subtotal_minor
    }
    pub const fn discount_minor(&self) -> i64 {
        self.discount_minor
    }
    pub const fn tax_minor(&self) -> i64 {
        self.tax_minor
    }
    pub const fn total_minor(&self) -> i64 {
        self.total_minor
    }
    pub const fn refunded_minor(&self) -> i64 {
        self.refunded_minor
    }
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }
    pub fn finalized_at_utc(&self) -> &str {
        &self.finalized_at_utc
    }
    pub fn lines(&self) -> &[LocalInvoiceLine] {
        &self.lines
    }
    pub fn payments(&self) -> &[LocalInvoicePayment] {
        &self.payments
    }
}

impl LocalInvoiceLine {
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    /// Immutable catalogue-option names selected for this historical line.
    /// Prices remain represented by the effective unit price and total below.
    pub fn modifier_names(&self) -> &[String] {
        &self.modifier_names
    }
    pub const fn quantity(&self) -> i64 {
        self.quantity
    }
    pub const fn unit_price_minor(&self) -> i64 {
        self.unit_price_minor
    }
    pub const fn line_total_minor(&self) -> i64 {
        self.line_total_minor
    }
}

impl LocalInvoicePayment {
    pub fn payment_method(&self) -> &str {
        &self.payment_method
    }
    pub const fn amount_minor(&self) -> i64 {
        self.amount_minor
    }
}

impl LocalExpenseSummary {
    pub fn expense_id(&self) -> &EntityId {
        &self.expense_id
    }
    pub fn category(&self) -> &str {
        &self.category
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub const fn amount_minor(&self) -> i64 {
        self.amount_minor
    }
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }
    pub fn payment_method(&self) -> &str {
        &self.payment_method
    }
    pub fn incurred_at_utc(&self) -> &str {
        &self.incurred_at_utc
    }
}

impl LocalStaffAccount {
    pub fn staff_id(&self) -> &EntityId {
        &self.staff_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub const fn role(&self) -> ActorRole {
        self.role
    }

    pub const fn is_active(&self) -> bool {
        self.is_active
    }

    pub const fn has_pin(&self) -> bool {
        self.has_pin
    }
}

impl LocalStaffSecurityState {
    pub const fn owner_pin_setup_required(&self) -> bool {
        self.owner_pin_setup_required
    }

    pub fn active_staff(&self) -> Option<&LocalStaffAccount> {
        self.active_staff.as_ref()
    }
}

impl LocalCustomer {
    pub fn customer_id(&self) -> &EntityId {
        &self.customer_id
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn phone_number(&self) -> Option<&str> {
        self.phone_number.as_deref()
    }
    pub fn email_address(&self) -> Option<&str> {
        self.email_address.as_deref()
    }
    pub const fn marketing_consent(&self) -> bool {
        self.marketing_consent
    }
    pub const fn revision(&self) -> i64 {
        self.revision
    }
}

impl KitchenTicket {
    pub fn ticket_id(&self) -> &EntityId {
        &self.ticket_id
    }
    pub fn draft_order_id(&self) -> &EntityId {
        &self.draft_order_id
    }
    pub const fn draft_revision(&self) -> i64 {
        self.draft_revision
    }
    pub fn state(&self) -> &str {
        &self.state
    }
    pub fn table_label(&self) -> Option<&str> {
        self.table_label.as_deref()
    }
    pub fn line_snapshot_json(&self) -> &str {
        &self.line_snapshot_json
    }
    /// The immutable instruction snapshot sent with this ticket. A missing
    /// value means the draft revision had no kitchen instruction.
    pub fn kitchen_note(&self) -> Option<&str> {
        self.kitchen_note.as_deref()
    }
    pub const fn revision(&self) -> i64 {
        self.revision
    }
    /// A counter-side management cancellation has been requested for this
    /// ticket. Kitchen must stop normal preparation and acknowledge the fact
    /// before it leaves the active queue.
    pub const fn cancellation_pending(&self) -> bool {
        self.cancellation_pending
    }
}

impl OpenDraftOrder {
    pub fn draft(&self) -> &DraftOrder {
        &self.draft
    }
    pub fn lines(&self) -> &[SaleLineInput] {
        &self.lines
    }
}

impl DraftOrder {
    pub fn draft_order_id(&self) -> &EntityId {
        &self.draft_order_id
    }
    pub const fn fulfillment(&self) -> OrderFulfillment {
        self.fulfillment
    }
    pub fn state(&self) -> &str {
        &self.state
    }
    pub fn table_name(&self) -> Option<&str> {
        self.table_name.as_deref()
    }
    /// The optional instruction saved with this immutable draft revision.
    pub fn kitchen_note(&self) -> Option<&str> {
        self.kitchen_note.as_deref()
    }
    pub const fn revision(&self) -> i64 {
        self.revision
    }
    pub fn subtotal(&self) -> &Money {
        &self.subtotal
    }
    pub const fn line_count(&self) -> i64 {
        self.line_count
    }
}

impl CompletedSale {
    fn new(
        order_id: EntityId,
        invoice_id: EntityId,
        invoice_number: i64,
        total: Money,
        payment_method: PaymentMethod,
    ) -> Self {
        Self {
            order_id,
            invoice_id,
            invoice_number,
            total,
            payment_method,
        }
    }

    pub fn order_id(&self) -> &EntityId {
        &self.order_id
    }

    pub fn invoice_id(&self) -> &EntityId {
        &self.invoice_id
    }

    pub const fn invoice_number(&self) -> i64 {
        self.invoice_number
    }

    pub fn total(&self) -> &Money {
        &self.total
    }

    pub const fn payment_method(&self) -> PaymentMethod {
        self.payment_method
    }
}

/// A validated image request for a menu item. Restaurant uploads have already
/// been decoded, normalized, and JPEG-encoded by the Rust application layer
/// before they reach encrypted storage. Keeping the storage contract narrow
/// prevents original device files, source paths, or unbounded media from ever
/// entering the database.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductImageContent {
    BuiltIn {
        asset_key: String,
    },
    RestaurantUpload {
        jpeg_bytes: Vec<u8>,
        pixel_width: i64,
        pixel_height: i64,
    },
    GotiginCatalog {
        jpeg_bytes: Vec<u8>,
        pixel_width: i64,
        pixel_height: i64,
        provenance: ProductImageCatalogProvenance,
    },
}

impl ProductImageContent {
    pub fn built_in(asset_key: impl Into<String>) -> Self {
        Self::BuiltIn {
            asset_key: asset_key.into(),
        }
    }

    pub fn restaurant_upload(jpeg_bytes: Vec<u8>, pixel_width: i64, pixel_height: i64) -> Self {
        Self::RestaurantUpload {
            jpeg_bytes,
            pixel_width,
            pixel_height,
        }
    }

    pub fn gotigin_catalog(
        jpeg_bytes: Vec<u8>,
        pixel_width: i64,
        pixel_height: i64,
        provenance: ProductImageCatalogProvenance,
    ) -> Self {
        Self::GotiginCatalog {
            jpeg_bytes,
            pixel_width,
            pixel_height,
            provenance,
        }
    }
}

/// Minimal immutable source record for a normalized Gotigin catalogue image.
/// The digest describes the originally downloaded bytes, while the image
/// version stores a separately hashed, metadata-free JPEG derivative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductImageCatalogProvenance {
    catalog_image_id: String,
    original_content_sha256: Vec<u8>,
    licence_label: String,
    licence_url: String,
    service_origin: String,
    service_schema_version: i64,
}

impl ProductImageCatalogProvenance {
    /// Creates provenance only after re-hashing the immutable bytes described
    /// by the catalogue response. This is the public construction boundary;
    /// callers cannot manufacture a trusted source digest without supplying
    /// the corresponding original image bytes.
    pub fn from_verified_original(
        catalog_image_id: impl Into<String>,
        original_image_bytes: &[u8],
        expected_content_sha256: &str,
        licence_label: impl Into<String>,
        licence_url: impl Into<String>,
        service_origin: impl Into<String>,
        service_schema_version: i64,
    ) -> Result<Self, StorageError> {
        if expected_content_sha256.len() != 64
            || !expected_content_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StorageError::InvalidProductImage(
                "the catalogue source digest is invalid",
            ));
        }
        let original_content_sha256 = Sha256::digest(original_image_bytes).to_vec();
        if lowercase_hex(&original_content_sha256) != expected_content_sha256 {
            return Err(StorageError::InvalidProductImage(
                "the catalogue source digest did not match the downloaded image",
            ));
        }
        Self::new(
            catalog_image_id,
            original_content_sha256,
            licence_label,
            licence_url,
            service_origin,
            service_schema_version,
        )
    }

    fn new(
        catalog_image_id: impl Into<String>,
        original_content_sha256: Vec<u8>,
        licence_label: impl Into<String>,
        licence_url: impl Into<String>,
        service_origin: impl Into<String>,
        service_schema_version: i64,
    ) -> Result<Self, StorageError> {
        let provenance = Self {
            catalog_image_id: catalog_image_id.into(),
            original_content_sha256,
            licence_label: licence_label.into(),
            licence_url: licence_url.into(),
            service_origin: service_origin.into(),
            service_schema_version,
        };
        validate_product_image_catalog_provenance(&provenance)?;
        Ok(provenance)
    }

    pub fn catalog_image_id(&self) -> &str {
        &self.catalog_image_id
    }

    pub fn original_content_sha256(&self) -> &[u8] {
        &self.original_content_sha256
    }

    pub fn licence_label(&self) -> &str {
        &self.licence_label
    }

    pub fn licence_url(&self) -> &str {
        &self.licence_url
    }

    pub fn service_origin(&self) -> &str {
        &self.service_origin
    }

    pub const fn service_schema_version(&self) -> i64 {
        self.service_schema_version
    }
}

/// A current menu-image projection. Earlier versions remain in the encrypted
/// database; this value contains only the assignment currently shown at the
/// counter and in menu management.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductImage {
    product_id: EntityId,
    source_kind: ProductImageSource,
    asset_key: Option<String>,
    image_bytes: Option<Vec<u8>>,
    catalog_provenance: Option<ProductImageCatalogProvenance>,
}

impl ProductImage {
    fn new(
        product_id: EntityId,
        source_kind: ProductImageSource,
        asset_key: Option<String>,
        image_bytes: Option<Vec<u8>>,
        catalog_provenance: Option<ProductImageCatalogProvenance>,
    ) -> Self {
        Self {
            product_id,
            source_kind,
            asset_key,
            image_bytes,
            catalog_provenance,
        }
    }

    pub fn product_id(&self) -> &EntityId {
        &self.product_id
    }

    pub const fn source_kind(&self) -> ProductImageSource {
        self.source_kind
    }

    pub fn asset_key(&self) -> Option<&str> {
        self.asset_key.as_deref()
    }

    pub fn image_bytes(&self) -> Option<&[u8]> {
        self.image_bytes.as_deref()
    }

    pub fn catalog_provenance(&self) -> Option<&ProductImageCatalogProvenance> {
        self.catalog_provenance.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductImageSource {
    BuiltIn,
    RestaurantUpload,
    GotiginCatalog,
}

impl ProductImageSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::BuiltIn => "built_in",
            Self::RestaurantUpload => "restaurant_upload",
            Self::GotiginCatalog => "gotigin_catalog",
        }
    }

    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "built_in" => Ok(Self::BuiltIn),
            "restaurant_upload" => Ok(Self::RestaurantUpload),
            "gotigin_catalog" => Ok(Self::GotiginCatalog),
            _ => Err(StorageError::InvalidPersistedData(
                "product image source was invalid".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
struct SaleLineSnapshot {
    product_id: EntityId,
    product_name: String,
    quantity: i64,
    base_unit_price_minor: i64,
    modifier_total_minor: i64,
    modifiers: Vec<SaleLineModifierSnapshot>,
    unit_price_minor: i64,
    line_total_minor: i64,
}

#[derive(Clone, Debug)]
struct SaleLineModifierSnapshot {
    modifier_option_id: EntityId,
    display_name: String,
    price_delta_minor: i64,
}

/// Opens the restaurant database using a 256-bit key held exclusively by the
/// current operating-system credential store. There is intentionally no
/// plaintext-file, environment-variable, or Dart-level fallback.
#[cfg(all(
    feature = "platform-keyring",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub fn open_or_create_platform_database(
    path: impl AsRef<Path>,
) -> Result<LocalDatabase, DatabaseBootstrapError> {
    open_or_create_with_key_store(
        path.as_ref(),
        &PlatformDatabaseKeyStore,
        DatabaseKeySlot::community_default(),
    )
}

/// Mobile builds deliberately stop here until a native secure-store adapter is
/// reviewed and signed-device tests prove the same fail-closed contract.
#[cfg(all(
    feature = "platform-keyring",
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
pub fn open_or_create_platform_database(
    _path: impl AsRef<Path>,
) -> Result<LocalDatabase, DatabaseBootstrapError> {
    Err(DatabaseBootstrapError::UnsupportedSecureStoragePlatform)
}

#[cfg(feature = "platform-keyring")]
fn open_or_create_with_key_store(
    path: &Path,
    key_store: &impl DatabaseKeyStore,
    slot: DatabaseKeySlot,
) -> Result<LocalDatabase, DatabaseBootstrapError> {
    let _bootstrap_lock = acquire_bootstrap_lock(path)?;
    let database_exists = path.exists();
    let stored_key = key_store
        .load(slot)
        .map_err(DatabaseBootstrapError::KeyStore)?;

    match (database_exists, stored_key) {
        (true, Some(key)) => {
            LocalDatabase::open(path, &key).map_err(DatabaseBootstrapError::Storage)
        }
        (true, None) => Err(DatabaseBootstrapError::DatabaseKeyMissing),
        (false, Some(_)) => Err(DatabaseBootstrapError::ExistingKeyWithoutDatabase),
        (false, None) => {
            let generated_key =
                DatabaseKey::generate().map_err(DatabaseBootstrapError::KeyStore)?;
            key_store
                .store_new(slot, &generated_key)
                .map_err(DatabaseBootstrapError::KeyStore)?;
            let verified_key = key_store
                .load(slot)
                .map_err(DatabaseBootstrapError::KeyStore)?
                .ok_or(DatabaseBootstrapError::KeyStore(
                    KeyStoreError::WriteVerificationFailed,
                ))?;

            if !generated_key.same_as(&verified_key) {
                return Err(DatabaseBootstrapError::KeyStore(
                    KeyStoreError::WriteVerificationFailed,
                ));
            }

            LocalDatabase::open(path, &generated_key).map_err(DatabaseBootstrapError::Storage)
        }
    }
}

fn acquire_bootstrap_lock(path: &Path) -> Result<File, DatabaseBootstrapError> {
    let lock_path = path.with_extension("bootstrap.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(DatabaseBootstrapError::Lock)?;
    lock_file.lock().map_err(DatabaseBootstrapError::Lock)?;

    Ok(lock_file)
}

impl LocalDatabase {
    /// Opens or creates an encrypted database and applies all local migration
    /// and connection-hardening policies before returning it to the caller.
    pub(crate) fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, StorageError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(StorageError::Sql)?;

        configure_connection(&connection, key)?;
        migrate(&connection)?;

        Ok(Self {
            connection,
            key: DatabaseKey::from_bytes(*key.as_bytes()),
        })
    }

    pub fn schema_version(&self) -> Result<i64, StorageError> {
        self.connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(StorageError::Sql)
    }

    pub fn cipher_version(&self) -> Result<String, StorageError> {
        read_cipher_version(&self.connection)
    }

    pub fn migration_checksum(&self, version: i64) -> Result<Option<String>, StorageError> {
        self.connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [version],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sql)
    }

    pub fn is_community_provisioned(&self) -> Result<bool, StorageError> {
        self.connection
            .query_row("SELECT EXISTS(SELECT 1 FROM branches)", [], |row| {
                row.get(0)
            })
            .map_err(StorageError::Sql)
    }

    /// Creates the single local organization and branch used by Community
    /// Edition. This is intentionally a one-time operation; Professional later
    /// adds branch provisioning through its authenticated cloud workflow.
    pub fn provision_community(&self, setup: &CommunitySetup) -> Result<Branch, StorageError> {
        let transaction = begin_immediate_transaction(&self.connection)?;
        let already_provisioned: bool = transaction
            .query_row("SELECT EXISTS(SELECT 1 FROM branches)", [], |row| {
                row.get(0)
            })
            .map_err(StorageError::Sql)?;

        if already_provisioned {
            return Err(StorageError::CommunityAlreadyProvisioned);
        }

        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let owner_actor_id = EntityId::new_v7();
        let created_at = utc_timestamp(&transaction)?;

        transaction
            .execute(
                "
                INSERT INTO organizations (organization_id, display_name, created_at_utc)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    organization_id.to_string(),
                    setup.organization_name().display(),
                    created_at,
                ],
            )
            .map_err(StorageError::Sql)?;

        transaction
            .execute(
                "
                INSERT INTO branches (
                    branch_id,
                    organization_id,
                    display_name,
                    currency_code,
                    time_zone,
                    created_at_utc
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    branch_id.to_string(),
                    organization_id.to_string(),
                    setup.branch_name().display(),
                    setup.currency(),
                    setup.time_zone(),
                    created_at,
                ],
            )
            .map_err(StorageError::Sql)?;

        transaction
            .execute(
                "
                INSERT INTO local_installation_identity (
                    singleton,
                    device_id,
                    owner_actor_id,
                    created_at_utc
                ) VALUES (1, ?1, ?2, ?3)
                ",
                params![
                    device_id.to_string(),
                    owner_actor_id.to_string(),
                    created_at
                ],
            )
            .map_err(StorageError::Sql)?;

        // No default credential is created. The first person at this device
        // must explicitly choose the owner PIN in the security setup flow.
        transaction
            .execute(
                "
                INSERT INTO staff_accounts (
                    staff_id,
                    branch_id,
                    display_name,
                    name_key,
                    role,
                    created_at_utc,
                    created_by_actor_id
                ) VALUES (?1, ?2, 'Owner', 'owner', 'owner', ?3, ?1)
                ",
                params![
                    owner_actor_id.to_string(),
                    branch_id.to_string(),
                    created_at
                ],
            )
            .map_err(StorageError::Sql)?;
        transaction
            .execute(
                "
                INSERT INTO staff_status_events (
                    staff_status_event_id,
                    staff_id,
                    status,
                    occurred_at_utc,
                    occurred_by_actor_id
                ) VALUES (?1, ?2, 'active', ?3, ?2)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    owner_actor_id.to_string(),
                    created_at,
                ],
            )
            .map_err(StorageError::Sql)?;

        transaction.commit().map_err(StorageError::Sql)?;

        Ok(Branch::new(
            organization_id,
            branch_id,
            setup.branch_name().clone(),
            setup.currency_code(),
            setup.time_zone_id(),
        ))
    }

    pub fn community_branch(&self) -> Result<Branch, StorageError> {
        read_community_branch(&self.connection)
    }

    /// Returns the stable local owner/device attribution used until local staff
    /// accounts and PIN sessions are introduced. The identity is generated
    /// once for migrated installations that pre-date migration v3.
    pub fn community_owner_context(&self) -> Result<MutationContext, StorageError> {
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let (device_id, owner_actor_id) = ensure_local_installation_identity(&transaction)?;
        transaction.commit().map_err(StorageError::Sql)?;

        Ok(MutationContext::new(
            branch.branch_id().clone(),
            owner_actor_id,
            device_id,
            EntityId::new_v7(),
            ros_core::ActorRole::Owner,
        ))
    }

    /// Returns the local staff needed to render the unlock surface. The PIN
    /// digest and failed-attempt history never leave Rust storage.
    pub fn list_local_staff(&self) -> Result<Vec<LocalStaffAccount>, StorageError> {
        let branch = self.community_branch()?;
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT
                    staff.staff_id,
                    staff.display_name,
                    COALESCE((
                        SELECT role.role
                        FROM staff_role_events AS role
                        JOIN local_security_fact_order AS role_order
                            ON role_order.fact_kind = 'staff_role'
                            AND role_order.fact_id = role.staff_role_event_id
                        WHERE role.staff_id = staff.staff_id
                        ORDER BY role_order.fact_sequence DESC
                        LIMIT 1
                    ), staff.role),
                    COALESCE((
                        SELECT status.status = 'active'
                        FROM staff_status_events AS status
                        JOIN local_security_fact_order AS status_order
                            ON status_order.fact_kind = 'staff_status'
                            AND status_order.fact_id = status.staff_status_event_id
                        WHERE status.staff_id = staff.staff_id
                        ORDER BY status_order.fact_sequence DESC
                        LIMIT 1
                    ), 0),
                    EXISTS(
                        SELECT 1
                        FROM staff_pin_credentials AS credential
                        WHERE credential.staff_id = staff.staff_id
                    )
                FROM staff_accounts AS staff
                WHERE staff.branch_id = ?1
                ORDER BY staff.display_name COLLATE NOCASE, staff.staff_id
                ",
            )
            .map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch.branch_id().to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, bool>(4)?,
                ))
            })
            .map_err(StorageError::Sql)?;

        rows.map(|row| {
            let (staff_id, display_name, role, is_active, has_pin) =
                row.map_err(StorageError::Sql)?;
            Ok(LocalStaffAccount {
                staff_id: parse_persisted_id(&staff_id)?,
                display_name,
                role: parse_actor_role(&role)?,
                is_active,
                has_pin,
            })
        })
        .collect()
    }

    /// Identifies whether onboarding must collect the first owner PIN, and
    /// whether an unexpired local session is currently attributable.
    pub fn local_staff_security_state(&self) -> Result<LocalStaffSecurityState, StorageError> {
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let (device_id, owner_actor_id) = ensure_local_installation_identity(&transaction)?;
        let owner_has_pin: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM staff_pin_credentials WHERE staff_id = ?1)",
                [owner_actor_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        let active_staff = read_active_local_staff(&transaction, &branch, &device_id)?;
        transaction.commit().map_err(StorageError::Sql)?;

        Ok(LocalStaffSecurityState {
            owner_pin_setup_required: !owner_has_pin,
            active_staff,
        })
    }

    /// Sets the owner credential exactly once. The plaintext PIN is accepted
    /// only for this call, transformed to an Argon2id digest in Rust, and is
    /// never placed in an audit payload or Flutter-visible model.
    pub fn set_initial_owner_pin(&self, pin: &str) -> Result<(), StorageError> {
        validate_local_staff_pin(pin)?;
        let pin_hash = hash_local_staff_pin(pin)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        ensure_active_branch(&transaction, branch.branch_id())?;
        let (device_id, owner_actor_id) = ensure_local_installation_identity(&transaction)?;
        let owner = read_local_staff_account(&transaction, &branch, &owner_actor_id)?;
        if !owner.is_active() || owner.role() != ActorRole::Owner {
            return Err(StorageError::PermissionDenied);
        }
        let context = MutationContext::new(
            branch.branch_id().clone(),
            owner_actor_id,
            device_id,
            EntityId::new_v7(),
            ActorRole::Owner,
        );
        let credential_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM staff_pin_credentials WHERE staff_id = ?1)",
                [context.actor_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if credential_exists {
            return Err(StorageError::OwnerPinAlreadyConfigured);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        let audit_event_id = append_hashed_audit_event_without_session(
            &transaction,
            &context,
            "staff.owner_pin.configured",
            &json!({
                "schema_version": 1,
                "entity_type": "staff_pin_credential",
                "entity_id": context.actor_id().to_string(),
                "actor_role": "owner",
                "correlation_id": context.correlation_id().to_string(),
            })
            .to_string(),
        )?;
        append_sync_outbox_event(
            &transaction,
            &context,
            &audit_event_id,
            "staff_pin_credential",
            context.actor_id(),
            "staff.owner_pin.configured",
            &occurred_at,
        )?;
        transaction
            .execute(
                "
                INSERT INTO staff_pin_credentials (
                    staff_pin_credential_id,
                    staff_id,
                    argon2id_hash,
                    created_at_utc,
                    created_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    context.actor_id().to_string(),
                    pin_hash.as_str(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Opens a short-lived device-local session after a rate-limited Argon2id
    /// verification. A selected staff identity is required so a PIN alone is
    /// never an account-discovery mechanism.
    pub fn unlock_local_staff(
        &self,
        staff_id: &EntityId,
        pin: &str,
    ) -> Result<LocalStaffAccount, StorageError> {
        validate_local_staff_pin(pin)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let (device_id, _) = ensure_local_installation_identity(&transaction)?;
        let staff = read_local_staff_account(&transaction, &branch, staff_id)?;
        if !staff.is_active() || !staff.has_pin() {
            return Err(StorageError::InvalidStaffPin);
        }
        let attempted_at = observe_local_security_clock(&transaction)?
            .ok_or(StorageError::StaffPinTemporarilyLocked)?;
        let failed_attempts: i64 = transaction
            .query_row(
                "
                SELECT COUNT(*)
                FROM staff_pin_attempts
                WHERE staff_id = ?1
                  AND succeeded = 0
                  AND attempted_at_utc >= strftime('%Y-%m-%dT%H:%M:%fZ', ?2, ?3)
                ",
                params![
                    staff_id.to_string(),
                    attempted_at,
                    format!("-{} minutes", LOCAL_STAFF_PIN_THROTTLE_MINUTES),
                ],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if failed_attempts >= LOCAL_STAFF_PIN_MAXIMUM_FAILED_ATTEMPTS {
            return Err(StorageError::StaffPinTemporarilyLocked);
        }
        let digest: String = transaction
            .query_row(
                "
                SELECT argon2id_hash
                FROM staff_pin_credentials AS credential
                JOIN local_security_fact_order AS credential_order
                    ON credential_order.fact_kind = 'staff_pin_credential'
                    AND credential_order.fact_id = credential.staff_pin_credential_id
                WHERE credential.staff_id = ?1
                ORDER BY credential_order.fact_sequence DESC
                LIMIT 1
                ",
                [staff_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        let verified = verify_local_staff_pin(pin, &digest);
        transaction
            .execute(
                "
                INSERT INTO staff_pin_attempts (
                    staff_pin_attempt_id,
                    staff_id,
                    attempted_at_utc,
                    succeeded
                ) VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    staff_id.to_string(),
                    attempted_at,
                    if verified { 1 } else { 0 },
                ],
            )
            .map_err(StorageError::Sql)?;
        if !verified {
            // Failed attempts are intentionally committed before returning the
            // generic rejection. Rolling them back would make the throttle
            // ineffective after a process restart or repeated offline attack.
            transaction.commit().map_err(StorageError::Sql)?;
            return Err(StorageError::InvalidStaffPin);
        }

        let context = MutationContext::new(
            branch.branch_id().clone(),
            staff.staff_id().clone(),
            device_id.clone(),
            EntityId::new_v7(),
            staff.role(),
        );
        let audit_event_id = append_hashed_audit_event_without_session(
            &transaction,
            &context,
            "staff.session.unlocked",
            &json!({
                "schema_version": 1,
                "entity_type": "local_staff_session",
                "entity_id": device_id.to_string(),
                "staff_id": staff.staff_id().to_string(),
                "actor_role": staff.role().as_str(),
                "correlation_id": context.correlation_id().to_string(),
            })
            .to_string(),
        )?;
        transaction
            .execute(
                "
                INSERT INTO local_staff_session_events (
                    local_staff_session_event_id,
                    device_id,
                    staff_id,
                    event_type,
                    occurred_at_utc,
                    expires_at_utc,
                    audit_event_id
                ) VALUES (
                    ?1, ?2, ?3, 'unlocked', ?4,
                    strftime('%Y-%m-%dT%H:%M:%fZ', ?4, ?5), ?6
                )
                ",
                params![
                    EntityId::new_v7().to_string(),
                    device_id.to_string(),
                    staff.staff_id().to_string(),
                    attempted_at,
                    format!("+{} minutes", LOCAL_STAFF_SESSION_MINUTES),
                    audit_event_id.to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(staff)
    }

    /// Resolves an expiring trusted staff session. Mutation callers must use
    /// this rather than allowing Flutter to select a role or actor id.
    pub fn community_active_staff_context(&self) -> Result<MutationContext, StorageError> {
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let (device_id, _) = ensure_local_installation_identity(&transaction)?;
        let staff = read_active_local_staff(&transaction, &branch, &device_id)?
            .ok_or(StorageError::StaffSessionRequired)?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(MutationContext::new(
            branch.branch_id().clone(),
            staff.staff_id().clone(),
            device_id,
            EntityId::new_v7(),
            staff.role(),
        ))
    }

    /// Appends an explicit local lock transition. It is idempotent when no
    /// current session exists and intentionally records no secret material.
    pub fn lock_local_staff(&self) -> Result<(), StorageError> {
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let (device_id, _) = ensure_local_installation_identity(&transaction)?;
        let Some((event_type, staff_id, _)) =
            read_latest_local_staff_session(&transaction, &device_id)?
        else {
            transaction.commit().map_err(StorageError::Sql)?;
            return Ok(());
        };
        if event_type != "unlocked" {
            transaction.commit().map_err(StorageError::Sql)?;
            return Ok(());
        }
        let staff_id = parse_persisted_id(&staff_id)?;
        let staff = read_local_staff_account(&transaction, &branch, &staff_id)?;
        let context = MutationContext::new(
            branch.branch_id().clone(),
            staff.staff_id().clone(),
            device_id.clone(),
            EntityId::new_v7(),
            staff.role(),
        );
        let audit_event_id = append_hashed_audit_event_without_session(
            &transaction,
            &context,
            "staff.session.locked",
            &json!({
                "schema_version": 1,
                "entity_type": "local_staff_session",
                "entity_id": device_id.to_string(),
                "staff_id": staff.staff_id().to_string(),
                "actor_role": staff.role().as_str(),
                "correlation_id": context.correlation_id().to_string(),
            })
            .to_string(),
        )?;
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO local_staff_session_events (
                    local_staff_session_event_id,
                    device_id,
                    staff_id,
                    event_type,
                    occurred_at_utc,
                    expires_at_utc,
                    audit_event_id
                ) VALUES (?1, ?2, ?3, 'locked', ?4, NULL, ?5)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    device_id.to_string(),
                    staff.staff_id().to_string(),
                    occurred_at,
                    audit_event_id.to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Enrolls a non-owner local staff account with a first credential. The
    /// immutable account, active status, credential, audit fact, and future
    /// sync envelope are committed together so a half-created staff identity
    /// can never appear at the lock screen.
    pub fn create_local_staff(
        &self,
        display_name: &str,
        role: ActorRole,
        pin: &str,
        context: &MutationContext,
    ) -> Result<LocalStaffAccount, StorageError> {
        require_owner_authority(context)?;
        if role == ActorRole::Owner {
            return Err(StorageError::OwnerRoleReserved);
        }
        validate_local_staff_pin(pin)?;
        let display_name = DisplayName::staff(display_name)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let pin_hash = hash_local_staff_pin(pin)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let staff_id = EntityId::new_v7();
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO staff_accounts (
                    staff_id, branch_id, display_name, name_key, role,
                    created_at_utc, created_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    staff_id.to_string(),
                    context.branch_id().to_string(),
                    display_name.display(),
                    display_name.key(),
                    role.as_str(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        transaction
            .execute(
                "
                INSERT INTO staff_status_events (
                    staff_status_event_id, staff_id, status, occurred_at_utc,
                    occurred_by_actor_id
                ) VALUES (?1, ?2, 'active', ?3, ?4)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    staff_id.to_string(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        transaction
            .execute(
                "
                INSERT INTO staff_pin_credentials (
                    staff_pin_credential_id, staff_id, argon2id_hash,
                    created_at_utc, created_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    staff_id.to_string(),
                    pin_hash.as_str(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "staff_account",
            "entity_id": staff_id.to_string(),
            "display_name": display_name.display(),
            "role": role.as_str(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
        })
        .to_string();
        let audit_event_id =
            append_hashed_audit_event(&transaction, context, "staff.account.created", &payload)?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "staff_account",
            &staff_id,
            "staff.account.created",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(LocalStaffAccount {
            staff_id,
            display_name: display_name.display().to_owned(),
            role,
            is_active: true,
            has_pin: true,
        })
    }

    /// Rotates a staff credential by appending a newer Argon2id verifier. The
    /// previous verifier is retained for forensic history but only the newest
    /// credential can authenticate a future unlock.
    pub fn rotate_local_staff_pin(
        &self,
        staff_id: &EntityId,
        new_pin: &str,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_owner_authority(context)?;
        validate_local_staff_pin(new_pin)?;
        let pin_hash = hash_local_staff_pin(new_pin)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let staff = read_local_staff_account(&transaction, &branch, staff_id)?;
        if !staff.is_active() {
            return Err(StorageError::StaffNotActive);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO staff_pin_credentials (
                    staff_pin_credential_id, staff_id, argon2id_hash,
                    created_at_utc, created_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    staff_id.to_string(),
                    pin_hash.as_str(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "staff.pin.rotated",
            &json!({
                "schema_version": 1,
                "entity_type": "staff_pin_credential",
                "entity_id": staff_id.to_string(),
                "staff_role": staff.role().as_str(),
                "correlation_id": context.correlation_id().to_string(),
                "actor_role": context.actor_role().as_str(),
            })
            .to_string(),
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "staff_pin_credential",
            staff_id,
            "staff.pin.rotated",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Revokes a non-owner account as a new retained status fact. Any current
    /// session for that account immediately stops resolving as active.
    pub fn revoke_local_staff(
        &self,
        staff_id: &EntityId,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_owner_authority(context)?;
        if staff_id == context.actor_id() {
            return Err(StorageError::CannotRevokeCurrentOwner);
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let staff = read_local_staff_account(&transaction, &branch, staff_id)?;
        if staff.role() == ActorRole::Owner {
            return Err(StorageError::OwnerRoleReserved);
        }
        if !staff.is_active() {
            return Err(StorageError::StaffNotActive);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO staff_status_events (
                    staff_status_event_id, staff_id, status, occurred_at_utc,
                    occurred_by_actor_id
                ) VALUES (?1, ?2, 'revoked', ?3, ?4)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    staff_id.to_string(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "staff.account.revoked",
            &json!({
                "schema_version": 1,
                "entity_type": "staff_account",
                "entity_id": staff_id.to_string(),
                "staff_role": staff.role().as_str(),
                "reason": reason.as_str(),
                "correlation_id": context.correlation_id().to_string(),
                "actor_role": context.actor_role().as_str(),
            })
            .to_string(),
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "staff_account",
            staff_id,
            "staff.account.revoked",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Appends a reasoned change to a non-owner staff member's effective role.
    /// The original account is never altered, so prior session and audit
    /// attribution remains historically correct.
    pub fn change_local_staff_role(
        &self,
        staff_id: &EntityId,
        new_role: ActorRole,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_owner_authority(context)?;
        if new_role == ActorRole::Owner {
            return Err(StorageError::OwnerRoleReserved);
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let staff = read_local_staff_account(&transaction, &branch, staff_id)?;
        if staff.role() == ActorRole::Owner {
            return Err(StorageError::OwnerRoleReserved);
        }
        if !staff.is_active() {
            return Err(StorageError::StaffNotActive);
        }
        if staff.role() == new_role {
            return Err(StorageError::CatalogConflict);
        }

        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO staff_role_events (
                    staff_role_event_id, staff_id, role, reason,
                    occurred_at_utc, occurred_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    staff_id.to_string(),
                    new_role.as_str(),
                    reason.as_str(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "staff.role.changed",
            &json!({
                "schema_version": 1,
                "entity_type": "staff_account",
                "entity_id": staff_id.to_string(),
                "reason": reason.as_str(),
                "before": { "role": staff.role().as_str() },
                "after": { "role": new_role.as_str() },
                "correlation_id": context.correlation_id().to_string(),
                "actor_role": context.actor_role().as_str(),
            })
            .to_string(),
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "staff_account",
            staff_id,
            "staff.role.changed",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    pub fn create_customer(
        &self,
        display_name: &str,
        phone_number: Option<&str>,
        email_address: Option<&str>,
        marketing_consent: bool,
        context: &MutationContext,
    ) -> Result<LocalCustomer, StorageError> {
        require_counter_authority(context)?;
        let display_name = DisplayName::customer(display_name)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let phone_number = normalize_customer_phone(phone_number)?;
        let email_address = normalize_customer_email(email_address)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let customer_id = EntityId::new_v7();
        let occurred_at = utc_timestamp(&transaction)?;
        transaction.execute("INSERT INTO customers (customer_id, branch_id, created_at_utc, created_by_actor_id) VALUES (?1, ?2, ?3, ?4)", params![customer_id.to_string(), context.branch_id().to_string(), occurred_at, context.actor_id().to_string()]).map_err(StorageError::Sql)?;
        transaction.execute("INSERT INTO customer_profile_revisions (customer_profile_revision_id, customer_id, revision, display_name, phone_number, email_address, marketing_consent, profile_state, changed_at_utc, changed_by_actor_id, reason) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, 'active', ?7, ?8, NULL)", params![EntityId::new_v7().to_string(), customer_id.to_string(), display_name.display(), phone_number, email_address, if marketing_consent { 1 } else { 0 }, occurred_at, context.actor_id().to_string()]).map_err(StorageError::Sql)?;
        let audit = append_hashed_audit_event(&transaction, context, "customer.created", &json!({"schema_version":1,"entity_type":"customer","entity_id":customer_id.to_string(),"revision":1,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string())?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit,
            "customer",
            &customer_id,
            "customer.created",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(LocalCustomer {
            customer_id,
            display_name: display_name.display().to_owned(),
            phone_number,
            email_address,
            marketing_consent,
            revision: 1,
        })
    }

    pub fn list_active_customers(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<LocalCustomer>, StorageError> {
        let mut statement = self.connection.prepare("SELECT customer.customer_id, profile.display_name, profile.phone_number, profile.email_address, profile.marketing_consent, profile.revision FROM customers AS customer JOIN customer_profile_revisions AS profile ON profile.customer_id = customer.customer_id WHERE customer.branch_id = ?1 AND profile.revision = (SELECT MAX(latest.revision) FROM customer_profile_revisions AS latest WHERE latest.customer_id = customer.customer_id) AND profile.profile_state = 'active' ORDER BY profile.display_name COLLATE NOCASE, customer.customer_id").map_err(StorageError::Sql)?;
        statement
            .query_map([branch_id.to_string()], |row| {
                Ok(LocalCustomer {
                    customer_id: EntityId::parse(&row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    display_name: row.get(1)?,
                    phone_number: row.get(2)?,
                    email_address: row.get(3)?,
                    marketing_consent: row.get::<_, i64>(4)? != 0,
                    revision: row.get(5)?,
                })
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    /// Appends a corrected customer profile revision. Historical profiles are
    /// intentionally retained, so a counter correction can be audited without
    /// mutating the customer fact that was in effect for an earlier sale.
    pub fn revise_customer(
        &self,
        customer_id: &EntityId,
        profile: CustomerProfileInput<'_>,
        reason: &str,
        context: &MutationContext,
    ) -> Result<LocalCustomer, StorageError> {
        require_management_authority(context)?;
        let display_name = DisplayName::customer(profile.display_name)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let phone_number = normalize_customer_phone(profile.phone_number)?;
        let email_address = normalize_customer_email(profile.email_address)?;
        let reason = normalize_customer_reason(reason)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_customer(&transaction, context.branch_id(), customer_id)?;
        let revision = next_customer_profile_revision(&transaction, customer_id)?;
        let occurred_at = utc_timestamp(&transaction)?;
        transaction.execute("INSERT INTO customer_profile_revisions (customer_profile_revision_id, customer_id, revision, display_name, phone_number, email_address, marketing_consent, profile_state, changed_at_utc, changed_by_actor_id, reason) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?9, ?10)", params![EntityId::new_v7().to_string(), customer_id.to_string(), revision, display_name.display(), phone_number, email_address, if profile.marketing_consent { 1 } else { 0 }, occurred_at, context.actor_id().to_string(), reason]).map_err(StorageError::Sql)?;
        let audit = append_hashed_audit_event(&transaction, context, "customer.profile.corrected", &json!({"schema_version":1,"entity_type":"customer","entity_id":customer_id.to_string(),"revision":revision,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string())?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit,
            "customer",
            customer_id,
            "customer.profile.corrected",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(LocalCustomer {
            customer_id: customer_id.clone(),
            display_name: display_name.display().to_owned(),
            phone_number,
            email_address,
            marketing_consent: profile.marketing_consent,
            revision,
        })
    }

    /// Stops the customer record from being selected for future sales while
    /// retaining immutable sales, audit, and profile history. PII is redacted
    /// in the new current revision rather than deleting a record used by a
    /// counter transaction.
    pub fn anonymize_customer(
        &self,
        customer_id: &EntityId,
        reason: &str,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_management_authority(context)?;
        let reason = normalize_customer_reason(reason)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_customer(&transaction, context.branch_id(), customer_id)?;
        let revision = next_customer_profile_revision(&transaction, customer_id)?;
        let occurred_at = utc_timestamp(&transaction)?;
        transaction.execute("INSERT INTO customer_profile_revisions (customer_profile_revision_id, customer_id, revision, display_name, phone_number, email_address, marketing_consent, profile_state, changed_at_utc, changed_by_actor_id, reason) VALUES (?1, ?2, ?3, 'Anonymized customer', NULL, NULL, 0, 'anonymized', ?4, ?5, ?6)", params![EntityId::new_v7().to_string(), customer_id.to_string(), revision, occurred_at, context.actor_id().to_string(), reason]).map_err(StorageError::Sql)?;
        let audit = append_hashed_audit_event(&transaction, context, "customer.anonymized", &json!({"schema_version":1,"entity_type":"customer","entity_id":customer_id.to_string(),"revision":revision,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string())?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit,
            "customer",
            customer_id,
            "customer.anonymized",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Creates a sellable category and its audit event in one durable local
    /// transaction. A failed audit append rolls back the category as well.
    pub fn create_category(
        &self,
        command: &CreateCategory,
        context: &MutationContext,
    ) -> Result<Category, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;

        let category_id = EntityId::new_v7();
        let occurred_at = utc_timestamp(&transaction)?;
        let category = Category::new(
            category_id.clone(),
            context.branch_id().clone(),
            command.display_name().clone(),
            command.sort_order(),
            1,
            false,
        );

        transaction
            .execute(
                "
                INSERT INTO categories (
                    category_id,
                    branch_id,
                    display_name,
                    name_key,
                    sort_order,
                    revision,
                    created_at_utc,
                    created_by_actor_id,
                    updated_at_utc,
                    updated_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?6, ?7)
                ",
                params![
                    category_id.to_string(),
                    context.branch_id().to_string(),
                    command.display_name().display(),
                    command.display_name().key(),
                    command.sort_order(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;

        let payload = json!({
            "schema_version": 1,
            "entity_type": "category",
            "entity_id": category_id.to_string(),
            "revision": 1,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "after": {
                "display_name": command.display_name().display(),
                "sort_order": command.sort_order(),
            },
        })
        .to_string();
        append_hashed_audit_event(&transaction, context, "catalog.category.created", &payload)?;

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(category)
    }

    /// Creates a product with a price in the branch's immutable operating
    /// currency and appends a linked audit record atomically.
    pub fn create_product(
        &self,
        command: &CreateProduct,
        context: &MutationContext,
    ) -> Result<Product, StorageError> {
        self.create_product_with_image(command, None, context)
    }

    /// Creates a product and, when selected, its first menu-image version in
    /// the same durable transaction. A failed image validation or audit append
    /// therefore never leaves behind a partially-created menu item.
    pub fn create_product_with_image(
        &self,
        command: &CreateProduct,
        image: Option<&ProductImageContent>,
        context: &MutationContext,
    ) -> Result<Product, StorageError> {
        require_management_authority(context)?;
        if let Some(image) = image {
            validate_product_image_content(image)?;
        }

        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;

        if command.unit_price().currency() != branch_currency {
            return Err(StorageError::CurrencyMismatch {
                expected: branch_currency,
                actual: command.unit_price().currency().to_owned(),
            });
        }

        if let Some(category_id) = command.category_id() {
            ensure_active_category(&transaction, context.branch_id(), category_id)?;
        }

        let product_id = EntityId::new_v7();
        let category_id = command.category_id().map(ToString::to_string);
        let occurred_at = utc_timestamp(&transaction)?;
        let product = Product::new(
            product_id.clone(),
            context.branch_id().clone(),
            command.category_id().cloned(),
            command.display_name().clone(),
            command.unit_price().clone(),
            command.sku().map(ToOwned::to_owned),
            command.barcode().map(ToOwned::to_owned),
            true,
            command.sort_order(),
            1,
            false,
        );

        transaction
            .execute(
                "
                INSERT INTO products (
                    product_id,
                    branch_id,
                    category_id,
                    display_name,
                    name_key,
                    sku,
                    barcode,
                    unit_price_minor,
                    currency_code,
                    is_available,
                    sort_order,
                    revision,
                    created_at_utc,
                    created_by_actor_id,
                    updated_at_utc,
                    updated_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, 1, ?11, ?12, ?11, ?12)
                ",
                params![
                    product_id.to_string(),
                    context.branch_id().to_string(),
                    category_id,
                    command.display_name().display(),
                    command.display_name().key(),
                    command.sku(),
                    command.barcode(),
                    command.unit_price().minor_units(),
                    command.unit_price().currency(),
                    command.sort_order(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;

        let payload = json!({
            "schema_version": 1,
            "entity_type": "product",
            "entity_id": product_id.to_string(),
            "revision": 1,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "after": {
                "category_id": product.category_id().map(ToString::to_string),
                "display_name": command.display_name().display(),
                "unit_price_minor": command.unit_price().minor_units(),
                "currency_code": command.unit_price().currency(),
                "sku": command.sku(),
                "barcode": command.barcode(),
                "sort_order": command.sort_order(),
                "is_available": true,
            },
        })
        .to_string();
        append_hashed_audit_event(&transaction, context, "catalog.product.created", &payload)?;

        if let Some(image) = image {
            assign_product_image(&transaction, product.product_id(), image, context)?;
        }

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(product)
    }

    /// Creates one product-bound modifier option. Its name and non-negative
    /// price delta are immutable after this transaction; a future correction
    /// must archive this option and create a replacement so historic sales
    /// continue to describe the guest's original choice truthfully.
    pub fn create_product_modifier_option(
        &self,
        product_id: &EntityId,
        command: &CreateModifierOption,
        context: &MutationContext,
    ) -> Result<ModifierOption, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;
        if command.price_delta().currency() != branch_currency {
            return Err(StorageError::CurrencyMismatch {
                expected: branch_currency,
                actual: command.price_delta().currency().to_owned(),
            });
        }
        let product = read_product(&transaction, context.branch_id(), product_id)?;
        if product.archived() {
            return Err(StorageError::ProductUnavailable);
        }

        let option_id = EntityId::new_v7();
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO product_modifier_options (
                    modifier_option_id,
                    branch_id,
                    product_id,
                    display_name,
                    name_key,
                    price_delta_minor,
                    currency_code,
                    revision,
                    created_at_utc,
                    created_by_actor_id,
                    updated_at_utc,
                    updated_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?8, ?9)
                ",
                params![
                    option_id.to_string(),
                    context.branch_id().to_string(),
                    product_id.to_string(),
                    command.display_name().display(),
                    command.display_name().key(),
                    command.price_delta().minor_units(),
                    command.price_delta().currency(),
                    occurred_at,
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;

        let option = ModifierOption::new(
            option_id.clone(),
            context.branch_id().clone(),
            product_id.clone(),
            command.display_name().clone(),
            command.price_delta().clone(),
            1,
            false,
        );
        let payload = json!({
            "schema_version": 1,
            "entity_type": "product_modifier_option",
            "entity_id": option_id.to_string(),
            "product_id": product_id.to_string(),
            "revision": 1,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "after": {
                "display_name": option.display_name().display(),
                "price_delta_minor": option.price_delta().minor_units(),
                "currency_code": option.price_delta().currency(),
                "archived": false,
            },
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "catalog.product_modifier_option.created",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "product_modifier_option",
            &option_id,
            "catalog.product_modifier_option.created",
            &occurred_at,
        )?;

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(option)
    }

    /// Archives an option without removing it from the retained catalogue.
    /// The database trigger requires the audit event written here, so neither
    /// a direct update nor an unaudited reactivation can rewrite history.
    pub fn archive_product_modifier_option(
        &self,
        modifier_option_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<ModifierOption, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let existing = read_modifier_option(&transaction, context.branch_id(), modifier_option_id)?;
        if existing.archived() || expected_revision < 1 || existing.revision() != expected_revision
        {
            return Err(StorageError::CatalogConflict);
        }
        let revision = existing
            .revision()
            .checked_add(1)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        let occurred_at = utc_timestamp(&transaction)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "product_modifier_option",
            "entity_id": modifier_option_id.to_string(),
            "product_id": existing.product_id().to_string(),
            "revision": revision,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "reason": reason.as_str(),
            "before": {
                "display_name": existing.display_name().display(),
                "price_delta_minor": existing.price_delta().minor_units(),
                "currency_code": existing.price_delta().currency(),
                "archived": false,
            },
            "after": {"archived": true},
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "catalog.product_modifier_option.archived",
            &payload,
        )?;
        let updated = transaction
            .execute(
                "
                UPDATE product_modifier_options
                SET archived_at_utc = ?1,
                    archived_by_actor_id = ?2,
                    archive_reason = ?3,
                    archive_audit_event_id = ?4,
                    updated_at_utc = ?1,
                    updated_by_actor_id = ?2,
                    revision = revision + 1
                WHERE modifier_option_id = ?5
                  AND branch_id = ?6
                  AND revision = ?7
                  AND archived_at_utc IS NULL
                ",
                params![
                    occurred_at,
                    context.actor_id().to_string(),
                    reason.as_str(),
                    audit_event_id.to_string(),
                    modifier_option_id.to_string(),
                    context.branch_id().to_string(),
                    expected_revision,
                ],
            )
            .map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "product_modifier_option",
            modifier_option_id,
            "catalog.product_modifier_option.archived",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(ModifierOption::new(
            existing.modifier_option_id().clone(),
            existing.branch_id().clone(),
            existing.product_id().clone(),
            existing.display_name().clone(),
            existing.price_delta().clone(),
            revision,
            true,
        ))
    }

    /// Creates a named branch tax rate used by exclusive/inclusive product
    /// treatments. Rates are arithmetic configuration only—not a jurisdiction
    /// compliance claim.
    pub fn create_branch_tax_rate(
        &self,
        display_name: &str,
        basis_points: u32,
        context: &MutationContext,
    ) -> Result<LocalBranchTaxRate, StorageError> {
        require_management_authority(context)?;
        let name = DisplayName::category(display_name)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let rate = TaxRate::new(name.display(), basis_points)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let active_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM branch_tax_rates WHERE branch_id = ?1 AND archived_at_utc IS NULL",
                [context.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if active_count >= i64::try_from(ros_core::pricing::MAX_TAX_RATES_PER_LINE).unwrap_or(8) {
            return Err(StorageError::InvalidPersistedData(
                "a branch may have at most eight active tax rates".to_owned(),
            ));
        }
        let tax_rate_id = EntityId::new_v7();
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO branch_tax_rates (
                    tax_rate_id,
                    branch_id,
                    display_name,
                    name_key,
                    basis_points,
                    revision,
                    created_at_utc,
                    created_by_actor_id,
                    updated_at_utc,
                    updated_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?6, ?7)
                ",
                params![
                    tax_rate_id.to_string(),
                    context.branch_id().to_string(),
                    rate.name(),
                    name.key(),
                    i64::from(rate.basis_points()),
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "branch_tax_rate",
            "entity_id": tax_rate_id.to_string(),
            "revision": 1,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "after": {
                "display_name": rate.name(),
                "basis_points": rate.basis_points(),
                "archived": false,
            },
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "catalog.branch_tax_rate.created",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "branch_tax_rate",
            &tax_rate_id,
            "catalog.branch_tax_rate.created",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(LocalBranchTaxRate {
            tax_rate_id,
            display_name: rate.name().to_owned(),
            basis_points: rate.basis_points(),
            revision: 1,
            archived: false,
        })
    }

    pub fn list_branch_tax_rates(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<LocalBranchTaxRate>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT tax_rate_id, display_name, basis_points, revision, archived_at_utc \
                 FROM branch_tax_rates WHERE branch_id = ?1 \
                 ORDER BY archived_at_utc IS NOT NULL, name_key, tax_rate_id",
            )
            .map_err(StorageError::Sql)?;
        statement
            .query_map([branch_id.to_string()], |row| {
                Ok(LocalBranchTaxRate {
                    tax_rate_id: EntityId::parse(&row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    display_name: row.get(1)?,
                    basis_points: u32::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    revision: row.get(3)?,
                    archived: row.get::<_, Option<String>>(4)?.is_some(),
                })
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    pub fn archive_branch_tax_rate(
        &self,
        tax_rate_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<LocalBranchTaxRate, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let existing: Option<(String, i64, i64, Option<String>)> = transaction
            .query_row(
                "SELECT display_name, basis_points, revision, archived_at_utc \
                 FROM branch_tax_rates WHERE tax_rate_id = ?1 AND branch_id = ?2",
                params![tax_rate_id.to_string(), context.branch_id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        let Some((display_name, basis_points, revision, archived_at)) = existing else {
            return Err(StorageError::CatalogConflict);
        };
        if archived_at.is_some() || expected_revision < 1 || revision != expected_revision {
            return Err(StorageError::CatalogConflict);
        }
        let next_revision = revision
            .checked_add(1)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        let occurred_at = utc_timestamp(&transaction)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "branch_tax_rate",
            "entity_id": tax_rate_id.to_string(),
            "revision": next_revision,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "reason": reason.as_str(),
            "before": {
                "display_name": display_name,
                "basis_points": basis_points,
                "archived": false,
            },
            "after": {"archived": true},
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "catalog.branch_tax_rate.archived",
            &payload,
        )?;
        let updated = transaction
            .execute(
                "
                UPDATE branch_tax_rates
                SET archived_at_utc = ?1,
                    archived_by_actor_id = ?2,
                    archive_reason = ?3,
                    archive_audit_event_id = ?4,
                    updated_at_utc = ?1,
                    updated_by_actor_id = ?2,
                    revision = revision + 1
                WHERE tax_rate_id = ?5
                  AND branch_id = ?6
                  AND revision = ?7
                  AND archived_at_utc IS NULL
                ",
                params![
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                    reason.as_str(),
                    audit_event_id.to_string(),
                    tax_rate_id.to_string(),
                    context.branch_id().to_string(),
                    expected_revision,
                ],
            )
            .map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "branch_tax_rate",
            tax_rate_id,
            "catalog.branch_tax_rate.archived",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(LocalBranchTaxRate {
            tax_rate_id: tax_rate_id.clone(),
            display_name,
            basis_points: u32::try_from(basis_points).map_err(|_| {
                StorageError::InvalidPersistedData("tax rate basis points are invalid".to_owned())
            })?,
            revision: next_revision,
            archived: true,
        })
    }

    /// Sets a product's tax treatment. Exclusive/inclusive products use every
    /// currently active branch tax rate at sale time.
    pub fn set_product_tax_treatment(
        &self,
        product_id: &EntityId,
        tax_treatment: &str,
        expected_revision: i64,
        context: &MutationContext,
    ) -> Result<Product, StorageError> {
        require_management_authority(context)?;
        if !matches!(tax_treatment, "no_tax" | "exclusive" | "inclusive") {
            return Err(StorageError::InvalidPersistedData(
                "tax treatment must be no_tax, exclusive, or inclusive".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let existing = read_product(&transaction, context.branch_id(), product_id)?;
        if existing.archived() || expected_revision < 1 || existing.revision() != expected_revision
        {
            return Err(StorageError::CatalogConflict);
        }
        let revision = existing
            .revision()
            .checked_add(1)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        let occurred_at = utc_timestamp(&transaction)?;
        let previous: String = transaction
            .query_row(
                "SELECT tax_treatment FROM products WHERE product_id = ?1 AND branch_id = ?2",
                params![product_id.to_string(), context.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "product",
            "entity_id": product_id.to_string(),
            "revision": revision,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "before": {"tax_treatment": previous},
            "after": {"tax_treatment": tax_treatment},
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "catalog.product.tax_treatment_updated",
            &payload,
        )?;
        let updated = transaction
            .execute(
                "
                UPDATE products
                SET tax_treatment = ?1,
                    revision = revision + 1,
                    updated_at_utc = ?2,
                    updated_by_actor_id = ?3
                WHERE product_id = ?4
                  AND branch_id = ?5
                  AND revision = ?6
                  AND archived_at_utc IS NULL
                ",
                params![
                    tax_treatment,
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                    product_id.to_string(),
                    context.branch_id().to_string(),
                    expected_revision,
                ],
            )
            .map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "product",
            product_id,
            "catalog.product.tax_treatment_updated",
            &occurred_at,
        )?;
        let product = read_product(&transaction, context.branch_id(), product_id)?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(product)
    }

    /// Saves an open restaurant order as an append-only operational revision.
    /// This never creates an invoice or payment: financial finalization stays
    /// in `complete_sale` until the cashier records payment.
    pub fn save_draft_order(
        &self,
        draft_order_id: Option<&EntityId>,
        expected_revision: Option<i64>,
        fulfillment: OrderFulfillment,
        table_name: Option<&str>,
        lines: &[SaleLineInput],
        context: &MutationContext,
    ) -> Result<DraftOrder, StorageError> {
        self.save_draft_order_internal(
            DraftOrderSaveRequest::preserving(
                draft_order_id,
                expected_revision,
                fulfillment,
                table_name,
                lines,
            ),
            context,
        )
    }

    /// Saves a draft revision with an explicit kitchen-note value. Whitespace
    /// around a note is normalized away, blank input becomes absent, and a
    /// note is never copied into audit payload text.
    pub fn save_draft_order_with_kitchen_note(
        &self,
        request: DraftOrderSaveRequest<'_>,
        context: &MutationContext,
    ) -> Result<DraftOrder, StorageError> {
        self.save_draft_order_internal(request, context)
    }

    fn save_draft_order_internal(
        &self,
        request: DraftOrderSaveRequest<'_>,
        context: &MutationContext,
    ) -> Result<DraftOrder, StorageError> {
        let DraftOrderSaveRequest {
            draft_order_id,
            expected_revision,
            fulfillment,
            table_name,
            kitchen_note_override,
            lines,
        } = request;
        let kitchen_note_override = kitchen_note_override
            .map(normalize_kitchen_note)
            .transpose()?;
        require_counter_authority(context)?;
        if lines.is_empty() {
            return Err(StorageError::InvalidPersistedData(
                "a draft order needs at least one line".to_owned(),
            ));
        }
        ensure_unique_sale_line_configurations(
            lines,
            "a draft order cannot contain the same product and modifier combination more than once",
        )?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;
        let occurred_at = utc_timestamp(&transaction)?;
        let table = match (fulfillment, table_name) {
            (OrderFulfillment::DineIn, Some(value)) => Some(
                DisplayName::category(value)
                    .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?,
            ),
            (OrderFulfillment::DineIn, None) => {
                return Err(StorageError::InvalidPersistedData(
                    "dine-in orders need a table".to_owned(),
                ));
            }
            (OrderFulfillment::Takeaway, _) => None,
        };
        let table_id = if let Some(table) = &table {
            let existing: Option<String> = transaction.query_row(
                "SELECT table_id FROM restaurant_tables WHERE branch_id = ?1 AND name_key = ?2 AND is_active = 1",
                params![context.branch_id().to_string(), table.key()], |row| row.get(0),
            ).optional().map_err(StorageError::Sql)?;
            match existing {
                Some(value) => Some(value),
                None => {
                    let table_id = EntityId::new_v7();
                    transaction.execute(
                        "INSERT INTO restaurant_tables (table_id, branch_id, display_name, name_key, is_active, revision, created_at_utc, created_by_actor_id, updated_at_utc, updated_by_actor_id) VALUES (?1, ?2, ?3, ?4, 1, 1, ?5, ?6, ?5, ?6)",
                        params![table_id.to_string(), context.branch_id().to_string(), table.display(), table.key(), occurred_at.as_str(), context.actor_id().to_string()],
                    ).map_err(StorageError::Sql)?;
                    Some(table_id.to_string())
                }
            }
        } else {
            None
        };

        let mut subtotal_minor = 0_i64;
        let mut snapshots = Vec::with_capacity(lines.len());
        for line in lines {
            let snapshot = resolve_sale_line_snapshot(
                &transaction,
                context.branch_id(),
                &branch_currency,
                line,
            )?;
            subtotal_minor = subtotal_minor
                .checked_add(snapshot.line_total_minor)
                .ok_or(StorageError::FinancialAmountOverflow)?;
            snapshots.push(snapshot);
        }
        let subtotal = Money::new(subtotal_minor, &branch_currency)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let (draft_order_id, revision, kitchen_note) = match draft_order_id {
            Some(draft_order_id) => {
                let current: Option<(i64, String, Option<String>)> = transaction.query_row(
                    "SELECT d.current_revision, d.draft_state, r.kitchen_note FROM draft_orders d JOIN draft_order_revisions r ON r.draft_order_id = d.draft_order_id AND r.revision = d.current_revision WHERE d.draft_order_id = ?1 AND d.branch_id = ?2",
                    params![draft_order_id.to_string(), context.branch_id().to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                ).optional().map_err(StorageError::Sql)?;
                let Some((current_revision, state, existing_kitchen_note)) = current else {
                    return Err(StorageError::CatalogConflict);
                };
                if state != "open" || expected_revision != Some(current_revision) {
                    return Err(StorageError::CatalogConflict);
                }
                let next_revision = current_revision
                    .checked_add(1)
                    .ok_or(StorageError::FinancialAmountOverflow)?;
                transaction.execute(
                    "UPDATE draft_orders SET table_id = ?1, fulfillment = ?2, current_revision = ?3, updated_at_utc = ?4, updated_by_actor_id = ?5 WHERE draft_order_id = ?6",
                    params![table_id, fulfillment.as_str(), next_revision, occurred_at.as_str(), context.actor_id().to_string(), draft_order_id.to_string()],
                ).map_err(StorageError::Sql)?;
                (
                    draft_order_id.clone(),
                    next_revision,
                    kitchen_note_override.unwrap_or(existing_kitchen_note),
                )
            }
            None => {
                let draft_order_id = EntityId::new_v7();
                transaction.execute(
                    "INSERT INTO draft_orders (draft_order_id, branch_id, table_id, fulfillment, draft_state, currency_code, current_revision, created_at_utc, created_by_actor_id, updated_at_utc, updated_by_actor_id) VALUES (?1, ?2, ?3, ?4, 'open', ?5, 1, ?6, ?7, ?6, ?7)",
                    params![draft_order_id.to_string(), context.branch_id().to_string(), table_id, fulfillment.as_str(), subtotal.currency(), occurred_at.as_str(), context.actor_id().to_string()],
                ).map_err(StorageError::Sql)?;
                (draft_order_id, 1, kitchen_note_override.unwrap_or(None))
            }
        };
        let snapshot_values = snapshots
            .iter()
            .map(sale_line_snapshot_value)
            .collect::<Vec<_>>();
        let line_snapshot_json = serde_json::to_string(&snapshot_values).map_err(|_| {
            StorageError::InvalidPersistedData("draft snapshot could not be encoded".to_owned())
        })?;
        transaction.execute(
            "INSERT INTO draft_order_revisions (draft_order_revision_id, draft_order_id, revision, subtotal_minor, line_count, line_snapshot_json, kitchen_note, saved_at_utc, saved_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![EntityId::new_v7().to_string(), draft_order_id.to_string(), revision, subtotal_minor, i64::try_from(lines.len()).map_err(|_| StorageError::FinancialAmountOverflow)?, line_snapshot_json, kitchen_note.as_deref(), occurred_at.as_str(), context.actor_id().to_string()],
        ).map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"draft_order","entity_id":draft_order_id.to_string(),"revision":revision,"fulfillment":fulfillment.as_str(),"table_name":table.as_ref().map(|value| value.display()),"kitchen_note_present":kitchen_note.is_some(),"subtotal_minor":subtotal_minor,"currency_code":subtotal.currency(),"lines":snapshot_values,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
        let event_type = if revision == 1 {
            "operations.draft_order.created"
        } else {
            "operations.draft_order.updated"
        };
        let audit_event_id =
            append_hashed_audit_event(&transaction, context, event_type, &payload)?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "draft_order",
            &draft_order_id,
            event_type,
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(DraftOrder {
            draft_order_id,
            fulfillment,
            state: "open".to_owned(),
            table_name: table.map(|value| value.display().to_owned()),
            kitchen_note,
            revision,
            subtotal,
            line_count: i64::try_from(lines.len())
                .map_err(|_| StorageError::FinancialAmountOverflow)?,
        })
    }

    pub fn send_draft_to_kitchen(
        &self,
        draft_order_id: &EntityId,
        expected_revision: i64,
        context: &MutationContext,
    ) -> Result<KitchenTicket, StorageError> {
        require_counter_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let occurred_at = utc_timestamp(&transaction)?;
        let draft: Option<DraftKitchenSendSource> = transaction.query_row(
            "SELECT d.current_revision, d.draft_state, t.display_name, r.line_snapshot_json, r.kitchen_note FROM draft_orders d JOIN draft_order_revisions r ON r.draft_order_id = d.draft_order_id AND r.revision = d.current_revision LEFT JOIN restaurant_tables t ON t.table_id = d.table_id WHERE d.draft_order_id = ?1 AND d.branch_id = ?2",
            params![draft_order_id.to_string(), context.branch_id().to_string()],
            |row| {
                Ok(DraftKitchenSendSource {
                    revision: row.get(0)?,
                    state: row.get(1)?,
                    table_label: row.get(2)?,
                    line_snapshot_json: row.get(3)?,
                    kitchen_note: row.get(4)?,
                })
            },
        ).optional().map_err(StorageError::Sql)?;
        let Some(DraftKitchenSendSource {
            revision,
            state,
            table_label,
            line_snapshot_json,
            kitchen_note,
        }) = draft
        else {
            return Err(StorageError::CatalogConflict);
        };
        if revision != expected_revision || state != "open" {
            return Err(StorageError::CatalogConflict);
        }
        // Do not copy a malformed historical snapshot into an immutable
        // kitchen ticket. Valid drafts are checked before persistence, but
        // this protects the kitchen workflow from corrupted local data too.
        parse_draft_snapshot_lines(&line_snapshot_json)?;
        let ticket_id = EntityId::new_v7();
        transaction.execute(
            "INSERT INTO kitchen_tickets (kitchen_ticket_id, branch_id, draft_order_id, draft_revision, ticket_state, table_label_snapshot, line_snapshot_json, kitchen_note_snapshot, created_at_utc, created_by_actor_id, updated_at_utc, updated_by_actor_id, revision) VALUES (?1, ?2, ?3, ?4, 'new', ?5, ?6, ?7, ?8, ?9, ?8, ?9, 1)",
            params![ticket_id.to_string(), context.branch_id().to_string(), draft_order_id.to_string(), revision, table_label, line_snapshot_json, kitchen_note.as_deref(), occurred_at.as_str(), context.actor_id().to_string()],
        ).map_err(StorageError::Sql)?;
        transaction.execute(
            "UPDATE draft_orders SET draft_state = 'sent_to_kitchen', updated_at_utc = ?1, updated_by_actor_id = ?2 WHERE draft_order_id = ?3 AND current_revision = ?4 AND draft_state = 'open'",
            params![occurred_at.as_str(), context.actor_id().to_string(), draft_order_id.to_string(), revision],
        ).map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"kitchen_ticket","entity_id":ticket_id.to_string(),"draft_order_id":draft_order_id.to_string(),"draft_revision":revision,"table_label":table_label,"kitchen_note_present":kitchen_note.is_some(),"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "operations.kitchen_ticket.sent",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "kitchen_ticket",
            &ticket_id,
            "operations.kitchen_ticket.sent",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(KitchenTicket {
            ticket_id,
            draft_order_id: draft_order_id.clone(),
            draft_revision: revision,
            state: "new".to_owned(),
            table_label,
            line_snapshot_json,
            kitchen_note,
            revision: 1,
            cancellation_pending: false,
        })
    }

    /// Cancels an unsent draft while preserving the draft and all saved
    /// revisions for counter-audit purposes. Kitchen-sent orders must be
    /// handled by an explicit kitchen cancellation workflow instead.
    pub fn cancel_open_draft_order(
        &self,
        draft_order_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let occurred_at = utc_timestamp(&transaction)?;
        let updated = transaction.execute(
            "UPDATE draft_orders SET draft_state = 'cancelled', updated_at_utc = ?1, updated_by_actor_id = ?2 WHERE draft_order_id = ?3 AND branch_id = ?4 AND current_revision = ?5 AND draft_state = 'open'",
            params![occurred_at.as_str(), context.actor_id().to_string(), draft_order_id.to_string(), context.branch_id().to_string(), expected_revision],
        ).map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }
        let payload = json!({"schema_version":1,"entity_type":"draft_order","entity_id":draft_order_id.to_string(),"revision":expected_revision,"reason":reason.as_str(),"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str(),"after":{"draft_state":"cancelled"}}).to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "operations.draft_order.cancelled",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "draft_order",
            draft_order_id,
            "operations.draft_order.cancelled",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Requests a stop-work cancellation for a kitchen-sent draft. The draft
    /// itself becomes cancelled, while the linked kitchen ticket remains
    /// visible until kitchen records a separate acknowledgement.
    pub fn cancel_sent_draft_order(
        &self,
        draft_order_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let target = load_sent_draft_for_kitchen_cancellation(
            &transaction,
            draft_order_id,
            expected_revision,
            context,
        )?;
        let occurred_at = utc_timestamp(&transaction)?;
        let notice_id = append_kitchen_ticket_cancellation_notice(
            &transaction,
            draft_order_id,
            &target,
            reason,
            context,
            &occurred_at,
        )?;
        let updated = transaction
            .execute(
                "UPDATE draft_orders SET draft_state = 'cancelled', updated_at_utc = ?1, updated_by_actor_id = ?2 WHERE draft_order_id = ?3 AND branch_id = ?4 AND current_revision = ?5 AND draft_state = 'sent_to_kitchen'",
                params![occurred_at.as_str(), context.actor_id().to_string(), draft_order_id.to_string(), context.branch_id().to_string(), expected_revision],
            )
            .map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }
        let payload = json!({
            "schema_version": 1,
            "entity_type": "draft_order",
            "entity_id": draft_order_id.to_string(),
            "revision": expected_revision,
            "reason": reason.as_str(),
            "kitchen_ticket_id": target.ticket_id.to_string(),
            "cancellation_notice_id": notice_id.to_string(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "after": {"draft_state": "cancelled"},
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "operations.draft_order.cancelled_after_kitchen_send",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "draft_order",
            draft_order_id,
            "operations.draft_order.cancelled_after_kitchen_send",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Creates a new, editable revision of a kitchen-sent draft. The old
    /// ticket is first cancelled as an immutable kitchen fact; the new draft
    /// revision can then be sent as a distinct ticket without overwriting the
    /// original snapshot.
    pub fn reopen_sent_draft_order(
        &self,
        draft_order_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<DraftOrder, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let target = load_sent_draft_for_kitchen_cancellation(
            &transaction,
            draft_order_id,
            expected_revision,
            context,
        )?;
        let fulfillment = OrderFulfillment::parse(&target.fulfillment)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let subtotal = Money::new(target.subtotal_minor, &target.currency_code)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        // Reopening preserves the original saved cart in a new revision.
        // Refuse to make a second immutable copy when that snapshot cannot be
        // treated as one unambiguous product-to-quantity set.
        parse_draft_snapshot_lines(&target.line_snapshot_json)?;
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        let occurred_at = utc_timestamp(&transaction)?;
        let notice_id = append_kitchen_ticket_cancellation_notice(
            &transaction,
            draft_order_id,
            &target,
            reason,
            context,
            &occurred_at,
        )?;
        transaction
            .execute(
            "INSERT INTO draft_order_revisions (draft_order_revision_id, draft_order_id, revision, subtotal_minor, line_count, line_snapshot_json, kitchen_note, saved_at_utc, saved_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![EntityId::new_v7().to_string(), draft_order_id.to_string(), next_revision, target.subtotal_minor, target.line_count, target.line_snapshot_json.as_str(), target.kitchen_note.as_deref(), occurred_at.as_str(), context.actor_id().to_string()],
            )
            .map_err(StorageError::Sql)?;
        let updated = transaction
            .execute(
                "UPDATE draft_orders SET draft_state = 'open', current_revision = ?1, updated_at_utc = ?2, updated_by_actor_id = ?3 WHERE draft_order_id = ?4 AND branch_id = ?5 AND current_revision = ?6 AND draft_state = 'sent_to_kitchen'",
                params![next_revision, occurred_at.as_str(), context.actor_id().to_string(), draft_order_id.to_string(), context.branch_id().to_string(), expected_revision],
            )
            .map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }
        let payload = json!({
            "schema_version": 1,
            "entity_type": "draft_order",
            "entity_id": draft_order_id.to_string(),
            "previous_revision": expected_revision,
            "revision": next_revision,
            "reason": reason.as_str(),
            "kitchen_ticket_id": target.ticket_id.to_string(),
            "cancellation_notice_id": notice_id.to_string(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "after": {
                "draft_state": "open",
                "kitchen_note_present": target.kitchen_note.is_some(),
            },
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "operations.draft_order.reopened_after_kitchen_send",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "draft_order",
            draft_order_id,
            "operations.draft_order.reopened_after_kitchen_send",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(DraftOrder {
            draft_order_id: draft_order_id.clone(),
            fulfillment,
            state: "open".to_owned(),
            table_name: target.table_name,
            kitchen_note: target.kitchen_note,
            revision: next_revision,
            subtotal,
            line_count: target.line_count,
        })
    }

    /// Records that kitchen has seen a cancellation request. This does not
    /// delete or mutate the ticket; it simply removes the stop-work notice
    /// from the active kitchen queue after acknowledgement.
    pub fn acknowledge_kitchen_ticket_cancellation(
        &self,
        ticket_id: &EntityId,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_kitchen_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let notice_id: Option<EntityId> = transaction
            .query_row(
                "SELECT notice.kitchen_ticket_cancellation_notice_id FROM kitchen_ticket_cancellation_notices notice LEFT JOIN kitchen_ticket_cancellation_acknowledgements acknowledgement ON acknowledgement.kitchen_ticket_cancellation_notice_id = notice.kitchen_ticket_cancellation_notice_id WHERE notice.kitchen_ticket_id = ?1 AND notice.branch_id = ?2 AND acknowledgement.kitchen_ticket_cancellation_notice_id IS NULL",
                params![ticket_id.to_string(), context.branch_id().to_string()],
                |row| EntityId::parse(&row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        let Some(notice_id) = notice_id else {
            return Err(StorageError::CatalogConflict);
        };
        let occurred_at = utc_timestamp(&transaction)?;
        let acknowledgement_id = EntityId::new_v7();
        let payload = json!({
            "schema_version": 1,
            "entity_type": "kitchen_ticket_cancellation_acknowledgement",
            "entity_id": acknowledgement_id.to_string(),
            "kitchen_ticket_id": ticket_id.to_string(),
            "cancellation_notice_id": notice_id.to_string(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "operations.kitchen_ticket.cancellation.acknowledged",
            &payload,
        )?;
        transaction
            .execute(
                "INSERT INTO kitchen_ticket_cancellation_acknowledgements (kitchen_ticket_cancellation_acknowledgement_id, kitchen_ticket_cancellation_notice_id, branch_id, acknowledged_at_utc, acknowledged_by_actor_id, audit_event_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![acknowledgement_id.to_string(), notice_id.to_string(), context.branch_id().to_string(), occurred_at.as_str(), context.actor_id().to_string(), audit_event_id.to_string()],
            )
            .map_err(StorageError::Sql)?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "kitchen_ticket_cancellation_acknowledgement",
            &acknowledgement_id,
            "operations.kitchen_ticket.cancellation.acknowledged",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    pub fn list_active_kitchen_tickets(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<KitchenTicket>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT t.kitchen_ticket_id, t.draft_order_id, t.draft_revision, t.ticket_state, t.table_label_snapshot, t.line_snapshot_json, t.kitchen_note_snapshot, t.revision, notice.kitchen_ticket_cancellation_notice_id IS NOT NULL AND acknowledgement.kitchen_ticket_cancellation_notice_id IS NULL FROM kitchen_tickets t LEFT JOIN kitchen_ticket_cancellation_notices notice ON notice.kitchen_ticket_id = t.kitchen_ticket_id LEFT JOIN kitchen_ticket_cancellation_acknowledgements acknowledgement ON acknowledgement.kitchen_ticket_cancellation_notice_id = notice.kitchen_ticket_cancellation_notice_id WHERE t.branch_id = ?1 AND t.ticket_state <> 'completed' AND (notice.kitchen_ticket_cancellation_notice_id IS NULL OR acknowledgement.kitchen_ticket_cancellation_notice_id IS NULL) ORDER BY t.created_at_utc ASC"
        ).map_err(StorageError::Sql)?;
        statement
            .query_map([branch_id.to_string()], |row| {
                Ok(KitchenTicket {
                    ticket_id: EntityId::parse(&row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    draft_order_id: EntityId::parse(&row.get::<_, String>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    draft_revision: row.get(2)?,
                    state: row.get(3)?,
                    table_label: row.get(4)?,
                    line_snapshot_json: row.get(5)?,
                    kitchen_note: row.get(6)?,
                    revision: row.get(7)?,
                    cancellation_pending: row.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    /// Prepares an owner-only CSV from immutable, branch-scoped financial
    /// facts. This is deliberately a read-only storage operation: it verifies
    /// the SQLCipher/database/audit contract and the active local Owner
    /// session inside one immediate transaction before any bytes are exposed.
    ///
    /// The CSV contains UTC-day/tender aggregates for payments, refunds, and
    /// expenses plus an all-time summary. It excludes customer records,
    /// free-text descriptions, product names, identifiers, credentials, and
    /// audit payloads. Flutter must still ask the person for a destination;
    /// this method never writes a plaintext report to disk.
    pub fn export_verified_community_financial_csv(
        &self,
    ) -> Result<VerifiedFinancialCsvExport, StorageError> {
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let (device_id, _) = ensure_local_installation_identity(&transaction)?;
        let staff = read_active_local_staff(&transaction, &branch, &device_id)?
            .ok_or(StorageError::StaffSessionRequired)?;
        if staff.role() != ActorRole::Owner {
            return Err(StorageError::PermissionDenied);
        }
        ensure_active_branch(&transaction, branch.branch_id())?;

        // Hold the immediate transaction lock across verification and the
        // report snapshot. A second local process cannot change financial
        // facts between the verified audit chain and CSV generation.
        verify_local_integrity_on_connection(&transaction)?;
        verify_financial_export_facts(&transaction, branch.branch_id())?;
        let export = build_bounded_financial_csv(&transaction, branch.branch_id())?;

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(export)
    }

    /// Returns a branch-local sales summary for one UTC accounting day,
    /// calculated only from finalized invoice and payment facts. Carts and
    /// drafts are excluded. Day boundaries match financial CSV
    /// `accounting_date_utc` (`substr(timestamp, 1, 10)`).
    pub fn local_sales_summary(
        &self,
        branch_id: &EntityId,
        accounting_date_utc: &str,
    ) -> Result<LocalSalesSummary, StorageError> {
        validate_accounting_date_utc(accounting_date_utc)?;
        let currency_code: String = self
            .connection
            .query_row(
                "SELECT currency_code FROM branches WHERE branch_id = ?1 AND archived_at_utc IS NULL",
                [branch_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sql)?
            .ok_or(StorageError::BranchNotFound)?;
        let (invoice_count, gross_total_minor, discount_minor, tax_minor): (i64, i64, i64, i64) =
            self.connection
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(total_minor), 0), \
                        COALESCE(SUM(discount_minor), 0), COALESCE(SUM(tax_minor), 0) \
                     FROM invoices \
                     WHERE branch_id = ?1 AND substr(finalized_at_utc, 1, 10) = ?2 \
                       AND NOT EXISTS (SELECT 1 FROM invoice_voids v WHERE v.invoice_id = invoices.invoice_id)",
                    params![branch_id.to_string(), accounting_date_utc],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(StorageError::Sql)?;
        let (gross_cash_minor, gross_card_minor, gross_upi_minor): (i64, i64, i64) = self
            .connection
            .query_row(
                "SELECT \
                    COALESCE(SUM(CASE WHEN payment_method = 'cash' THEN amount_minor ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN payment_method = 'card' THEN amount_minor ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN payment_method = 'upi' THEN amount_minor ELSE 0 END), 0) \
                 FROM payments WHERE branch_id = ?1 AND substr(recorded_at_utc, 1, 10) = ?2 \
                   AND NOT EXISTS (SELECT 1 FROM invoice_voids v WHERE v.invoice_id = payments.invoice_id)",
                params![branch_id.to_string(), accounting_date_utc],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(StorageError::Sql)?;
        let (refund_minor, cash_refund_minor, card_refund_minor, upi_refund_minor):
            (i64, i64, i64, i64) = self
            .connection
            .query_row(
                "SELECT \
                    COALESCE(SUM(amount_minor), 0), \
                    COALESCE(SUM(CASE WHEN payment_method_snapshot = 'cash' THEN amount_minor ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN payment_method_snapshot = 'card' THEN amount_minor ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN payment_method_snapshot = 'upi' THEN amount_minor ELSE 0 END), 0) \
                 FROM invoice_refunds WHERE branch_id = ?1 AND substr(refunded_at_utc, 1, 10) = ?2 \
                   AND NOT EXISTS (SELECT 1 FROM invoice_voids v WHERE v.invoice_id = invoice_refunds.invoice_id)",
                params![branch_id.to_string(), accounting_date_utc],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(StorageError::Sql)?;
        let expense_minor: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(SUM(amount_minor), 0) FROM expenses \
                 WHERE branch_id = ?1 AND substr(incurred_at_utc, 1, 10) = ?2",
                params![branch_id.to_string(), accounting_date_utc],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;

        Ok(LocalSalesSummary {
            invoice_count,
            total_minor: gross_total_minor
                .checked_sub(refund_minor)
                .ok_or(StorageError::FinancialAmountOverflow)?,
            cash_minor: gross_cash_minor
                .checked_sub(cash_refund_minor)
                .ok_or(StorageError::FinancialAmountOverflow)?,
            card_minor: gross_card_minor
                .checked_sub(card_refund_minor)
                .ok_or(StorageError::FinancialAmountOverflow)?,
            upi_minor: gross_upi_minor
                .checked_sub(upi_refund_minor)
                .ok_or(StorageError::FinancialAmountOverflow)?,
            refund_minor,
            expense_minor,
            discount_minor,
            tax_minor,
            currency_code,
        })
    }

    /// Current UTC calendar day (`YYYY-MM-DD`) from the encrypted database clock.
    pub fn current_accounting_date_utc(&self) -> Result<String, StorageError> {
        let day: String = self
            .connection
            .query_row("SELECT strftime('%Y-%m-%d', 'now')", [], |row| row.get(0))
            .map_err(StorageError::Sql)?;
        validate_accounting_date_utc(&day)?;
        Ok(day)
    }

    /// Retrieves recorded invoices for one UTC accounting day from immutable
    /// financial facts. It does not expose draft/cart state and uses a bounded
    /// result size so a long operating history cannot freeze the local client.
    pub fn list_recent_invoices(
        &self,
        branch_id: &EntityId,
        limit: i64,
        accounting_date_utc: &str,
    ) -> Result<Vec<LocalInvoiceSummary>, StorageError> {
        validate_accounting_date_utc(accounting_date_utc)?;
        if !(1..=100).contains(&limit) {
            return Err(StorageError::InvalidPersistedData(
                "invoice list limit was invalid".to_owned(),
            ));
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT i.invoice_id, i.invoice_number, i.total_minor, i.currency_code, \
                    i.finalized_at_utc, CASE WHEN COUNT(p.payment_id) = 1 THEN MIN(p.payment_method) ELSE 'split' END \
             FROM invoices i \
             JOIN payments p ON p.invoice_id = i.invoice_id \
             WHERE i.branch_id = ?1 AND substr(i.finalized_at_utc, 1, 10) = ?2 \
               AND NOT EXISTS (SELECT 1 FROM invoice_voids v WHERE v.invoice_id = i.invoice_id) \
             GROUP BY i.invoice_id \
             ORDER BY i.finalized_at_utc DESC, i.invoice_number DESC \
             LIMIT ?3",
            )
            .map_err(StorageError::Sql)?;
        statement
            .query_map(
                params![branch_id.to_string(), accounting_date_utc, limit],
                |row| {
                    Ok(LocalInvoiceSummary {
                        invoice_id: EntityId::parse(&row.get::<_, String>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        invoice_number: row.get(1)?,
                        total_minor: row.get(2)?,
                        currency_code: row.get(3)?,
                        finalized_at_utc: row.get(4)?,
                        payment_method: row.get(5)?,
                    })
                },
            )
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    /// Lists a bounded, branch-scoped audit timeline for the owner. The
    /// projection intentionally omits actor identifiers, device identifiers,
    /// hashes, and JSON payloads so reporting cannot become a data-exfiltration
    /// path for credentials or operational details.
    pub fn list_recent_audit_events(
        &self,
        limit: i64,
        context: &MutationContext,
    ) -> Result<Vec<LocalAuditEventSummary>, StorageError> {
        require_owner_authority(context)?;
        if !(1..=100).contains(&limit) {
            return Err(StorageError::InvalidPersistedData(
                "audit event list limit was invalid".to_owned(),
            ));
        }
        let active_branch: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM branches WHERE branch_id = ?1 AND archived_at_utc IS NULL)",
                [context.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if !active_branch {
            return Err(StorageError::BranchNotFound);
        }
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT sequence, event_type, occurred_at_utc
                FROM audit_events
                WHERE branch_id = ?1
                ORDER BY occurred_at_utc DESC, sequence DESC
                LIMIT ?2
                ",
            )
            .map_err(StorageError::Sql)?;
        statement
            .query_map(params![context.branch_id().to_string(), limit], |row| {
                Ok(LocalAuditEventSummary {
                    sequence: row.get(0)?,
                    event_type: row.get(1)?,
                    occurred_at_utc: row.get(2)?,
                })
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    /// Loads a historical receipt from immutable snapshots, never from the
    /// current catalogue. Counter-capable staff may reprint a receipt without
    /// receiving unrelated report or customer data.
    pub fn load_invoice_detail(
        &self,
        invoice_id: &EntityId,
        context: &MutationContext,
    ) -> Result<LocalInvoiceDetail, StorageError> {
        require_counter_authority(context)?;
        #[allow(clippy::type_complexity)]
        let invoice: Option<(i64, String, i64, i64, i64, i64, String, String)> = self
            .connection
            .query_row(
                "
                SELECT
                    invoices.invoice_number,
                    orders.order_type,
                    invoices.subtotal_minor,
                    invoices.discount_minor,
                    invoices.tax_minor,
                    invoices.total_minor,
                    invoices.currency_code,
                    invoices.finalized_at_utc
                FROM invoices
                JOIN orders ON orders.order_id = invoices.order_id
                    AND orders.branch_id = invoices.branch_id
                WHERE invoices.invoice_id = ?1 AND invoices.branch_id = ?2
                ",
                params![invoice_id.to_string(), context.branch_id().to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()
            .map_err(StorageError::Sql)?;
        let (
            invoice_number,
            fulfillment,
            subtotal_minor,
            discount_minor,
            tax_minor,
            total_minor,
            currency_code,
            finalized_at_utc,
        ) = invoice.ok_or(StorageError::InvoiceNotFound)?;

        let mut line_statement = self
            .connection
            .prepare(
                "
                SELECT product_name_snapshot, modifier_snapshot_json, quantity, unit_price_minor, line_total_minor
                FROM order_lines
                WHERE order_id = (
                    SELECT order_id FROM invoices
                    WHERE invoice_id = ?1 AND branch_id = ?2
                )
                ORDER BY line_number
                ",
            )
            .map_err(StorageError::Sql)?;
        let lines = line_statement
            .query_map(
                params![invoice_id.to_string(), context.branch_id().to_string()],
                |row| {
                    Ok(LocalInvoiceLine {
                        display_name: row.get(0)?,
                        modifier_names: parse_modifier_snapshot_names(&row.get::<_, String>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        quantity: row.get(2)?,
                        unit_price_minor: row.get(3)?,
                        line_total_minor: row.get(4)?,
                    })
                },
            )
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)?;
        if lines.is_empty() {
            return Err(StorageError::InvoiceNotFound);
        }

        let mut payment_statement = self
            .connection
            .prepare(
                "
                SELECT payment_method, amount_minor
                FROM payments
                WHERE invoice_id = ?1 AND branch_id = ?2
                ORDER BY payment_sequence, payment_id
                ",
            )
            .map_err(StorageError::Sql)?;
        let payments = payment_statement
            .query_map(
                params![invoice_id.to_string(), context.branch_id().to_string()],
                |row| {
                    Ok(LocalInvoicePayment {
                        payment_method: row.get(0)?,
                        amount_minor: row.get(1)?,
                    })
                },
            )
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)?;
        if payments.is_empty() {
            return Err(StorageError::InvoiceNotFound);
        }
        let refunded_minor: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(SUM(amount_minor), 0) FROM invoice_refunds WHERE invoice_id = ?1 AND branch_id = ?2",
                params![invoice_id.to_string(), context.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;

        Ok(LocalInvoiceDetail {
            invoice_id: invoice_id.clone(),
            invoice_number,
            fulfillment,
            subtotal_minor,
            discount_minor,
            tax_minor,
            total_minor,
            refunded_minor,
            currency_code,
            finalized_at_utc,
            lines,
            payments,
        })
    }

    /// Lists the highest-grossing items for one UTC accounting day from
    /// immutable finalized invoice line snapshots. This never joins the current
    /// menu, so renamed, repriced, or archived products cannot alter historical
    /// reporting.
    pub fn list_top_selling_items(
        &self,
        branch_id: &EntityId,
        limit: i64,
        accounting_date_utc: &str,
    ) -> Result<Vec<LocalItemSalesSummary>, StorageError> {
        validate_accounting_date_utc(accounting_date_utc)?;
        if !(1..=100).contains(&limit) {
            return Err(StorageError::InvalidPersistedData(
                "item sales list limit was invalid".to_owned(),
            ));
        }
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT
                    lines.product_name_snapshot,
                    SUM(lines.quantity),
                    SUM(lines.line_total_minor),
                    MIN(lines.currency_code)
                FROM order_lines AS lines
                JOIN orders ON orders.order_id = lines.order_id
                JOIN invoices ON invoices.order_id = orders.order_id
                    AND invoices.branch_id = orders.branch_id
                WHERE orders.branch_id = ?1
                  AND substr(invoices.finalized_at_utc, 1, 10) = ?2
                  AND NOT EXISTS (
                      SELECT 1 FROM invoice_voids v WHERE v.invoice_id = invoices.invoice_id
                  )
                GROUP BY lines.product_name_snapshot, lines.currency_code
                ORDER BY SUM(lines.line_total_minor) DESC,
                    SUM(lines.quantity) DESC,
                    lines.product_name_snapshot COLLATE NOCASE
                LIMIT ?3
                ",
            )
            .map_err(StorageError::Sql)?;
        statement
            .query_map(
                params![branch_id.to_string(), accounting_date_utc, limit],
                |row| {
                    Ok(LocalItemSalesSummary {
                        display_name: row.get(0)?,
                        quantity: row.get(1)?,
                        gross_total_minor: row.get(2)?,
                        currency_code: row.get(3)?,
                    })
                },
            )
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    /// Records an accountable operating expense as an immutable branch fact.
    /// Cashiers and kitchen users cannot create expenses; they require an
    /// owner/manager session and retain category, rationale, payment method,
    /// audit event, and future-sync envelope.
    pub fn record_expense(
        &self,
        category: &str,
        description: &MutationReason,
        amount: &Money,
        payment_method: PaymentMethod,
        context: &MutationContext,
    ) -> Result<EntityId, StorageError> {
        require_inventory_control_authority(context)?;
        let category = category.trim();
        if category.is_empty()
            || category.chars().count() > 120
            || category.chars().any(char::is_control)
        {
            return Err(StorageError::InvalidPersistedData(
                "expense category was invalid".to_owned(),
            ));
        }
        if amount.minor_units() <= 0 {
            return Err(StorageError::InvalidPersistedData(
                "expense amount must be positive".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;
        if amount.currency() != branch_currency {
            return Err(StorageError::CurrencyMismatch {
                expected: branch_currency,
                actual: amount.currency().to_owned(),
            });
        }
        let expense_id = EntityId::new_v7();
        let incurred_at = utc_timestamp(&transaction)?;
        transaction.execute(
            "INSERT INTO expenses (expense_id, branch_id, category, description, amount_minor, currency_code, payment_method, incurred_at_utc, recorded_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![expense_id.to_string(), context.branch_id().to_string(), category, description.as_str(), amount.minor_units(), amount.currency(), payment_method.as_str(), incurred_at, context.actor_id().to_string()],
        ).map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"expense","entity_id":expense_id.to_string(),"category":category,"description":description.as_str(),"amount_minor":amount.minor_units(),"currency_code":amount.currency(),"payment_method":payment_method.as_str(),"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "financial.expense.recorded",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "expense",
            &expense_id,
            "financial.expense.recorded",
            &incurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(expense_id)
    }

    pub fn list_recent_expenses(
        &self,
        branch_id: &EntityId,
        limit: i64,
    ) -> Result<Vec<LocalExpenseSummary>, StorageError> {
        if !(1..=100).contains(&limit) {
            return Err(StorageError::InvalidPersistedData(
                "expense list limit was invalid".to_owned(),
            ));
        }
        let mut statement = self.connection.prepare(
            "SELECT expense_id, category, description, amount_minor, currency_code, payment_method, incurred_at_utc FROM expenses WHERE branch_id = ?1 ORDER BY incurred_at_utc DESC, expense_id DESC LIMIT ?2",
        ).map_err(StorageError::Sql)?;
        statement
            .query_map(params![branch_id.to_string(), limit], |row| {
                Ok(LocalExpenseSummary {
                    expense_id: EntityId::parse(&row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    category: row.get(1)?,
                    description: row.get(2)?,
                    amount_minor: row.get(3)?,
                    currency_code: row.get(4)?,
                    payment_method: row.get(5)?,
                    incurred_at_utc: row.get(6)?,
                })
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    pub fn current_open_cash_drawer(
        &self,
        branch_id: &EntityId,
    ) -> Result<Option<OpenCashDrawerSession>, StorageError> {
        self.connection.query_row(
            "SELECT s.cash_drawer_session_id, s.opening_cash_minor, s.currency_code FROM cash_drawer_sessions s WHERE s.branch_id = ?1 AND NOT EXISTS(SELECT 1 FROM cash_drawer_closures c WHERE c.cash_drawer_session_id = s.cash_drawer_session_id) ORDER BY s.opened_at_utc DESC LIMIT 1",
            [branch_id.to_string()],
            |row| Ok(OpenCashDrawerSession { session_id: EntityId::parse(&row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?, opening_cash_minor: row.get(1)?, currency_code: row.get(2)? }),
        ).optional().map_err(StorageError::Sql)
    }

    pub fn open_cash_drawer(
        &self,
        opening_cash_minor: i64,
        context: &MutationContext,
    ) -> Result<EntityId, StorageError> {
        require_inventory_control_authority(context)?;
        if opening_cash_minor < 0 {
            return Err(StorageError::InvalidPersistedData(
                "opening cash cannot be negative".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let currency = ensure_active_branch(&transaction, context.branch_id())?;
        let already_open: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM cash_drawer_sessions s WHERE s.branch_id = ?1 AND NOT EXISTS(SELECT 1 FROM cash_drawer_closures c WHERE c.cash_drawer_session_id = s.cash_drawer_session_id))",
            [context.branch_id().to_string()], |row| row.get(0)).map_err(StorageError::Sql)?;
        if already_open {
            return Err(StorageError::CatalogConflict);
        }
        let session_id = EntityId::new_v7();
        let opened_at = utc_timestamp(&transaction)?;
        transaction.execute("INSERT INTO cash_drawer_sessions (cash_drawer_session_id, branch_id, opening_cash_minor, currency_code, opened_at_utc, opened_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![session_id.to_string(), context.branch_id().to_string(), opening_cash_minor, currency, opened_at, context.actor_id().to_string()]).map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"cash_drawer_session","entity_id":session_id.to_string(),"opening_cash_minor":opening_cash_minor,"currency_code":currency,"correlation_id":context.correlation_id().to_string()}).to_string();
        let audit = append_hashed_audit_event(
            &transaction,
            context,
            "financial.cash_drawer.opened",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit,
            "cash_drawer_session",
            &session_id,
            "financial.cash_drawer.opened",
            &opened_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(session_id)
    }

    pub fn close_cash_drawer(
        &self,
        session_id: &EntityId,
        counted_cash_minor: i64,
        context: &MutationContext,
    ) -> Result<CashDrawerClosure, StorageError> {
        require_inventory_control_authority(context)?;
        if counted_cash_minor < 0 {
            return Err(StorageError::InvalidPersistedData(
                "counted cash cannot be negative".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let session: Option<(i64, String)> = transaction
            .query_row(
                "SELECT opening_cash_minor, opened_at_utc FROM cash_drawer_sessions s WHERE s.cash_drawer_session_id = ?1 AND s.branch_id = ?2 AND NOT EXISTS(SELECT 1 FROM cash_drawer_closures c WHERE c.cash_drawer_session_id = s.cash_drawer_session_id)",
                params![session_id.to_string(), context.branch_id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        let Some((opening_cash_minor, opened_at)) = session else {
            return Err(StorageError::CatalogConflict);
        };
        let cash_sales: i64 = transaction.query_row(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM payments WHERE branch_id = ?1 AND payment_method = 'cash' AND recorded_at_utc >= ?2",
            params![context.branch_id().to_string(), opened_at], |row| row.get(0)).map_err(StorageError::Sql)?;
        let cash_refunds: i64 = transaction.query_row(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM invoice_refunds WHERE branch_id = ?1 AND payment_method_snapshot = 'cash' AND refunded_at_utc >= ?2",
            params![context.branch_id().to_string(), opened_at], |row| row.get(0)).map_err(StorageError::Sql)?;
        let cash_expenses: i64 = transaction.query_row(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM expenses WHERE branch_id = ?1 AND payment_method = 'cash' AND incurred_at_utc >= ?2",
            params![context.branch_id().to_string(), opened_at], |row| row.get(0)).map_err(StorageError::Sql)?;
        let expected_cash_minor = opening_cash_minor
            .checked_add(cash_sales)
            .and_then(|value| value.checked_sub(cash_refunds))
            .and_then(|value| value.checked_sub(cash_expenses))
            .ok_or(StorageError::FinancialAmountOverflow)?;
        if expected_cash_minor < 0 {
            return Err(StorageError::FinancialAmountOverflow);
        }
        let variance_minor = counted_cash_minor
            .checked_sub(expected_cash_minor)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        let closure_id = EntityId::new_v7();
        let closed_at = utc_timestamp(&transaction)?;
        transaction.execute("INSERT INTO cash_drawer_closures (cash_drawer_closure_id, cash_drawer_session_id, counted_cash_minor, expected_cash_minor, variance_minor, closed_at_utc, closed_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![closure_id.to_string(), session_id.to_string(), counted_cash_minor, expected_cash_minor, variance_minor, closed_at, context.actor_id().to_string()]).map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"cash_drawer_closure","entity_id":closure_id.to_string(),"cash_drawer_session_id":session_id.to_string(),"expected_cash_minor":expected_cash_minor,"counted_cash_minor":counted_cash_minor,"variance_minor":variance_minor,"correlation_id":context.correlation_id().to_string()}).to_string();
        let audit = append_hashed_audit_event(
            &transaction,
            context,
            "financial.cash_drawer.closed",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit,
            "cash_drawer_closure",
            &closure_id,
            "financial.cash_drawer.closed",
            &closed_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(CashDrawerClosure {
            session_id: session_id.clone(),
            expected_cash_minor,
            counted_cash_minor,
            variance_minor,
        })
    }

    /// Derives a product's current tracked stock from immutable ledger facts.
    /// Products with no movements are deliberately treated as untracked/zero.
    pub fn inventory_balance(
        &self,
        branch_id: &EntityId,
        product_id: &EntityId,
    ) -> Result<i64, StorageError> {
        self.connection
            .query_row(
                "SELECT COALESCE(SUM(quantity_delta), 0) FROM inventory_movements WHERE branch_id = ?1 AND product_id = ?2",
                params![branch_id.to_string(), product_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)
    }

    /// Identifies whether inventory control has been explicitly enabled for a
    /// product by recording at least one ledger fact. A zero balance can still
    /// be tracked, so this cannot be inferred from `inventory_balance`.
    pub fn is_inventory_tracked(
        &self,
        branch_id: &EntityId,
        product_id: &EntityId,
    ) -> Result<bool, StorageError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM inventory_movements WHERE branch_id = ?1 AND product_id = ?2)",
                params![branch_id.to_string(), product_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)
    }

    /// Returns the current low-stock policy for a tracked product. Absence is
    /// intentional: restaurants choose their own replenishment threshold.
    pub fn inventory_low_stock_threshold(
        &self,
        branch_id: &EntityId,
        product_id: &EntityId,
    ) -> Result<Option<i64>, StorageError> {
        let threshold: Option<Option<i64>> = self
            .connection
            .query_row(
                "
                SELECT threshold_quantity FROM (
                    SELECT threshold_quantity, occurred_at_utc,
                        inventory_low_stock_threshold_event_id AS event_id
                    FROM inventory_low_stock_threshold_events
                    WHERE branch_id = ?1 AND product_id = ?2
                    UNION ALL
                    SELECT NULL, occurred_at_utc,
                        inventory_low_stock_threshold_clear_event_id
                    FROM inventory_low_stock_threshold_clear_events
                    WHERE branch_id = ?1 AND product_id = ?2
                )
                ORDER BY occurred_at_utc DESC, event_id DESC
                LIMIT 1
                ",
                params![branch_id.to_string(), product_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        Ok(threshold.flatten())
    }

    pub fn clear_inventory_low_stock_threshold(
        &self,
        product_id: &EntityId,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_inventory_control_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        read_product(&transaction, context.branch_id(), product_id)?;
        let current_policy: Option<Option<i64>> = transaction
            .query_row(
                "
                SELECT threshold_quantity FROM (
                    SELECT threshold_quantity, occurred_at_utc,
                        inventory_low_stock_threshold_event_id AS event_id
                    FROM inventory_low_stock_threshold_events
                    WHERE branch_id = ?1 AND product_id = ?2
                    UNION ALL
                    SELECT NULL, occurred_at_utc,
                        inventory_low_stock_threshold_clear_event_id
                    FROM inventory_low_stock_threshold_clear_events
                    WHERE branch_id = ?1 AND product_id = ?2
                )
                ORDER BY occurred_at_utc DESC, event_id DESC
                LIMIT 1
                ",
                params![context.branch_id().to_string(), product_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        if current_policy.flatten().is_none() {
            return Err(StorageError::CatalogConflict);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        let event_id = EntityId::new_v7();
        transaction.execute("INSERT INTO inventory_low_stock_threshold_clear_events (inventory_low_stock_threshold_clear_event_id, branch_id, product_id, reason, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![event_id.to_string(), context.branch_id().to_string(), product_id.to_string(), reason.as_str(), occurred_at, context.actor_id().to_string()]).map_err(StorageError::Sql)?;
        let audit = append_hashed_audit_event(&transaction, context, "inventory.low_stock_threshold.cleared", &json!({"schema_version":1,"entity_type":"inventory_low_stock_threshold","entity_id":event_id.to_string(),"product_id":product_id.to_string(),"reason":reason.as_str(),"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string())?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit,
            "inventory_low_stock_threshold",
            &event_id,
            "inventory.low_stock_threshold.cleared",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Appends an owner/manager-selected low-stock threshold for a product
    /// already opted into stock tracking. This changes only alert policy, not
    /// historical or current quantity.
    pub fn set_inventory_low_stock_threshold(
        &self,
        product_id: &EntityId,
        threshold_quantity: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_inventory_control_authority(context)?;
        if threshold_quantity < 0 {
            return Err(StorageError::InvalidPersistedData(
                "low-stock threshold cannot be negative".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        read_product(&transaction, context.branch_id(), product_id)?;
        let tracked: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM inventory_movements WHERE branch_id = ?1 AND product_id = ?2)",
                params![context.branch_id().to_string(), product_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if !tracked {
            return Err(StorageError::InventoryNotTracked);
        }
        // A clear event is an intentional policy state, so compare against
        // the latest threshold *or* clear event rather than historical sets.
        let existing: Option<Option<i64>> = transaction
            .query_row(
                "
                SELECT threshold_quantity FROM (
                    SELECT threshold_quantity, occurred_at_utc,
                        inventory_low_stock_threshold_event_id AS event_id
                    FROM inventory_low_stock_threshold_events
                    WHERE branch_id = ?1 AND product_id = ?2
                    UNION ALL
                    SELECT NULL, occurred_at_utc,
                        inventory_low_stock_threshold_clear_event_id
                    FROM inventory_low_stock_threshold_clear_events
                    WHERE branch_id = ?1 AND product_id = ?2
                )
                ORDER BY occurred_at_utc DESC, event_id DESC
                LIMIT 1
                ",
                params![context.branch_id().to_string(), product_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        let existing = existing.flatten();
        if existing == Some(threshold_quantity) {
            return Err(StorageError::CatalogConflict);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        let threshold_event_id = EntityId::new_v7();
        transaction
            .execute(
                "INSERT INTO inventory_low_stock_threshold_events (inventory_low_stock_threshold_event_id, branch_id, product_id, threshold_quantity, reason, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![threshold_event_id.to_string(), context.branch_id().to_string(), product_id.to_string(), threshold_quantity, reason.as_str(), occurred_at, context.actor_id().to_string()],
            )
            .map_err(StorageError::Sql)?;
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "inventory.low_stock_threshold.set",
            &json!({
                "schema_version": 1,
                "entity_type": "inventory_low_stock_threshold",
                "entity_id": threshold_event_id.to_string(),
                "product_id": product_id.to_string(),
                "reason": reason.as_str(),
                "before": { "threshold_quantity": existing },
                "after": { "threshold_quantity": threshold_quantity },
                "correlation_id": context.correlation_id().to_string(),
                "actor_role": context.actor_role().as_str(),
            })
            .to_string(),
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "inventory_low_stock_threshold",
            &threshold_event_id,
            "inventory.low_stock_threshold.set",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    pub fn record_inventory_opening(
        &self,
        product_id: &EntityId,
        quantity: i64,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        if quantity <= 0 {
            return Err(StorageError::InvalidPersistedData(
                "opening stock must be positive".to_owned(),
            ));
        }
        require_inventory_control_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let product = read_product(&transaction, context.branch_id(), product_id)?;
        let already_tracked: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM inventory_movements WHERE branch_id = ?1 AND product_id = ?2)",
                params![context.branch_id().to_string(), product_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if already_tracked {
            return Err(StorageError::CatalogConflict);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        let movement_id = EntityId::new_v7();
        transaction.execute("INSERT INTO inventory_movements (inventory_movement_id, branch_id, product_id, movement_type, quantity_delta, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, 'opening', ?4, ?5, ?6)", params![movement_id.to_string(), context.branch_id().to_string(), product_id.to_string(), quantity, occurred_at, context.actor_id().to_string()]).map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"inventory_movement","entity_id":movement_id.to_string(),"product_id":product.product_id().to_string(),"movement_type":"opening","quantity_delta":quantity,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "inventory.opening.recorded",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "inventory_movement",
            &movement_id,
            "inventory.opening.recorded",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    pub fn record_inventory_purchase(
        &self,
        product_id: &EntityId,
        quantity: i64,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        if quantity <= 0 {
            return Err(StorageError::InvalidPersistedData(
                "purchase quantity must be positive".to_owned(),
            ));
        }
        require_inventory_control_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        read_product(&transaction, context.branch_id(), product_id)?;
        let occurred_at = utc_timestamp(&transaction)?;
        let movement_id = EntityId::new_v7();
        transaction
            .execute(
                "INSERT INTO inventory_movements (inventory_movement_id, branch_id, product_id, movement_type, quantity_delta, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, 'purchase', ?4, ?5, ?6)",
                params![movement_id.to_string(), context.branch_id().to_string(), product_id.to_string(), quantity, occurred_at, context.actor_id().to_string()],
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"inventory_movement","entity_id":movement_id.to_string(),"product_id":product_id.to_string(),"movement_type":"purchase","quantity_delta":quantity,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "inventory.purchase.recorded",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "inventory_movement",
            &movement_id,
            "inventory.purchase.recorded",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Records stock discarded through spoilage, breakage, or another
    /// accountable operational loss. The positive input is stored as a
    /// negative immutable ledger delta.
    pub fn record_inventory_waste(
        &self,
        product_id: &EntityId,
        quantity: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        if quantity <= 0 {
            return Err(StorageError::InvalidPersistedData(
                "waste quantity must be positive".to_owned(),
            ));
        }
        self.record_inventory_reasoned_movement(product_id, -quantity, "waste", reason, context)
    }

    /// Records a counted-stock correction. Its signed delta must be non-zero
    /// and the rationale is preserved with the immutable movement.
    pub fn record_inventory_adjustment(
        &self,
        product_id: &EntityId,
        quantity_delta: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        if quantity_delta == 0 {
            return Err(StorageError::InvalidPersistedData(
                "inventory adjustment must not be zero".to_owned(),
            ));
        }
        self.record_inventory_reasoned_movement(
            product_id,
            quantity_delta,
            "adjustment",
            reason,
            context,
        )
    }

    fn record_inventory_reasoned_movement(
        &self,
        product_id: &EntityId,
        quantity_delta: i64,
        movement_type: &'static str,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_inventory_control_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        read_product(&transaction, context.branch_id(), product_id)?;
        let occurred_at = utc_timestamp(&transaction)?;
        let movement_id = EntityId::new_v7();
        transaction
            .execute(
                "INSERT INTO inventory_movements (inventory_movement_id, branch_id, product_id, movement_type, quantity_delta, reason, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![movement_id.to_string(), context.branch_id().to_string(), product_id.to_string(), movement_type, quantity_delta, reason.as_str(), occurred_at, context.actor_id().to_string()],
            )
            .map_err(StorageError::Sql)?;
        let event_type = match movement_type {
            "waste" => "inventory.waste.recorded",
            "adjustment" => "inventory.adjustment.recorded",
            _ => {
                return Err(StorageError::InvalidPersistedData(
                    "unsupported inventory movement type".to_owned(),
                ));
            }
        };
        let payload = json!({"schema_version":1,"entity_type":"inventory_movement","entity_id":movement_id.to_string(),"product_id":product_id.to_string(),"movement_type":movement_type,"quantity_delta":quantity_delta,"reason":reason.as_str(),"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
        let audit_event_id =
            append_hashed_audit_event(&transaction, context, event_type, &payload)?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "inventory_movement",
            &movement_id,
            event_type,
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    /// Records a full or partial refund as a new immutable financial fact.
    /// The original invoice and payment remain unchanged for auditability.
    /// Requires a distinct Owner/Manager approver per ADR 0006.
    pub fn refund_invoice(
        &self,
        invoice_id: &EntityId,
        amount_minor: i64,
        reason: &MutationReason,
        context: &MutationContext,
        approval: &DualPersonApproval<'_>,
    ) -> Result<EntityId, StorageError> {
        require_management_authority(context)?;
        if amount_minor <= 0 {
            return Err(StorageError::InvalidPersistedData(
                "refund amount must be positive".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;
        let invoice: Option<(i64, String)> = transaction
            .query_row(
                "SELECT i.total_minor, i.currency_code \
                 FROM invoices i \
                 WHERE i.invoice_id = ?1 AND i.branch_id = ?2",
                params![invoice_id.to_string(), context.branch_id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        let Some((invoice_total, invoice_currency)) = invoice else {
            return Err(StorageError::CatalogConflict);
        };
        if invoice_currency != branch_currency {
            return Err(StorageError::CurrencyMismatch {
                expected: branch_currency,
                actual: invoice_currency,
            });
        }
        let already_refunded: i64 = transaction
            .query_row(
                "SELECT COALESCE(SUM(amount_minor), 0) FROM invoice_refunds WHERE invoice_id = ?1",
                [invoice_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        let remaining = invoice_total
            .checked_sub(already_refunded)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        if amount_minor > remaining {
            return Err(StorageError::CatalogConflict);
        }
        let mut payments = transaction.prepare("SELECT payment_method, amount_minor FROM payments WHERE invoice_id = ?1 ORDER BY payment_sequence, payment_id").map_err(StorageError::Sql)?;
        let payment_allocations = payments
            .query_map([invoice_id.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)?;
        drop(payments);
        let mut remaining_refund = amount_minor;
        let refunded_at = utc_timestamp(&transaction)?;
        let mut first_refund_id = None;
        for (payment_method, paid_minor) in payment_allocations {
            let refunded_for_method: i64 = transaction.query_row("SELECT COALESCE(SUM(amount_minor), 0) FROM invoice_refunds WHERE invoice_id = ?1 AND payment_method_snapshot = ?2", params![invoice_id.to_string(), payment_method], |row| row.get(0)).map_err(StorageError::Sql)?;
            let available = paid_minor
                .checked_sub(refunded_for_method)
                .ok_or(StorageError::FinancialAmountOverflow)?;
            let allocation = remaining_refund.min(available);
            if allocation <= 0 {
                continue;
            }
            let refund_id = EntityId::new_v7();
            transaction.execute("INSERT INTO invoice_refunds (invoice_refund_id, branch_id, invoice_id, payment_method_snapshot, amount_minor, currency_code, reason, refunded_at_utc, refunded_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", params![refund_id.to_string(), context.branch_id().to_string(), invoice_id.to_string(), payment_method, allocation, branch_currency, reason.as_str(), refunded_at.as_str(), context.actor_id().to_string()]).map_err(StorageError::Sql)?;
            let payload = json!({"schema_version":1,"entity_type":"invoice_refund","entity_id":refund_id.to_string(),"invoice_id":invoice_id.to_string(),"amount_minor":allocation,"currency_code":branch_currency,"reason":reason.as_str(),"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
            let audit_event_id = append_hashed_audit_event(
                &transaction,
                context,
                "financial.invoice.refunded",
                &payload,
            )?;
            append_sync_outbox_event(
                &transaction,
                context,
                &audit_event_id,
                "invoice_refund",
                &refund_id,
                "financial.invoice.refunded",
                &refunded_at,
            )?;
            first_refund_id.get_or_insert(refund_id);
            remaining_refund = remaining_refund
                .checked_sub(allocation)
                .ok_or(StorageError::FinancialAmountOverflow)?;
            if remaining_refund == 0 {
                break;
            }
        }
        let refund_id = first_refund_id.ok_or(StorageError::CatalogConflict)?;
        if remaining_refund != 0 {
            return Err(StorageError::CatalogConflict);
        }
        record_dual_person_approval(
            &transaction,
            context,
            approval,
            "refund",
            "invoice",
            invoice_id,
            Some(amount_minor),
            Some(&branch_currency),
            reason,
            "invoice_refund",
            &refund_id,
            &refunded_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(refund_id)
    }

    /// Records a full invoice void as a new immutable financial fact. The
    /// original invoice and payments remain unchanged. Voids are refused when
    /// any refund already exists. Requires a distinct Owner/Manager approver
    /// per ADR 0006.
    pub fn void_invoice(
        &self,
        invoice_id: &EntityId,
        reason: &MutationReason,
        context: &MutationContext,
        approval: &DualPersonApproval<'_>,
    ) -> Result<EntityId, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let invoice_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM invoices
                    WHERE invoice_id = ?1 AND branch_id = ?2
                 )",
                params![invoice_id.to_string(), context.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if !invoice_exists {
            return Err(StorageError::CatalogConflict);
        }
        let already_voided: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM invoice_voids WHERE invoice_id = ?1)",
                [invoice_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if already_voided {
            return Err(StorageError::CatalogConflict);
        }
        let has_refunds: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM invoice_refunds WHERE invoice_id = ?1)",
                [invoice_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if has_refunds {
            return Err(StorageError::CatalogConflict);
        }
        let void_id = EntityId::new_v7();
        let voided_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "INSERT INTO invoice_voids (
                    invoice_void_id, branch_id, invoice_id, reason,
                    voided_at_utc, voided_by_actor_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    void_id.to_string(),
                    context.branch_id().to_string(),
                    invoice_id.to_string(),
                    reason.as_str(),
                    voided_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "invoice_void",
            "entity_id": void_id.to_string(),
            "invoice_id": invoice_id.to_string(),
            "reason": reason.as_str(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
        })
        .to_string();
        let audit_event_id =
            append_hashed_audit_event(&transaction, context, "financial.invoice.voided", &payload)?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "invoice_void",
            &void_id,
            "financial.invoice.voided",
            &voided_at,
        )?;
        record_dual_person_approval(
            &transaction,
            context,
            approval,
            "void",
            "invoice",
            invoice_id,
            None,
            None,
            reason,
            "invoice_void",
            &void_id,
            &voided_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(void_id)
    }

    /// Records an append-only close for one UTC accounting day. Snapshot totals
    /// are frozen at close time. A second close for the same day is rejected;
    /// reopen is intentionally unsupported without founder policy.
    pub fn close_accounting_day(
        &self,
        accounting_date_utc: &str,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<LocalAccountingDayClose, StorageError> {
        require_management_authority(context)?;
        validate_accounting_date_utc(accounting_date_utc)?;
        let summary = self.local_sales_summary(context.branch_id(), accounting_date_utc)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let already_closed: bool = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM accounting_day_closes
                    WHERE branch_id = ?1 AND accounting_date_utc = ?2
                 )",
                params![context.branch_id().to_string(), accounting_date_utc],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if already_closed {
            return Err(StorageError::CatalogConflict);
        }
        let close_id = EntityId::new_v7();
        let closed_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "INSERT INTO accounting_day_closes (
                    accounting_day_close_id, branch_id, accounting_date_utc,
                    invoice_count, total_minor, cash_minor, card_minor, upi_minor,
                    refund_minor, expense_minor, discount_minor, tax_minor,
                    currency_code, reason, closed_at_utc, closed_by_actor_id
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
                 )",
                params![
                    close_id.to_string(),
                    context.branch_id().to_string(),
                    accounting_date_utc,
                    summary.invoice_count(),
                    summary.total_minor(),
                    summary.cash_minor(),
                    summary.card_minor(),
                    summary.upi_minor(),
                    summary.refund_minor(),
                    summary.expense_minor(),
                    summary.discount_minor(),
                    summary.tax_minor(),
                    summary.currency_code(),
                    reason.as_str(),
                    closed_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "accounting_day_close",
            "entity_id": close_id.to_string(),
            "accounting_date_utc": accounting_date_utc,
            "invoice_count": summary.invoice_count(),
            "total_minor": summary.total_minor(),
            "discount_minor": summary.discount_minor(),
            "tax_minor": summary.tax_minor(),
            "refund_minor": summary.refund_minor(),
            "expense_minor": summary.expense_minor(),
            "currency_code": summary.currency_code(),
            "reason": reason.as_str(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "financial.accounting_day.closed",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "accounting_day_close",
            &close_id,
            "financial.accounting_day.closed",
            &closed_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(LocalAccountingDayClose {
            close_id,
            accounting_date_utc: accounting_date_utc.to_owned(),
            invoice_count: summary.invoice_count(),
            total_minor: summary.total_minor(),
            cash_minor: summary.cash_minor(),
            card_minor: summary.card_minor(),
            upi_minor: summary.upi_minor(),
            refund_minor: summary.refund_minor(),
            expense_minor: summary.expense_minor(),
            discount_minor: summary.discount_minor(),
            tax_minor: summary.tax_minor(),
            currency_code: summary.currency_code().to_owned(),
            reason: reason.as_str().to_owned(),
            closed_at_utc: closed_at.as_str().to_owned(),
        })
    }

    pub fn accounting_day_close(
        &self,
        branch_id: &EntityId,
        accounting_date_utc: &str,
    ) -> Result<Option<LocalAccountingDayClose>, StorageError> {
        validate_accounting_date_utc(accounting_date_utc)?;
        self.connection
            .query_row(
                "SELECT accounting_day_close_id, accounting_date_utc, invoice_count,
                        total_minor, cash_minor, card_minor, upi_minor, refund_minor,
                        expense_minor, discount_minor, tax_minor, currency_code,
                        reason, closed_at_utc
                 FROM accounting_day_closes
                 WHERE branch_id = ?1 AND accounting_date_utc = ?2",
                params![branch_id.to_string(), accounting_date_utc],
                |row| {
                    Ok(LocalAccountingDayClose {
                        close_id: EntityId::parse(&row.get::<_, String>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        accounting_date_utc: row.get(1)?,
                        invoice_count: row.get(2)?,
                        total_minor: row.get(3)?,
                        cash_minor: row.get(4)?,
                        card_minor: row.get(5)?,
                        upi_minor: row.get(6)?,
                        refund_minor: row.get(7)?,
                        expense_minor: row.get(8)?,
                        discount_minor: row.get(9)?,
                        tax_minor: row.get(10)?,
                        currency_code: row.get(11)?,
                        reason: row.get(12)?,
                        closed_at_utc: row.get(13)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::Sql)
    }

    pub fn advance_kitchen_ticket(
        &self,
        ticket_id: &EntityId,
        expected_revision: i64,
        next_state: &str,
        context: &MutationContext,
    ) -> Result<KitchenTicket, StorageError> {
        require_kitchen_authority(context)?;
        let valid = matches!(next_state, "preparing" | "ready" | "completed");
        if !valid {
            return Err(StorageError::InvalidPersistedData(
                "invalid kitchen state".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let current: Option<MutableKitchenTicketState> = transaction.query_row(
            "SELECT ticket_state, revision, draft_order_id, draft_revision, table_label_snapshot, line_snapshot_json, kitchen_note_snapshot FROM kitchen_tickets WHERE kitchen_ticket_id = ?1 AND branch_id = ?2",
            params![ticket_id.to_string(), context.branch_id().to_string()],
            |row| {
                Ok(MutableKitchenTicketState {
                    state: row.get(0)?,
                    revision: row.get(1)?,
                    draft_order_id: EntityId::parse(&row.get::<_, String>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    draft_revision: row.get(3)?,
                    table_label: row.get(4)?,
                    line_snapshot_json: row.get(5)?,
                    kitchen_note: row.get(6)?,
                })
            },
        ).optional().map_err(StorageError::Sql)?;
        let Some(MutableKitchenTicketState {
            state,
            revision,
            draft_order_id,
            draft_revision,
            table_label,
            line_snapshot_json: snapshot,
            kitchen_note,
        }) = current
        else {
            return Err(StorageError::CatalogConflict);
        };
        let cancellation_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM kitchen_ticket_cancellation_notices WHERE kitchen_ticket_id = ?1)",
                [ticket_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        // A cancellation is a stop-work signal, not another ticket state.
        // Even after kitchen acknowledges it, the original ticket remains a
        // preserved fact and cannot accidentally resume its normal flow.
        if cancellation_exists {
            return Err(StorageError::CatalogConflict);
        }
        if revision != expected_revision
            || !matches!(
                (state.as_str(), next_state),
                ("new", "preparing") | ("preparing", "ready") | ("ready", "completed")
            )
        {
            return Err(StorageError::CatalogConflict);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        let next_revision = revision
            .checked_add(1)
            .ok_or(StorageError::FinancialAmountOverflow)?;
        transaction.execute("UPDATE kitchen_tickets SET ticket_state = ?1, revision = ?2, updated_at_utc = ?3, updated_by_actor_id = ?4 WHERE kitchen_ticket_id = ?5 AND revision = ?6", params![next_state, next_revision, occurred_at.as_str(), context.actor_id().to_string(), ticket_id.to_string(), expected_revision]).map_err(StorageError::Sql)?;
        let payload = json!({"schema_version":1,"entity_type":"kitchen_ticket","entity_id":ticket_id.to_string(),"from_state":state,"to_state":next_state,"revision":next_revision,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "operations.kitchen_ticket.progressed",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "kitchen_ticket",
            ticket_id,
            "operations.kitchen_ticket.progressed",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(KitchenTicket {
            ticket_id: ticket_id.clone(),
            draft_order_id,
            draft_revision,
            state: next_state.to_owned(),
            table_label,
            line_snapshot_json: snapshot,
            kitchen_note,
            revision: next_revision,
            cancellation_pending: false,
        })
    }

    /// Finalizes a local counter sale in one durable transaction. Flutter only
    /// supplies product identities, quantities, fulfillment, and payment
    /// method; Rust snapshots catalog facts, allocates the invoice number, and
    /// records immutable financial/audit/outbox facts atomically.
    pub fn complete_sale(
        &self,
        command: &CompleteSale,
        context: &MutationContext,
    ) -> Result<CompletedSale, StorageError> {
        self.complete_sale_with_draft(command, None, context)
    }

    /// Calculates the trusted payable total without recording a sale. The
    /// counter uses this so split allocations and displayed totals match the
    /// same tax/discount path as checkout.
    pub fn preview_sale_pricing(
        &self,
        lines: &[SaleLineInput],
        discount: Option<OrderDiscount>,
        context: &MutationContext,
    ) -> Result<SalePricingPreview, StorageError> {
        require_counter_authority(context)?;
        if lines.is_empty() {
            return Err(StorageError::InvalidPersistedData(
                "a pricing preview requires at least one sale line".to_owned(),
            ));
        }
        if discount.is_some() {
            require_management_authority(context)?;
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;
        let mut snapshots = Vec::with_capacity(lines.len());
        for line in lines {
            snapshots.push(resolve_sale_line_snapshot(
                &transaction,
                context.branch_id(),
                &branch_currency,
                line,
            )?);
        }
        let pricing = calculate_sale_pricing(
            &transaction,
            context.branch_id(),
            &branch_currency,
            &snapshots,
            discount,
        )?;
        Ok(SalePricingPreview {
            subtotal_minor: pricing.net_before_discount_minor(),
            discount_minor: pricing.discount_minor(),
            tax_minor: pricing.tax_minor(),
            payable_minor: pricing.payable_minor(),
            currency_code: branch_currency,
        })
    }

    /// Returns the current tax treatment for every active catalogue product.
    pub fn list_product_tax_treatments(
        &self,
        branch_id: &EntityId,
    ) -> Result<HashMap<EntityId, String>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT product_id, tax_treatment FROM products \
                 WHERE branch_id = ?1 AND archived_at_utc IS NULL",
            )
            .map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch_id.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)?;
        let mut treatments = HashMap::with_capacity(rows.len());
        for (product_id, treatment) in rows {
            treatments.insert(parse_persisted_id(&product_id)?, treatment);
        }
        Ok(treatments)
    }

    pub fn complete_draft_sale(
        &self,
        command: &CompleteSale,
        draft_order_id: &EntityId,
        expected_revision: i64,
        context: &MutationContext,
    ) -> Result<CompletedSale, StorageError> {
        self.complete_sale_with_draft(command, Some((draft_order_id, expected_revision)), context)
    }

    fn complete_sale_with_draft(
        &self,
        command: &CompleteSale,
        draft: Option<(&EntityId, i64)>,
        context: &MutationContext,
    ) -> Result<CompletedSale, StorageError> {
        require_counter_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;
        if let Some(customer_id) = command.customer_id() {
            ensure_active_customer(&transaction, context.branch_id(), customer_id)?;
        }
        if let Some((draft_order_id, expected_revision)) = draft {
            let current: Option<(i64, String, bool, String, String)> = transaction.query_row(
                "SELECT d.current_revision, d.draft_state, EXISTS(SELECT 1 FROM draft_order_settlements s WHERE s.draft_order_id = d.draft_order_id), d.fulfillment, r.line_snapshot_json FROM draft_orders d JOIN draft_order_revisions r ON r.draft_order_id = d.draft_order_id AND r.revision = d.current_revision WHERE d.draft_order_id = ?1 AND d.branch_id = ?2",
                params![draft_order_id.to_string(), context.branch_id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            ).optional().map_err(StorageError::Sql)?;
            let Some((revision, state, settled, fulfillment, line_snapshot_json)) = current else {
                return Err(StorageError::CatalogConflict);
            };
            if revision != expected_revision
                || !matches!(state.as_str(), "open" | "sent_to_kitchen")
                || settled
            {
                return Err(StorageError::CatalogConflict);
            }
            let saved_fulfillment = OrderFulfillment::parse(&fulfillment)
                .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
            let saved_lines = parse_draft_snapshot_lines(&line_snapshot_json)?;
            if command.fulfillment() != saved_fulfillment
                || !same_sale_lines(command.lines(), &saved_lines)
            {
                return Err(StorageError::CatalogConflict);
            }
        }
        let mut snapshots = Vec::with_capacity(command.lines().len());
        let mut catalog_subtotal_minor = 0_i64;

        for line in command.lines() {
            let snapshot = resolve_sale_line_snapshot(
                &transaction,
                context.branch_id(),
                &branch_currency,
                line,
            )?;
            catalog_subtotal_minor = catalog_subtotal_minor
                .checked_add(snapshot.line_total_minor)
                .ok_or(StorageError::FinancialAmountOverflow)?;
            snapshots.push(snapshot);
        }

        if command.discount().is_some() {
            require_management_authority(context)?;
        }

        let pricing = calculate_sale_pricing(
            &transaction,
            context.branch_id(),
            &branch_currency,
            &snapshots,
            command.discount().cloned(),
        )?;
        let subtotal_minor = pricing.net_before_discount_minor();
        let discount_minor = pricing.discount_minor();
        let tax_minor = pricing.tax_minor();
        let pricing_snapshot_json = encode_pricing_snapshot(&pricing)?;
        let total = Money::new(pricing.payable_minor(), &branch_currency)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let _ = catalog_subtotal_minor;
        let payment_allocations = command
            .payment_allocations()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                vec![
                    ros_core::PaymentAllocationInput::new(
                        command.payment_method(),
                        total.minor_units(),
                    )
                    .expect("trusted invoice total is positive"),
                ]
            });
        let allocated_minor = payment_allocations
            .iter()
            .try_fold(0_i64, |sum, allocation| {
                sum.checked_add(allocation.amount_minor())
                    .ok_or(StorageError::FinancialAmountOverflow)
            })?;
        if allocated_minor != total.minor_units() {
            return Err(StorageError::InvalidPersistedData(
                "payment allocations must equal the trusted invoice total".to_owned(),
            ));
        }
        let completed_payment_method = payment_allocations
            .first()
            .expect("validated allocation set")
            .payment_method();
        let occurred_at = utc_timestamp(&transaction)?;
        let order_id = EntityId::new_v7();
        let invoice_id = EntityId::new_v7();
        let invoice_number = next_invoice_number(&transaction, context.branch_id())?;

        transaction
            .execute(
                "
                INSERT INTO orders (
                    order_id,
                    branch_id,
                    customer_id,
                    order_type,
                    order_state,
                    currency_code,
                    subtotal_minor,
                    discount_minor,
                    tax_minor,
                    pricing_snapshot_json,
                    created_at_utc,
                    created_by_actor_id,
                    finalized_at_utc,
                    finalized_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, 'finalized', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?10, ?11)
                ",
                params![
                    order_id.to_string(),
                    context.branch_id().to_string(),
                    command.customer_id().map(ToString::to_string),
                    command.fulfillment().as_str(),
                    branch_currency.as_str(),
                    subtotal_minor,
                    discount_minor,
                    tax_minor,
                    pricing_snapshot_json.as_str(),
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;

        for (position, snapshot) in snapshots.iter().enumerate() {
            let line_number = i64::try_from(position)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(StorageError::FinancialAmountOverflow)?;
            transaction
                .execute(
                    "
                    INSERT INTO order_lines (
                        order_line_id,
                        order_id,
                        product_id,
                        line_number,
                        product_name_snapshot,
                        quantity,
                        unit_price_minor,
                        modifier_total_minor,
                        modifier_snapshot_json,
                        line_total_minor,
                        currency_code,
                        created_at_utc
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        order_id.to_string(),
                        snapshot.product_id.to_string(),
                        line_number,
                        snapshot.product_name.as_str(),
                        snapshot.quantity,
                        snapshot.unit_price_minor,
                        snapshot.modifier_total_minor,
                        modifier_snapshot_json(snapshot)?,
                        snapshot.line_total_minor,
                        total.currency(),
                        occurred_at.as_str(),
                    ],
                )
                .map_err(StorageError::Sql)?;
        }

        transaction
            .execute(
                "
                INSERT INTO invoices (
                    invoice_id,
                    branch_id,
                    order_id,
                    invoice_number,
                    invoice_state,
                    subtotal_minor,
                    discount_minor,
                    tax_minor,
                    total_minor,
                    pricing_snapshot_json,
                    currency_code,
                    finalized_at_utc,
                    finalized_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, 'finalized', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ",
                params![
                    invoice_id.to_string(),
                    context.branch_id().to_string(),
                    order_id.to_string(),
                    invoice_number,
                    subtotal_minor,
                    discount_minor,
                    tax_minor,
                    total.minor_units(),
                    pricing_snapshot_json.as_str(),
                    total.currency(),
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;

        // Advance only after the invoice exists. Besides preserving atomicity,
        // this lets the database trigger prove that the sequence remains one
        // greater than the highest persisted invoice number.
        advance_invoice_number(&transaction, context.branch_id(), invoice_number)?;

        let payment_records = payment_allocations
            .iter()
            .enumerate()
            .map(|(position, allocation)| {
                let sequence = i64::try_from(position)
                    .ok()
                    .and_then(|value| value.checked_add(1))
                    .ok_or(StorageError::FinancialAmountOverflow)?;
                let payment_id = EntityId::new_v7();
                transaction.execute(
                    "INSERT INTO payments (payment_id, branch_id, invoice_id, payment_sequence, payment_method, payment_state, amount_minor, currency_code, recorded_at_utc, recorded_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, 'recorded', ?6, ?7, ?8, ?9)",
                    params![payment_id.to_string(), context.branch_id().to_string(), invoice_id.to_string(), sequence, allocation.payment_method().as_str(), allocation.amount_minor(), total.currency(), occurred_at.as_str(), context.actor_id().to_string()],
                ).map_err(StorageError::Sql)?;
                Ok((payment_id, sequence, *allocation))
            })
            .collect::<Result<Vec<_>, StorageError>>()?;

        // A product becomes inventory-tracked only when the owner or manager
        // has recorded its first stock movement. This preserves the Community
        // Edition's ability to sell services and untracked menu items while
        // making tracked-product stock enforcement part of this same sale
        // transaction.
        let mut sale_inventory_movements: Vec<(EntityId, String, i64)> = Vec::new();
        for snapshot in &snapshots {
            let recipe_id: Option<String> = transaction
                .query_row(
                    "
                    SELECT product_recipe_id
                    FROM product_recipes
                    WHERE branch_id = ?1
                      AND finished_product_id = ?2
                      AND archived_at_utc IS NULL
                    ",
                    params![
                        context.branch_id().to_string(),
                        snapshot.product_id.to_string()
                    ],
                    |row| row.get(0),
                )
                .optional()
                .map_err(StorageError::Sql)?;
            if let Some(recipe_id) = recipe_id {
                let mut lines = transaction
                    .prepare(
                        "
                        SELECT ingredient_product_id, quantity_per_unit
                        FROM product_recipe_lines
                        WHERE product_recipe_id = ?1
                        ORDER BY line_number
                        ",
                    )
                    .map_err(StorageError::Sql)?;
                let ingredient_rows = lines
                    .query_map([recipe_id], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })
                    .map_err(StorageError::Sql)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(StorageError::Sql)?;
                drop(lines);
                for (ingredient_product_id, quantity_per_unit) in ingredient_rows {
                    let is_tracked: bool = transaction
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM inventory_movements WHERE branch_id = ?1 AND product_id = ?2)",
                            params![context.branch_id().to_string(), &ingredient_product_id],
                            |row| row.get(0),
                        )
                        .map_err(StorageError::Sql)?;
                    if !is_tracked {
                        continue;
                    }
                    let delta = quantity_per_unit
                        .checked_mul(snapshot.quantity)
                        .ok_or(StorageError::FinancialAmountOverflow)?;
                    let movement_id = EntityId::new_v7();
                    transaction
                        .execute(
                            "INSERT INTO inventory_movements (inventory_movement_id, branch_id, product_id, movement_type, quantity_delta, source_document_id, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, 'sale', ?4, ?5, ?6, ?7)",
                            params![
                                movement_id.to_string(),
                                context.branch_id().to_string(),
                                &ingredient_product_id,
                                -delta,
                                invoice_id.to_string(),
                                occurred_at.as_str(),
                                context.actor_id().to_string()
                            ],
                        )
                        .map_err(StorageError::Sql)?;
                    sale_inventory_movements.push((movement_id, ingredient_product_id, -delta));
                }
                continue;
            }
            let is_tracked: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM inventory_movements WHERE branch_id = ?1 AND product_id = ?2)",
                    params![context.branch_id().to_string(), snapshot.product_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(StorageError::Sql)?;
            if !is_tracked {
                continue;
            }
            let movement_id = EntityId::new_v7();
            transaction
                .execute(
                    "INSERT INTO inventory_movements (inventory_movement_id, branch_id, product_id, movement_type, quantity_delta, source_document_id, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, 'sale', ?4, ?5, ?6, ?7)",
                    params![movement_id.to_string(), context.branch_id().to_string(), snapshot.product_id.to_string(), -snapshot.quantity, invoice_id.to_string(), occurred_at.as_str(), context.actor_id().to_string()],
                )
                .map_err(StorageError::Sql)?;
            sale_inventory_movements.push((
                movement_id,
                snapshot.product_id.to_string(),
                -snapshot.quantity,
            ));
        }

        let line_payload = snapshots
            .iter()
            .map(|snapshot| {
                json!({
                    "product_id": snapshot.product_id.to_string(),
                    "product_name_snapshot": snapshot.product_name.as_str(),
                    "quantity": snapshot.quantity,
                    "base_unit_price_minor": snapshot.base_unit_price_minor,
                    "modifier_total_minor": snapshot.modifier_total_minor,
                    "modifier_snapshot": modifier_snapshot_value(snapshot),
                    "unit_price_minor": snapshot.unit_price_minor,
                    "line_total_minor": snapshot.line_total_minor,
                })
            })
            .collect::<Vec<_>>();
        let order_payload = json!({
            "schema_version": 1,
            "entity_type": "order",
            "entity_id": order_id.to_string(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "fulfillment": command.fulfillment().as_str(),
            "state": "finalized",
            "subtotal_minor": subtotal_minor,
            "discount_minor": discount_minor,
            "tax_minor": tax_minor,
            "total_minor": total.minor_units(),
            "currency_code": total.currency(),
            "lines": line_payload,
        })
        .to_string();
        let invoice_payload = json!({
            "schema_version": 1,
            "entity_type": "invoice",
            "entity_id": invoice_id.to_string(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "source_order_id": order_id.to_string(),
            "invoice_number": invoice_number,
            "state": "finalized",
            "subtotal_minor": subtotal_minor,
            "discount_minor": discount_minor,
            "tax_minor": tax_minor,
            "total_minor": total.minor_units(),
            "currency_code": total.currency(),
            "lines": snapshots.iter().map(|snapshot| json!({
                "product_id": snapshot.product_id.to_string(),
                "product_name_snapshot": snapshot.product_name.as_str(),
                "quantity": snapshot.quantity,
                "base_unit_price_minor": snapshot.base_unit_price_minor,
                "modifier_total_minor": snapshot.modifier_total_minor,
                "modifier_snapshot": modifier_snapshot_value(snapshot),
                "unit_price_minor": snapshot.unit_price_minor,
                "line_total_minor": snapshot.line_total_minor,
            })).collect::<Vec<_>>(),
        })
        .to_string();

        let order_audit_id = append_hashed_audit_event(
            &transaction,
            context,
            "sales.order.finalized",
            &order_payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &order_audit_id,
            "order",
            &order_id,
            "sales.order.finalized",
            &occurred_at,
        )?;
        let invoice_audit_id = append_hashed_audit_event(
            &transaction,
            context,
            "sales.invoice.finalized",
            &invoice_payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &invoice_audit_id,
            "invoice",
            &invoice_id,
            "sales.invoice.finalized",
            &occurred_at,
        )?;
        for (payment_id, sequence, allocation) in &payment_records {
            let payment_payload = json!({
                "schema_version": 1,
                "entity_type": "payment",
                "entity_id": payment_id.to_string(),
                "correlation_id": context.correlation_id().to_string(),
                "actor_role": context.actor_role().as_str(),
                "invoice_id": invoice_id.to_string(),
                "payment_sequence": sequence,
                "payment_method": allocation.payment_method().as_str(),
                "amount_minor": allocation.amount_minor(),
                "currency_code": total.currency(),
                "state": "recorded",
            })
            .to_string();
            let payment_audit_id = append_hashed_audit_event(
                &transaction,
                context,
                "sales.payment.recorded",
                &payment_payload,
            )?;
            append_sync_outbox_event(
                &transaction,
                context,
                &payment_audit_id,
                "payment",
                payment_id,
                "sales.payment.recorded",
                &occurred_at,
            )?;
        }
        for (movement_id, product_id, quantity_delta) in sale_inventory_movements {
            let payload = json!({
                "schema_version": 1,
                "entity_type": "inventory_movement",
                "entity_id": movement_id.to_string(),
                "product_id": product_id,
                "movement_type": "sale",
                "quantity_delta": quantity_delta,
                "source_invoice_id": invoice_id.to_string(),
                "correlation_id": context.correlation_id().to_string(),
                "actor_role": context.actor_role().as_str(),
            })
            .to_string();
            let audit_event_id = append_hashed_audit_event(
                &transaction,
                context,
                "inventory.sale.recorded",
                &payload,
            )?;
            append_sync_outbox_event(
                &transaction,
                context,
                &audit_event_id,
                "inventory_movement",
                &movement_id,
                "inventory.sale.recorded",
                &occurred_at,
            )?;
        }

        if let Some((draft_order_id, settled_revision)) = draft {
            transaction.execute(
                "INSERT INTO draft_order_settlements (draft_order_id, order_id, settled_revision, settled_at_utc, settled_by_actor_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![draft_order_id.to_string(), order_id.to_string(), settled_revision, occurred_at.as_str(), context.actor_id().to_string()],
            ).map_err(StorageError::Sql)?;
            let payload = json!({"schema_version":1,"entity_type":"draft_order","entity_id":draft_order_id.to_string(),"settled_revision":settled_revision,"order_id":order_id.to_string(),"invoice_id":invoice_id.to_string(),"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str()}).to_string();
            let audit_event_id = append_hashed_audit_event(
                &transaction,
                context,
                "operations.draft_order.settled",
                &payload,
            )?;
            append_sync_outbox_event(
                &transaction,
                context,
                &audit_event_id,
                "draft_order",
                draft_order_id,
                "operations.draft_order.settled",
                &occurred_at,
            )?;
        }

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(CompletedSale::new(
            order_id,
            invoice_id,
            invoice_number,
            total,
            completed_payment_method,
        ))
    }

    /// Archives a category without destroying its history. Categories with
    /// active products must be cleaned up explicitly first, avoiding an
    /// accidental disappearance from a live menu.
    pub fn archive_category(
        &self,
        category_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<Category, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let existing = read_category(&transaction, context.branch_id(), category_id)?;

        if existing.archived() || expected_revision < 1 {
            return Err(StorageError::CatalogConflict);
        }

        let has_active_products: bool = transaction
            .query_row(
                "
                SELECT EXISTS(
                    SELECT 1
                    FROM products
                    WHERE branch_id = ?1
                      AND category_id = ?2
                      AND archived_at_utc IS NULL
                )
                ",
                params![context.branch_id().to_string(), category_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;

        if has_active_products {
            return Err(StorageError::CategoryHasActiveProducts);
        }

        let occurred_at = utc_timestamp(&transaction)?;
        let updated = transaction
            .execute(
                "
                UPDATE categories
                SET archived_at_utc = ?1,
                    archived_by_actor_id = ?2,
                    archive_reason = ?3,
                    updated_at_utc = ?1,
                    updated_by_actor_id = ?2,
                    revision = revision + 1
                WHERE category_id = ?4
                  AND branch_id = ?5
                  AND revision = ?6
                  AND archived_at_utc IS NULL
                ",
                params![
                    occurred_at,
                    context.actor_id().to_string(),
                    reason.as_str(),
                    category_id.to_string(),
                    context.branch_id().to_string(),
                    expected_revision,
                ],
            )
            .map_err(StorageError::Sql)?;

        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }

        let revision = existing
            .revision()
            .checked_add(1)
            .ok_or(StorageError::CatalogConflict)?;
        let archived = Category::new(
            category_id.clone(),
            context.branch_id().clone(),
            existing.display_name().clone(),
            existing.sort_order(),
            revision,
            true,
        );
        let payload = json!({
            "schema_version": 1,
            "entity_type": "category",
            "entity_id": category_id.to_string(),
            "revision": revision,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "reason": reason.as_str(),
            "before": { "archived": false },
            "after": { "archived": true },
        })
        .to_string();
        append_hashed_audit_event(&transaction, context, "catalog.category.archived", &payload)?;

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(archived)
    }

    /// Archives a product by setting it unavailable and preserving all of its
    /// identifiers, price history, and audit record.
    pub fn archive_product(
        &self,
        product_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<Product, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let existing = read_product(&transaction, context.branch_id(), product_id)?;

        if existing.archived() || expected_revision < 1 {
            return Err(StorageError::CatalogConflict);
        }

        let occurred_at = utc_timestamp(&transaction)?;
        let updated = transaction
            .execute(
                "
                UPDATE products
                SET is_available = 0,
                    archived_at_utc = ?1,
                    archived_by_actor_id = ?2,
                    archive_reason = ?3,
                    updated_at_utc = ?1,
                    updated_by_actor_id = ?2,
                    revision = revision + 1
                WHERE product_id = ?4
                  AND branch_id = ?5
                  AND revision = ?6
                  AND archived_at_utc IS NULL
                ",
                params![
                    occurred_at,
                    context.actor_id().to_string(),
                    reason.as_str(),
                    product_id.to_string(),
                    context.branch_id().to_string(),
                    expected_revision,
                ],
            )
            .map_err(StorageError::Sql)?;

        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }

        let revision = existing
            .revision()
            .checked_add(1)
            .ok_or(StorageError::CatalogConflict)?;
        let archived = Product::new(
            product_id.clone(),
            context.branch_id().clone(),
            existing.category_id().cloned(),
            existing.display_name().clone(),
            existing.unit_price().clone(),
            existing.sku().map(ToOwned::to_owned),
            existing.barcode().map(ToOwned::to_owned),
            false,
            existing.sort_order(),
            revision,
            true,
        );
        let payload = json!({
            "schema_version": 1,
            "entity_type": "product",
            "entity_id": product_id.to_string(),
            "revision": revision,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "reason": reason.as_str(),
            "before": {
                "archived": false,
                "is_available": existing.is_available(),
            },
            "after": {
                "archived": true,
                "is_available": false,
            },
        })
        .to_string();
        append_hashed_audit_event(&transaction, context, "catalog.product.archived", &payload)?;

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(archived)
    }

    /// Temporarily pauses or resumes an active product without archiving or
    /// deleting it. Each transition is revisioned and accountable so a counter
    /// cannot silently make a menu item disappear.
    pub fn set_product_availability(
        &self,
        product_id: &EntityId,
        expected_revision: i64,
        is_available: bool,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<Product, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let existing = read_product(&transaction, context.branch_id(), product_id)?;
        if existing.archived()
            || expected_revision < 1
            || existing.revision() != expected_revision
            || existing.is_available() == is_available
        {
            return Err(StorageError::CatalogConflict);
        }
        let occurred_at = utc_timestamp(&transaction)?;
        let updated = transaction.execute(
            "UPDATE products SET is_available = ?1, updated_at_utc = ?2, updated_by_actor_id = ?3, revision = revision + 1 WHERE product_id = ?4 AND branch_id = ?5 AND revision = ?6 AND archived_at_utc IS NULL",
            params![if is_available { 1 } else { 0 }, occurred_at, context.actor_id().to_string(), product_id.to_string(), context.branch_id().to_string(), expected_revision],
        ).map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }
        let revision = existing
            .revision()
            .checked_add(1)
            .ok_or(StorageError::CatalogConflict)?;
        let product = Product::new(
            product_id.clone(),
            context.branch_id().clone(),
            existing.category_id().cloned(),
            existing.display_name().clone(),
            existing.unit_price().clone(),
            existing.sku().map(ToOwned::to_owned),
            existing.barcode().map(ToOwned::to_owned),
            is_available,
            existing.sort_order(),
            revision,
            false,
        );
        let event_type = if is_available {
            "catalog.product.resumed"
        } else {
            "catalog.product.paused"
        };
        let payload = json!({"schema_version":1,"entity_type":"product","entity_id":product_id.to_string(),"revision":revision,"correlation_id":context.correlation_id().to_string(),"actor_role":context.actor_role().as_str(),"reason":reason.as_str(),"before":{"is_available":existing.is_available()},"after":{"is_available":is_available}}).to_string();
        let audit = append_hashed_audit_event(&transaction, context, event_type, &payload)?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit,
            "product",
            product_id,
            event_type,
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(product)
    }

    /// Changes the current selling price through a revisioned, auditable
    /// catalogue command. Existing order lines already snapshot their price,
    /// so this affects only future counter sales.
    pub fn update_product_price(
        &self,
        product_id: &EntityId,
        expected_revision: i64,
        new_price: &Money,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<Product, StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch_currency = ensure_active_branch(&transaction, context.branch_id())?;
        if new_price.currency() != branch_currency {
            return Err(StorageError::CurrencyMismatch {
                expected: branch_currency,
                actual: new_price.currency().to_owned(),
            });
        }

        let existing = read_product(&transaction, context.branch_id(), product_id)?;
        if existing.archived() || expected_revision < 1 {
            return Err(StorageError::CatalogConflict);
        }
        if existing.revision() != expected_revision
            || existing.unit_price().minor_units() == new_price.minor_units()
        {
            return Err(StorageError::CatalogConflict);
        }

        let occurred_at = utc_timestamp(&transaction)?;
        let updated = transaction
            .execute(
                "
                UPDATE products
                SET unit_price_minor = ?1,
                    updated_at_utc = ?2,
                    updated_by_actor_id = ?3,
                    revision = revision + 1
                WHERE product_id = ?4
                  AND branch_id = ?5
                  AND revision = ?6
                  AND archived_at_utc IS NULL
                ",
                params![
                    new_price.minor_units(),
                    occurred_at,
                    context.actor_id().to_string(),
                    product_id.to_string(),
                    context.branch_id().to_string(),
                    expected_revision,
                ],
            )
            .map_err(StorageError::Sql)?;
        if updated != 1 {
            return Err(StorageError::CatalogConflict);
        }

        let revision = existing
            .revision()
            .checked_add(1)
            .ok_or(StorageError::CatalogConflict)?;
        let product = Product::new(
            product_id.clone(),
            context.branch_id().clone(),
            existing.category_id().cloned(),
            existing.display_name().clone(),
            new_price.clone(),
            existing.sku().map(ToOwned::to_owned),
            existing.barcode().map(ToOwned::to_owned),
            existing.is_available(),
            existing.sort_order(),
            revision,
            false,
        );
        let payload = json!({
            "schema_version": 1,
            "entity_type": "product",
            "entity_id": product_id.to_string(),
            "revision": revision,
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "reason": reason.as_str(),
            "before": {
                "unit_price_minor": existing.unit_price().minor_units(),
                "currency_code": existing.unit_price().currency(),
            },
            "after": {
                "unit_price_minor": new_price.minor_units(),
                "currency_code": new_price.currency(),
            },
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "catalog.product.price.updated",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "product",
            product_id,
            "catalog.product.price.updated",
            &occurred_at,
        )?;

        transaction.commit().map_err(StorageError::Sql)?;
        Ok(product)
    }

    /// Permanently removes a catalogue item only when it has no retained
    /// financial, media, or synchronization history. The deletion itself is
    /// still recorded in the immutable audit chain.
    pub fn delete_unused_product(
        &self,
        product_id: &EntityId,
        expected_revision: i64,
        reason: &MutationReason,
        context: &MutationContext,
    ) -> Result<(), StorageError> {
        require_management_authority(context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let existing = read_product(&transaction, context.branch_id(), product_id)?;
        if expected_revision < 1 || existing.revision() != expected_revision {
            return Err(StorageError::CatalogConflict);
        }
        if has_retained_product_history(&transaction, context.branch_id(), product_id)? {
            return Err(StorageError::ProductHasRetainedHistory);
        }

        let payload = json!({
            "schema_version": 1,
            "entity_type": "product",
            "entity_id": product_id.to_string(),
            "revision": existing.revision(),
            "correlation_id": context.correlation_id().to_string(),
            "actor_role": context.actor_role().as_str(),
            "reason": reason.as_str(),
            "before": {
                "display_name": existing.display_name().display(),
                "unit_price_minor": existing.unit_price().minor_units(),
                "currency_code": existing.unit_price().currency(),
                "archived": existing.archived(),
            },
            "after": { "deleted": true },
        })
        .to_string();
        let audit_event_id =
            append_hashed_audit_event(&transaction, context, "catalog.product.deleted", &payload)?;
        let deleted_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO product_deletion_authorizations (
                    product_id,
                    branch_id,
                    audit_event_id,
                    deleted_at_utc
                ) VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    product_id.to_string(),
                    context.branch_id().to_string(),
                    audit_event_id.to_string(),
                    deleted_at,
                ],
            )
            .map_err(StorageError::Sql)?;

        let deleted = transaction
            .execute(
                "
                DELETE FROM products
                WHERE product_id = ?1
                  AND branch_id = ?2
                  AND revision = ?3
                ",
                params![
                    product_id.to_string(),
                    context.branch_id().to_string(),
                    expected_revision,
                ],
            )
            .map_err(StorageError::Sql)?;
        if deleted != 1 {
            return Err(StorageError::CatalogConflict);
        }

        transaction.commit().map_err(StorageError::Sql)
    }

    pub fn list_active_categories(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<Category>, StorageError> {
        let category_ids = select_identifiers(
            &self.connection,
            "
            SELECT category_id
            FROM categories
            WHERE branch_id = ?1 AND archived_at_utc IS NULL
            ORDER BY sort_order, name_key, category_id
            ",
            branch_id,
        )?;

        category_ids
            .iter()
            .map(|category_id| read_category(&self.connection, branch_id, category_id))
            .collect()
    }

    /// Loads only unsettled, open drafts and their latest immutable line
    /// snapshot. The POS can restore these lines, but final pricing is still
    /// revalidated by the settlement transaction.
    pub fn list_open_draft_orders(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<OpenDraftOrder>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT d.draft_order_id, d.fulfillment, d.draft_state, t.display_name, d.current_revision, r.subtotal_minor, d.currency_code, r.line_count, r.line_snapshot_json, r.kitchen_note FROM draft_orders d JOIN draft_order_revisions r ON r.draft_order_id = d.draft_order_id AND r.revision = d.current_revision LEFT JOIN restaurant_tables t ON t.table_id = d.table_id WHERE d.branch_id = ?1 AND d.draft_state IN ('open', 'sent_to_kitchen') AND NOT EXISTS (SELECT 1 FROM draft_order_settlements s WHERE s.draft_order_id = d.draft_order_id) ORDER BY d.updated_at_utc DESC"
        ).map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })
            .map_err(StorageError::Sql)?;
        rows.map(|row| {
            let (
                id,
                fulfillment,
                state,
                table_name,
                revision,
                subtotal_minor,
                currency,
                line_count,
                snapshot,
                kitchen_note,
            ) = row.map_err(StorageError::Sql)?;
            let fulfillment = OrderFulfillment::parse(&fulfillment)
                .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
            let lines = parse_draft_snapshot_lines(&snapshot)?;
            Ok(OpenDraftOrder {
                draft: DraftOrder {
                    draft_order_id: EntityId::parse(&id)
                        .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?,
                    fulfillment,
                    state,
                    table_name,
                    kitchen_note,
                    revision,
                    subtotal: Money::new(subtotal_minor, &currency)
                        .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?,
                    line_count,
                },
                lines,
            })
        })
        .collect()
    }

    pub fn list_sale_products(&self, branch_id: &EntityId) -> Result<Vec<Product>, StorageError> {
        let product_ids = select_identifiers(
            &self.connection,
            "
            SELECT product_id
            FROM products
            WHERE branch_id = ?1
              AND archived_at_utc IS NULL
              AND is_available = 1
            ORDER BY sort_order, name_key, product_id
            ",
            branch_id,
        )?;

        product_ids
            .iter()
            .map(|product_id| read_product(&self.connection, branch_id, product_id))
            .collect()
    }

    /// Loads every non-archived catalogue item, including items that are
    /// temporarily unavailable for sale. Management uses this projection to
    /// resume an item after a sell-out without ever exposing it to checkout.
    pub fn list_catalog_products(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<Product>, StorageError> {
        let product_ids = select_identifiers(
            &self.connection,
            "
            SELECT product_id
            FROM products
            WHERE branch_id = ?1
              AND archived_at_utc IS NULL
            ORDER BY sort_order, name_key, product_id
            ",
            branch_id,
        )?;

        product_ids
            .iter()
            .map(|product_id| read_product(&self.connection, branch_id, product_id))
            .collect()
    }

    /// Returns every retained modifier option for the active catalogue,
    /// including archived options. Keeping archived values in this projection
    /// lets a restored open order and historical receipt render the exact
    /// choice that was made, while storage separately refuses archived options
    /// in every new/revised sale.
    pub fn list_catalog_product_modifier_options(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<ModifierOption>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT
                    modifier_option_id,
                    branch_id,
                    product_id,
                    display_name,
                    name_key,
                    price_delta_minor,
                    currency_code,
                    revision,
                    archived_at_utc
                FROM product_modifier_options
                WHERE branch_id = ?1
                ORDER BY product_id, archived_at_utc IS NOT NULL, name_key, modifier_option_id
                ",
            )
            .map_err(StorageError::Sql)?;
        statement
            .query_map([branch_id.to_string()], |row| {
                Ok(PersistedModifierOption {
                    modifier_option_id: row.get(0)?,
                    branch_id: row.get(1)?,
                    product_id: row.get(2)?,
                    display_name: row.get(3)?,
                    name_key: row.get(4)?,
                    price_delta_minor: row.get(5)?,
                    currency_code: row.get(6)?,
                    revision: row.get(7)?,
                    archived_at_utc: row.get(8)?,
                })
            })
            .map_err(StorageError::Sql)?
            .map(|record| {
                record
                    .map_err(StorageError::Sql)
                    .and_then(modifier_option_from_persisted)
            })
            .collect()
    }

    /// Replaces the active menu image by appending a new immutable version and
    /// advancing its assignment revision. This is intentionally separate from
    /// financial product facts: historic sales already snapshot their menu
    /// name and price, while image history remains available for catalogue
    /// review and future synchronization.
    pub fn replace_product_image(
        &self,
        product_id: &EntityId,
        image: &ProductImageContent,
        context: &MutationContext,
    ) -> Result<ProductImage, StorageError> {
        require_management_authority(context)?;
        validate_product_image_content(image)?;

        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let product = read_product(&transaction, context.branch_id(), product_id)?;
        if product.archived() || !product.is_available() {
            return Err(StorageError::ProductUnavailable);
        }

        let assigned = assign_product_image(&transaction, product_id, image, context)?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(assigned)
    }

    /// Loads only images assigned to active sale products. Each restaurant
    /// upload is capped at 64 KiB after Rust-side normalization, making this
    /// compact enough for the Community Edition's offline catalogue view.
    pub fn list_sale_product_images(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<ProductImage>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT
                    versions.product_id,
                    versions.source_kind,
                    versions.asset_key,
                    versions.content_type,
                    versions.image_bytes,
                    versions.pixel_width,
                    versions.pixel_height,
                    versions.byte_length,
                    versions.content_sha256,
                    provenance.catalog_image_id,
                    provenance.original_content_sha256,
                    provenance.licence_label,
                    provenance.licence_url,
                    provenance.service_origin,
                    provenance.service_schema_version
                FROM product_image_assignments AS assignments
                JOIN product_image_versions AS versions
                    ON versions.branch_id = assignments.branch_id
                    AND versions.product_id = assignments.product_id
                    AND versions.image_version_id = assignments.image_version_id
                LEFT JOIN product_image_catalog_provenance AS provenance
                    ON provenance.branch_id = versions.branch_id
                    AND provenance.product_id = versions.product_id
                    AND provenance.image_version_id = versions.image_version_id
                JOIN products
                    ON products.branch_id = assignments.branch_id
                    AND products.product_id = assignments.product_id
                WHERE assignments.branch_id = ?1
                  AND products.archived_at_utc IS NULL
                  AND products.is_available = 1
                ORDER BY products.sort_order, products.name_key, products.product_id
                ",
            )
            .map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch_id.to_string()], |row| {
                Ok(PersistedProductImage {
                    product_id: row.get(0)?,
                    source_kind: row.get(1)?,
                    asset_key: row.get(2)?,
                    content_type: row.get(3)?,
                    image_bytes: row.get(4)?,
                    pixel_width: row.get(5)?,
                    pixel_height: row.get(6)?,
                    byte_length: row.get(7)?,
                    content_sha256: row.get(8)?,
                    catalog_image_id: row.get(9)?,
                    catalog_original_content_sha256: row.get(10)?,
                    catalog_licence_label: row.get(11)?,
                    catalog_licence_url: row.get(12)?,
                    catalog_service_origin: row.get(13)?,
                    catalog_service_schema_version: row.get(14)?,
                })
            })
            .map_err(StorageError::Sql)?;

        rows.map(|row| {
            let record = row.map_err(StorageError::Sql)?;
            product_image_from_persisted(record)
        })
        .collect()
    }

    /// Loads current images for every non-archived catalogue item. Unlike
    /// `list_sale_product_images`, this retains imagery in the management
    /// catalogue while an item is temporarily sold out.
    pub fn list_catalog_product_images(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<ProductImage>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT
                    versions.product_id,
                    versions.source_kind,
                    versions.asset_key,
                    versions.content_type,
                    versions.image_bytes,
                    versions.pixel_width,
                    versions.pixel_height,
                    versions.byte_length,
                    versions.content_sha256,
                    provenance.catalog_image_id,
                    provenance.original_content_sha256,
                    provenance.licence_label,
                    provenance.licence_url,
                    provenance.service_origin,
                    provenance.service_schema_version
                FROM product_image_assignments AS assignments
                JOIN product_image_versions AS versions
                    ON versions.branch_id = assignments.branch_id
                    AND versions.product_id = assignments.product_id
                    AND versions.image_version_id = assignments.image_version_id
                LEFT JOIN product_image_catalog_provenance AS provenance
                    ON provenance.branch_id = versions.branch_id
                    AND provenance.product_id = versions.product_id
                    AND provenance.image_version_id = versions.image_version_id
                JOIN products
                    ON products.branch_id = assignments.branch_id
                    AND products.product_id = assignments.product_id
                WHERE assignments.branch_id = ?1
                  AND products.archived_at_utc IS NULL
                ORDER BY products.sort_order, products.name_key, products.product_id
                ",
            )
            .map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch_id.to_string()], |row| {
                Ok(PersistedProductImage {
                    product_id: row.get(0)?,
                    source_kind: row.get(1)?,
                    asset_key: row.get(2)?,
                    content_type: row.get(3)?,
                    image_bytes: row.get(4)?,
                    pixel_width: row.get(5)?,
                    pixel_height: row.get(6)?,
                    byte_length: row.get(7)?,
                    content_sha256: row.get(8)?,
                    catalog_image_id: row.get(9)?,
                    catalog_original_content_sha256: row.get(10)?,
                    catalog_licence_label: row.get(11)?,
                    catalog_licence_url: row.get(12)?,
                    catalog_service_origin: row.get(13)?,
                    catalog_service_schema_version: row.get(14)?,
                })
            })
            .map_err(StorageError::Sql)?;

        rows.map(|row| {
            let record = row.map_err(StorageError::Sql)?;
            product_image_from_persisted(record)
        })
        .collect()
    }

    /// Verifies the locally generated portion of the audit hash chain for a
    /// device. Professional sync will additionally anchor this chain remotely.
    pub fn verify_device_audit_chain(&self, device_id: &EntityId) -> Result<(), StorageError> {
        verify_device_audit_chain_on_connection(&self.connection, device_id)
    }

    /// Verifies the local invariants that are safe to check while the app is
    /// running: SQLCipher page HMACs, schema contract, foreign keys, and every
    /// device's append-only audit chain. It intentionally performs no repair;
    /// any failure remains fail-closed for a recovery workflow.
    pub fn verify_local_integrity(&self) -> Result<LocalIntegrityReport, StorageError> {
        verify_local_integrity_on_connection(&self.connection)
    }

    /// Reads unacknowledged immutable operations in their audit-chain order.
    /// This is intentionally a read-only boundary: retry state belongs to the
    /// future sync client, while acknowledgement is modeled as a new record.
    pub fn list_pending_sync_operations(
        &self,
        branch_id: &EntityId,
    ) -> Result<Vec<PendingSyncOperation>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT o.operation_id, o.audit_event_id, o.branch_id, a.actor_id, o.device_id, \
                    a.sequence, a.event_type, a.payload_json, a.occurred_at_utc, \
                    a.previous_hash, a.event_hash, o.entity_type, o.entity_id, \
                    o.correlation_id, o.created_at_utc \
             FROM sync_outbox_events o \
             JOIN audit_events a ON a.event_id = o.audit_event_id \
             LEFT JOIN sync_acknowledgements k ON k.operation_id = o.operation_id \
             WHERE o.branch_id = ?1 AND k.operation_id IS NULL \
             ORDER BY a.device_id, a.sequence, o.operation_id",
            )
            .map_err(StorageError::Sql)?;
        statement
            .query_map([branch_id.to_string()], |row| {
                let parse = |index| {
                    EntityId::parse(&row.get::<_, String>(index)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)
                };
                Ok(PendingSyncOperation {
                    operation_id: parse(0)?,
                    audit_event_id: parse(1)?,
                    branch_id: parse(2)?,
                    actor_id: parse(3)?,
                    device_id: parse(4)?,
                    sequence: row.get(5)?,
                    event_type: row.get(6)?,
                    payload_json: row.get(7)?,
                    occurred_at_utc: row.get(8)?,
                    previous_hash: row.get(9)?,
                    event_hash: row.get(10)?,
                    entity_type: row.get(11)?,
                    entity_id: row.get(12)?,
                    correlation_id: row.get(13)?,
                    created_at_utc: row.get(14)?,
                })
            })
            .map_err(StorageError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Sql)
    }

    /// Records a cloud acknowledgement exactly once. Repeating the identical
    /// acknowledgement is safe; a conflicting server event id is rejected so
    /// a faulty or malicious sync client cannot rewrite delivery history.
    pub fn acknowledge_sync_operation(
        &self,
        operation_id: &EntityId,
        server_event_id: &str,
    ) -> Result<(), StorageError> {
        if server_event_id.trim().is_empty() || server_event_id.len() > 256 {
            return Err(StorageError::InvalidPersistedData(
                "server event identity was invalid".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT server_event_id FROM sync_acknowledgements WHERE operation_id = ?1",
                [operation_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sql)?;
        if let Some(existing) = existing {
            if existing == server_event_id {
                transaction.commit().map_err(StorageError::Sql)?;
                return Ok(());
            }
            return Err(StorageError::CatalogConflict);
        }
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sync_outbox_events WHERE operation_id = ?1)",
                [operation_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if !exists {
            return Err(StorageError::CatalogConflict);
        }
        let acknowledged_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "INSERT INTO sync_acknowledgements (operation_id, server_event_id, acknowledged_at_utc) VALUES (?1, ?2, ?3)",
                params![operation_id.to_string(), server_event_id, acknowledged_at],
            )
            .map_err(StorageError::Sql)?;
        transaction.commit().map_err(StorageError::Sql)
    }

    #[cfg(test)]
    fn append_audit_event(&self, event: NewAuditEvent<'_>) -> Result<(), StorageError> {
        if event.event_id.trim().is_empty()
            || event.branch_id.trim().is_empty()
            || event.actor_id.trim().is_empty()
            || event.device_id.trim().is_empty()
            || event.event_type.trim().is_empty()
            || event.occurred_at_utc.trim().is_empty()
        {
            return Err(StorageError::InvalidAuditEvent(
                "Audit identity fields must not be empty.",
            ));
        }

        if event.sequence < 1 {
            return Err(StorageError::InvalidAuditEvent(
                "Audit event sequence must start at one.",
            ));
        }

        self.connection
            .execute(
                "
                INSERT INTO audit_events (
                    event_id,
                    branch_id,
                    actor_id,
                    device_id,
                    sequence,
                    event_type,
                    payload_json,
                    occurred_at_utc,
                    previous_hash,
                    event_hash
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ",
                params![
                    event.event_id,
                    event.branch_id,
                    event.actor_id,
                    event.device_id,
                    event.sequence,
                    event.event_type,
                    event.payload_json,
                    event.occurred_at_utc,
                    event.previous_hash,
                    event.event_hash,
                ],
            )
            .map_err(StorageError::Sql)?;

        Ok(())
    }

    pub fn audit_event_count(&self) -> Result<i64, StorageError> {
        audit_event_count_on_connection(&self.connection)
    }

    pub fn integrity_check(&self) -> Result<(), StorageError> {
        integrity_check_on_connection(&self.connection)
    }

    /// Creates a consistent encrypted snapshot through SQLite's online backup
    /// API. A live WAL database is never copied as raw files. Only the active
    /// Owner context may create an encrypted database copy, and the caller
    /// must select a new destination; this method never overwrites one. The
    /// selected path is only published after a same-directory staging file has
    /// been verified, using an atomic no-replacement hard-link operation.
    ///
    /// This preserves the SQLCipher key boundary: the snapshot is encrypted
    /// with the current installation key and is therefore suitable for
    /// same-installation recovery. Portable restore additionally needs the
    /// owner-authorized recovery-envelope policy.
    pub fn create_verified_local_backup(
        &self,
        destination: impl AsRef<Path>,
        context: &MutationContext,
    ) -> Result<VerifiedLocalBackup, StorageError> {
        require_owner_authority(context)?;
        let authorization = begin_immediate_transaction(&self.connection)?;
        revalidate_mutation_context(&authorization, context)?;
        ensure_active_branch(&authorization, context.branch_id())?;
        authorization.commit().map_err(StorageError::Sql)?;

        let destination = destination.as_ref();
        let reserved_destination = ReservedLocalBackupDestination::reserve(destination)?;
        let mut backup_connection = Connection::open_with_flags(
            reserved_destination.path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(StorageError::Sql)?;
        configure_connection(&backup_connection, &self.key)?;
        {
            let backup = rusqlite::backup::Backup::new(&self.connection, &mut backup_connection)
                .map_err(StorageError::Sql)?;
            backup
                .run_to_completion(64, Duration::from_millis(5), None)
                .map_err(StorageError::Sql)?;
        }
        let backup_database = Self {
            connection: backup_connection,
            key: DatabaseKey::from_bytes(*self.key.as_bytes()),
        };
        backup_database.integrity_check()?;
        verify_local_schema_contract(&backup_database.connection)?;
        let schema_version = backup_database.schema_version()?;
        drop(backup_database);
        ensure_backup_sidecars_are_absent(reserved_destination.path())?;
        let (byte_length, sha256) = reserved_destination.sync_and_hash()?;
        reserved_destination.finalize_without_replacement(destination)?;
        Ok(VerifiedLocalBackup {
            sha256,
            schema_version,
            byte_length,
        })
    }

    /// Opens a same-installation backup with the current key and verifies
    /// SQLCipher integrity plus the local schema contract. Wrong keys and
    /// corrupt snapshots fail closed.
    pub fn verify_local_backup(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<VerifiedLocalBackup, StorageError> {
        let source = source.as_ref();
        let backup_connection = Connection::open_with_flags(
            source,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(StorageError::Sql)?;
        configure_connection(&backup_connection, &self.key)?;
        let backup_database = Self {
            connection: backup_connection,
            key: DatabaseKey::from_bytes(*self.key.as_bytes()),
        };
        backup_database.integrity_check()?;
        verify_local_schema_contract(&backup_database.connection)?;
        let schema_version = backup_database.schema_version()?;
        let byte_length = fs::metadata(source).map_err(StorageError::Io)?.len();
        let file = OpenOptions::new()
            .read(true)
            .open(source)
            .map_err(StorageError::Io)?;
        let (_, sha256) = sha256_file(&file)?;
        Ok(VerifiedLocalBackup {
            sha256,
            schema_version,
            byte_length,
        })
    }

    /// Restores a verified same-installation backup into `destination`.
    /// Owner-only. Never mutates the keyring. Refuses to overwrite an existing
    /// destination unless `replace_existing` is true. Portable restore remains
    /// out of scope.
    pub fn restore_verified_local_backup(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        context: &MutationContext,
        replace_existing: bool,
    ) -> Result<VerifiedLocalBackup, StorageError> {
        require_owner_authority(context)?;
        let authorization = begin_immediate_transaction(&self.connection)?;
        revalidate_mutation_context(&authorization, context)?;
        ensure_active_branch(&authorization, context.branch_id())?;
        authorization.commit().map_err(StorageError::Sql)?;

        let verified = self.verify_local_backup(&source)?;
        let destination = destination.as_ref();
        if destination.exists() {
            if !replace_existing {
                return Err(StorageError::InvalidPersistedData(
                    "restore destination already exists".to_owned(),
                ));
            }
            fs::remove_file(destination).map_err(StorageError::Io)?;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(StorageError::Io)?;
        }
        fs::copy(source.as_ref(), destination).map_err(StorageError::Io)?;
        let restored = Self::open(destination, &self.key)?;
        restored.integrity_check()?;
        verify_local_schema_contract(&restored.connection)?;
        Ok(verified)
    }

    /// Creates a portable backup plus recovery envelope (ADR 0005).
    pub fn create_portable_backup_with_envelope(
        &self,
        backup_destination: impl AsRef<Path>,
        envelope_destination: impl AsRef<Path>,
        recovery_passphrase: &str,
        context: &MutationContext,
    ) -> Result<(VerifiedLocalBackup, PortableRecoveryEnvelope), StorageError> {
        require_owner_authority(context)?;
        let verified = self.create_verified_local_backup(backup_destination.as_ref(), context)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        revalidate_mutation_context(&transaction, context)?;
        let (device_id, _) = ensure_local_installation_identity(&transaction)?;
        let organization_id: String = transaction
            .query_row(
                "SELECT organization_id FROM branches WHERE branch_id = ?1",
                [context.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        let created_at = utc_timestamp(&transaction)?;
        let verifier_hash = recovery::hash_recovery_passphrase(recovery_passphrase)?;
        transaction
            .execute(
                "
                UPDATE owner_recovery_verifiers
                SET superseded_at_utc = ?1
                WHERE organization_id = ?2 AND superseded_at_utc IS NULL
                ",
                params![created_at.as_str(), organization_id],
            )
            .map_err(StorageError::Sql)?;
        transaction
            .execute(
                "
                INSERT INTO owner_recovery_verifiers (
                    owner_recovery_verifier_id, organization_id, argon2id_hash,
                    created_at_utc, created_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    organization_id,
                    verifier_hash,
                    created_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        transaction.commit().map_err(StorageError::Sql)?;
        let envelope = recovery::wrap_database_key(
            self.key.as_bytes(),
            recovery_passphrase,
            &device_id.to_string(),
            verified.schema_version(),
            verified.sha256(),
            &created_at,
            &context.actor_id().to_string(),
        )?;
        fs::write(
            envelope_destination.as_ref(),
            envelope.envelope_json.as_bytes(),
        )
        .map_err(StorageError::Io)?;
        Ok((verified, envelope))
    }

    /// Restores a portable backup into a new destination using the recovery
    /// envelope. Never overwrites an existing destination. Does not mutate the
    /// current installation keyring; the caller stores the unwrapped key.
    pub fn restore_portable_backup_to_clean_path(
        backup_source: impl AsRef<Path>,
        envelope_json: &str,
        recovery_passphrase: &str,
        destination: impl AsRef<Path>,
    ) -> Result<(VerifiedLocalBackup, [u8; 32]), StorageError> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(StorageError::InvalidPersistedData(
                "clean-install restore destination already exists".to_owned(),
            ));
        }
        let byte_length = fs::metadata(backup_source.as_ref())
            .map_err(StorageError::Io)?
            .len();
        let file = OpenOptions::new()
            .read(true)
            .open(backup_source.as_ref())
            .map_err(StorageError::Io)?;
        let (_, backup_sha256) = sha256_file(&file)?;
        let key_bytes =
            recovery::unwrap_database_key(envelope_json, recovery_passphrase, &backup_sha256)?;
        let key = DatabaseKey::from_bytes(key_bytes);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(StorageError::Io)?;
        }
        fs::copy(backup_source.as_ref(), destination).map_err(StorageError::Io)?;
        let restored = Self::open(destination, &key)?;
        restored.integrity_check()?;
        verify_local_schema_contract(&restored.connection)?;
        let schema_version = restored.schema_version()?;
        Ok((
            VerifiedLocalBackup {
                sha256: backup_sha256,
                schema_version,
                byte_length,
            },
            key_bytes,
        ))
    }

    /// Resets the Owner PIN after proving the recovery passphrase (ADR 0007).
    pub fn recover_owner_pin_with_recovery_passphrase(
        &self,
        recovery_passphrase: &str,
        new_owner_pin: &str,
    ) -> Result<(), StorageError> {
        validate_local_staff_pin(new_owner_pin)?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        let branch = read_community_branch(&transaction)?;
        let digest: String = transaction
            .query_row(
                "
                SELECT argon2id_hash
                FROM owner_recovery_verifiers
                WHERE organization_id = ?1 AND superseded_at_utc IS NULL
                ORDER BY created_at_utc DESC
                LIMIT 1
                ",
                [branch.organization_id().to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sql)?
            .ok_or_else(|| {
                StorageError::InvalidPersistedData(
                    "no owner recovery verifier is configured".to_owned(),
                )
            })?;
        if !recovery::verify_recovery_passphrase(recovery_passphrase, &digest) {
            return Err(StorageError::InvalidStaffPin);
        }
        let owner_id: String = transaction
            .query_row(
                "
                SELECT staff_id FROM staff_accounts
                WHERE branch_id = ?1 AND role = 'owner'
                ORDER BY created_at_utc ASC LIMIT 1
                ",
                [branch.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        let pin_hash = hash_local_staff_pin(new_owner_pin)?;
        let occurred_at = utc_timestamp(&transaction)?;
        let (device_id, _) = ensure_local_installation_identity(&transaction)?;
        let context = MutationContext::new(
            branch.branch_id().clone(),
            parse_persisted_id(&owner_id)?,
            device_id,
            EntityId::new_v7(),
            ActorRole::Owner,
        );
        transaction
            .execute(
                "
                INSERT INTO staff_pin_credentials (
                    staff_pin_credential_id, staff_id, argon2id_hash,
                    created_at_utc, created_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    EntityId::new_v7().to_string(),
                    owner_id,
                    pin_hash,
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "staff_pin_credential",
            "entity_id": owner_id,
            "recovery": "credential-recovery.v1",
        })
        .to_string();
        append_hashed_audit_event_without_session(
            &transaction,
            &context,
            "staff.owner_pin.recovered",
            &payload,
        )?;
        transaction.commit().map_err(StorageError::Sql)
    }

    pub fn create_supplier(
        &self,
        display_name: &str,
        context: &MutationContext,
    ) -> Result<Supplier, StorageError> {
        require_management_authority(context)?;
        let display_name = DisplayName::category(display_name)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        let supplier_id = EntityId::new_v7();
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO suppliers (
                    supplier_id, branch_id, display_name, name_key, revision,
                    created_at_utc, created_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)
                ",
                params![
                    supplier_id.to_string(),
                    context.branch_id().to_string(),
                    display_name.display(),
                    display_name.key(),
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        let payload = json!({
            "schema_version": 1,
            "entity_type": "supplier",
            "entity_id": supplier_id.to_string(),
            "display_name": display_name.display(),
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "inventory.supplier.created",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "supplier",
            &supplier_id,
            "inventory.supplier.created",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(Supplier {
            supplier_id,
            display_name: display_name.display().to_owned(),
            revision: 1,
        })
    }

    pub fn receive_purchase_document(
        &self,
        supplier_id: &EntityId,
        supplier_reference: Option<&str>,
        reason: &MutationReason,
        lines: &[PurchaseLineInput],
        context: &MutationContext,
    ) -> Result<EntityId, StorageError> {
        require_management_authority(context)?;
        if lines.is_empty() {
            return Err(StorageError::InvalidPersistedData(
                "purchase document requires at least one line".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        let currency = ensure_active_branch(&transaction, context.branch_id())?;
        let supplier_exists: bool = transaction
            .query_row(
                "
                SELECT EXISTS(
                    SELECT 1 FROM suppliers
                    WHERE supplier_id = ?1 AND branch_id = ?2 AND archived_at_utc IS NULL
                )
                ",
                params![supplier_id.to_string(), context.branch_id().to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        if !supplier_exists {
            return Err(StorageError::CatalogConflict);
        }
        let document_id = EntityId::new_v7();
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                INSERT INTO purchase_documents (
                    purchase_document_id, branch_id, supplier_id, supplier_reference,
                    currency_code, reason, received_at_utc, received_by_actor_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    document_id.to_string(),
                    context.branch_id().to_string(),
                    supplier_id.to_string(),
                    supplier_reference,
                    currency,
                    reason.as_str(),
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        for (index, line) in lines.iter().enumerate() {
            if line.quantity <= 0 || line.unit_cost_minor < 0 {
                return Err(StorageError::InvalidPersistedData(
                    "purchase line quantity/cost invalid".to_owned(),
                ));
            }
            read_product(&transaction, context.branch_id(), &line.product_id)?;
            let line_id = EntityId::new_v7();
            transaction
                .execute(
                    "
                    INSERT INTO purchase_document_lines (
                        purchase_document_line_id, purchase_document_id, branch_id,
                        product_id, line_number, quantity, unit_cost_minor
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    ",
                    params![
                        line_id.to_string(),
                        document_id.to_string(),
                        context.branch_id().to_string(),
                        line.product_id.to_string(),
                        (index as i64) + 1,
                        line.quantity,
                        line.unit_cost_minor,
                    ],
                )
                .map_err(StorageError::Sql)?;
            let movement_id = EntityId::new_v7();
            transaction
                .execute(
                    "
                    INSERT INTO inventory_movements (
                        inventory_movement_id, branch_id, product_id, movement_type,
                        quantity_delta, source_document_id, occurred_at_utc,
                        occurred_by_actor_id
                    ) VALUES (?1, ?2, ?3, 'purchase', ?4, ?5, ?6, ?7)
                    ",
                    params![
                        movement_id.to_string(),
                        context.branch_id().to_string(),
                        line.product_id.to_string(),
                        line.quantity,
                        document_id.to_string(),
                        occurred_at.as_str(),
                        context.actor_id().to_string(),
                    ],
                )
                .map_err(StorageError::Sql)?;
        }
        let payload = json!({
            "schema_version": 1,
            "entity_type": "purchase_document",
            "entity_id": document_id.to_string(),
            "supplier_id": supplier_id.to_string(),
            "line_count": lines.len(),
            "reason": reason.as_str(),
        })
        .to_string();
        let audit_event_id = append_hashed_audit_event(
            &transaction,
            context,
            "inventory.purchase_document.received",
            &payload,
        )?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "purchase_document",
            &document_id,
            "inventory.purchase_document.received",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(document_id)
    }

    pub fn set_product_recipe(
        &self,
        finished_product_id: &EntityId,
        lines: &[RecipeLineInput],
        context: &MutationContext,
    ) -> Result<EntityId, StorageError> {
        require_management_authority(context)?;
        if lines.is_empty() {
            return Err(StorageError::InvalidPersistedData(
                "recipe requires at least one ingredient line".to_owned(),
            ));
        }
        let transaction = begin_immediate_transaction(&self.connection)?;
        ensure_active_branch(&transaction, context.branch_id())?;
        read_product(&transaction, context.branch_id(), finished_product_id)?;
        let occurred_at = utc_timestamp(&transaction)?;
        transaction
            .execute(
                "
                UPDATE product_recipes
                SET archived_at_utc = ?1
                WHERE branch_id = ?2
                  AND finished_product_id = ?3
                  AND archived_at_utc IS NULL
                ",
                params![
                    occurred_at.as_str(),
                    context.branch_id().to_string(),
                    finished_product_id.to_string()
                ],
            )
            .map_err(StorageError::Sql)?;
        let recipe_id = EntityId::new_v7();
        transaction
            .execute(
                "
                INSERT INTO product_recipes (
                    product_recipe_id, branch_id, finished_product_id, revision,
                    created_at_utc, created_by_actor_id
                ) VALUES (?1, ?2, ?3, 1, ?4, ?5)
                ",
                params![
                    recipe_id.to_string(),
                    context.branch_id().to_string(),
                    finished_product_id.to_string(),
                    occurred_at.as_str(),
                    context.actor_id().to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
        for (index, line) in lines.iter().enumerate() {
            if line.quantity_per_unit <= 0 {
                return Err(StorageError::InvalidPersistedData(
                    "recipe quantity must be positive".to_owned(),
                ));
            }
            if line.ingredient_product_id == *finished_product_id {
                return Err(StorageError::InvalidPersistedData(
                    "a product cannot be an ingredient of itself".to_owned(),
                ));
            }
            read_product(
                &transaction,
                context.branch_id(),
                &line.ingredient_product_id,
            )?;
            transaction
                .execute(
                    "
                    INSERT INTO product_recipe_lines (
                        product_recipe_line_id, product_recipe_id, branch_id,
                        ingredient_product_id, quantity_per_unit, line_number
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        recipe_id.to_string(),
                        context.branch_id().to_string(),
                        line.ingredient_product_id.to_string(),
                        line.quantity_per_unit,
                        (index as i64) + 1,
                    ],
                )
                .map_err(StorageError::Sql)?;
        }
        let payload = json!({
            "schema_version": 1,
            "entity_type": "product_recipe",
            "entity_id": recipe_id.to_string(),
            "finished_product_id": finished_product_id.to_string(),
            "line_count": lines.len(),
        })
        .to_string();
        let audit_event_id =
            append_hashed_audit_event(&transaction, context, "inventory.recipe.set", &payload)?;
        append_sync_outbox_event(
            &transaction,
            context,
            &audit_event_id,
            "product_recipe",
            &recipe_id,
            "inventory.recipe.set",
            &occurred_at,
        )?;
        transaction.commit().map_err(StorageError::Sql)?;
        Ok(recipe_id)
    }
}

#[cfg(test)]
struct NewAuditEvent<'event> {
    pub event_id: &'event str,
    pub branch_id: &'event str,
    pub actor_id: &'event str,
    pub device_id: &'event str,
    pub sequence: i64,
    pub event_type: &'event str,
    pub payload_json: &'event str,
    pub occurred_at_utc: &'event str,
    pub previous_hash: Option<&'event [u8]>,
    pub event_hash: &'event [u8],
}

struct PersistedCategory {
    category_id: String,
    branch_id: String,
    display_name: String,
    name_key: String,
    sort_order: i64,
    revision: i64,
    archived_at_utc: Option<String>,
}

struct PersistedProduct {
    product_id: String,
    branch_id: String,
    category_id: Option<String>,
    display_name: String,
    name_key: String,
    sku: Option<String>,
    barcode: Option<String>,
    unit_price_minor: i64,
    currency_code: String,
    is_available: i64,
    sort_order: i64,
    revision: i64,
    archived_at_utc: Option<String>,
}

struct PersistedModifierOption {
    modifier_option_id: String,
    branch_id: String,
    product_id: String,
    display_name: String,
    name_key: String,
    price_delta_minor: i64,
    currency_code: String,
    revision: i64,
    archived_at_utc: Option<String>,
}

struct PersistedProductImageAssignment {
    image_version_id: String,
    revision: i64,
}

struct PersistedProductImage {
    product_id: String,
    source_kind: String,
    asset_key: Option<String>,
    content_type: Option<String>,
    image_bytes: Option<Vec<u8>>,
    pixel_width: Option<i64>,
    pixel_height: Option<i64>,
    byte_length: i64,
    content_sha256: Option<Vec<u8>>,
    catalog_image_id: Option<String>,
    catalog_original_content_sha256: Option<Vec<u8>>,
    catalog_licence_label: Option<String>,
    catalog_licence_url: Option<String>,
    catalog_service_origin: Option<String>,
    catalog_service_schema_version: Option<i64>,
}

struct PersistedAuditEvent {
    event_id: String,
    branch_id: String,
    actor_id: String,
    sequence: i64,
    event_type: String,
    payload_json: String,
    occurred_at_utc: String,
    previous_hash: Option<Vec<u8>>,
    event_hash: Vec<u8>,
}

struct PersistedCommunityBranch {
    organization_id: String,
    branch_id: String,
    display_name: String,
    currency_code: String,
    time_zone: String,
}

fn validate_product_image_content(image: &ProductImageContent) -> Result<(), StorageError> {
    match image {
        ProductImageContent::BuiltIn { asset_key } => {
            if !is_supported_builtin_menu_image_key(asset_key) {
                return Err(StorageError::InvalidProductImage(
                    "choose a supported built-in menu image",
                ));
            }
        }
        ProductImageContent::RestaurantUpload {
            jpeg_bytes,
            pixel_width,
            pixel_height,
        } => validate_normalized_product_image(jpeg_bytes, *pixel_width, *pixel_height)?,
        ProductImageContent::GotiginCatalog {
            jpeg_bytes,
            pixel_width,
            pixel_height,
            provenance,
        } => {
            validate_normalized_product_image(jpeg_bytes, *pixel_width, *pixel_height)?;
            validate_product_image_catalog_provenance(provenance)?;
        }
    }

    Ok(())
}

fn validate_normalized_product_image(
    jpeg_bytes: &[u8],
    pixel_width: i64,
    pixel_height: i64,
) -> Result<(), StorageError> {
    if jpeg_bytes.is_empty() || jpeg_bytes.len() > MAX_MENU_IMAGE_BYTES {
        return Err(StorageError::InvalidProductImage(
            "the normalized menu image must be between 1 byte and 64 KiB",
        ));
    }
    if !(1..=MAX_MENU_IMAGE_WIDTH).contains(&pixel_width)
        || !(1..=MAX_MENU_IMAGE_HEIGHT).contains(&pixel_height)
    {
        return Err(StorageError::InvalidProductImage(
            "the normalized menu image dimensions are outside the supported tile size",
        ));
    }
    if !looks_like_jpeg(jpeg_bytes) {
        return Err(StorageError::InvalidProductImage(
            "menu images must be normalized JPEG images",
        ));
    }
    Ok(())
}

fn validate_product_image_catalog_provenance(
    provenance: &ProductImageCatalogProvenance,
) -> Result<(), StorageError> {
    let image_id = provenance.catalog_image_id();
    if image_id.is_empty()
        || image_id.len() > MAX_CATALOG_IMAGE_ID_BYTES
        || image_id.trim() != image_id
        || !image_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(StorageError::InvalidProductImage(
            "the catalogue image identifier is invalid",
        ));
    }
    if provenance.original_content_sha256().len() != 32 {
        return Err(StorageError::InvalidProductImage(
            "the catalogue source digest is invalid",
        ));
    }
    let licence_label = provenance.licence_label();
    if licence_label.is_empty()
        || licence_label.len() > MAX_CATALOG_LICENCE_LABEL_BYTES
        || licence_label.trim() != licence_label
        || licence_label.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidProductImage(
            "the catalogue licence label is invalid",
        ));
    }
    let licence_url = provenance.licence_url();
    let parsed_licence_url = Url::parse(licence_url)
        .map_err(|_| StorageError::InvalidProductImage("the catalogue licence URL is invalid"))?;
    if licence_url.len() > MAX_CATALOG_LICENCE_URL_BYTES
        || licence_url.trim() != licence_url
        || parsed_licence_url.scheme() != "https"
        || parsed_licence_url.host_str().is_none()
        || !parsed_licence_url.username().is_empty()
        || parsed_licence_url.password().is_some()
        || parsed_licence_url.fragment().is_some()
    {
        return Err(StorageError::InvalidProductImage(
            "the catalogue licence URL is invalid",
        ));
    }
    if provenance.service_origin() != GOTIGIN_CATALOG_SERVICE_ORIGIN
        || provenance.service_schema_version() != GOTIGIN_CATALOG_SERVICE_SCHEMA_VERSION
    {
        return Err(StorageError::InvalidProductImage(
            "the catalogue service contract is unsupported",
        ));
    }
    Ok(())
}

fn is_supported_builtin_menu_image_key(asset_key: &str) -> bool {
    BUILT_IN_MENU_IMAGE_KEYS.contains(&asset_key)
}

fn looks_like_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes.starts_with(&[0xFF, 0xD8]) && bytes.ends_with(&[0xFF, 0xD9])
}

fn assign_product_image(
    transaction: &Transaction<'_>,
    product_id: &EntityId,
    image: &ProductImageContent,
    context: &MutationContext,
) -> Result<ProductImage, StorageError> {
    validate_product_image_content(image)?;

    let existing: Option<PersistedProductImageAssignment> = transaction
        .query_row(
            "
            SELECT image_version_id, revision
            FROM product_image_assignments
            WHERE branch_id = ?1 AND product_id = ?2
            ",
            params![context.branch_id().to_string(), product_id.to_string()],
            |row| {
                Ok(PersistedProductImageAssignment {
                    image_version_id: row.get(0)?,
                    revision: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::Sql)?;

    let (
        source_kind,
        persisted_source_kind,
        asset_key,
        content_type,
        image_bytes,
        pixel_width,
        pixel_height,
        byte_length,
        content_sha256,
        catalog_provenance,
    ) = match image {
        ProductImageContent::BuiltIn { asset_key } => (
            ProductImageSource::BuiltIn,
            "built_in",
            Some(asset_key.clone()),
            None,
            None,
            None,
            None,
            0_i64,
            None,
            None,
        ),
        ProductImageContent::RestaurantUpload {
            jpeg_bytes,
            pixel_width,
            pixel_height,
        } => (
            ProductImageSource::RestaurantUpload,
            "restaurant_upload",
            None,
            Some("image/jpeg"),
            Some(jpeg_bytes.clone()),
            Some(*pixel_width),
            Some(*pixel_height),
            i64::try_from(jpeg_bytes.len()).map_err(|_| {
                StorageError::InvalidProductImage("the menu image is too large to store")
            })?,
            Some(Sha256::digest(jpeg_bytes).to_vec()),
            None,
        ),
        ProductImageContent::GotiginCatalog {
            jpeg_bytes,
            pixel_width,
            pixel_height,
            provenance,
        } => (
            ProductImageSource::GotiginCatalog,
            "restaurant_upload",
            None,
            Some("image/jpeg"),
            Some(jpeg_bytes.clone()),
            Some(*pixel_width),
            Some(*pixel_height),
            i64::try_from(jpeg_bytes.len()).map_err(|_| {
                StorageError::InvalidProductImage("the menu image is too large to store")
            })?,
            Some(Sha256::digest(jpeg_bytes).to_vec()),
            Some(provenance.clone()),
        ),
    };

    let image_version_id = EntityId::new_v7();
    let assigned_at = utc_timestamp(transaction)?;
    transaction
        .execute(
            "
            INSERT INTO product_image_versions (
                image_version_id,
                branch_id,
                product_id,
                source_kind,
                asset_key,
                content_type,
                image_bytes,
                pixel_width,
                pixel_height,
                byte_length,
                content_sha256,
                created_at_utc,
                created_by_actor_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
            )
            ",
            params![
                image_version_id.to_string(),
                context.branch_id().to_string(),
                product_id.to_string(),
                persisted_source_kind,
                asset_key.as_deref(),
                content_type,
                image_bytes.as_deref(),
                pixel_width,
                pixel_height,
                byte_length,
                content_sha256.as_deref(),
                assigned_at,
                context.actor_id().to_string(),
            ],
        )
        .map_err(StorageError::Sql)?;

    let (assignment_revision, previous_image_version_id, event_type) = match existing {
        Some(existing) => {
            if existing.revision < 1 {
                return Err(StorageError::InvalidPersistedData(
                    "product image assignment revision was invalid".to_owned(),
                ));
            }
            let revision = existing
                .revision
                .checked_add(1)
                .ok_or(StorageError::CatalogConflict)?;
            let updated = transaction
                .execute(
                    "
                    UPDATE product_image_assignments
                    SET image_version_id = ?1,
                        revision = ?2,
                        assigned_at_utc = ?3,
                        assigned_by_actor_id = ?4
                    WHERE branch_id = ?5
                      AND product_id = ?6
                      AND revision = ?7
                    ",
                    params![
                        image_version_id.to_string(),
                        revision,
                        assigned_at,
                        context.actor_id().to_string(),
                        context.branch_id().to_string(),
                        product_id.to_string(),
                        existing.revision,
                    ],
                )
                .map_err(StorageError::Sql)?;
            if updated != 1 {
                return Err(StorageError::CatalogConflict);
            }
            (
                revision,
                Some(existing.image_version_id),
                "catalog.product.image.replaced",
            )
        }
        None => {
            transaction
                .execute(
                    "
                    INSERT INTO product_image_assignments (
                        product_id,
                        branch_id,
                        image_version_id,
                        revision,
                        assigned_at_utc,
                        assigned_by_actor_id
                    ) VALUES (?1, ?2, ?3, 1, ?4, ?5)
                    ",
                    params![
                        product_id.to_string(),
                        context.branch_id().to_string(),
                        image_version_id.to_string(),
                        assigned_at,
                        context.actor_id().to_string(),
                    ],
                )
                .map_err(StorageError::Sql)?;
            (1, None, "catalog.product.image.assigned")
        }
    };

    let payload = json!({
        "schema_version": 1,
        "entity_type": "product_image",
        "entity_id": image_version_id.to_string(),
        "product_id": product_id.to_string(),
        "assignment_revision": assignment_revision,
        "correlation_id": context.correlation_id().to_string(),
        "actor_role": context.actor_role().as_str(),
        "before": {
            "image_version_id": previous_image_version_id,
        },
        "after": {
            "source_kind": source_kind.as_str(),
            "asset_key": asset_key.clone(),
            "content_sha256": content_sha256.as_deref().map(lowercase_hex),
            "pixel_width": pixel_width,
            "pixel_height": pixel_height,
            "byte_length": byte_length,
            "catalog": catalog_provenance.as_ref().map(|provenance| json!({
                "image_id": provenance.catalog_image_id(),
                "original_content_sha256": lowercase_hex(provenance.original_content_sha256()),
                "licence_label": provenance.licence_label(),
                "licence_url": provenance.licence_url(),
                "service_origin": provenance.service_origin(),
                "service_schema_version": provenance.service_schema_version(),
            })),
        },
    })
    .to_string();
    let audit_event_id = append_hashed_audit_event(transaction, context, event_type, &payload)?;

    if let Some(provenance) = catalog_provenance.as_ref() {
        transaction
            .execute(
                "
                INSERT INTO product_image_catalog_provenance (
                    image_version_id,
                    branch_id,
                    product_id,
                    catalog_image_id,
                    original_content_sha256,
                    licence_label,
                    licence_url,
                    service_origin,
                    service_schema_version,
                    audit_event_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ",
                params![
                    image_version_id.to_string(),
                    context.branch_id().to_string(),
                    product_id.to_string(),
                    provenance.catalog_image_id(),
                    provenance.original_content_sha256(),
                    provenance.licence_label(),
                    provenance.licence_url(),
                    provenance.service_origin(),
                    provenance.service_schema_version(),
                    audit_event_id.to_string(),
                ],
            )
            .map_err(StorageError::Sql)?;
    }

    Ok(ProductImage::new(
        product_id.clone(),
        source_kind,
        asset_key,
        image_bytes,
        catalog_provenance,
    ))
}

fn product_image_from_persisted(
    record: PersistedProductImage,
) -> Result<ProductImage, StorageError> {
    let product_id = parse_persisted_id(&record.product_id)?;
    let source_kind = ProductImageSource::parse(&record.source_kind)?;

    match source_kind {
        ProductImageSource::BuiltIn => {
            let asset_key = record.asset_key.ok_or_else(|| {
                StorageError::InvalidPersistedData(
                    "built-in product image did not have an asset key".to_owned(),
                )
            })?;
            if !is_supported_builtin_menu_image_key(&asset_key)
                || record.content_type.is_some()
                || record.image_bytes.is_some()
                || record.pixel_width.is_some()
                || record.pixel_height.is_some()
                || record.byte_length != 0
                || record.content_sha256.is_some()
                || record.catalog_image_id.is_some()
                || record.catalog_original_content_sha256.is_some()
                || record.catalog_licence_label.is_some()
                || record.catalog_licence_url.is_some()
                || record.catalog_service_origin.is_some()
                || record.catalog_service_schema_version.is_some()
            {
                return Err(StorageError::InvalidPersistedData(
                    "built-in product image did not satisfy its storage contract".to_owned(),
                ));
            }

            Ok(ProductImage::new(
                product_id,
                source_kind,
                Some(asset_key),
                None,
                None,
            ))
        }
        ProductImageSource::RestaurantUpload => {
            let image_bytes = record.image_bytes.ok_or_else(|| {
                StorageError::InvalidPersistedData(
                    "restaurant product image did not have encoded bytes".to_owned(),
                )
            })?;
            let pixel_width = record.pixel_width.ok_or_else(|| {
                StorageError::InvalidPersistedData(
                    "restaurant product image did not have a width".to_owned(),
                )
            })?;
            let pixel_height = record.pixel_height.ok_or_else(|| {
                StorageError::InvalidPersistedData(
                    "restaurant product image did not have a height".to_owned(),
                )
            })?;
            let content_sha256 = record.content_sha256.ok_or_else(|| {
                StorageError::InvalidPersistedData(
                    "restaurant product image did not have a content hash".to_owned(),
                )
            })?;
            let image = ProductImageContent::restaurant_upload(
                image_bytes.clone(),
                pixel_width,
                pixel_height,
            );
            validate_product_image_content(&image)?;
            let expected_byte_length = i64::try_from(image_bytes.len()).map_err(|_| {
                StorageError::InvalidPersistedData(
                    "restaurant product image byte length was invalid".to_owned(),
                )
            })?;

            if record.asset_key.is_some()
                || record.content_type.as_deref() != Some("image/jpeg")
                || record.byte_length != expected_byte_length
                || content_sha256.len() != 32
                || content_sha256.as_slice() != Sha256::digest(&image_bytes).as_slice()
            {
                return Err(StorageError::InvalidPersistedData(
                    "restaurant product image did not satisfy its storage contract".to_owned(),
                ));
            }

            let catalog_provenance = match record.catalog_image_id {
                Some(catalog_image_id) => Some(ProductImageCatalogProvenance::new(
                    catalog_image_id,
                    record.catalog_original_content_sha256.ok_or_else(|| {
                        StorageError::InvalidPersistedData(
                            "catalogue image provenance did not have a source digest".to_owned(),
                        )
                    })?,
                    record.catalog_licence_label.ok_or_else(|| {
                        StorageError::InvalidPersistedData(
                            "catalogue image provenance did not have a licence label".to_owned(),
                        )
                    })?,
                    record.catalog_licence_url.ok_or_else(|| {
                        StorageError::InvalidPersistedData(
                            "catalogue image provenance did not have a licence URL".to_owned(),
                        )
                    })?,
                    record.catalog_service_origin.ok_or_else(|| {
                        StorageError::InvalidPersistedData(
                            "catalogue image provenance did not have a service origin".to_owned(),
                        )
                    })?,
                    record.catalog_service_schema_version.ok_or_else(|| {
                        StorageError::InvalidPersistedData(
                            "catalogue image provenance did not have a schema version".to_owned(),
                        )
                    })?,
                )?),
                None => {
                    if record.catalog_original_content_sha256.is_some()
                        || record.catalog_licence_label.is_some()
                        || record.catalog_licence_url.is_some()
                        || record.catalog_service_origin.is_some()
                        || record.catalog_service_schema_version.is_some()
                    {
                        return Err(StorageError::InvalidPersistedData(
                            "catalogue image provenance was incomplete".to_owned(),
                        ));
                    }
                    None
                }
            };
            let source_kind = if catalog_provenance.is_some() {
                ProductImageSource::GotiginCatalog
            } else {
                ProductImageSource::RestaurantUpload
            };

            Ok(ProductImage::new(
                product_id,
                source_kind,
                None,
                Some(image_bytes),
                catalog_provenance,
            ))
        }
        ProductImageSource::GotiginCatalog => Err(StorageError::InvalidPersistedData(
            "catalogue product image used an unsupported physical source kind".to_owned(),
        )),
    }
}

/// The immutable source snapshot that a management cancellation is allowed to
/// act upon. It is loaded under the caller's immediate transaction so a stale
/// POS revision, a settlement, or an already-recorded cancellation fails
/// without leaving partial kitchen facts behind.
struct SentDraftKitchenCancellationTarget {
    ticket_id: EntityId,
    current_revision: i64,
    fulfillment: String,
    table_name: Option<String>,
    currency_code: String,
    subtotal_minor: i64,
    line_count: i64,
    line_snapshot_json: String,
    kitchen_note: Option<String>,
}

fn load_sent_draft_for_kitchen_cancellation(
    transaction: &Transaction<'_>,
    draft_order_id: &EntityId,
    expected_revision: i64,
    context: &MutationContext,
) -> Result<SentDraftKitchenCancellationTarget, StorageError> {
    let target: Option<SentDraftKitchenCancellationTarget> = transaction
        .query_row(
            "SELECT k.kitchen_ticket_id, d.current_revision, d.fulfillment, t.display_name, d.currency_code, r.subtotal_minor, r.line_count, r.line_snapshot_json, r.kitchen_note FROM draft_orders d JOIN draft_order_revisions r ON r.draft_order_id = d.draft_order_id AND r.revision = d.current_revision JOIN kitchen_tickets k ON k.draft_order_id = d.draft_order_id AND k.draft_revision = d.current_revision AND k.branch_id = d.branch_id LEFT JOIN restaurant_tables t ON t.table_id = d.table_id WHERE d.draft_order_id = ?1 AND d.branch_id = ?2 AND d.draft_state = 'sent_to_kitchen' AND k.ticket_state <> 'completed' AND NOT EXISTS (SELECT 1 FROM draft_order_settlements settlement WHERE settlement.draft_order_id = d.draft_order_id) AND NOT EXISTS (SELECT 1 FROM kitchen_ticket_cancellation_notices notice WHERE notice.kitchen_ticket_id = k.kitchen_ticket_id)",
            params![draft_order_id.to_string(), context.branch_id().to_string()],
            |row| {
                Ok(SentDraftKitchenCancellationTarget {
                    ticket_id: EntityId::parse(&row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    current_revision: row.get(1)?,
                    fulfillment: row.get(2)?,
                    table_name: row.get(3)?,
                    currency_code: row.get(4)?,
                    subtotal_minor: row.get(5)?,
                    line_count: row.get(6)?,
                    line_snapshot_json: row.get(7)?,
                    kitchen_note: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::Sql)?;
    let Some(target) = target else {
        return Err(StorageError::CatalogConflict);
    };
    if target.current_revision != expected_revision {
        return Err(StorageError::CatalogConflict);
    }
    Ok(target)
}

fn append_kitchen_ticket_cancellation_notice(
    transaction: &Transaction<'_>,
    draft_order_id: &EntityId,
    target: &SentDraftKitchenCancellationTarget,
    reason: &MutationReason,
    context: &MutationContext,
    occurred_at: &str,
) -> Result<EntityId, StorageError> {
    let notice_id = EntityId::new_v7();
    let payload = json!({
        "schema_version": 1,
        "entity_type": "kitchen_ticket_cancellation_notice",
        "entity_id": notice_id.to_string(),
        "kitchen_ticket_id": target.ticket_id.to_string(),
        "draft_order_id": draft_order_id.to_string(),
        "draft_revision": target.current_revision,
        "reason": reason.as_str(),
        "correlation_id": context.correlation_id().to_string(),
        "actor_role": context.actor_role().as_str(),
    })
    .to_string();
    let audit_event_id = append_hashed_audit_event(
        transaction,
        context,
        "operations.kitchen_ticket.cancellation.requested",
        &payload,
    )?;
    transaction
        .execute(
            "INSERT INTO kitchen_ticket_cancellation_notices (kitchen_ticket_cancellation_notice_id, kitchen_ticket_id, branch_id, draft_order_id, draft_revision, reason, occurred_at_utc, occurred_by_actor_id, audit_event_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![notice_id.to_string(), target.ticket_id.to_string(), context.branch_id().to_string(), draft_order_id.to_string(), target.current_revision, reason.as_str(), occurred_at, context.actor_id().to_string(), audit_event_id.to_string()],
        )
        .map_err(StorageError::Sql)?;
    append_sync_outbox_event(
        transaction,
        context,
        &audit_event_id,
        "kitchen_ticket_cancellation_notice",
        &notice_id,
        "operations.kitchen_ticket.cancellation.requested",
        occurred_at,
    )?;
    Ok(notice_id)
}

/// Reloads the only money-bearing facts permitted for a sale line from the
/// encrypted catalogue. The Flutter/bridge command contains option identities
/// and a quantity only; every name, price delta, effective unit price, and
/// snapshot written below is derived within the same SQLite transaction.
fn resolve_sale_line_snapshot(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
    branch_currency: &str,
    line: &SaleLineInput,
) -> Result<SaleLineSnapshot, StorageError> {
    let product = read_product(transaction, branch_id, line.product_id())?;
    if product.archived() || !product.is_available() {
        return Err(StorageError::ProductUnavailable);
    }
    if product.unit_price().currency() != branch_currency {
        return Err(StorageError::CurrencyMismatch {
            expected: branch_currency.to_owned(),
            actual: product.unit_price().currency().to_owned(),
        });
    }

    let mut modifier_total_minor = 0_i64;
    let mut modifiers = Vec::with_capacity(line.modifier_option_ids().len());
    for modifier_option_id in line.modifier_option_ids() {
        let option = read_modifier_option(transaction, branch_id, modifier_option_id)?;
        if option.archived() || option.product_id() != product.product_id() {
            return Err(StorageError::ModifierOptionUnavailable);
        }
        if option.price_delta().currency() != branch_currency {
            return Err(StorageError::CurrencyMismatch {
                expected: branch_currency.to_owned(),
                actual: option.price_delta().currency().to_owned(),
            });
        }
        modifier_total_minor = modifier_total_minor
            .checked_add(option.price_delta().minor_units())
            .ok_or(StorageError::FinancialAmountOverflow)?;
        modifiers.push(SaleLineModifierSnapshot {
            modifier_option_id: option.modifier_option_id().clone(),
            display_name: option.display_name().display().to_owned(),
            price_delta_minor: option.price_delta().minor_units(),
        });
    }
    let base_unit_price_minor = product.unit_price().minor_units();
    let unit_price_minor = base_unit_price_minor
        .checked_add(modifier_total_minor)
        .ok_or(StorageError::FinancialAmountOverflow)?;
    let line_total_minor = unit_price_minor
        .checked_mul(line.quantity())
        .ok_or(StorageError::FinancialAmountOverflow)?;
    Ok(SaleLineSnapshot {
        product_id: product.product_id().clone(),
        product_name: product.display_name().display().to_owned(),
        quantity: line.quantity(),
        base_unit_price_minor,
        modifier_total_minor,
        modifiers,
        unit_price_minor,
        line_total_minor,
    })
}

fn calculate_sale_pricing(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
    branch_currency: &str,
    snapshots: &[SaleLineSnapshot],
    discount: Option<OrderDiscount>,
) -> Result<PricingBreakdown, StorageError> {
    let active_rates = load_active_branch_tax_rates(transaction, branch_id)?;
    let mut pricing_lines = Vec::with_capacity(snapshots.len());
    for (index, snapshot) in snapshots.iter().enumerate() {
        let treatment_kind: String = transaction
            .query_row(
                "SELECT tax_treatment FROM products WHERE product_id = ?1 AND branch_id = ?2",
                params![snapshot.product_id.to_string(), branch_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StorageError::Sql)?;
        let tax_treatment = match treatment_kind.as_str() {
            "no_tax" => TaxTreatment::no_tax(),
            "exclusive" => {
                if active_rates.is_empty() {
                    return Err(StorageError::InvalidPersistedData(
                        "exclusive tax treatment requires at least one active branch tax rate"
                            .to_owned(),
                    ));
                }
                TaxTreatment::exclusive(active_rates.clone())
                    .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?
            }
            "inclusive" => {
                if active_rates.is_empty() {
                    return Err(StorageError::InvalidPersistedData(
                        "inclusive tax treatment requires at least one active branch tax rate"
                            .to_owned(),
                    ));
                }
                TaxTreatment::inclusive(active_rates.clone())
                    .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?
            }
            _ => {
                return Err(StorageError::InvalidPersistedData(
                    "product tax treatment is invalid".to_owned(),
                ));
            }
        };
        let unit_price = Money::new(snapshot.unit_price_minor, branch_currency)
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
        let line_reference = format!("{}:{}", snapshot.product_id, index + 1);
        pricing_lines.push(
            PricingLineInput::new(
                &line_reference,
                &snapshot.product_name,
                unit_price,
                snapshot.quantity,
                tax_treatment,
            )
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?,
        );
    }
    ros_core::pricing::calculate_pricing(pricing_lines, discount)
        .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))
}

fn load_active_branch_tax_rates(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
) -> Result<Vec<TaxRate>, StorageError> {
    let mut statement = transaction
        .prepare(
            "SELECT display_name, basis_points FROM branch_tax_rates \
             WHERE branch_id = ?1 AND archived_at_utc IS NULL \
             ORDER BY name_key, tax_rate_id",
        )
        .map_err(StorageError::Sql)?;
    let rates = statement
        .query_map([branch_id.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(StorageError::Sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::Sql)?;
    if rates.len() > ros_core::pricing::MAX_TAX_RATES_PER_LINE {
        return Err(StorageError::InvalidPersistedData(
            "a branch may have at most eight active tax rates".to_owned(),
        ));
    }
    rates
        .into_iter()
        .map(|(name, basis_points)| {
            let basis_points = u32::try_from(basis_points).map_err(|_| {
                StorageError::InvalidPersistedData("tax rate basis points are invalid".to_owned())
            })?;
            TaxRate::new(&name, basis_points)
                .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))
        })
        .collect()
}

fn encode_pricing_snapshot(pricing: &PricingBreakdown) -> Result<String, StorageError> {
    let discount = pricing.discount().map(|discount| {
        json!({
            "kind": match discount.kind() {
                ros_core::pricing::DiscountKind::Fixed => "fixed",
                ros_core::pricing::DiscountKind::Percentage => "percentage",
            },
            "reason": discount.reason(),
            "applied_minor": pricing.discount_minor(),
            "authorization_intent": "manager_or_owner",
        })
    });
    serde_json::to_string(&json!({
        "schema_version": 1,
        "currency_code": pricing.currency(),
        "input_price_subtotal_minor": pricing.input_price_subtotal_minor(),
        "net_before_discount_minor": pricing.net_before_discount_minor(),
        "discount_minor": pricing.discount_minor(),
        "net_minor": pricing.net_minor(),
        "tax_before_discount_minor": pricing.tax_before_discount_minor(),
        "tax_minor": pricing.tax_minor(),
        "payable_before_discount_minor": pricing.payable_before_discount_minor(),
        "payable_minor": pricing.payable_minor(),
        "discount": discount,
        "lines": pricing.lines().iter().map(|line| json!({
            "line_reference": line.line_reference(),
            "display_name": line.display_name(),
            "quantity": line.quantity(),
            "unit_price_minor": line.unit_price_minor(),
            "net_before_discount_minor": line.net_before_discount_minor(),
            "allocated_discount_minor": line.allocated_discount_minor(),
            "net_minor": line.net_minor(),
            "tax_minor": line.tax_minor(),
            "payable_minor": line.payable_minor(),
        })).collect::<Vec<_>>(),
    }))
    .map_err(|_| {
        StorageError::InvalidPersistedData("pricing snapshot could not be encoded".to_owned())
    })
}

fn modifier_snapshot_value(snapshot: &SaleLineSnapshot) -> serde_json::Value {
    serde_json::Value::Array(
        snapshot
            .modifiers
            .iter()
            .map(|modifier| {
                json!({
                    "modifier_option_id": modifier.modifier_option_id.to_string(),
                    "display_name_snapshot": modifier.display_name,
                    "price_delta_minor": modifier.price_delta_minor,
                })
            })
            .collect(),
    )
}

fn modifier_snapshot_json(snapshot: &SaleLineSnapshot) -> Result<String, StorageError> {
    serde_json::to_string(&modifier_snapshot_value(snapshot)).map_err(|_| {
        StorageError::InvalidPersistedData("modifier snapshot could not be encoded".to_owned())
    })
}

fn parse_modifier_snapshot_names(snapshot: &str) -> Result<Vec<String>, StorageError> {
    let values: Vec<serde_json::Value> = serde_json::from_str(snapshot).map_err(|_| {
        StorageError::InvalidPersistedData("modifier snapshot is invalid".to_owned())
    })?;
    if values.len() > ros_core::MAX_MODIFIER_OPTIONS_PER_SALE_LINE {
        return Err(StorageError::InvalidPersistedData(
            "modifier snapshot exceeds its supported size".to_owned(),
        ));
    }
    let mut option_ids = HashSet::with_capacity(values.len());
    values
        .into_iter()
        .map(|value| {
            let option_id = value
                .get("modifier_option_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    StorageError::InvalidPersistedData("modifier snapshot is invalid".to_owned())
                })?;
            let option_id = EntityId::parse(option_id)
                .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
            if !option_ids.insert(option_id) {
                return Err(StorageError::InvalidPersistedData(
                    "modifier snapshot contains duplicate options".to_owned(),
                ));
            }
            let name = value
                .get("display_name_snapshot")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    StorageError::InvalidPersistedData("modifier snapshot is invalid".to_owned())
                })?;
            DisplayName::modifier_option(name)
                .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?;
            let delta = value
                .get("price_delta_minor")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| {
                    StorageError::InvalidPersistedData("modifier snapshot is invalid".to_owned())
                })?;
            if delta < 0 {
                return Err(StorageError::InvalidPersistedData(
                    "modifier snapshot has a negative delta".to_owned(),
                ));
            }
            Ok(name.to_owned())
        })
        .collect()
}

fn sale_line_snapshot_value(snapshot: &SaleLineSnapshot) -> serde_json::Value {
    json!({
        "product_id": snapshot.product_id.to_string(),
        "product_name_snapshot": snapshot.product_name,
        "quantity": snapshot.quantity,
        "base_unit_price_minor": snapshot.base_unit_price_minor,
        "modifier_option_ids": snapshot.modifiers.iter().map(|modifier| modifier.modifier_option_id.to_string()).collect::<Vec<_>>(),
        "modifier_total_minor": snapshot.modifier_total_minor,
        "modifier_snapshot": modifier_snapshot_value(snapshot),
        "unit_price_minor": snapshot.unit_price_minor,
        "line_total_minor": snapshot.line_total_minor,
    })
}

fn parse_draft_snapshot_lines(snapshot: &str) -> Result<Vec<SaleLineInput>, StorageError> {
    let values: Vec<serde_json::Value> = serde_json::from_str(snapshot)
        .map_err(|_| StorageError::InvalidPersistedData("draft snapshot is invalid".to_owned()))?;
    let lines = values
        .into_iter()
        .map(|value| {
            let product_id = value
                .get("product_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    StorageError::InvalidPersistedData("draft snapshot is invalid".to_owned())
                })?;
            let quantity = value
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| {
                    StorageError::InvalidPersistedData("draft snapshot is invalid".to_owned())
                })?;
            let modifier_option_ids = match value.get("modifier_option_ids") {
                None => Vec::new(),
                Some(serde_json::Value::Array(values)) => values
                    .iter()
                    .map(|value| {
                        let value = value.as_str().ok_or_else(|| {
                            StorageError::InvalidPersistedData(
                                "draft modifier snapshot is invalid".to_owned(),
                            )
                        })?;
                        EntityId::parse(value)
                            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                Some(_) => {
                    return Err(StorageError::InvalidPersistedData(
                        "draft modifier snapshot is invalid".to_owned(),
                    ));
                }
            };
            SaleLineInput::new(
                EntityId::parse(product_id)
                    .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))?,
                quantity,
            )
            .and_then(|line| line.with_modifier_options(modifier_option_ids))
            .map_err(|error| StorageError::InvalidPersistedData(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if lines.is_empty() {
        return Err(StorageError::InvalidPersistedData(
            "draft snapshot must contain at least one line".to_owned(),
        ));
    }
    ensure_unique_sale_line_configurations(
        &lines,
        "draft snapshot contains duplicate product and modifier combinations",
    )?;
    Ok(lines)
}

fn ensure_unique_sale_line_configurations(
    lines: &[SaleLineInput],
    message: &'static str,
) -> Result<(), StorageError> {
    let mut line_configurations = HashSet::with_capacity(lines.len());
    if lines.iter().any(|line| {
        !line_configurations.insert((
            line.product_id().clone(),
            line.modifier_option_ids().to_vec(),
        ))
    }) {
        return Err(StorageError::InvalidPersistedData(message.to_owned()));
    }
    Ok(())
}

fn same_sale_lines(left: &[SaleLineInput], right: &[SaleLineInput]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut right_by_configuration = HashMap::with_capacity(right.len());
    for line in right {
        if right_by_configuration
            .insert(
                (
                    line.product_id().clone(),
                    line.modifier_option_ids().to_vec(),
                ),
                line.quantity(),
            )
            .is_some()
        {
            return false;
        }
    }
    left.iter().all(|line| {
        right_by_configuration.remove(&(
            line.product_id().clone(),
            line.modifier_option_ids().to_vec(),
        )) == Some(line.quantity())
    }) && right_by_configuration.is_empty()
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    output
}

fn select_identifiers(
    connection: &Connection,
    query: &str,
    branch_id: &EntityId,
) -> Result<Vec<EntityId>, StorageError> {
    let mut statement = connection.prepare(query).map_err(StorageError::Sql)?;
    let rows = statement
        .query_map([branch_id.to_string()], |row| row.get::<_, String>(0))
        .map_err(StorageError::Sql)?;

    rows.map(|row| {
        let value = row.map_err(StorageError::Sql)?;
        parse_persisted_id(&value)
    })
    .collect()
}

fn read_community_branch(connection: &Connection) -> Result<Branch, StorageError> {
    let record: Option<PersistedCommunityBranch> = connection
        .query_row(
            "
            SELECT organization_id, branch_id, display_name, currency_code, time_zone
            FROM branches
            WHERE archived_at_utc IS NULL
            ORDER BY created_at_utc, branch_id
            LIMIT 1
            ",
            [],
            |row| {
                Ok(PersistedCommunityBranch {
                    organization_id: row.get(0)?,
                    branch_id: row.get(1)?,
                    display_name: row.get(2)?,
                    currency_code: row.get(3)?,
                    time_zone: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::Sql)?;

    let record = record.ok_or(StorageError::CommunityNotProvisioned)?;
    let organization_id = parse_persisted_id(&record.organization_id)?;
    let branch_id = parse_persisted_id(&record.branch_id)?;
    let display_name = DisplayName::branch(&record.display_name).map_err(invalid_persisted_data)?;
    let currency = CurrencyCode::parse(&record.currency_code).map_err(invalid_persisted_data)?;
    let time_zone = TimeZoneId::parse(&record.time_zone).map_err(invalid_persisted_data)?;

    Ok(Branch::new(
        organization_id,
        branch_id,
        display_name,
        currency,
        time_zone,
    ))
}

fn ensure_local_installation_identity(
    transaction: &Transaction<'_>,
) -> Result<(EntityId, EntityId), StorageError> {
    let existing: Option<(String, String)> = transaction
        .query_row(
            "SELECT device_id, owner_actor_id FROM local_installation_identity WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(StorageError::Sql)?;

    if let Some((device_id, owner_actor_id)) = existing {
        return Ok((
            parse_persisted_id(&device_id)?,
            parse_persisted_id(&owner_actor_id)?,
        ));
    }

    let device_id = EntityId::new_v7();
    let owner_actor_id = EntityId::new_v7();
    transaction
        .execute(
            "
            INSERT INTO local_installation_identity (
                singleton,
                device_id,
                owner_actor_id,
                created_at_utc
            ) VALUES (1, ?1, ?2, ?3)
            ",
            params![
                device_id.to_string(),
                owner_actor_id.to_string(),
                utc_timestamp(transaction)?,
            ],
        )
        .map_err(StorageError::Sql)?;

    Ok((device_id, owner_actor_id))
}

fn parse_actor_role(value: &str) -> Result<ActorRole, StorageError> {
    match value {
        "owner" => Ok(ActorRole::Owner),
        "manager" => Ok(ActorRole::Manager),
        "cashier" => Ok(ActorRole::Cashier),
        "kitchen" => Ok(ActorRole::Kitchen),
        _ => Err(StorageError::InvalidPersistedData(
            "staff role was invalid".to_owned(),
        )),
    }
}

fn read_local_staff_account(
    transaction: &Transaction<'_>,
    branch: &Branch,
    staff_id: &EntityId,
) -> Result<LocalStaffAccount, StorageError> {
    let record: Option<(String, String, String, i64, bool)> = transaction
        .query_row(
            "
            SELECT
                staff.staff_id,
                staff.display_name,
                COALESCE((
                    SELECT role.role
                    FROM staff_role_events AS role
                    JOIN local_security_fact_order AS role_order
                        ON role_order.fact_kind = 'staff_role'
                        AND role_order.fact_id = role.staff_role_event_id
                    WHERE role.staff_id = staff.staff_id
                    ORDER BY role_order.fact_sequence DESC
                    LIMIT 1
                ), staff.role),
                COALESCE((
                    SELECT status.status = 'active'
                    FROM staff_status_events AS status
                    JOIN local_security_fact_order AS status_order
                        ON status_order.fact_kind = 'staff_status'
                        AND status_order.fact_id = status.staff_status_event_id
                    WHERE status.staff_id = staff.staff_id
                    ORDER BY status_order.fact_sequence DESC
                    LIMIT 1
                ), 0),
                EXISTS(
                    SELECT 1
                    FROM staff_pin_credentials AS credential
                    WHERE credential.staff_id = staff.staff_id
                )
            FROM staff_accounts AS staff
            WHERE staff.staff_id = ?1 AND staff.branch_id = ?2
            ",
            params![staff_id.to_string(), branch.branch_id().to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(StorageError::Sql)?;
    let (persisted_id, display_name, role, is_active, has_pin) =
        record.ok_or(StorageError::StaffNotFound)?;
    Ok(LocalStaffAccount {
        staff_id: parse_persisted_id(&persisted_id)?,
        display_name,
        role: parse_actor_role(&role)?,
        is_active: is_active != 0,
        has_pin,
    })
}

fn read_active_local_staff(
    transaction: &Transaction<'_>,
    branch: &Branch,
    device_id: &EntityId,
) -> Result<Option<LocalStaffAccount>, StorageError> {
    let Some(observed_now) = observe_local_security_clock(transaction)? else {
        // A backwards wall-clock adjustment must never lengthen an existing
        // session. The explicit lock path can still append a lock event while
        // the clock is behind this high-water mark.
        return Ok(None);
    };
    let latest = read_latest_local_staff_session(transaction, device_id)?;
    let Some((event_type, staff_id, expires_at)) = latest else {
        return Ok(None);
    };
    if event_type != "unlocked" {
        return Ok(None);
    }
    let Some(expires_at) = expires_at else {
        return Err(StorageError::InvalidPersistedData(
            "unlocked staff session did not have an expiry".to_owned(),
        ));
    };
    if expires_at <= observed_now {
        return Ok(None);
    }
    let staff_id = parse_persisted_id(&staff_id)?;
    let staff = read_local_staff_account(transaction, branch, &staff_id)?;
    Ok((staff.is_active() && staff.has_pin()).then_some(staff))
}

fn read_latest_local_staff_session(
    transaction: &Transaction<'_>,
    device_id: &EntityId,
) -> Result<Option<(String, String, Option<String>)>, StorageError> {
    let latest: Option<(String, String, Option<String>)> = transaction
        .query_row(
            "
            SELECT event_type, staff_id, expires_at_utc
            FROM local_staff_session_events AS session
            JOIN local_security_fact_order AS session_order
                ON session_order.fact_kind = 'staff_session'
                AND session_order.fact_id = session.local_staff_session_event_id
            WHERE session.device_id = ?1
            ORDER BY session_order.fact_sequence DESC
            LIMIT 1
            ",
            [device_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(StorageError::Sql)?;
    Ok(latest)
}

/// Returns a trusted current time and advances the bounded, monotonic
/// high-water mark. A value behind any previously accepted observation fails
/// closed; repeated polling never creates additional rows.
fn observe_local_security_clock(
    transaction: &Transaction<'_>,
) -> Result<Option<String>, StorageError> {
    let observed_now = utc_timestamp(transaction)?;
    let high_water: String = transaction
        .query_row(
            "SELECT high_water_utc FROM local_security_clock_state WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(StorageError::Sql)?;
    if observed_now < high_water {
        return Ok(None);
    }
    if observed_now > high_water {
        transaction
            .execute(
                "UPDATE local_security_clock_state SET high_water_utc = ?1 WHERE singleton = 1",
                [&observed_now],
            )
            .map_err(StorageError::Sql)?;
    }
    Ok(Some(observed_now))
}

/// Re-resolves every authorization input while holding the caller's
/// `BEGIN IMMEDIATE` transaction. Caller-provided IDs and roles are only
/// attribution hints until they match the active local installation, latest
/// append-only staff facts, and current unexpired session here.
fn revalidate_mutation_context(
    transaction: &Transaction<'_>,
    context: &MutationContext,
) -> Result<(), StorageError> {
    let branch = read_community_branch(transaction)?;
    if branch.branch_id() != context.branch_id() {
        return Err(StorageError::StaffSessionRequired);
    }
    ensure_active_branch(transaction, context.branch_id())?;
    let (device_id, _) = ensure_local_installation_identity(transaction)?;
    if &device_id != context.device_id() {
        return Err(StorageError::StaffSessionRequired);
    }
    let staff = read_active_local_staff(transaction, &branch, &device_id)?
        .ok_or(StorageError::StaffSessionRequired)?;
    if staff.staff_id() != context.actor_id() {
        return Err(StorageError::StaffSessionRequired);
    }
    if staff.role() != context.actor_role() {
        return Err(StorageError::PermissionDenied);
    }
    Ok(())
}

fn validate_local_staff_pin(pin: &str) -> Result<(), StorageError> {
    if !(6..=12).contains(&pin.len()) || !pin.bytes().all(|value| value.is_ascii_digit()) {
        return Err(StorageError::InvalidStaffPin);
    }
    Ok(())
}

fn normalize_customer_phone(value: Option<&str>) -> Result<Option<String>, StorageError> {
    value
        .map(|value| {
            let normalized = value
                .trim()
                .chars()
                .filter(|character| character.is_ascii_digit())
                .collect::<String>();
            if !(7..=15).contains(&normalized.len()) {
                return Err(StorageError::InvalidPersistedData(
                    "customer phone number was invalid".to_owned(),
                ));
            }
            Ok(normalized)
        })
        .transpose()
}

fn normalize_customer_email(value: Option<&str>) -> Result<Option<String>, StorageError> {
    value
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.len() > 254
                || normalized.chars().any(char::is_control)
                || !normalized.contains('@')
            {
                return Err(StorageError::InvalidPersistedData(
                    "customer email address was invalid".to_owned(),
                ));
            }
            Ok(normalized)
        })
        .transpose()
}

fn normalize_customer_reason(value: &str) -> Result<String, StorageError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 280 || value.chars().any(char::is_control) {
        return Err(StorageError::InvalidPersistedData(
            "customer privacy reason was invalid".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn normalize_kitchen_note(value: Option<&str>) -> Result<Option<String>, StorageError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized.chars().count() > 500 || normalized.chars().any(char::is_control) {
        return Err(StorageError::InvalidPersistedData(
            "kitchen note must be at most 500 characters and cannot contain control characters"
                .to_owned(),
        ));
    }
    Ok(Some(normalized.to_owned()))
}

fn next_customer_profile_revision(
    transaction: &Transaction<'_>,
    customer_id: &EntityId,
) -> Result<i64, StorageError> {
    transaction
        .query_row(
            "SELECT COALESCE(MAX(revision), 0) + 1 FROM customer_profile_revisions WHERE customer_id = ?1",
            [customer_id.to_string()],
            |row| row.get(0),
        )
        .map_err(StorageError::Sql)
}

fn local_staff_argon2() -> Result<Argon2<'static>, StorageError> {
    // Argon2id v1.3, 64 MiB, three passes, one lane. This intentionally
    // exceeds a fast PIN checksum while remaining suitable for an occasional
    // local unlock on supported desktop/tablet hardware.
    #[cfg(not(test))]
    let parameters = Argon2Params::new(65_536, 3, 1, None)
        .map_err(|_| StorageError::StaffCredentialUnavailable)?;
    // The production parameters above are deliberately expensive. Unit tests
    // still exercise the same Argon2id code path with reduced cost so the
    // full storage suite remains practical in continuous integration.
    #[cfg(test)]
    let parameters = Argon2Params::new(4_096, 1, 1, None)
        .map_err(|_| StorageError::StaffCredentialUnavailable)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters))
}

fn hash_local_staff_pin(pin: &str) -> Result<String, StorageError> {
    let pin = Zeroizing::new(pin.as_bytes().to_vec());
    let salt = SaltString::generate(&mut OsRng);
    local_staff_argon2()?
        .hash_password(pin.as_slice(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| StorageError::StaffCredentialUnavailable)
}

fn verify_local_staff_pin(pin: &str, digest: &str) -> bool {
    let pin = Zeroizing::new(pin.as_bytes().to_vec());
    let Ok(parsed) = PasswordHash::new(digest) else {
        return false;
    };
    local_staff_argon2()
        .and_then(|argon2| {
            argon2
                .verify_password(pin.as_slice(), &parsed)
                .map_err(|_| StorageError::InvalidStaffPin)
        })
        .is_ok()
}

fn read_category(
    connection: &Connection,
    branch_id: &EntityId,
    category_id: &EntityId,
) -> Result<Category, StorageError> {
    let record: Option<PersistedCategory> = connection
        .query_row(
            "
            SELECT
                category_id,
                branch_id,
                display_name,
                name_key,
                sort_order,
                revision,
                archived_at_utc
            FROM categories
            WHERE category_id = ?1 AND branch_id = ?2
            ",
            params![category_id.to_string(), branch_id.to_string()],
            |row| {
                Ok(PersistedCategory {
                    category_id: row.get(0)?,
                    branch_id: row.get(1)?,
                    display_name: row.get(2)?,
                    name_key: row.get(3)?,
                    sort_order: row.get(4)?,
                    revision: row.get(5)?,
                    archived_at_utc: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::Sql)?;

    record
        .ok_or(StorageError::CategoryNotFound)
        .and_then(category_from_persisted)
}

fn category_from_persisted(record: PersistedCategory) -> Result<Category, StorageError> {
    let category_id = parse_persisted_id(&record.category_id)?;
    let branch_id = parse_persisted_id(&record.branch_id)?;
    let command = CreateCategory::new(&record.display_name, record.sort_order)
        .map_err(invalid_persisted_data)?;

    if command.display_name().key() != record.name_key || record.revision < 1 {
        return Err(StorageError::InvalidPersistedData(
            "category name key or revision did not match its schema contract".to_owned(),
        ));
    }

    Ok(Category::new(
        category_id,
        branch_id,
        command.display_name().clone(),
        command.sort_order(),
        record.revision,
        record.archived_at_utc.is_some(),
    ))
}

fn read_product(
    connection: &Connection,
    branch_id: &EntityId,
    product_id: &EntityId,
) -> Result<Product, StorageError> {
    let record: Option<PersistedProduct> = connection
        .query_row(
            "
            SELECT
                product_id,
                branch_id,
                category_id,
                display_name,
                name_key,
                sku,
                barcode,
                unit_price_minor,
                currency_code,
                is_available,
                sort_order,
                revision,
                archived_at_utc
            FROM products
            WHERE product_id = ?1 AND branch_id = ?2
            ",
            params![product_id.to_string(), branch_id.to_string()],
            |row| {
                Ok(PersistedProduct {
                    product_id: row.get(0)?,
                    branch_id: row.get(1)?,
                    category_id: row.get(2)?,
                    display_name: row.get(3)?,
                    name_key: row.get(4)?,
                    sku: row.get(5)?,
                    barcode: row.get(6)?,
                    unit_price_minor: row.get(7)?,
                    currency_code: row.get(8)?,
                    is_available: row.get(9)?,
                    sort_order: row.get(10)?,
                    revision: row.get(11)?,
                    archived_at_utc: row.get(12)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::Sql)?;

    record
        .ok_or(StorageError::ProductNotFound)
        .and_then(product_from_persisted)
}

fn product_from_persisted(record: PersistedProduct) -> Result<Product, StorageError> {
    let product_id = parse_persisted_id(&record.product_id)?;
    let branch_id = parse_persisted_id(&record.branch_id)?;
    let category_id = record
        .category_id
        .as_deref()
        .map(parse_persisted_id)
        .transpose()?;
    let unit_price = Money::new(record.unit_price_minor, &record.currency_code)
        .map_err(invalid_persisted_data)?;
    let command = CreateProduct::new(
        &record.display_name,
        category_id.clone(),
        unit_price,
        record.sku.as_deref(),
        record.barcode.as_deref(),
        record.sort_order,
    )
    .map_err(invalid_persisted_data)?;

    if command.display_name().key() != record.name_key
        || record.revision < 1
        || !matches!(record.is_available, 0 | 1)
    {
        return Err(StorageError::InvalidPersistedData(
            "product name key, availability, or revision did not match its schema contract"
                .to_owned(),
        ));
    }

    Ok(Product::new(
        product_id,
        branch_id,
        category_id,
        command.display_name().clone(),
        command.unit_price().clone(),
        command.sku().map(ToOwned::to_owned),
        command.barcode().map(ToOwned::to_owned),
        record.is_available == 1,
        command.sort_order(),
        record.revision,
        record.archived_at_utc.is_some(),
    ))
}

fn read_modifier_option(
    connection: &Connection,
    branch_id: &EntityId,
    modifier_option_id: &EntityId,
) -> Result<ModifierOption, StorageError> {
    let record: Option<PersistedModifierOption> = connection
        .query_row(
            "
            SELECT
                modifier_option_id,
                branch_id,
                product_id,
                display_name,
                name_key,
                price_delta_minor,
                currency_code,
                revision,
                archived_at_utc
            FROM product_modifier_options
            WHERE modifier_option_id = ?1 AND branch_id = ?2
            ",
            params![modifier_option_id.to_string(), branch_id.to_string()],
            |row| {
                Ok(PersistedModifierOption {
                    modifier_option_id: row.get(0)?,
                    branch_id: row.get(1)?,
                    product_id: row.get(2)?,
                    display_name: row.get(3)?,
                    name_key: row.get(4)?,
                    price_delta_minor: row.get(5)?,
                    currency_code: row.get(6)?,
                    revision: row.get(7)?,
                    archived_at_utc: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::Sql)?;
    record
        .ok_or(StorageError::ModifierOptionNotFound)
        .and_then(modifier_option_from_persisted)
}

fn modifier_option_from_persisted(
    record: PersistedModifierOption,
) -> Result<ModifierOption, StorageError> {
    let modifier_option_id = parse_persisted_id(&record.modifier_option_id)?;
    let branch_id = parse_persisted_id(&record.branch_id)?;
    let product_id = parse_persisted_id(&record.product_id)?;
    let price_delta = Money::new(record.price_delta_minor, &record.currency_code)
        .map_err(invalid_persisted_data)?;
    let command = CreateModifierOption::new(&record.display_name, price_delta)
        .map_err(invalid_persisted_data)?;
    if command.display_name().key() != record.name_key || record.revision < 1 {
        return Err(StorageError::InvalidPersistedData(
            "modifier option name key or revision did not match its schema contract".to_owned(),
        ));
    }
    Ok(ModifierOption::new(
        modifier_option_id,
        branch_id,
        product_id,
        command.display_name().clone(),
        command.price_delta().clone(),
        record.revision,
        record.archived_at_utc.is_some(),
    ))
}

fn parse_persisted_id(value: &str) -> Result<EntityId, StorageError> {
    EntityId::parse(value).map_err(invalid_persisted_data)
}

fn invalid_persisted_data(error: ros_core::DomainError) -> StorageError {
    StorageError::InvalidPersistedData(error.to_string())
}

fn configure_connection(connection: &Connection, key: &DatabaseKey) -> Result<(), StorageError> {
    ros_sqlcipher_ffi::apply_database_key(connection, key.as_bytes()).map_err(StorageError::Sql)?;

    connection
        .execute_batch(
            "
            PRAGMA cipher_log = NONE;
            PRAGMA cipher_log_level = NONE;
            ",
        )
        .map_err(StorageError::Sql)?;

    read_cipher_version(connection)?;
    read_cipher_status(connection)?;

    require_db_config(
        connection,
        DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY,
        true,
        "foreign-key enforcement",
    )?;
    require_db_config(
        connection,
        DbConfig::SQLITE_DBCONFIG_DEFENSIVE,
        true,
        "defensive mode",
    )?;
    require_db_config(
        connection,
        DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA,
        false,
        "trusted-schema protection",
    )?;

    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = FULL;
            PRAGMA busy_timeout = 5000;
            PRAGMA temp_store = MEMORY;
            PRAGMA secure_delete = ON;
            PRAGMA recursive_triggers = ON;
            ",
        )
        .map_err(StorageError::Sql)?;

    verify_connection_policy(connection)
}

fn read_cipher_version(connection: &Connection) -> Result<String, StorageError> {
    let cipher_version: Option<String> = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .optional()
        .map_err(StorageError::Sql)?;

    let cipher_version = cipher_version
        .filter(|version| !version.trim().is_empty())
        .ok_or(StorageError::CipherUnavailable)?;

    #[cfg(feature = "production-sqlcipher")]
    if !cipher_version.starts_with(PRODUCTION_SQLCIPHER_VERSION_PREFIX) {
        return Err(StorageError::CipherVersionMismatch {
            expected_prefix: PRODUCTION_SQLCIPHER_VERSION_PREFIX,
            actual: cipher_version,
        });
    }

    Ok(cipher_version)
}

fn read_cipher_status(connection: &Connection) -> Result<(), StorageError> {
    let status: Option<String> = connection
        .query_row("PRAGMA cipher_status", [], |row| row.get(0))
        .optional()
        .map_err(StorageError::Sql)?;

    if status.as_deref().is_some_and(|value| value.trim() == "1") {
        Ok(())
    } else {
        Err(StorageError::CipherUnavailable)
    }
}

fn require_db_config(
    connection: &Connection,
    configuration: DbConfig,
    expected: bool,
    policy_name: &'static str,
) -> Result<(), StorageError> {
    let actual = connection
        .set_db_config(configuration, expected)
        .map_err(StorageError::Sql)?;
    if actual != expected {
        return Err(StorageError::ConnectionPolicyRejected(policy_name));
    }

    let read_back = connection
        .db_config(configuration)
        .map_err(StorageError::Sql)?;
    if read_back != expected {
        return Err(StorageError::ConnectionPolicyRejected(policy_name));
    }

    Ok(())
}

fn verify_connection_policy(connection: &Connection) -> Result<(), StorageError> {
    require_integer_pragma(
        connection,
        "PRAGMA synchronous",
        2,
        "full synchronous durability",
    )?;
    require_integer_pragma(connection, "PRAGMA busy_timeout", 5_000, "busy timeout")?;
    require_integer_pragma(
        connection,
        "PRAGMA temp_store",
        2,
        "in-memory temporary storage",
    )?;
    require_integer_pragma(connection, "PRAGMA secure_delete", 1, "secure delete")?;
    require_integer_pragma(
        connection,
        "PRAGMA recursive_triggers",
        1,
        "recursive trigger enforcement",
    )?;

    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(StorageError::Sql)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(StorageError::ConnectionPolicyRejected(
            "write-ahead logging",
        ));
    }

    Ok(())
}

fn require_integer_pragma(
    connection: &Connection,
    pragma: &'static str,
    expected: i64,
    policy_name: &'static str,
) -> Result<(), StorageError> {
    let actual: i64 = connection
        .query_row(pragma, [], |row| row.get(0))
        .map_err(StorageError::Sql)?;
    if actual == expected {
        Ok(())
    } else {
        Err(StorageError::ConnectionPolicyRejected(policy_name))
    }
}

fn begin_immediate_transaction(connection: &Connection) -> Result<Transaction<'_>, StorageError> {
    Transaction::new_unchecked(connection, TransactionBehavior::Immediate)
        .map_err(StorageError::Sql)
}

fn utc_timestamp(transaction: &Transaction<'_>) -> Result<String, StorageError> {
    transaction
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(StorageError::Sql)
}

fn ensure_active_branch(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
) -> Result<String, StorageError> {
    let branch: Option<(String, Option<String>)> = transaction
        .query_row(
            "
            SELECT currency_code, archived_at_utc
            FROM branches
            WHERE branch_id = ?1
            ",
            [branch_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(StorageError::Sql)?;

    match branch {
        Some((_, Some(_))) => Err(StorageError::BranchArchived),
        Some((currency, None)) => Ok(currency),
        None => Err(StorageError::BranchNotFound),
    }
}

fn verify_device_audit_chain_on_connection(
    connection: &Connection,
    device_id: &EntityId,
) -> Result<(), StorageError> {
    let mut statement = connection
        .prepare(
            "
            SELECT
                event_id,
                branch_id,
                actor_id,
                sequence,
                event_type,
                payload_json,
                occurred_at_utc,
                previous_hash,
                event_hash
            FROM audit_events
            WHERE device_id = ?1
            ORDER BY sequence
            ",
        )
        .map_err(StorageError::Sql)?;
    let rows = statement
        .query_map([device_id.to_string()], |row| {
            Ok(PersistedAuditEvent {
                event_id: row.get(0)?,
                branch_id: row.get(1)?,
                actor_id: row.get(2)?,
                sequence: row.get(3)?,
                event_type: row.get(4)?,
                payload_json: row.get(5)?,
                occurred_at_utc: row.get(6)?,
                previous_hash: row.get(7)?,
                event_hash: row.get(8)?,
            })
        })
        .map_err(StorageError::Sql)?;

    let mut expected_sequence = 1_i64;
    let mut previous_hash: Option<Vec<u8>> = None;

    for row in rows {
        let event = row.map_err(StorageError::Sql)?;
        if event.sequence != expected_sequence || event.previous_hash != previous_hash {
            return Err(StorageError::AuditChainInvalid(device_id.to_string()));
        }

        let event_id = parse_persisted_id(&event.event_id)?;
        let branch_id = parse_persisted_id(&event.branch_id)?;
        let actor_id = parse_persisted_id(&event.actor_id)?;
        let expected_hash = audit_event_hash(AuditHashInput {
            event_id: &event_id,
            branch_id: &branch_id,
            actor_id: &actor_id,
            device_id,
            sequence: event.sequence,
            event_type: &event.event_type,
            payload_json: &event.payload_json,
            occurred_at: &event.occurred_at_utc,
            previous_hash: event.previous_hash.as_deref(),
        });

        if event.event_hash.as_slice() != expected_hash.as_slice() {
            return Err(StorageError::AuditChainInvalid(device_id.to_string()));
        }

        previous_hash = Some(event.event_hash);
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| StorageError::AuditChainInvalid(device_id.to_string()))?;
    }

    Ok(())
}

fn integrity_check_on_connection(connection: &Connection) -> Result<(), StorageError> {
    let result: Option<String> = connection
        .query_row("PRAGMA cipher_integrity_check", [], |row| row.get(0))
        .optional()
        .map_err(StorageError::Sql)?;

    // SQLCipher intentionally returns no rows when every page HMAC is valid;
    // each returned row describes a problem.
    if let Some(problem) = result {
        return Err(StorageError::IntegrityCheckFailed(problem));
    }

    Ok(())
}

fn audit_event_count_on_connection(connection: &Connection) -> Result<i64, StorageError> {
    connection
        .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
        .map_err(StorageError::Sql)
}

fn schema_version_on_connection(connection: &Connection) -> Result<i64, StorageError> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(StorageError::Sql)
}

fn verify_local_integrity_on_connection(
    connection: &Connection,
) -> Result<LocalIntegrityReport, StorageError> {
    integrity_check_on_connection(connection)?;
    verify_local_schema_contract(connection)?;
    let foreign_key_problem: Option<String> = connection
        .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
        .optional()
        .map_err(StorageError::Sql)?;
    if let Some(problem) = foreign_key_problem {
        return Err(StorageError::IntegrityCheckFailed(format!(
            "foreign key integrity: {problem}"
        )));
    }
    let mut statement = connection
        .prepare("SELECT DISTINCT device_id FROM audit_events ORDER BY device_id")
        .map_err(StorageError::Sql)?;
    let device_ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(StorageError::Sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::Sql)?;
    for device_id in device_ids {
        verify_device_audit_chain_on_connection(connection, &parse_persisted_id(&device_id)?)?;
    }
    Ok(LocalIntegrityReport {
        schema_version: schema_version_on_connection(connection)?,
        audit_event_count: audit_event_count_on_connection(connection)?,
    })
}

/// Verifies the financial facts that feed the bounded report in addition to
/// the database-wide integrity check. These checks intentionally fail closed
/// if an invoice does not have a complete immutable tender allocation or if a
/// correction exceeds its original sale.
fn verify_financial_export_facts(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
) -> Result<(), StorageError> {
    let branch_currency = ensure_active_branch(transaction, branch_id)?;
    let invoice_problem: bool = transaction
        .query_row(
            "
            SELECT EXISTS (
                SELECT 1
                FROM invoices AS invoice
                WHERE invoice.branch_id = ?1
                  AND (
                    invoice.currency_code <> ?2
                    OR NOT EXISTS (
                        SELECT 1
                        FROM payments AS payment
                        WHERE payment.invoice_id = invoice.invoice_id
                          AND payment.branch_id = invoice.branch_id
                    )
                    OR invoice.total_minor <> COALESCE((
                        SELECT SUM(payment.amount_minor)
                        FROM payments AS payment
                        WHERE payment.invoice_id = invoice.invoice_id
                          AND payment.branch_id = invoice.branch_id
                    ), 0)
                    OR EXISTS (
                        SELECT 1
                        FROM payments AS payment
                        WHERE payment.invoice_id = invoice.invoice_id
                          AND (
                            payment.branch_id <> invoice.branch_id
                            OR payment.currency_code <> invoice.currency_code
                          )
                    )
                    OR COALESCE((
                        SELECT SUM(refund.amount_minor)
                        FROM invoice_refunds AS refund
                        WHERE refund.invoice_id = invoice.invoice_id
                          AND refund.branch_id = invoice.branch_id
                    ), 0) > invoice.total_minor
                    OR EXISTS (
                        SELECT 1
                        FROM invoice_refunds AS refund
                        WHERE refund.invoice_id = invoice.invoice_id
                          AND (
                            refund.branch_id <> invoice.branch_id
                            OR refund.currency_code <> invoice.currency_code
                          )
                    )
                  )
            )
            ",
            params![branch_id.to_string(), &branch_currency],
            |row| row.get(0),
        )
        .map_err(StorageError::Sql)?;
    let expense_problem: bool = transaction
        .query_row(
            "
            SELECT EXISTS (
                SELECT 1
                FROM expenses
                WHERE branch_id = ?1
                  AND (amount_minor <= 0 OR currency_code <> ?2)
            )
            ",
            params![branch_id.to_string(), &branch_currency],
            |row| row.get(0),
        )
        .map_err(StorageError::Sql)?;

    if invoice_problem || expense_problem {
        Err(StorageError::FinancialExportIntegrityMismatch)
    } else {
        Ok(())
    }
}

fn build_bounded_financial_csv(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
) -> Result<VerifiedFinancialCsvExport, StorageError> {
    let aggregate_count: i64 = transaction
        .query_row(
            "
            SELECT COUNT(*)
            FROM (
                SELECT 1
                FROM payments
                WHERE branch_id = ?1
                GROUP BY substr(recorded_at_utc, 1, 10), payment_method, currency_code
                UNION ALL
                SELECT 1
                FROM invoice_refunds
                WHERE branch_id = ?1
                GROUP BY substr(refunded_at_utc, 1, 10), payment_method_snapshot, currency_code
                UNION ALL
                SELECT 1
                FROM expenses
                WHERE branch_id = ?1
                GROUP BY substr(incurred_at_utc, 1, 10), payment_method, currency_code
            )
            ",
            [branch_id.to_string()],
            |row| row.get(0),
        )
        .map_err(StorageError::Sql)?;
    let record_count = aggregate_count
        .checked_add(1)
        .ok_or(StorageError::FinancialAmountOverflow)?;
    if record_count > MAX_FINANCIAL_CSV_RECORDS {
        return Err(StorageError::FinancialExportTooLarge);
    }

    let (gross_sales_minor, refund_minor, expense_minor, currency_code): (i64, i64, i64, String) =
        transaction
            .query_row(
                "
                SELECT
                    (SELECT COALESCE(SUM(total_minor), 0) FROM invoices WHERE branch_id = ?1),
                    (SELECT COALESCE(SUM(amount_minor), 0) FROM invoice_refunds WHERE branch_id = ?1),
                    (SELECT COALESCE(SUM(amount_minor), 0) FROM expenses WHERE branch_id = ?1),
                    (SELECT currency_code FROM branches WHERE branch_id = ?1 AND archived_at_utc IS NULL)
                ",
                [branch_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(StorageError::Sql)?;
    let net_sales_minor = gross_sales_minor
        .checked_sub(refund_minor)
        .ok_or(StorageError::FinancialExportIntegrityMismatch)?;

    let mut writer = BoundedCsvWriter::new();
    writer.append_row(&[
        "record_type",
        "accounting_date_utc",
        "payment_method",
        "gross_sales_minor",
        "refund_minor",
        "net_sales_minor",
        "expense_minor",
        "currency_code",
    ])?;
    let mut written_record_count = 0_i64;

    {
        let mut statement = transaction
            .prepare(
                "
                SELECT
                    substr(recorded_at_utc, 1, 10),
                    payment_method,
                    SUM(amount_minor),
                    currency_code
                FROM payments
                WHERE branch_id = ?1
                GROUP BY substr(recorded_at_utc, 1, 10), payment_method, currency_code
                ORDER BY substr(recorded_at_utc, 1, 10), payment_method, currency_code
                ",
            )
            .map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(StorageError::Sql)?;
        for row in rows {
            let (accounting_date, payment_method, amount_minor, currency) =
                row.map_err(StorageError::Sql)?;
            validate_financial_csv_date(&accounting_date)?;
            validate_financial_csv_payment_method(&payment_method)?;
            validate_financial_csv_currency_code(&currency)?;
            let amount_minor = amount_minor.to_string();
            writer.append_row(&[
                "sale_payment",
                &accounting_date,
                &payment_method,
                &amount_minor,
                "",
                "",
                "",
                &currency,
            ])?;
            written_record_count = written_record_count
                .checked_add(1)
                .ok_or(StorageError::FinancialAmountOverflow)?;
        }
    }

    {
        let mut statement = transaction
            .prepare(
                "
                SELECT
                    substr(refunded_at_utc, 1, 10),
                    payment_method_snapshot,
                    SUM(amount_minor),
                    currency_code
                FROM invoice_refunds
                WHERE branch_id = ?1
                GROUP BY substr(refunded_at_utc, 1, 10), payment_method_snapshot, currency_code
                ORDER BY substr(refunded_at_utc, 1, 10), payment_method_snapshot, currency_code
                ",
            )
            .map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(StorageError::Sql)?;
        for row in rows {
            let (accounting_date, payment_method, amount_minor, currency) =
                row.map_err(StorageError::Sql)?;
            validate_financial_csv_date(&accounting_date)?;
            validate_financial_csv_payment_method(&payment_method)?;
            validate_financial_csv_currency_code(&currency)?;
            let amount_minor = amount_minor.to_string();
            writer.append_row(&[
                "refund",
                &accounting_date,
                &payment_method,
                "",
                &amount_minor,
                "",
                "",
                &currency,
            ])?;
            written_record_count = written_record_count
                .checked_add(1)
                .ok_or(StorageError::FinancialAmountOverflow)?;
        }
    }

    {
        let mut statement = transaction
            .prepare(
                "
                SELECT
                    substr(incurred_at_utc, 1, 10),
                    payment_method,
                    SUM(amount_minor),
                    currency_code
                FROM expenses
                WHERE branch_id = ?1
                GROUP BY substr(incurred_at_utc, 1, 10), payment_method, currency_code
                ORDER BY substr(incurred_at_utc, 1, 10), payment_method, currency_code
                ",
            )
            .map_err(StorageError::Sql)?;
        let rows = statement
            .query_map([branch_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(StorageError::Sql)?;
        for row in rows {
            let (accounting_date, payment_method, amount_minor, currency) =
                row.map_err(StorageError::Sql)?;
            validate_financial_csv_date(&accounting_date)?;
            validate_financial_csv_payment_method(&payment_method)?;
            validate_financial_csv_currency_code(&currency)?;
            let amount_minor = amount_minor.to_string();
            writer.append_row(&[
                "expense",
                &accounting_date,
                &payment_method,
                "",
                "",
                "",
                &amount_minor,
                &currency,
            ])?;
            written_record_count = written_record_count
                .checked_add(1)
                .ok_or(StorageError::FinancialAmountOverflow)?;
        }
    }

    let gross_sales_minor = gross_sales_minor.to_string();
    let refund_minor = refund_minor.to_string();
    let net_sales_minor = net_sales_minor.to_string();
    let expense_minor = expense_minor.to_string();
    validate_financial_csv_currency_code(&currency_code)?;
    writer.append_row(&[
        "summary",
        "",
        "all",
        &gross_sales_minor,
        &refund_minor,
        &net_sales_minor,
        &expense_minor,
        &currency_code,
    ])?;
    written_record_count = written_record_count
        .checked_add(1)
        .ok_or(StorageError::FinancialAmountOverflow)?;

    if written_record_count != record_count {
        return Err(StorageError::FinancialExportIntegrityMismatch);
    }

    Ok(VerifiedFinancialCsvExport {
        csv_bytes: writer.into_bytes(),
        record_count,
    })
}

struct BoundedCsvWriter {
    bytes: Vec<u8>,
    maximum_bytes: usize,
}

impl BoundedCsvWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            maximum_bytes: MAX_FINANCIAL_CSV_BYTES,
        }
    }

    #[cfg(test)]
    fn with_maximum_bytes(maximum_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum_bytes,
        }
    }

    fn append_row(&mut self, fields: &[&str]) -> Result<(), StorageError> {
        let encoded_length = csv_row_encoded_length(fields)?;
        let resulting_length = self
            .bytes
            .len()
            .checked_add(encoded_length)
            .ok_or(StorageError::FinancialExportTooLarge)?;
        if resulting_length > self.maximum_bytes {
            return Err(StorageError::FinancialExportTooLarge);
        }
        self.bytes
            .try_reserve_exact(encoded_length)
            .map_err(|_| StorageError::FinancialExportTooLarge)?;

        for (index, field) in fields.iter().enumerate() {
            if index != 0 {
                self.bytes.push(b',');
            }
            append_rfc4180_csv_field(&mut self.bytes, field);
        }
        self.bytes.extend_from_slice(b"\r\n");
        Ok(())
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

fn csv_row_encoded_length(fields: &[&str]) -> Result<usize, StorageError> {
    let mut length = 2_usize; // RFC 4180 record terminator: CRLF.
    for (index, field) in fields.iter().enumerate() {
        if index != 0 {
            length = length
                .checked_add(1)
                .ok_or(StorageError::FinancialExportTooLarge)?;
        }
        length = length
            .checked_add(rfc4180_csv_field_length(field)?)
            .ok_or(StorageError::FinancialExportTooLarge)?;
    }
    Ok(length)
}

fn rfc4180_csv_field_length(field: &str) -> Result<usize, StorageError> {
    if !csv_field_requires_quotes(field) {
        return Ok(field.len());
    }

    let quote_count = field.bytes().filter(|byte| *byte == b'"').count();
    field
        .len()
        .checked_add(quote_count)
        .and_then(|length| length.checked_add(2))
        .ok_or(StorageError::FinancialExportTooLarge)
}

fn csv_field_requires_quotes(field: &str) -> bool {
    field
        .bytes()
        .any(|byte| matches!(byte, b',' | b'"' | b'\r' | b'\n'))
}

fn append_rfc4180_csv_field(output: &mut Vec<u8>, field: &str) {
    if !csv_field_requires_quotes(field) {
        output.extend_from_slice(field.as_bytes());
        return;
    }

    output.push(b'"');
    for byte in field.bytes() {
        if byte == b'"' {
            output.push(b'"');
        }
        output.push(byte);
    }
    output.push(b'"');
}

fn validate_accounting_date_utc(value: &str) -> Result<(), StorageError> {
    let bytes = value.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(StorageError::InvalidPersistedData(
            "accounting date must be YYYY-MM-DD UTC".to_owned(),
        ))
    }
}

// Dynamic cells are intentionally restricted to storage-owned dates, tender
// names, ISO currency codes, and decimal integers. This prevents a corrupted
// database record from becoming a spreadsheet formula while the RFC-4180
// encoder below still safely handles commas, quotes, and record terminators.
fn validate_financial_csv_date(value: &str) -> Result<(), StorageError> {
    validate_accounting_date_utc(value).map_err(|_| StorageError::FinancialExportIntegrityMismatch)
}

fn validate_financial_csv_payment_method(value: &str) -> Result<(), StorageError> {
    if matches!(value, "cash" | "card" | "upi") {
        Ok(())
    } else {
        Err(StorageError::FinancialExportIntegrityMismatch)
    }
}

fn validate_financial_csv_currency_code(value: &str) -> Result<(), StorageError> {
    if value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(StorageError::FinancialExportIntegrityMismatch)
    }
}

/// Owner and manager authority is required for catalog, inventory, refunds,
/// expenses, and cash reconciliation. The function keeps its historical name
/// as a compatibility shim for the earlier inventory-only callers.
fn require_management_authority(context: &MutationContext) -> Result<(), StorageError> {
    match context.actor_role() {
        ActorRole::Owner | ActorRole::Manager => Ok(()),
        ActorRole::Cashier | ActorRole::Kitchen => Err(StorageError::PermissionDenied),
    }
}

fn require_owner_authority(context: &MutationContext) -> Result<(), StorageError> {
    if context.actor_role() == ActorRole::Owner {
        Ok(())
    } else {
        Err(StorageError::PermissionDenied)
    }
}

fn require_inventory_control_authority(context: &MutationContext) -> Result<(), StorageError> {
    require_management_authority(context)
}

#[allow(clippy::too_many_arguments)]
fn record_dual_person_approval(
    transaction: &Transaction<'_>,
    requester: &MutationContext,
    approval: &DualPersonApproval<'_>,
    action_type: &str,
    target_entity_type: &str,
    target_entity_id: &EntityId,
    amount_minor: Option<i64>,
    currency_code: Option<&str>,
    reason: &MutationReason,
    resulting_entity_type: &str,
    resulting_entity_id: &EntityId,
    occurred_at: &str,
) -> Result<(), StorageError> {
    if approval.approver_actor_id == *requester.actor_id() {
        return Err(StorageError::PermissionDenied);
    }
    validate_local_staff_pin(approval.approver_pin)?;
    let approver = transaction
        .query_row(
            "
            SELECT
                COALESCE((
                    SELECT role.role
                    FROM staff_role_events AS role
                    JOIN local_security_fact_order AS role_order
                        ON role_order.fact_kind = 'staff_role'
                        AND role_order.fact_id = role.staff_role_event_id
                    WHERE role.staff_id = staff.staff_id
                    ORDER BY role_order.fact_sequence DESC
                    LIMIT 1
                ), staff.role) AS effective_role,
                COALESCE((
                    SELECT status.status = 'active'
                    FROM staff_status_events AS status
                    JOIN local_security_fact_order AS status_order
                        ON status_order.fact_kind = 'staff_status'
                        AND status_order.fact_id = status.staff_status_event_id
                    WHERE status.staff_id = staff.staff_id
                    ORDER BY status_order.fact_sequence DESC
                    LIMIT 1
                ), 0) AS is_active
            FROM staff_accounts AS staff
            WHERE staff.staff_id = ?1 AND staff.branch_id = ?2
            ",
            params![
                approval.approver_actor_id.to_string(),
                requester.branch_id().to_string()
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(StorageError::Sql)?
        .ok_or(StorageError::StaffNotFound)?;
    let (approver_role, is_active) = approver;
    if is_active == 0 {
        return Err(StorageError::StaffNotActive);
    }
    if !matches!(approver_role.as_str(), "owner" | "manager") {
        return Err(StorageError::PermissionDenied);
    }
    let digest: String = transaction
        .query_row(
            "
            SELECT argon2id_hash
            FROM staff_pin_credentials
            WHERE staff_id = ?1
            ORDER BY created_at_utc DESC, staff_pin_credential_id DESC
            LIMIT 1
            ",
            [approval.approver_actor_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::Sql)?
        .ok_or(StorageError::StaffCredentialUnavailable)?;
    if !verify_local_staff_pin(approval.approver_pin, &digest) {
        return Err(StorageError::InvalidStaffPin);
    }
    let request_id = EntityId::new_v7();
    let decision_id = EntityId::new_v7();
    let consumption_id = EntityId::new_v7();
    transaction
        .execute(
            "
            INSERT INTO correction_approval_requests (
                approval_request_id, branch_id, action_type, target_entity_type,
                target_entity_id, amount_minor, currency_code, policy_version,
                reason, requested_at_utc, requested_by_actor_id, expires_at_utc
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, 'correction-approval.v1',
                ?8, ?9, ?10, strftime('%Y-%m-%dT%H:%M:%fZ', ?9, '+15 minutes')
            )
            ",
            params![
                request_id.to_string(),
                requester.branch_id().to_string(),
                action_type,
                target_entity_type,
                target_entity_id.to_string(),
                amount_minor,
                currency_code,
                reason.as_str(),
                occurred_at,
                requester.actor_id().to_string(),
            ],
        )
        .map_err(StorageError::Sql)?;
    transaction
        .execute(
            "
            INSERT INTO correction_approval_decisions (
                approval_decision_id, approval_request_id, branch_id, decision,
                decided_at_utc, decided_by_actor_id
            ) VALUES (?1, ?2, ?3, 'approved', ?4, ?5)
            ",
            params![
                decision_id.to_string(),
                request_id.to_string(),
                requester.branch_id().to_string(),
                occurred_at,
                approval.approver_actor_id.to_string(),
            ],
        )
        .map_err(StorageError::Sql)?;
    transaction
        .execute(
            "
            INSERT INTO correction_approval_consumptions (
                approval_consumption_id, approval_request_id, branch_id,
                consumed_at_utc, consumed_by_actor_id, resulting_entity_type,
                resulting_entity_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                consumption_id.to_string(),
                request_id.to_string(),
                requester.branch_id().to_string(),
                occurred_at,
                requester.actor_id().to_string(),
                resulting_entity_type,
                resulting_entity_id.to_string(),
            ],
        )
        .map_err(StorageError::Sql)?;
    Ok(())
}

fn require_counter_authority(context: &MutationContext) -> Result<(), StorageError> {
    match context.actor_role() {
        ActorRole::Owner | ActorRole::Manager | ActorRole::Cashier => Ok(()),
        ActorRole::Kitchen => Err(StorageError::PermissionDenied),
    }
}

fn require_kitchen_authority(context: &MutationContext) -> Result<(), StorageError> {
    match context.actor_role() {
        ActorRole::Owner | ActorRole::Manager | ActorRole::Kitchen => Ok(()),
        ActorRole::Cashier => Err(StorageError::PermissionDenied),
    }
}

/// Reads a human-facing invoice sequence under the sale transaction's
/// `BEGIN IMMEDIATE` lock. The sequence advances only after its invoice has
/// been inserted, letting database constraints bind the allocator to durable
/// financial facts.
fn next_invoice_number(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
) -> Result<i64, StorageError> {
    let branch_id = branch_id.to_string();
    let existing: Option<i64> = transaction
        .query_row(
            "
            SELECT next_invoice_number
            FROM branch_document_sequences
            WHERE branch_id = ?1
            ",
            [&branch_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::Sql)?;

    let invoice_number = match existing {
        Some(invoice_number) => invoice_number,
        None => {
            transaction
                .execute(
                    "
                    INSERT INTO branch_document_sequences (branch_id, next_invoice_number)
                    VALUES (?1, 1)
                    ",
                    [&branch_id],
                )
                .map_err(StorageError::Sql)?;
            1
        }
    };

    if invoice_number == i64::MAX {
        return Err(StorageError::InvoiceNumberExhausted);
    }

    Ok(invoice_number)
}

fn advance_invoice_number(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
    invoice_number: i64,
) -> Result<(), StorageError> {
    let updated = transaction
        .execute(
            "
            UPDATE branch_document_sequences
            SET next_invoice_number = next_invoice_number + 1
            WHERE branch_id = ?1 AND next_invoice_number = ?2
            ",
            params![branch_id.to_string(), invoice_number],
        )
        .map_err(StorageError::Sql)?;
    if updated != 1 {
        return Err(StorageError::InvoiceNumberExhausted);
    }

    Ok(())
}

fn ensure_active_category(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
    category_id: &EntityId,
) -> Result<(), StorageError> {
    let category: Option<Option<String>> = transaction
        .query_row(
            "
            SELECT archived_at_utc
            FROM categories
            WHERE category_id = ?1 AND branch_id = ?2
            ",
            params![category_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::Sql)?;

    match category {
        Some(Some(_)) => Err(StorageError::CategoryArchived),
        Some(None) => Ok(()),
        None => Err(StorageError::CategoryNotFound),
    }
}

fn ensure_active_customer(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
    customer_id: &EntityId,
) -> Result<(), StorageError> {
    let active: bool = transaction
        .query_row(
            "
            SELECT EXISTS(
                SELECT 1
                FROM customers AS customer
                JOIN customer_profile_revisions AS profile
                    ON profile.customer_id = customer.customer_id
                WHERE customer.customer_id = ?1
                  AND customer.branch_id = ?2
                  AND profile.revision = (
                    SELECT MAX(latest.revision)
                    FROM customer_profile_revisions AS latest
                    WHERE latest.customer_id = customer.customer_id
                  )
                  AND profile.profile_state = 'active'
            )
            ",
            params![customer_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .map_err(StorageError::Sql)?;
    if active {
        Ok(())
    } else {
        Err(StorageError::CustomerUnavailable)
    }
}

fn has_retained_product_history(
    transaction: &Transaction<'_>,
    branch_id: &EntityId,
    product_id: &EntityId,
) -> Result<bool, StorageError> {
    transaction
        .query_row(
            "
            SELECT EXISTS(
                SELECT 1
                FROM order_lines
                WHERE order_lines.product_id = ?1
            )
            OR EXISTS(
                SELECT 1
                FROM product_image_versions
                WHERE product_image_versions.branch_id = ?2
                  AND product_image_versions.product_id = ?1
            )
            OR EXISTS(
                SELECT 1
                FROM product_modifier_options
                WHERE product_modifier_options.branch_id = ?2
                  AND product_modifier_options.product_id = ?1
            )
            OR EXISTS(
                SELECT 1
                FROM sync_outbox_events
                JOIN audit_events
                    ON audit_events.event_id = sync_outbox_events.audit_event_id
                WHERE audit_events.branch_id = ?2
                  AND CASE
                      WHEN json_valid(audit_events.payload_json)
                          THEN json_extract(audit_events.payload_json, '$.entity_id')
                      ELSE NULL
                  END = ?1
            )
            ",
            params![product_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .map_err(StorageError::Sql)
}

fn append_hashed_audit_event(
    transaction: &Transaction<'_>,
    context: &MutationContext,
    event_type: &str,
    payload_json: &str,
) -> Result<EntityId, StorageError> {
    // Every normal audited mutation crosses this common boundary before its
    // transaction can commit. Bootstrap PIN setup, verified unlock, and the
    // explicit lock transition use the narrowly scoped unchecked helper
    // below because a current session cannot (or must not) authorize them.
    revalidate_mutation_context(transaction, context)?;
    append_hashed_audit_event_without_session(transaction, context, event_type, payload_json)
}

fn append_hashed_audit_event_without_session(
    transaction: &Transaction<'_>,
    context: &MutationContext,
    event_type: &str,
    payload_json: &str,
) -> Result<EntityId, StorageError> {
    let previous: Option<(i64, Vec<u8>)> = transaction
        .query_row(
            "
            SELECT sequence, event_hash
            FROM audit_events
            WHERE device_id = ?1
            ORDER BY sequence DESC
            LIMIT 1
            ",
            [context.device_id().to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(StorageError::Sql)?;

    let (sequence, previous_hash) = match previous {
        Some((last_sequence, hash)) => (
            last_sequence
                .checked_add(1)
                .ok_or_else(|| StorageError::AuditChainInvalid(context.device_id().to_string()))?,
            Some(hash),
        ),
        None => (1, None),
    };

    let event_id = EntityId::new_v7();
    let occurred_at = utc_timestamp(transaction)?;
    let event_hash = audit_event_hash(AuditHashInput {
        event_id: &event_id,
        branch_id: context.branch_id(),
        actor_id: context.actor_id(),
        device_id: context.device_id(),
        sequence,
        event_type,
        payload_json,
        occurred_at: &occurred_at,
        previous_hash: previous_hash.as_deref(),
    });

    transaction
        .execute(
            "
            INSERT INTO audit_events (
                event_id,
                branch_id,
                actor_id,
                device_id,
                sequence,
                event_type,
                payload_json,
                occurred_at_utc,
                previous_hash,
                event_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                event_id.to_string(),
                context.branch_id().to_string(),
                context.actor_id().to_string(),
                context.device_id().to_string(),
                sequence,
                event_type,
                payload_json,
                occurred_at,
                previous_hash,
                event_hash.as_slice(),
            ],
        )
        .map_err(StorageError::Sql)?;

    Ok(event_id)
}

/// Queues an immutable audit envelope for later Professional synchronization.
/// Delivery acknowledgement is modeled separately so neither the audit record
/// nor the original outbox operation needs to be mutated.
fn append_sync_outbox_event(
    transaction: &Transaction<'_>,
    context: &MutationContext,
    audit_event_id: &EntityId,
    entity_type: &str,
    entity_id: &EntityId,
    event_type: &str,
    created_at: &str,
) -> Result<EntityId, StorageError> {
    let operation_id = EntityId::new_v7();
    transaction
        .execute(
            "
            INSERT INTO sync_outbox_events (
                operation_id,
                audit_event_id,
                branch_id,
                device_id,
                entity_type,
                entity_id,
                event_type,
                correlation_id,
                created_at_utc
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                operation_id.to_string(),
                audit_event_id.to_string(),
                context.branch_id().to_string(),
                context.device_id().to_string(),
                entity_type,
                entity_id.to_string(),
                event_type,
                context.correlation_id().to_string(),
                created_at,
            ],
        )
        .map_err(StorageError::Sql)?;

    Ok(operation_id)
}

struct AuditHashInput<'input> {
    event_id: &'input EntityId,
    branch_id: &'input EntityId,
    actor_id: &'input EntityId,
    device_id: &'input EntityId,
    sequence: i64,
    event_type: &'input str,
    payload_json: &'input str,
    occurred_at: &'input str,
    previous_hash: Option<&'input [u8]>,
}

fn audit_event_hash(input: AuditHashInput<'_>) -> [u8; 32] {
    let event_id = input.event_id.to_string();
    let branch_id = input.branch_id.to_string();
    let actor_id = input.actor_id.to_string();
    let device_id = input.device_id.to_string();
    ros_core::audit_event_hash(ros_core::AuditEventHashInput {
        event_id: &event_id,
        branch_id: &branch_id,
        actor_id: &actor_id,
        device_id: &device_id,
        sequence: input.sequence,
        event_type: input.event_type,
        payload_json: input.payload_json,
        occurred_at_utc: input.occurred_at,
        previous_hash: input.previous_hash,
    })
}

fn migrate(connection: &Connection) -> Result<(), StorageError> {
    // The checksum manifest is a release-time security boundary, not merely
    // bookkeeping stored alongside a migration. Verify that every embedded
    // source blob still hashes to its reviewed manifest value before reading
    // or mutating a local database.
    verify_local_migration_manifest()?;

    let current_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(StorageError::Sql)?;

    if current_version > SCHEMA_VERSION {
        return Err(StorageError::UnsupportedSchemaVersion(current_version));
    }

    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)
        .map_err(StorageError::Sql)?;

    for migration in &LOCAL_MIGRATIONS {
        let recorded_checksum: Option<String> = if migration.version == 1 && current_version == 0 {
            None
        } else {
            transaction
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| row.get(0),
                )
                .optional()
                .map_err(StorageError::Sql)?
        };

        if migration.version <= current_version {
            verify_recorded_migration(migration, recorded_checksum.as_deref())?;
            continue;
        }

        if recorded_checksum.is_some() {
            return Err(StorageError::MigrationHistoryInvalid(migration.version));
        }

        transaction
            .execute_batch(migration.sql)
            .map_err(StorageError::Sql)?;

        transaction
            .execute(
                "
                INSERT INTO schema_migrations (
                    version,
                    applied_at_utc,
                    checksum
                ) VALUES (
                    ?1,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    ?2
                )
                ",
                params![migration.version, migration.checksum],
            )
            .map_err(StorageError::Sql)?;

        transaction
            .execute_batch(&format!("PRAGMA user_version = {};", migration.version))
            .map_err(StorageError::Sql)?;
    }

    transaction.commit().map_err(StorageError::Sql)?;
    verify_local_schema_contract(connection)
}

fn verify_local_migration_manifest() -> Result<(), StorageError> {
    for migration in &LOCAL_MIGRATIONS {
        verify_migration_source_checksum(migration)?;
    }
    Ok(())
}

fn verify_migration_source_checksum(migration: &LocalMigration) -> Result<(), StorageError> {
    let actual = migration_source_checksum(migration.sql);
    if actual == migration.checksum {
        Ok(())
    } else {
        Err(StorageError::MigrationSourceChecksumMismatch {
            version: migration.version,
            expected: migration.checksum,
            actual,
        })
    }
}

fn migration_source_checksum(sql: &str) -> String {
    format!("sha256:{}", lowercase_hex(&Sha256::digest(sql.as_bytes())))
}

/// Migration checksums prove which migration source was recorded, while this
/// additional contract check proves that security-critical objects actually
/// exist with their expected constraints. It prevents `CREATE ... IF NOT
/// EXISTS` from silently stamping a pre-existing partial table as a valid
/// migration.
fn verify_local_schema_contract(connection: &Connection) -> Result<(), StorageError> {
    for (name, fragments, policy_name) in [
        (
            "audit_events",
            &["strict", "unique(device_id,sequence)"][..],
            "audit events table contract",
        ),
        (
            "branches",
            &["strict", "referencesorganizations(organization_id)"][..],
            "branches table contract",
        ),
        (
            "products",
            &[
                "strict",
                "foreignkey(branch_id,category_id)referencescategories(branch_id,category_id)",
                "check(tax_treatmentin('no_tax','exclusive','inclusive'))",
            ][..],
            "products table contract",
        ),
        (
            "product_image_versions",
            &[
                "strict",
                "check(source_kindin('built_in','restaurant_upload'))",
                "unique(branch_id,product_id,image_version_id)",
            ][..],
            "product image versions table contract",
        ),
        (
            "product_image_assignments",
            &[
                "strict",
                "referencesproduct_image_versions(branch_id,product_id,image_version_id)",
                "unique(branch_id,product_id)",
            ][..],
            "product image assignments table contract",
        ),
        (
            "product_image_catalog_provenance",
            &[
                "strict",
                "original_content_sha256blobnotnull",
                "service_origin='https://ros.gotigin.com'",
                "service_schema_version=1",
                "audit_event_idtextnotnullunique",
                "referencesproduct_image_versions(branch_id,product_id,image_version_id)",
                "referencesaudit_events(event_id)",
            ][..],
            "product image catalogue provenance table contract",
        ),
        (
            "product_deletion_authorizations",
            &[
                "strict",
                "audit_event_idtextnotnullunique",
                "referencesaudit_events(event_id)",
            ][..],
            "product deletion authorization table contract",
        ),
        (
            "branch_document_sequences",
            &[
                "strict",
                "check(next_invoice_numberbetween1and9223372036854775807)",
            ][..],
            "invoice sequence table contract",
        ),
        (
            "orders",
            &[
                "strict",
                "check(order_typein('dine_in','takeaway'))",
                "check(order_statein('open','finalized','void'))",
                "discount_minorintegernotnulldefault0",
                "tax_minorintegernotnulldefault0",
                "pricing_snapshot_jsontextnotnulldefault'{}'",
            ][..],
            "orders table contract",
        ),
        (
            "order_lines",
            &[
                "strict",
                "unique(order_id,line_number)",
                "modifier_total_minorintegernotnulldefault0",
                "modifier_snapshot_jsontextnotnulldefault'[]'",
            ][..],
            "order lines table contract",
        ),
        (
            "product_modifier_options",
            &[
                "strict",
                "check(price_delta_minor>=0)",
                "archive_audit_event_idtextunique",
                "referencesproducts(product_id)",
            ][..],
            "product modifier options table contract",
        ),
        (
            "restaurant_tables",
            &["strict", "unique(branch_id,name_key)"][..],
            "restaurant tables table contract",
        ),
        (
            "draft_orders",
            &[
                "strict",
                "check(fulfillmentin('dine_in','takeaway'))",
                "check(draft_statein('open','sent_to_kitchen','cancelled'))",
            ][..],
            "draft orders table contract",
        ),
        (
            "draft_order_revisions",
            &[
                "strict",
                "unique(draft_order_id,revision)",
                "kitchen_notetext",
            ][..],
            "draft order revisions table contract",
        ),
        (
            "draft_order_settlements",
            &["strict", "order_idtextnotnullunique"][..],
            "draft order settlement table contract",
        ),
        (
            "kitchen_tickets",
            &[
                "strict",
                "check(ticket_statein('new','preparing','ready','completed'))",
                "unique(draft_order_id,draft_revision)",
                "kitchen_note_snapshottext",
            ][..],
            "kitchen tickets table contract",
        ),
        (
            "kitchen_ticket_cancellation_notices",
            &[
                "strict",
                "kitchen_ticket_idtextnotnullunique",
                "reasontextnotnull",
                "audit_event_idtextnotnullunique",
                "referencesdraft_order_revisions(draft_order_id,revision)",
            ][..],
            "kitchen ticket cancellation notice table contract",
        ),
        (
            "kitchen_ticket_cancellation_acknowledgements",
            &[
                "strict",
                "kitchen_ticket_cancellation_notice_idtextnotnullunique",
                "audit_event_idtextnotnullunique",
            ][..],
            "kitchen ticket cancellation acknowledgement table contract",
        ),
        (
            "invoice_refunds",
            &[
                "strict",
                "check(payment_method_snapshotin('cash','card','upi'))",
                "check(amount_minor>0)",
            ][..],
            "invoice refunds table contract",
        ),
        (
            "invoice_voids",
            &["strict", "invoice_idtextnotnullunique", "reasontextnotnull"][..],
            "invoice voids table contract",
        ),
        (
            "correction_approval_requests",
            &[
                "strict",
                "policy_version='correction-approval.v1'",
                "check(action_typein('refund','void','discount','stock_adjustment'))",
            ][..],
            "correction approval requests table contract",
        ),
        (
            "correction_approval_decisions",
            &["strict", "approval_request_idtextnotnullunique"][..],
            "correction approval decisions table contract",
        ),
        (
            "correction_approval_consumptions",
            &["strict", "approval_request_idtextnotnullunique"][..],
            "correction approval consumptions table contract",
        ),
        (
            "suppliers",
            &["strict", "unique(branch_id,name_key)"][..],
            "suppliers table contract",
        ),
        (
            "purchase_documents",
            &["strict", "reasontextnotnull"][..],
            "purchase documents table contract",
        ),
        (
            "purchase_document_lines",
            &["strict", "check(quantity>0)"][..],
            "purchase document lines table contract",
        ),
        (
            "product_recipes",
            &["strict", "finished_product_idtextnotnull"][..],
            "product recipes table contract",
        ),
        (
            "product_recipe_lines",
            &["strict", "check(quantity_per_unit>0)"][..],
            "product recipe lines table contract",
        ),
        (
            "owner_recovery_verifiers",
            &["strict", "argon2id_hashtextnotnull"][..],
            "owner recovery verifiers table contract",
        ),
        (
            "inventory_movements",
            &[
                "strict",
                "check(movement_typein('opening','purchase','sale','waste','adjustment'))",
                "check(quantity_delta<>0)",
            ][..],
            "inventory movements table contract",
        ),
        (
            "expenses",
            &[
                "strict",
                "check(amount_minor>0)",
                "check(payment_methodin('cash','card','upi'))",
            ][..],
            "expenses table contract",
        ),
        (
            "cash_drawer_sessions",
            &["strict", "check(opening_cash_minor>=0)"][..],
            "cash drawer sessions table contract",
        ),
        (
            "cash_drawer_closures",
            &[
                "strict",
                "cash_drawer_session_idtextnotnullunique",
                "check(counted_cash_minor>=0)",
            ][..],
            "cash drawer closures table contract",
        ),
        (
            "invoices",
            &[
                "strict",
                "order_idtextnotnullunique",
                "unique(branch_id,invoice_number)",
                "discount_minorintegernotnulldefault0",
                "tax_minorintegernotnulldefault0",
                "pricing_snapshot_jsontextnotnulldefault'{}'",
            ][..],
            "invoices table contract",
        ),
        (
            "branch_tax_rates",
            &[
                "strict",
                "check(basis_pointsbetween0and10000)",
                "archive_audit_event_idtextunique",
            ][..],
            "branch tax rates table contract",
        ),
        (
            "payments",
            &["strict", "referencesinvoices(invoice_id)"][..],
            "payments table contract",
        ),
        (
            "sync_outbox_events",
            &["strict", "audit_event_idtextnotnullunique"][..],
            "sync outbox table contract",
        ),
        (
            "local_installation_identity",
            &["strict", "check(singleton=1)"][..],
            "local installation identity table contract",
        ),
        (
            "staff_accounts",
            &[
                "strict",
                "check(rolein('owner','manager','cashier','kitchen'))",
                "unique(branch_id,name_key)",
            ][..],
            "staff account table contract",
        ),
        (
            "staff_pin_credentials",
            &["strict", "argon2id_hashtextnotnull"][..],
            "staff credential table contract",
        ),
        (
            "staff_status_events",
            &["strict", "check(statusin('active','revoked'))"][..],
            "staff status table contract",
        ),
        (
            "staff_role_events",
            &[
                "strict",
                "check(rolein('manager','cashier','kitchen'))",
                "reasontextnotnull",
            ][..],
            "staff role history table contract",
        ),
        (
            "inventory_low_stock_threshold_events",
            &[
                "strict",
                "check(threshold_quantity>=0)",
                "reasontextnotnull",
            ][..],
            "low-stock threshold table contract",
        ),
        (
            "inventory_low_stock_threshold_clear_events",
            &["strict", "reasontextnotnull"][..],
            "low-stock threshold clear table contract",
        ),
        (
            "local_staff_session_events",
            &[
                "strict",
                "check(event_typein('unlocked','locked'))",
                "referencesaudit_events(event_id)",
            ][..],
            "staff session table contract",
        ),
        (
            "staff_pin_attempts",
            &["strict", "check(succeededin(0,1))"][..],
            "staff PIN attempt table contract",
        ),
        (
            "local_security_fact_order",
            &[
                "strict",
                "fact_sequenceintegerprimarykeyautoincrement",
                "unique(fact_kind,fact_id)",
            ][..],
            "local security fact ordering table contract",
        ),
        (
            "local_security_clock_state",
            &["strict", "check(singleton=1)", "high_water_utctextnotnull"][..],
            "local security clock high-water table contract",
        ),
        (
            "customers",
            &["strict", "referencesbranches(branch_id)"][..],
            "customer identity table contract",
        ),
        (
            "customer_profile_revisions",
            &[
                "strict",
                "check(profile_statein('active','anonymized'))",
                "unique(customer_id,revision)",
            ][..],
            "customer profile table contract",
        ),
    ] {
        require_schema_fragments(connection, "table", name, fragments, policy_name)?;
    }

    for name in [
        "audit_events_cannot_be_updated",
        "audit_events_cannot_be_deleted",
        "categories_cannot_be_deleted",
        "products_cannot_be_deleted",
        "product_modifier_options_must_match_product_and_branch",
        "product_modifier_options_immutable_facts",
        "product_modifier_options_archive_only",
        "product_modifier_options_cannot_be_deleted",
        "product_image_versions_cannot_be_updated",
        "product_image_versions_cannot_be_deleted",
        "product_image_assignments_cannot_be_deleted",
        "product_image_assignments_identity_is_immutable",
        "product_image_assignments_must_advance_revision",
        "product_image_catalog_provenance_cannot_be_updated",
        "product_image_catalog_provenance_cannot_be_deleted",
        "product_deletion_authorizations_cannot_be_updated",
        "product_deletion_authorizations_cannot_be_deleted",
        "branch_document_sequences_cannot_be_deleted",
        "branch_document_sequences_must_start_after_last_invoice",
        "branch_document_sequences_must_increment",
        "orders_cannot_be_deleted",
        "finalized_orders_cannot_be_updated",
        "order_lines_cannot_be_updated",
        "order_lines_cannot_be_deleted",
        "restaurant_tables_cannot_be_deleted",
        "draft_orders_cannot_be_deleted",
        "draft_order_revisions_cannot_be_updated",
        "draft_order_revisions_cannot_be_deleted",
        "draft_order_settlements_cannot_be_updated",
        "draft_order_settlements_cannot_be_deleted",
        "kitchen_tickets_cannot_be_deleted",
        "kitchen_tickets_must_match_draft_revision",
        "kitchen_ticket_note_snapshot_is_immutable",
        "kitchen_tickets_kitchen_note_must_match_draft_revision",
        "kitchen_ticket_cancellation_notices_cannot_be_updated",
        "kitchen_ticket_cancellation_notices_cannot_be_deleted",
        "kitchen_ticket_cancellation_notices_must_match_ticket_and_audit",
        "kitchen_ticket_cancellation_acknowledgements_cannot_be_updated",
        "kitchen_ticket_cancellation_acknowledgements_cannot_be_deleted",
        "kitchen_ticket_cancellation_acknowledgements_must_match_notice_and_audit",
        "invoice_refunds_cannot_be_updated",
        "invoice_refunds_cannot_be_deleted",
        "invoice_refunds_must_match_invoice",
        "invoice_voids_cannot_be_updated",
        "invoice_voids_cannot_be_deleted",
        "invoice_voids_must_match_invoice_without_refunds",
        "invoice_refunds_cannot_follow_void",
        "correction_approval_requests_cannot_be_updated",
        "correction_approval_requests_cannot_be_deleted",
        "correction_approval_decisions_cannot_be_updated",
        "correction_approval_decisions_cannot_be_deleted",
        "correction_approval_consumptions_cannot_be_updated",
        "correction_approval_consumptions_cannot_be_deleted",
        "correction_approval_decisions_require_distinct_actor",
        "suppliers_cannot_be_deleted",
        "purchase_documents_cannot_be_updated",
        "purchase_documents_cannot_be_deleted",
        "purchase_document_lines_cannot_be_updated",
        "purchase_document_lines_cannot_be_deleted",
        "product_recipes_cannot_be_deleted",
        "product_recipe_lines_cannot_be_updated",
        "product_recipe_lines_cannot_be_deleted",
        "product_recipe_lines_reject_self_ingredient",
        "owner_recovery_verifiers_cannot_be_deleted",
        "inventory_movements_cannot_be_updated",
        "inventory_movements_cannot_be_deleted",
        "inventory_movements_valid_direction",
        "inventory_movements_no_negative_balance",
        "expenses_cannot_be_updated",
        "expenses_cannot_be_deleted",
        "cash_drawer_sessions_cannot_be_updated",
        "cash_drawer_sessions_cannot_be_deleted",
        "cash_drawer_closures_cannot_be_updated",
        "cash_drawer_closures_cannot_be_deleted",
        "invoices_cannot_be_updated",
        "invoices_cannot_be_deleted",
        "payments_cannot_be_updated",
        "payments_cannot_be_deleted",
        "sync_outbox_events_cannot_be_deleted",
        "sync_outbox_events_cannot_be_updated",
        "sync_acknowledgements_cannot_be_updated",
        "sync_acknowledgements_cannot_be_deleted",
        "order_lines_must_match_order_and_product",
        "invoices_must_match_finalized_order",
        "payments_must_match_finalized_invoice",
        "sync_outbox_must_match_audit_identity",
        "staff_accounts_cannot_be_updated",
        "staff_accounts_cannot_be_deleted",
        "staff_pin_credentials_cannot_be_updated",
        "staff_pin_credentials_cannot_be_deleted",
        "staff_status_events_cannot_be_updated",
        "staff_status_events_cannot_be_deleted",
        "staff_role_events_cannot_be_updated",
        "staff_role_events_cannot_be_deleted",
        "inventory_low_stock_threshold_events_cannot_be_updated",
        "inventory_low_stock_threshold_events_cannot_be_deleted",
        "inventory_low_stock_threshold_clear_events_cannot_be_updated",
        "inventory_low_stock_threshold_clear_events_cannot_be_deleted",
        "local_staff_session_events_cannot_be_updated",
        "local_staff_session_events_cannot_be_deleted",
        "staff_pin_attempts_cannot_be_updated",
        "staff_pin_attempts_cannot_be_deleted",
        "local_security_fact_order_cannot_be_updated",
        "local_security_fact_order_cannot_be_deleted",
        "local_security_clock_state_monotonic_update_only",
        "local_security_clock_state_cannot_be_deleted",
        "customers_cannot_be_updated",
        "customers_cannot_be_deleted",
        "customer_profile_revisions_cannot_be_updated",
        "customer_profile_revisions_cannot_be_deleted",
    ] {
        require_schema_fragments(
            connection,
            "trigger",
            name,
            &["raise(abort,"],
            "security trigger contract",
        )?;
    }

    for (name, fragments, policy_name) in [
        (
            "product_image_catalog_provenance_must_match_version_and_audit",
            &[
                "image_version.source_kind='restaurant_upload'",
                "audit.event_typein('catalog.product.image.assigned','catalog.product.image.replaced')",
                "json_extract(audit.payload_json,'$.after.source_kind')='gotigin_catalog'",
                "json_extract(audit.payload_json,'$.after.catalog.image_id')=new.catalog_image_id",
                "json_extract(audit.payload_json,'$.after.catalog.original_content_sha256')=lower(hex(new.original_content_sha256))",
                "json_extract(audit.payload_json,'$.after.catalog.licence_label')=new.licence_label",
                "json_extract(audit.payload_json,'$.after.catalog.licence_url')=new.licence_url",
                "json_extract(audit.payload_json,'$.after.catalog.service_origin')=new.service_origin",
                "json_extract(audit.payload_json,'$.after.catalog.service_schema_version')=new.service_schema_version",
            ][..],
            "product image catalogue provenance relationship trigger contract",
        ),
        (
            "local_security_clock_state_monotonic_update_only",
            &[
                "beforeupdateonlocal_security_clock_state",
                "new.singleton<>old.singleton",
                "new.high_water_utc<old.high_water_utc",
            ][..],
            "local security clock monotonic update trigger contract",
        ),
        (
            "staff_pin_credentials_assign_monotonic_order",
            &[
                "afterinsertonstaff_pin_credentials",
                "values('staff_pin_credential',new.staff_pin_credential_id)",
            ][..],
            "staff credential monotonic ordering trigger contract",
        ),
        (
            "staff_status_events_assign_monotonic_order",
            &[
                "afterinsertonstaff_status_events",
                "values('staff_status',new.staff_status_event_id)",
            ][..],
            "staff status monotonic ordering trigger contract",
        ),
        (
            "staff_role_events_assign_monotonic_order",
            &[
                "afterinsertonstaff_role_events",
                "values('staff_role',new.staff_role_event_id)",
            ][..],
            "staff role monotonic ordering trigger contract",
        ),
        (
            "local_staff_session_events_assign_monotonic_order",
            &[
                "afterinsertonlocal_staff_session_events",
                "values('staff_session',new.local_staff_session_event_id)",
            ][..],
            "staff session monotonic ordering trigger contract",
        ),
        (
            "staff_pin_attempts_assign_monotonic_order",
            &[
                "afterinsertonstaff_pin_attempts",
                "values('staff_pin_attempt',new.staff_pin_attempt_id)",
            ][..],
            "staff PIN attempt monotonic ordering trigger contract",
        ),
        (
            "kitchen_tickets_must_match_draft_revision",
            &[
                "fromdraft_ordersasdraft",
                "draft_revision.revision=new.draft_revision",
                "draft.branch_id=new.branch_id",
                "draft_revision.line_snapshot_json=new.line_snapshot_json",
            ][..],
            "kitchen ticket/draft relationship trigger contract",
        ),
        (
            "kitchen_tickets_kitchen_note_must_match_draft_revision",
            &[
                "fromdraft_ordersasdraft",
                "draft_revision.revision=new.draft_revision",
                "draft.branch_id=new.branch_id",
                "new.kitchen_note_snapshotisdraft_revision.kitchen_note",
            ][..],
            "kitchen ticket note/draft relationship trigger contract",
        ),
        (
            "kitchen_ticket_note_snapshot_is_immutable",
            &[
                "beforeupdateofkitchen_note_snapshotonkitchen_tickets",
                "new.kitchen_note_snapshotisnotold.kitchen_note_snapshot",
            ][..],
            "kitchen ticket note immutability trigger contract",
        ),
        (
            "kitchen_ticket_cancellation_notices_must_match_ticket_and_audit",
            &[
                "ticket.kitchen_ticket_id=new.kitchen_ticket_id",
                "ticket.draft_order_id=new.draft_order_id",
                "ticket.draft_revision=new.draft_revision",
                "audit.actor_id=new.occurred_by_actor_id",
            ][..],
            "kitchen cancellation notice relationship trigger contract",
        ),
        (
            "kitchen_ticket_cancellation_acknowledgements_must_match_notice_and_audit",
            &[
                "notice.kitchen_ticket_cancellation_notice_id=new.kitchen_ticket_cancellation_notice_id",
                "notice.branch_id=new.branch_id",
                "audit.actor_id=new.acknowledged_by_actor_id",
            ][..],
            "kitchen cancellation acknowledgement relationship trigger contract",
        ),
    ] {
        require_schema_fragments(connection, "trigger", name, fragments, policy_name)?;
    }

    require_schema_fragments(
        connection,
        "trigger",
        "inventory_movements_valid_direction",
        &[
            "new.movement_type='opening'",
            "exists(select1frominventory_movements",
        ],
        "inventory opening-once trigger contract",
    )?;

    require_schema_fragments(
        connection,
        "trigger",
        "products_cannot_be_deleted",
        &[
            "productswithretainedhistorymustbearchived",
            "product_deletion_authorizations",
            "order_lines",
            "product_image_versions",
            "product_modifier_options",
            "sync_outbox_events",
        ],
        "unused product deletion policy",
    )?;

    require_schema_fragments(
        connection,
        "index",
        "payments_invoice_sequence",
        &[
            "createuniqueindex",
            "onpayments(invoice_id,payment_sequence)",
        ],
        "payment-allocation sequence index contract",
    )?;

    require_schema_fragments(
        connection,
        "index",
        "products_branch_product_identity",
        &["createuniqueindex", "onproducts(branch_id,product_id)"],
        "product branch identity index contract",
    )?;

    require_schema_fragments(
        connection,
        "trigger",
        "order_lines_must_match_order_and_product",
        &[
            "json_type(new.modifier_snapshot_json)<>'array'",
            "product_modifier_optionsasoption",
            "new.modifier_total_minor",
        ],
        "order modifier snapshot integrity trigger contract",
    )
}

fn require_schema_fragments(
    connection: &Connection,
    object_type: &'static str,
    object_name: &'static str,
    fragments: &[&str],
    policy_name: &'static str,
) -> Result<(), StorageError> {
    let sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = ?1 AND name = ?2",
            params![object_type, object_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::Sql)?;
    let normalized = sql
        .as_deref()
        .map(normalize_schema_sql)
        .ok_or(StorageError::SchemaContractRejected(policy_name))?;

    if fragments
        .iter()
        .all(|fragment| normalized.contains(fragment))
    {
        Ok(())
    } else {
        Err(StorageError::SchemaContractRejected(policy_name))
    }
}

fn normalize_schema_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn verify_recorded_migration(
    migration: &LocalMigration,
    recorded_checksum: Option<&str>,
) -> Result<(), StorageError> {
    match recorded_checksum {
        Some(checksum) if checksum == migration.checksum => Ok(()),
        Some(checksum) => Err(StorageError::MigrationChecksumMismatch {
            version: migration.version,
            expected: migration.checksum,
            actual: checksum.to_owned(),
        }),
        None => Err(StorageError::MigrationHistoryInvalid(migration.version)),
    }
}

#[derive(Debug)]
pub enum StorageError {
    AuditChainInvalid(String),
    BranchArchived,
    BranchNotFound,
    CannotRevokeCurrentOwner,
    CatalogConflict,
    CategoryArchived,
    CategoryHasActiveProducts,
    CategoryNotFound,
    CipherUnavailable,
    CipherVersionMismatch {
        expected_prefix: &'static str,
        actual: String,
    },
    ConnectionPolicyRejected(&'static str),
    CommunityAlreadyProvisioned,
    CommunityNotProvisioned,
    CurrencyMismatch {
        expected: String,
        actual: String,
    },
    CustomerUnavailable,
    FinancialAmountOverflow,
    FinancialExportIntegrityMismatch,
    FinancialExportTooLarge,
    IntegrityCheckFailed(String),
    InventoryNotTracked,
    Io(std::io::Error),
    InvoiceNumberExhausted,
    InvoiceNotFound,
    InvalidAuditEvent(&'static str),
    InvalidProductImage(&'static str),
    InvalidPersistedData(String),
    InvalidReason,
    InvalidStaffPin,
    MigrationChecksumMismatch {
        version: i64,
        expected: &'static str,
        actual: String,
    },
    MigrationSourceChecksumMismatch {
        version: i64,
        expected: &'static str,
        actual: String,
    },
    MigrationHistoryInvalid(i64),
    ModifierOptionNotFound,
    ModifierOptionUnavailable,
    ProductNotFound,
    ProductHasRetainedHistory,
    ProductUnavailable,
    PermissionDenied,
    OwnerPinAlreadyConfigured,
    OwnerRoleReserved,
    SchemaContractRejected(&'static str),
    Sql(rusqlite::Error),
    StaffCredentialUnavailable,
    StaffNotFound,
    StaffNotActive,
    StaffPinTemporarilyLocked,
    StaffSessionRequired,
    UnsupportedSchemaVersion(i64),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuditChainInvalid(device_id) => {
                write!(
                    formatter,
                    "Audit chain verification failed for device {device_id}."
                )
            }
            Self::BranchArchived => {
                formatter.write_str("The selected branch is archived and cannot accept changes.")
            }
            Self::BranchNotFound => formatter.write_str("The selected branch does not exist."),
            Self::CannotRevokeCurrentOwner => {
                formatter.write_str("The current owner account cannot be revoked.")
            }
            Self::CatalogConflict => formatter
                .write_str("This record changed before the requested update could be saved."),
            Self::CategoryArchived => {
                formatter.write_str("The selected category is archived and cannot accept products.")
            }
            Self::CategoryHasActiveProducts => {
                formatter.write_str("Archive active products before archiving their category.")
            }
            Self::CategoryNotFound => formatter.write_str("The selected category does not exist."),
            Self::CipherUnavailable => {
                formatter.write_str("SQLCipher was unavailable; refusing to use local storage.")
            }
            Self::CipherVersionMismatch {
                expected_prefix,
                actual,
            } => write!(
                formatter,
                "SQLCipher version did not match the required release artifact ({expected_prefix}*, found {actual})."
            ),
            Self::ConnectionPolicyRejected(policy_name) => {
                write!(
                    formatter,
                    "Required local database policy could not be enabled: {policy_name}."
                )
            }
            Self::CommunityAlreadyProvisioned => {
                formatter.write_str("This Community database has already been provisioned.")
            }
            Self::CommunityNotProvisioned => {
                formatter.write_str("Community setup has not been completed yet.")
            }
            Self::CurrencyMismatch { expected, actual } => {
                write!(
                    formatter,
                    "Expected branch currency {expected}, received {actual}."
                )
            }
            Self::CustomerUnavailable => {
                formatter.write_str("The selected customer is unavailable for this branch.")
            }
            Self::FinancialAmountOverflow => {
                formatter.write_str("The sale amount is outside the supported range.")
            }
            Self::FinancialExportIntegrityMismatch => formatter.write_str(
                "Financial facts did not satisfy the verified export contract.",
            ),
            Self::FinancialExportTooLarge => formatter.write_str(
                "The verified financial export is too large to prepare safely on this device.",
            ),
            Self::IntegrityCheckFailed(result) => {
                write!(
                    formatter,
                    "Encrypted database integrity check failed: {result}"
                )
            }
            Self::InventoryNotTracked => {
                formatter.write_str("Set opening stock before configuring a low-stock threshold.")
            }
            Self::Io(error) => write!(formatter, "Local backup file operation failed: {error}"),
            Self::InvalidAuditEvent(message) => formatter.write_str(message),
            Self::InvalidProductImage(message) => formatter.write_str(message),
            Self::InvalidPersistedData(message) => {
                write!(formatter, "Persisted local data was invalid: {message}")
            }
            Self::InvalidReason => {
                formatter.write_str("A correction or archive reason is required.")
            }
            Self::InvalidStaffPin => formatter.write_str("The staff PIN could not be verified."),
            Self::InvoiceNumberExhausted => {
                formatter.write_str("This branch has exhausted its invoice number range.")
            }
            Self::InvoiceNotFound => {
                formatter.write_str("The selected invoice does not exist in this branch.")
            }
            Self::MigrationChecksumMismatch {
                version,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "Migration {version} checksum mismatch (expected {expected}, found {actual})."
                )
            }
            Self::MigrationSourceChecksumMismatch {
                version,
                expected,
                actual,
            } => write!(
                formatter,
                "Embedded migration {version} source checksum did not match its reviewed manifest (expected {expected}, found {actual})."
            ),
            Self::MigrationHistoryInvalid(version) => {
                write!(
                    formatter,
                    "Migration history is incomplete at version {version}."
                )
            }
            Self::ModifierOptionNotFound => {
                formatter.write_str("The selected menu modifier does not exist.")
            }
            Self::ModifierOptionUnavailable => formatter.write_str(
                "The selected menu modifier is no longer available for this item.",
            ),
            Self::ProductNotFound => formatter.write_str("The selected product does not exist."),
            Self::ProductHasRetainedHistory => formatter.write_str(
                "The product has retained financial, image, or synchronization history and must be archived.",
            ),
            Self::ProductUnavailable => {
                formatter.write_str("The selected product is not currently available for sale.")
            }
            Self::PermissionDenied => {
                formatter.write_str("This role is not allowed to control inventory.")
            }
            Self::OwnerPinAlreadyConfigured => {
                formatter.write_str("An owner PIN is already configured for this restaurant.")
            }
            Self::OwnerRoleReserved => {
                formatter.write_str("The primary owner role cannot be created or revoked here.")
            }
            Self::SchemaContractRejected(policy_name) => {
                write!(
                    formatter,
                    "Local database schema did not satisfy the required security contract: {policy_name}."
                )
            }
            Self::Sql(error) => write!(formatter, "Local database operation failed: {error}"),
            Self::StaffCredentialUnavailable => {
                formatter.write_str("The staff credential could not be prepared securely.")
            }
            Self::StaffNotFound => formatter.write_str("The selected staff member does not exist."),
            Self::StaffNotActive => {
                formatter.write_str("The selected staff account is not active.")
            }
            Self::StaffPinTemporarilyLocked => formatter.write_str(
                "Too many incorrect PIN attempts. Wait before trying again.",
            ),
            Self::StaffSessionRequired => {
                formatter.write_str("Unlock with an active staff PIN before changing restaurant data.")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "Local database schema {version} is newer than this app."
                )
            }
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sql(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[derive(Default)]
    struct MemoryKeyStore {
        stored_key: Mutex<Option<[u8; 32]>>,
    }

    impl DatabaseKeyStore for MemoryKeyStore {
        fn load(&self, _slot: DatabaseKeySlot) -> Result<Option<DatabaseKey>, KeyStoreError> {
            Ok(self
                .stored_key
                .lock()
                .expect("test key-store lock")
                .as_ref()
                .copied()
                .map(DatabaseKey::from_bytes))
        }

        fn store_new(
            &self,
            _slot: DatabaseKeySlot,
            key: &DatabaseKey,
        ) -> Result<(), KeyStoreError> {
            *self.stored_key.lock().expect("test key-store lock") = Some(*key.as_bytes());
            Ok(())
        }
    }

    fn fresh_provisioned_database() -> (tempfile::TempDir, LocalDatabase, Branch) {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([11; 32]);
        let database = LocalDatabase::open(&path, &key).expect("encrypted database");
        let setup = CommunitySetup::new("Gotigin Café", "Koramangala", "INR", "Asia/Kolkata")
            .expect("valid Community setup");
        let branch = database
            .provision_community(&setup)
            .expect("provisioned branch");

        (temp, database, branch)
    }

    /// Most storage tests exercise a post-unlock business transaction. Seed a
    /// test-only active session directly so those tests keep their historical
    /// audit counts while still crossing the production authorization gate.
    /// Authentication behavior itself always uses `fresh_provisioned_database`
    /// and the real Argon2id unlock path below.
    fn provisioned_database() -> (tempfile::TempDir, LocalDatabase, Branch) {
        let (temp, database, branch) = fresh_provisioned_database();
        let transaction = begin_immediate_transaction(&database.connection)
            .expect("test authorization transaction");
        let (device_id, owner_actor_id) =
            ensure_local_installation_identity(&transaction).expect("test local identity");
        let owner_pin_hash = hash_local_staff_pin("123456").expect("test owner PIN digest");
        transaction
            .execute(
                "INSERT INTO staff_pin_credentials (staff_pin_credential_id, staff_id, argon2id_hash, created_at_utc, created_by_actor_id) VALUES (?1, ?2, ?3, '2026-01-01T00:00:00.000Z', ?2)",
                params![
                    EntityId::new_v7().to_string(),
                    owner_actor_id.to_string(),
                    owner_pin_hash,
                ],
            )
            .expect("test owner credential");
        transaction
            .execute(
                "INSERT INTO local_staff_session_events (local_staff_session_event_id, device_id, staff_id, event_type, occurred_at_utc, expires_at_utc, audit_event_id) VALUES (?1, ?2, ?3, 'unlocked', '2026-01-01T00:00:00.000Z', '9999-12-31T23:59:59.999Z', NULL)",
                params![
                    EntityId::new_v7().to_string(),
                    device_id.to_string(),
                    owner_actor_id.to_string(),
                ],
            )
            .expect("test owner session");
        transaction.commit().expect("test authorization commit");
        (temp, database, branch)
    }

    fn assert_no_backup_staging_artifacts(destination: &Path) {
        let parent = destination.parent().expect("backup parent");
        let destination_name = destination
            .file_name()
            .expect("backup name")
            .to_string_lossy();
        let staging_prefix = format!(".{destination_name}.ros-backup-");
        let artifacts: Vec<_> = fs::read_dir(parent)
            .expect("backup directory")
            .map(|entry| entry.expect("directory entry").file_name())
            .filter(|name| name.to_string_lossy().starts_with(&staging_prefix))
            .collect();
        assert!(
            artifacts.is_empty(),
            "unexpected staging artifacts: {artifacts:?}"
        );
    }

    fn assert_current_migration_manifest(database: &LocalDatabase) {
        assert_eq!(
            database.schema_version().expect("schema version"),
            SCHEMA_VERSION
        );
        for migration in &LOCAL_MIGRATIONS {
            assert_eq!(
                database
                    .migration_checksum(migration.version)
                    .expect("migration checksum"),
                Some(migration.checksum.to_owned()),
                "migration {} checksum",
                migration.version,
            );
        }
    }

    fn mutation_context(database: &LocalDatabase) -> MutationContext {
        database
            .community_owner_context()
            .expect("test owner context")
    }

    fn dual_approver<'a>(
        database: &LocalDatabase,
        owner_context: &MutationContext,
        pin: &'a str,
    ) -> (EntityId, DualPersonApproval<'a>) {
        let manager = database
            .create_local_staff("Approver Manager", ActorRole::Manager, pin, owner_context)
            .expect("approver enrolled");
        (
            manager.staff_id().clone(),
            DualPersonApproval {
                approver_actor_id: manager.staff_id().clone(),
                approver_pin: pin,
            },
        )
    }

    #[test]
    fn local_staff_pin_sessions_are_expiring_audited_and_append_only() {
        let (_temp, database, branch) = fresh_provisioned_database();
        let staff = database.list_local_staff().expect("owner staff account");
        assert_eq!(staff.len(), 1);
        assert_eq!(staff[0].display_name(), "Owner");
        assert_eq!(staff[0].role(), ActorRole::Owner);
        assert!(staff[0].is_active());
        assert!(!staff[0].has_pin());

        let initial_state = database
            .local_staff_security_state()
            .expect("initial staff security state");
        assert!(initial_state.owner_pin_setup_required());
        assert!(initial_state.active_staff().is_none());
        assert!(matches!(
            database.community_active_staff_context(),
            Err(StorageError::StaffSessionRequired)
        ));

        assert!(matches!(
            database.set_initial_owner_pin("12345"),
            Err(StorageError::InvalidStaffPin)
        ));
        database
            .set_initial_owner_pin("123456")
            .expect("owner PIN configured once");
        assert!(matches!(
            database.set_initial_owner_pin("123456"),
            Err(StorageError::OwnerPinAlreadyConfigured)
        ));
        assert!(matches!(
            database.unlock_local_staff(staff[0].staff_id(), "000000"),
            Err(StorageError::InvalidStaffPin)
        ));

        let unlocked = database
            .unlock_local_staff(staff[0].staff_id(), "123456")
            .expect("owner unlock");
        assert_eq!(unlocked.staff_id(), staff[0].staff_id());
        let context = database
            .community_active_staff_context()
            .expect("active staff context");
        assert_eq!(context.actor_id(), staff[0].staff_id());
        assert_eq!(context.actor_role(), ActorRole::Owner);
        let active_state = database
            .local_staff_security_state()
            .expect("active session state");
        assert!(!active_state.owner_pin_setup_required());
        assert_eq!(
            active_state.active_staff().map(LocalStaffAccount::staff_id),
            Some(staff[0].staff_id())
        );

        database.lock_local_staff().expect("lock transition");
        assert!(matches!(
            database.community_active_staff_context(),
            Err(StorageError::StaffSessionRequired)
        ));
        assert!(
            database
                .connection
                .execute("DELETE FROM local_staff_session_events", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("UPDATE staff_pin_credentials SET argon2id_hash = 'bad'", [])
                .is_err()
        );

        let owner_context = database.community_owner_context().expect("owner identity");
        database
            .verify_device_audit_chain(owner_context.device_id())
            .expect("staff security audit chain");

        let attempts = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM staff_pin_attempts WHERE staff_id = ?1",
                [staff[0].staff_id().to_string()],
                |row| row.get::<_, i64>(0),
            )
            .expect("PIN attempt facts");
        assert_eq!(attempts, 2);
        assert_eq!(branch.branch_id(), owner_context.branch_id());
    }

    #[test]
    fn initial_owner_pin_and_verified_unlock_are_the_only_sessionless_bootstrap() {
        let (_temp, database, _branch) = fresh_provisioned_database();
        let owner = database.community_owner_context().expect("owner identity");
        let command = CreateCategory::new("Bootstrap must not edit", 0).expect("category");

        assert!(matches!(
            database.create_category(&command, &owner),
            Err(StorageError::StaffSessionRequired)
        ));
        database
            .set_initial_owner_pin("123456")
            .expect("initial owner PIN bootstrap");
        assert!(matches!(
            database.create_category(&command, &owner),
            Err(StorageError::StaffSessionRequired)
        ));
        database
            .unlock_local_staff(owner.actor_id(), "123456")
            .expect("verified owner unlock");
        database
            .create_category(&command, &owner)
            .expect("unlocked owner mutation");
    }

    #[test]
    fn audited_mutations_reject_forged_and_stale_contexts_without_partial_writes() {
        let (_temp, database, _branch) = provisioned_database();
        let owner = mutation_context(&database);
        let categories_before = database
            .connection
            .query_row("SELECT COUNT(*) FROM categories", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("category count");
        let audit_before = database.audit_event_count().expect("audit count");
        let command = CreateCategory::new("Forged authority", 0).expect("category");

        let forged_role = MutationContext::new(
            owner.branch_id().clone(),
            owner.actor_id().clone(),
            owner.device_id().clone(),
            EntityId::new_v7(),
            ActorRole::Manager,
        );
        assert!(matches!(
            database.create_category(&command, &forged_role),
            Err(StorageError::PermissionDenied)
        ));
        let forged_actor = MutationContext::new(
            owner.branch_id().clone(),
            EntityId::new_v7(),
            owner.device_id().clone(),
            EntityId::new_v7(),
            ActorRole::Owner,
        );
        assert!(matches!(
            database.create_category(&command, &forged_actor),
            Err(StorageError::StaffSessionRequired)
        ));
        let forged_device = MutationContext::new(
            owner.branch_id().clone(),
            owner.actor_id().clone(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            ActorRole::Owner,
        );
        assert!(matches!(
            database.create_category(&command, &forged_device),
            Err(StorageError::StaffSessionRequired)
        ));

        let manager = database
            .create_local_staff("Stale manager", ActorRole::Manager, "654321", &owner)
            .expect("manager enrollment");
        database.lock_local_staff().expect("owner lock");
        database
            .unlock_local_staff(manager.staff_id(), "654321")
            .expect("manager unlock");
        let stale_manager = database
            .community_active_staff_context()
            .expect("manager context");
        let audit_after_session_transition =
            database.audit_event_count().expect("session audit count");

        // Simulate a role-change fact committed by another authorized process.
        // Its deliberately older wall-clock value must still take effect by
        // durable insertion sequence before this stale context can commit.
        database
            .connection
            .execute(
                "INSERT INTO staff_role_events (staff_role_event_id, staff_id, role, reason, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, 'cashier', 'test concurrent demotion', '1900-01-01T00:00:00.000Z', ?3)",
                params![
                    EntityId::new_v7().to_string(),
                    manager.staff_id().to_string(),
                    owner.actor_id().to_string(),
                ],
            )
            .expect("backdated role fact");
        assert!(matches!(
            database.create_category(&command, &stale_manager),
            Err(StorageError::PermissionDenied)
        ));

        database
            .connection
            .execute(
                "INSERT INTO staff_status_events (staff_status_event_id, staff_id, status, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, 'revoked', '1900-01-01T00:00:00.000Z', ?3)",
                params![
                    EntityId::new_v7().to_string(),
                    manager.staff_id().to_string(),
                    owner.actor_id().to_string(),
                ],
            )
            .expect("backdated revocation fact");
        assert!(matches!(
            database.create_category(&command, &stale_manager),
            Err(StorageError::StaffSessionRequired)
        ));

        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM categories", [], |row| row
                    .get::<_, i64>(0))
                .expect("category count after rejection"),
            categories_before
        );
        // Failed category writes and their audit rows were rolled back
        // together; direct test-only role/status facts are intentionally not
        // audit API calls.
        assert_eq!(
            database
                .audit_event_count()
                .expect("audit count after rejection"),
            audit_after_session_transition
        );
        assert!(audit_after_session_transition > audit_before);
    }

    #[test]
    fn latest_security_facts_use_monotonic_order_instead_of_wall_clock() {
        let (_temp, database, _branch) = provisioned_database();
        let owner = mutation_context(&database);
        let cashier = database
            .create_local_staff("Clock test cashier", ActorRole::Cashier, "654321", &owner)
            .expect("cashier enrollment");
        let replacement_hash = hash_local_staff_pin("111111").expect("replacement PIN digest");
        database
            .connection
            .execute(
                "INSERT INTO staff_pin_credentials (staff_pin_credential_id, staff_id, argon2id_hash, created_at_utc, created_by_actor_id) VALUES (?1, ?2, ?3, '1900-01-01T00:00:00.000Z', ?4)",
                params![
                    EntityId::new_v7().to_string(),
                    cashier.staff_id().to_string(),
                    replacement_hash,
                    owner.actor_id().to_string(),
                ],
            )
            .expect("backdated replacement credential");
        database.lock_local_staff().expect("owner lock");
        assert!(matches!(
            database.unlock_local_staff(cashier.staff_id(), "654321"),
            Err(StorageError::InvalidStaffPin)
        ));
        database
            .unlock_local_staff(cashier.staff_id(), "111111")
            .expect("latest inserted credential unlocks");

        database
            .connection
            .execute(
                "INSERT INTO staff_role_events (staff_role_event_id, staff_id, role, reason, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, 'manager', 'future-dated predecessor', '2999-01-01T00:00:00.000Z', ?3)",
                params![
                    EntityId::new_v7().to_string(),
                    cashier.staff_id().to_string(),
                    owner.actor_id().to_string(),
                ],
            )
            .expect("future-dated role fact");
        database
            .connection
            .execute(
                "INSERT INTO staff_role_events (staff_role_event_id, staff_id, role, reason, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, 'kitchen', 'later inserted role', '1900-01-01T00:00:00.000Z', ?3)",
                params![
                    EntityId::new_v7().to_string(),
                    cashier.staff_id().to_string(),
                    owner.actor_id().to_string(),
                ],
            )
            .expect("backdated latest role fact");
        let current_staff = database.list_local_staff().expect("staff projection");
        assert!(current_staff.iter().any(|staff| {
            staff.staff_id() == cashier.staff_id() && staff.role() == ActorRole::Kitchen
        }));

        database
            .connection
            .execute(
                "INSERT INTO staff_status_events (staff_status_event_id, staff_id, status, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, 'revoked', '1900-01-01T00:00:00.000Z', ?3)",
                params![
                    EntityId::new_v7().to_string(),
                    cashier.staff_id().to_string(),
                    owner.actor_id().to_string(),
                ],
            )
            .expect("backdated latest status fact");
        let revoked_staff = database.list_local_staff().expect("revoked projection");
        assert!(
            revoked_staff
                .iter()
                .any(|staff| { staff.staff_id() == cashier.staff_id() && !staff.is_active() })
        );
        assert!(matches!(
            database.community_active_staff_context(),
            Err(StorageError::StaffSessionRequired)
        ));
    }

    #[test]
    fn clock_rollback_fails_closed_and_polling_keeps_bounded_state() {
        let (_temp, database, _branch) = provisioned_database();
        let owner = mutation_context(&database);
        for _ in 0..100 {
            assert!(
                database
                    .local_staff_security_state()
                    .expect("security poll")
                    .active_staff()
                    .is_some()
            );
        }
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM local_security_clock_state",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("bounded clock row count"),
            1
        );

        database
            .connection
            .execute(
                "INSERT INTO local_staff_session_events (local_staff_session_event_id, device_id, staff_id, event_type, occurred_at_utc, expires_at_utc, audit_event_id) VALUES (?1, ?2, ?3, 'unlocked', '2999-01-01T00:00:00.000Z', '3000-01-01T00:00:00.000Z', NULL)",
                params![
                    EntityId::new_v7().to_string(),
                    owner.device_id().to_string(),
                    owner.actor_id().to_string(),
                ],
            )
            .expect("future-dated session fact");
        database
            .connection
            .execute(
                "UPDATE local_security_clock_state SET high_water_utc = '2999-01-01T00:00:00.000Z' WHERE singleton = 1",
                [],
            )
            .expect("simulate a previously observed future clock");
        assert!(matches!(
            database.community_active_staff_context(),
            Err(StorageError::StaffSessionRequired)
        ));
        assert!(matches!(
            database.unlock_local_staff(owner.actor_id(), "123456"),
            Err(StorageError::StaffPinTemporarilyLocked)
        ));

        // Lock must remain available during rollback and its backdated wall
        // time must still be authoritative because session order is durable.
        database.lock_local_staff().expect("fail-safe lock");
        let latest_event: String = database
            .connection
            .query_row(
                "SELECT session.event_type FROM local_staff_session_events AS session JOIN local_security_fact_order AS fact_order ON fact_order.fact_kind = 'staff_session' AND fact_order.fact_id = session.local_staff_session_event_id WHERE session.device_id = ?1 ORDER BY fact_order.fact_sequence DESC LIMIT 1",
                [owner.device_id().to_string()],
                |row| row.get(0),
            )
            .expect("latest session event");
        assert_eq!(latest_event, "locked");
        assert!(
            database
                .connection
                .execute(
                    "UPDATE local_security_clock_state SET high_water_utc = '1900-01-01T00:00:00.000Z' WHERE singleton = 1",
                    [],
                )
                .is_err()
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM local_security_clock_state",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("bounded clock row count after rollback"),
            1
        );
    }

    #[test]
    fn customer_profiles_are_append_only_and_anonymized_customers_cannot_be_billed() {
        let (_temp, database, branch) = provisioned_database();
        let context = mutation_context(&database);
        let customer = database
            .create_customer(
                "Aisha Khan",
                Some("+91 98765 43210"),
                Some("AISHA@example.com"),
                true,
                &context,
            )
            .expect("customer created");
        assert_eq!(customer.phone_number(), Some("919876543210"));
        assert_eq!(customer.email_address(), Some("aisha@example.com"));
        assert_eq!(customer.revision(), 1);

        let corrected = database
            .revise_customer(
                customer.customer_id(),
                CustomerProfileInput {
                    display_name: "Aisha Khan",
                    phone_number: Some("9876543210"),
                    email_address: None,
                    marketing_consent: false,
                },
                "Customer corrected contact details at counter",
                &context,
            )
            .expect("customer corrected");
        assert_eq!(corrected.revision(), 2);
        assert_eq!(corrected.phone_number(), Some("9876543210"));
        assert_eq!(
            database
                .list_active_customers(branch.branch_id())
                .expect("customers")
                .len(),
            1
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE customer_profile_revisions SET display_name = 'edited'",
                    []
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM customers", [])
                .is_err()
        );

        let category = database
            .create_category(
                &CreateCategory::new("Counter", 0).expect("valid category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("price"),
                    None,
                    None,
                    0,
                )
                .expect("product"),
                &context,
            )
            .expect("product created");
        let sale = CompleteSale::new(
            ros_core::OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![
                ros_core::SaleLineInput::new(product.product_id().clone(), 1)
                    .expect("positive quantity"),
            ],
        )
        .expect("sale")
        .with_customer(Some(customer.customer_id().clone()));
        database
            .complete_sale(&sale, &context)
            .expect("sale committed");
        let attached_customer: Option<String> = database
            .connection
            .query_row("SELECT customer_id FROM orders", [], |row| row.get(0))
            .expect("attached customer");
        let customer_id = customer.customer_id().to_string();
        assert_eq!(attached_customer.as_deref(), Some(customer_id.as_str()));

        database
            .anonymize_customer(
                customer.customer_id(),
                "Customer requested removal of contact data",
                &context,
            )
            .expect("customer anonymized");
        assert!(
            database
                .list_active_customers(branch.branch_id())
                .expect("active customers")
                .is_empty()
        );
        let retained_profile: (String, Option<String>, Option<String>, i64) = database
            .connection
            .query_row(
                "SELECT display_name, phone_number, email_address, marketing_consent FROM customer_profile_revisions WHERE customer_id = ?1 ORDER BY revision DESC LIMIT 1",
                [customer.customer_id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("anonymized profile");
        assert_eq!(retained_profile.0, "Anonymized customer");
        assert_eq!(retained_profile.1, None);
        assert_eq!(retained_profile.2, None);
        assert_eq!(retained_profile.3, 0);
        assert!(matches!(
            database.complete_sale(&sale, &context),
            Err(StorageError::CustomerUnavailable)
        ));
        database
            .verify_device_audit_chain(context.device_id())
            .expect("customer audit chain");
    }

    #[test]
    fn role_boundaries_reject_counter_or_kitchen_escalation() {
        let (_temp, database, branch) = provisioned_database();
        let cashier = MutationContext::new(
            branch.branch_id().clone(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            ActorRole::Cashier,
        );
        assert!(matches!(
            database.create_category(
                &CreateCategory::new("Cashier catalogue edit", 0).expect("category"),
                &cashier,
            ),
            Err(StorageError::PermissionDenied)
        ));

        let owner = mutation_context(&database);
        let manager_account = database
            .create_local_staff("Shift manager", ActorRole::Manager, "654321", &owner)
            .expect("manager enrollment");
        database.lock_local_staff().expect("owner lock");
        database
            .unlock_local_staff(manager_account.staff_id(), "654321")
            .expect("manager unlock");
        let manager = database
            .community_active_staff_context()
            .expect("active manager context");
        database
            .create_category(
                &CreateCategory::new("Manager catalogue edit", 0).expect("category"),
                &manager,
            )
            .expect("manager catalogue authority");
        assert!(matches!(
            database.list_recent_audit_events(20, &cashier),
            Err(StorageError::PermissionDenied)
        ));
        assert!(matches!(
            database.list_recent_audit_events(20, &manager),
            Err(StorageError::PermissionDenied)
        ));

        let kitchen = MutationContext::new(
            branch.branch_id().clone(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            ActorRole::Kitchen,
        );
        let sale = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(EntityId::new_v7(), 1).expect("line")],
        )
        .expect("sale command");
        assert!(matches!(
            database.complete_sale(&sale, &kitchen),
            Err(StorageError::PermissionDenied)
        ));
        assert!(matches!(
            database.load_invoice_detail(&EntityId::new_v7(), &kitchen),
            Err(StorageError::PermissionDenied)
        ));
    }

    #[test]
    fn owner_manages_append_only_local_staff_lifecycle() {
        let (_temp, database, branch) = fresh_provisioned_database();
        let owner = database.community_owner_context().expect("owner context");
        database
            .set_initial_owner_pin("123456")
            .expect("owner credential");
        database
            .unlock_local_staff(owner.actor_id(), "123456")
            .expect("owner unlock");
        let cashier = database
            .create_local_staff("Asha Counter", ActorRole::Cashier, "654321", &owner)
            .expect("cashier enrollment");
        assert!(matches!(
            database.create_local_staff("Another owner", ActorRole::Owner, "654321", &owner),
            Err(StorageError::OwnerRoleReserved)
        ));
        let manager = MutationContext::new(
            branch.branch_id().clone(),
            EntityId::new_v7(),
            owner.device_id().clone(),
            EntityId::new_v7(),
            ActorRole::Manager,
        );
        assert!(matches!(
            database.create_local_staff("Escalation", ActorRole::Cashier, "654321", &manager),
            Err(StorageError::PermissionDenied)
        ));

        let staff = database.list_local_staff().expect("staff list");
        assert_eq!(staff.len(), 2);
        assert!(
            staff
                .iter()
                .any(|staff| staff.staff_id() == cashier.staff_id()
                    && staff.has_pin()
                    && staff.is_active())
        );
        database
            .change_local_staff_role(
                cashier.staff_id(),
                ActorRole::Manager,
                &MutationReason::new("Promoted to shift manager").expect("reason"),
                &owner,
            )
            .expect("role change");
        let reassigned = database
            .list_local_staff()
            .expect("staff after role change");
        assert!(reassigned.iter().any(|staff| {
            staff.staff_id() == cashier.staff_id()
                && staff.role() == ActorRole::Manager
                && staff.is_active()
        }));
        assert!(matches!(
            database.change_local_staff_role(
                owner.actor_id(),
                ActorRole::Cashier,
                &MutationReason::new("Invalid owner reassignment").expect("reason"),
                &owner,
            ),
            Err(StorageError::OwnerRoleReserved)
        ));
        database
            .unlock_local_staff(cashier.staff_id(), "654321")
            .expect("cashier unlock");
        assert_eq!(
            database
                .community_active_staff_context()
                .expect("manager session")
                .actor_role(),
            ActorRole::Manager
        );
        database.lock_local_staff().expect("cashier lock");
        database
            .unlock_local_staff(owner.actor_id(), "123456")
            .expect("owner re-unlock for PIN rotation");
        database
            .rotate_local_staff_pin(cashier.staff_id(), "111111", &owner)
            .expect("owner rotates cashier PIN");
        assert!(matches!(
            database.unlock_local_staff(cashier.staff_id(), "654321"),
            Err(StorageError::InvalidStaffPin)
        ));
        database
            .unlock_local_staff(cashier.staff_id(), "111111")
            .expect("rotated PIN unlock");
        database.lock_local_staff().expect("cashier lock");
        database
            .unlock_local_staff(owner.actor_id(), "123456")
            .expect("owner re-unlock for revocation");
        database
            .revoke_local_staff(
                cashier.staff_id(),
                &MutationReason::new("No longer employed").expect("reason"),
                &owner,
            )
            .expect("cashier revoked");
        assert!(matches!(
            database.unlock_local_staff(cashier.staff_id(), "111111"),
            Err(StorageError::InvalidStaffPin)
        ));
        assert!(
            database
                .connection
                .execute("DELETE FROM staff_accounts", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("UPDATE staff_role_events SET role = 'cashier'", [])
                .is_err()
        );
        assert!(matches!(
            database.revoke_local_staff(
                owner.actor_id(),
                &MutationReason::new("Invalid self revoke").expect("reason"),
                &owner,
            ),
            Err(StorageError::CannotRevokeCurrentOwner)
        ));
        database
            .verify_device_audit_chain(owner.device_id())
            .expect("staff lifecycle audit chain");
    }

    #[test]
    fn open_draft_orders_restore_and_settle_exactly_once() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Meals", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Veg biryani",
                    Some(category.category_id().clone()),
                    Money::new(18_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 2).expect("line")];
        let draft = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::DineIn,
                Some("Table 7"),
                &lines,
                &context,
            )
            .expect("draft saved");
        let restored = database
            .list_open_draft_orders(branch.branch_id())
            .expect("open drafts");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].draft().draft_order_id(), draft.draft_order_id());
        assert_eq!(restored[0].lines(), lines.as_slice());

        let sale =
            CompleteSale::new(OrderFulfillment::DineIn, PaymentMethod::Cash, lines).expect("sale");
        database
            .complete_draft_sale(&sale, draft.draft_order_id(), draft.revision(), &context)
            .expect("draft settled");
        assert!(
            database
                .list_open_draft_orders(branch.branch_id())
                .expect("open drafts")
                .is_empty()
        );
        assert!(matches!(
            database.complete_draft_sale(&sale, draft.draft_order_id(), draft.revision(), &context),
            Err(StorageError::CatalogConflict)
        ));
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn draft_order_persistence_rejects_duplicate_product_modifier_configurations() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Veg biryani",
                    None,
                    Money::new(18_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product created");
        let duplicate_lines = vec![
            SaleLineInput::new(product.product_id().clone(), 1).expect("line"),
            SaleLineInput::new(product.product_id().clone(), 2).expect("line"),
        ];

        let error = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::Takeaway,
                None,
                &duplicate_lines,
                &context,
            )
            .expect_err("duplicate line configurations must not be persisted in a draft");
        assert!(matches!(
            error,
            StorageError::InvalidPersistedData(ref message)
                if message == "a draft order cannot contain the same product and modifier combination more than once"
        ));
        assert!(
            database
                .list_open_draft_orders(branch.branch_id())
                .expect("draft list")
                .is_empty()
        );
    }

    #[test]
    fn draft_snapshot_configuration_matching_is_unique_and_bijective() {
        let first_product = EntityId::new_v7();
        let second_product = EntityId::new_v7();
        let duplicate_snapshot = serde_json::json!([
            {"product_id": first_product.to_string(), "quantity": 1},
            {"product_id": first_product.to_string(), "quantity": 2},
        ])
        .to_string();
        assert!(matches!(
            parse_draft_snapshot_lines(&duplicate_snapshot),
            Err(StorageError::InvalidPersistedData(message))
                if message == "draft snapshot contains duplicate product and modifier combinations"
        ));

        let matching_in_a_different_order = vec![
            SaleLineInput::new(second_product.clone(), 3).expect("line"),
            SaleLineInput::new(first_product.clone(), 2).expect("line"),
        ];
        let saved_lines = vec![
            SaleLineInput::new(first_product.clone(), 2).expect("line"),
            SaleLineInput::new(second_product, 3).expect("line"),
        ];
        assert!(same_sale_lines(
            &matching_in_a_different_order,
            &saved_lines
        ));

        // This was previously accepted because both left-side lines could
        // match the same right-side entry. Exact settlement must require a
        // one-to-one product/modifier-configuration-to-quantity match.
        let ambiguous_lines = vec![
            SaleLineInput::new(first_product.clone(), 1).expect("line"),
            SaleLineInput::new(first_product, 1).expect("line"),
        ];
        let different_quantities = vec![
            SaleLineInput::new(matching_in_a_different_order[1].product_id().clone(), 1)
                .expect("line"),
            SaleLineInput::new(matching_in_a_different_order[1].product_id().clone(), 2)
                .expect("line"),
        ];
        assert!(!same_sale_lines(&ambiguous_lines, &different_quantities));
    }

    #[test]
    fn kitchen_ticket_progression_is_ordered_and_audited() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Meals", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Veg biryani",
                    Some(category.category_id().clone()),
                    Money::new(18_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 2).expect("line")];
        let draft = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::DineIn,
                Some("Table 3"),
                &lines,
                &context,
            )
            .expect("draft saved");
        let ticket = database
            .send_draft_to_kitchen(draft.draft_order_id(), draft.revision(), &context)
            .expect("ticket sent");
        assert_eq!(ticket.state(), "new");
        let sent_drafts = database
            .list_open_draft_orders(branch.branch_id())
            .expect("sent drafts remain settleable");
        assert_eq!(sent_drafts.len(), 1);
        assert_eq!(sent_drafts[0].draft().state(), "sent_to_kitchen");
        assert_eq!(
            sent_drafts[0].draft().draft_order_id(),
            draft.draft_order_id()
        );
        assert_eq!(
            database
                .list_active_kitchen_tickets(branch.branch_id())
                .expect("tickets")
                .len(),
            1
        );
        assert!(matches!(
            database.advance_kitchen_ticket(
                ticket.ticket_id(),
                ticket.revision(),
                "ready",
                &context
            ),
            Err(StorageError::CatalogConflict)
        ));
        let preparing = database
            .advance_kitchen_ticket(ticket.ticket_id(), ticket.revision(), "preparing", &context)
            .expect("preparing");
        let ready = database
            .advance_kitchen_ticket(
                preparing.ticket_id(),
                preparing.revision(),
                "ready",
                &context,
            )
            .expect("ready");
        let completed = database
            .advance_kitchen_ticket(ready.ticket_id(), ready.revision(), "completed", &context)
            .expect("completed");
        assert_eq!(completed.state(), "completed");
        assert!(
            database
                .list_active_kitchen_tickets(branch.branch_id())
                .expect("tickets")
                .is_empty()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM kitchen_tickets WHERE kitchen_ticket_id = ?1",
                    [ticket.ticket_id().to_string()],
                )
                .is_err()
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn kitchen_notes_are_normalized_snapshotted_and_immutable() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Meals", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Paneer tikka",
                    Some(category.category_id().clone()),
                    Money::new(22_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")];
        let note = "No onions, please";
        let draft = database
            .save_draft_order_with_kitchen_note(
                DraftOrderSaveRequest::with_kitchen_note(
                    None,
                    None,
                    OrderFulfillment::DineIn,
                    Some("Table 6"),
                    Some("  No onions, please  "),
                    &lines,
                ),
                &context,
            )
            .expect("draft with kitchen note");
        assert_eq!(draft.kitchen_note(), Some(note));
        let revision_note: Option<String> = database
            .connection
            .query_row(
                "SELECT kitchen_note FROM draft_order_revisions WHERE draft_order_id = ?1 AND revision = ?2",
                params![draft.draft_order_id().to_string(), draft.revision()],
                |row| row.get(0),
            )
            .expect("saved draft note");
        assert_eq!(revision_note.as_deref(), Some(note));
        assert!(database
            .connection
            .execute(
                "UPDATE draft_order_revisions SET kitchen_note = 'rewritten' WHERE draft_order_id = ?1 AND revision = ?2",
                params![draft.draft_order_id().to_string(), draft.revision()],
            )
            .is_err());

        let snapshot: String = database
            .connection
            .query_row(
                "SELECT line_snapshot_json FROM draft_order_revisions WHERE draft_order_id = ?1 AND revision = ?2",
                params![draft.draft_order_id().to_string(), draft.revision()],
                |row| row.get(0),
            )
            .expect("saved line snapshot");
        // All other direct-SQL fields are valid, but the note comes from no
        // draft revision. The note/draft relationship trigger must reject it.
        assert!(database
            .connection
            .execute(
                "INSERT INTO kitchen_tickets (kitchen_ticket_id, branch_id, draft_order_id, draft_revision, ticket_state, table_label_snapshot, line_snapshot_json, kitchen_note_snapshot, created_at_utc, created_by_actor_id, updated_at_utc, updated_by_actor_id, revision) VALUES (?1, ?2, ?3, ?4, 'new', 'Table 6', ?5, NULL, '2026-07-17T00:00:00Z', ?6, '2026-07-17T00:00:00Z', ?6, 1)",
                params![
                    EntityId::new_v7().to_string(),
                    branch.branch_id().to_string(),
                    draft.draft_order_id().to_string(),
                    draft.revision(),
                    snapshot,
                    context.actor_id().to_string(),
                ],
            )
            .is_err());

        let created_payload: String = database
            .connection
            .query_row(
                "SELECT payload_json FROM audit_events WHERE event_type = 'operations.draft_order.created' ORDER BY sequence DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("draft audit payload");
        assert!(created_payload.contains("\"kitchen_note_present\":true"));
        assert!(!created_payload.contains(note));

        let ticket = database
            .send_draft_to_kitchen(draft.draft_order_id(), draft.revision(), &context)
            .expect("ticket sent");
        assert_eq!(ticket.kitchen_note(), Some(note));
        let active = database
            .list_active_kitchen_tickets(branch.branch_id())
            .expect("active tickets");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].kitchen_note(), Some(note));
        let sent_payload: String = database
            .connection
            .query_row(
                "SELECT payload_json FROM audit_events WHERE event_type = 'operations.kitchen_ticket.sent' ORDER BY sequence DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("ticket audit payload");
        assert!(sent_payload.contains("\"kitchen_note_present\":true"));
        assert!(!sent_payload.contains(note));

        assert!(database
            .connection
            .execute(
                "UPDATE kitchen_tickets SET kitchen_note_snapshot = 'rewritten' WHERE kitchen_ticket_id = ?1",
                [ticket.ticket_id().to_string()],
            )
            .is_err());
        // State changes remain valid operational changes; the note survives
        // the transition because its snapshot trigger is field-specific.
        let preparing = database
            .advance_kitchen_ticket(ticket.ticket_id(), ticket.revision(), "preparing", &context)
            .expect("ticket preparing");
        assert_eq!(preparing.kitchen_note(), Some(note));

        let reopened = database
            .reopen_sent_draft_order(
                draft.draft_order_id(),
                draft.revision(),
                &MutationReason::new("Guest requested a corrected ticket").expect("reason"),
                &context,
            )
            .expect("reopened draft");
        assert_eq!(reopened.kitchen_note(), Some(note));
        let restored = database
            .list_open_draft_orders(branch.branch_id())
            .expect("reopened draft projection");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].draft().kitchen_note(), Some(note));
        let reopened_payload: String = database
            .connection
            .query_row(
                "SELECT payload_json FROM audit_events WHERE event_type = 'operations.draft_order.reopened_after_kitchen_send' ORDER BY sequence DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("reopen audit payload");
        assert!(reopened_payload.contains("\"kitchen_note_present\":true"));
        assert!(!reopened_payload.contains(note));
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn kitchen_note_input_normalizes_blank_and_rejects_unsafe_text() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Lemon tea",
                    None,
                    Money::new(8_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")];
        let blank = database
            .save_draft_order_with_kitchen_note(
                DraftOrderSaveRequest::with_kitchen_note(
                    None,
                    None,
                    OrderFulfillment::Takeaway,
                    None,
                    Some(" \t\n "),
                    &lines,
                ),
                &context,
            )
            .expect("blank note is absent");
        assert_eq!(blank.kitchen_note(), None);
        let listed = database
            .list_open_draft_orders(branch.branch_id())
            .expect("draft list");
        assert_eq!(listed[0].draft().kitchen_note(), None);

        let control_error = database
            .save_draft_order_with_kitchen_note(
                DraftOrderSaveRequest::with_kitchen_note(
                    None,
                    None,
                    OrderFulfillment::Takeaway,
                    None,
                    Some("No\u{0007} bell characters"),
                    &lines,
                ),
                &context,
            )
            .expect_err("control character must be rejected");
        assert!(matches!(
            control_error,
            StorageError::InvalidPersistedData(ref message)
                if message == "kitchen note must be at most 500 characters and cannot contain control characters"
        ));

        let too_long = "x".repeat(501);
        assert!(matches!(
            database.save_draft_order_with_kitchen_note(
                DraftOrderSaveRequest::with_kitchen_note(
                    None,
                    None,
                    OrderFulfillment::Takeaway,
                    None,
                    Some(&too_long),
                    &lines,
                ),
                &context,
            ),
            Err(StorageError::InvalidPersistedData(ref message))
                if message == "kitchen note must be at most 500 characters and cannot contain control characters"
        ));
    }

    #[test]
    fn sent_draft_settlement_requires_its_exact_saved_fulfillment_and_lines() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Meals", 1).expect("category"),
                &context,
            )
            .expect("category");
        let saved_product = database
            .create_product(
                &CreateProduct::new(
                    "Veg biryani",
                    Some(category.category_id().clone()),
                    Money::new(18_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("saved product");
        let other_product = database
            .create_product(
                &CreateProduct::new(
                    "Lime soda",
                    Some(category.category_id().clone()),
                    Money::new(4_000, "INR").expect("money"),
                    None,
                    None,
                    2,
                )
                .expect("product"),
                &context,
            )
            .expect("other product");
        let saved_lines =
            vec![SaleLineInput::new(saved_product.product_id().clone(), 2).expect("saved line")];
        let draft = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::DineIn,
                Some("Table 5"),
                &saved_lines,
                &context,
            )
            .expect("saved draft");
        database
            .send_draft_to_kitchen(draft.draft_order_id(), draft.revision(), &context)
            .expect("sent draft");

        let wrong_fulfillment = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            saved_lines.clone(),
        )
        .expect("sale command");
        assert!(matches!(
            database.complete_draft_sale(
                &wrong_fulfillment,
                draft.draft_order_id(),
                draft.revision(),
                &context,
            ),
            Err(StorageError::CatalogConflict)
        ));
        let wrong_lines = CompleteSale::new(
            OrderFulfillment::DineIn,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(other_product.product_id().clone(), 1).expect("other line")],
        )
        .expect("sale command");
        assert!(matches!(
            database.complete_draft_sale(
                &wrong_lines,
                draft.draft_order_id(),
                draft.revision(),
                &context,
            ),
            Err(StorageError::CatalogConflict)
        ));
        let settled = CompleteSale::new(OrderFulfillment::DineIn, PaymentMethod::Cash, saved_lines)
            .expect("matching sale");
        database
            .complete_draft_sale(&settled, draft.draft_order_id(), draft.revision(), &context)
            .expect("settle sent draft exactly once");
        assert!(
            database
                .list_open_draft_orders(branch.branch_id())
                .expect("settled draft omitted")
                .is_empty()
        );
    }

    #[test]
    fn sent_kitchen_cancellation_is_immutable_and_stops_ticket_progression() {
        let (_temp, database, branch) = provisioned_database();
        let owner = database.community_owner_context().expect("owner context");
        let kitchen_account = database
            .create_local_staff("Kitchen one", ActorRole::Kitchen, "654321", &owner)
            .expect("kitchen enrollment");
        let category = database
            .create_category(&CreateCategory::new("Meals", 1).expect("category"), &owner)
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Veg biryani",
                    Some(category.category_id().clone()),
                    Money::new(18_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &owner,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")];
        let draft = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::DineIn,
                Some("Table 12"),
                &lines,
                &owner,
            )
            .expect("draft");
        let ticket = database
            .send_draft_to_kitchen(draft.draft_order_id(), draft.revision(), &owner)
            .expect("ticket");
        let cashier = MutationContext::new(
            branch.branch_id().clone(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            ActorRole::Cashier,
        );
        let reason = MutationReason::new("Customer cancelled before preparation").expect("reason");
        assert!(matches!(
            database.cancel_sent_draft_order(
                draft.draft_order_id(),
                draft.revision(),
                &reason,
                &cashier,
            ),
            Err(StorageError::PermissionDenied)
        ));
        database
            .cancel_sent_draft_order(draft.draft_order_id(), draft.revision(), &reason, &owner)
            .expect("management cancellation");
        assert!(
            database
                .list_open_draft_orders(branch.branch_id())
                .expect("unsettled drafts")
                .is_empty()
        );
        let active_tickets = database
            .list_active_kitchen_tickets(branch.branch_id())
            .expect("active kitchen tickets");
        assert_eq!(active_tickets.len(), 1);
        assert!(active_tickets[0].cancellation_pending());
        assert!(matches!(
            database.advance_kitchen_ticket(
                ticket.ticket_id(),
                ticket.revision(),
                "preparing",
                &owner
            ),
            Err(StorageError::CatalogConflict)
        ));
        assert!(
            database
                .connection
                .execute(
                    "UPDATE kitchen_ticket_cancellation_notices SET reason = 'changed'",
                    [],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM kitchen_ticket_cancellation_notices", [])
                .is_err()
        );

        database.lock_local_staff().expect("owner lock");
        database
            .unlock_local_staff(kitchen_account.staff_id(), "654321")
            .expect("kitchen unlock");
        let kitchen = database
            .community_active_staff_context()
            .expect("active kitchen context");
        database
            .acknowledge_kitchen_ticket_cancellation(ticket.ticket_id(), &kitchen)
            .expect("kitchen acknowledgement");
        assert!(
            database
                .list_active_kitchen_tickets(branch.branch_id())
                .expect("acknowledged ticket omitted")
                .is_empty()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE kitchen_ticket_cancellation_acknowledgements SET acknowledged_at_utc = 'changed'",
                    [],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM kitchen_ticket_cancellation_acknowledgements",
                    []
                )
                .is_err()
        );
        assert!(matches!(
            database.acknowledge_kitchen_ticket_cancellation(ticket.ticket_id(), &kitchen),
            Err(StorageError::CatalogConflict)
        ));
        assert!(matches!(
            database.advance_kitchen_ticket(
                ticket.ticket_id(),
                ticket.revision(),
                "preparing",
                &kitchen
            ),
            Err(StorageError::CatalogConflict)
        ));
        database
            .verify_device_audit_chain(owner.device_id())
            .expect("management audit chain");
        database
            .verify_device_audit_chain(kitchen.device_id())
            .expect("kitchen audit chain");
    }

    #[test]
    fn kitchen_cancellation_facts_reject_direct_cross_linking() {
        let (_temp, database, branch) = provisioned_database();
        let owner = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(&CreateCategory::new("Meals", 1).expect("category"), &owner)
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Veg biryani",
                    Some(category.category_id().clone()),
                    Money::new(18_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &owner,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")];
        let first_draft = database
            .save_draft_order(None, None, OrderFulfillment::Takeaway, None, &lines, &owner)
            .expect("first draft");
        let first_ticket = database
            .send_draft_to_kitchen(first_draft.draft_order_id(), first_draft.revision(), &owner)
            .expect("first ticket");
        let second_draft = database
            .save_draft_order(None, None, OrderFulfillment::Takeaway, None, &lines, &owner)
            .expect("second draft");
        let second_ticket = database
            .send_draft_to_kitchen(
                second_draft.draft_order_id(),
                second_draft.revision(),
                &owner,
            )
            .expect("second ticket");
        let owner_audit_event_id: String = database
            .connection
            .query_row(
                "SELECT event_id FROM audit_events WHERE branch_id = ?1 AND actor_id = ?2 ORDER BY sequence LIMIT 1",
                params![branch.branch_id().to_string(), owner.actor_id().to_string()],
                |row| row.get(0),
            )
            .expect("owner audit event");

        // The ticket/draft pair and audit event all exist, but they do not
        // belong together. The relationship trigger must reject the fake
        // notice before it can become a stop-work instruction.
        assert!(database
            .connection
            .execute(
                "INSERT INTO kitchen_ticket_cancellation_notices (kitchen_ticket_cancellation_notice_id, kitchen_ticket_id, branch_id, draft_order_id, draft_revision, reason, occurred_at_utc, occurred_by_actor_id, audit_event_id) VALUES (?1, ?2, ?3, ?4, ?5, 'direct SQL mismatch', '2026-07-17T00:00:00Z', ?6, ?7)",
                params![
                    EntityId::new_v7().to_string(),
                    first_ticket.ticket_id().to_string(),
                    branch.branch_id().to_string(),
                    second_draft.draft_order_id().to_string(),
                    second_ticket.draft_revision(),
                    owner.actor_id().to_string(),
                    owner_audit_event_id,
                ],
            )
            .is_err());

        database
            .cancel_sent_draft_order(
                first_draft.draft_order_id(),
                first_draft.revision(),
                &MutationReason::new("Customer cancelled the first ticket").expect("reason"),
                &owner,
            )
            .expect("valid cancellation");
        let notice_id: String = database
            .connection
            .query_row(
                "SELECT kitchen_ticket_cancellation_notice_id FROM kitchen_ticket_cancellation_notices WHERE kitchen_ticket_id = ?1",
                [first_ticket.ticket_id().to_string()],
                |row| row.get(0),
            )
            .expect("notice");

        // Acknowledgement attribution must be bound to the audit actor, not
        // only to a valid branch and notice reference.
        assert!(database
            .connection
            .execute(
                "INSERT INTO kitchen_ticket_cancellation_acknowledgements (kitchen_ticket_cancellation_acknowledgement_id, kitchen_ticket_cancellation_notice_id, branch_id, acknowledged_at_utc, acknowledged_by_actor_id, audit_event_id) VALUES (?1, ?2, ?3, '2026-07-17T00:00:00Z', ?4, ?5)",
                params![
                    EntityId::new_v7().to_string(),
                    notice_id,
                    branch.branch_id().to_string(),
                    EntityId::new_v7().to_string(),
                    owner_audit_event_id,
                ],
            )
            .is_err());
        database
            .verify_device_audit_chain(owner.device_id())
            .expect("owner audit chain");
    }

    #[test]
    fn reopening_sent_draft_creates_a_new_revision_and_new_kitchen_ticket() {
        let (_temp, database, branch) = provisioned_database();
        let owner = database.community_owner_context().expect("owner context");
        let kitchen_account = database
            .create_local_staff("Kitchen two", ActorRole::Kitchen, "654321", &owner)
            .expect("kitchen enrollment");
        let category = database
            .create_category(&CreateCategory::new("Meals", 1).expect("category"), &owner)
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Paneer tikka",
                    Some(category.category_id().clone()),
                    Money::new(22_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &owner,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 2).expect("line")];
        let draft = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::DineIn,
                Some("Table 8"),
                &lines,
                &owner,
            )
            .expect("draft");
        let original_ticket = database
            .send_draft_to_kitchen(draft.draft_order_id(), draft.revision(), &owner)
            .expect("original ticket");
        let reopened = database
            .reopen_sent_draft_order(
                draft.draft_order_id(),
                draft.revision(),
                &MutationReason::new("Guest added a dietary change").expect("reason"),
                &owner,
            )
            .expect("reopened revision");
        assert_eq!(reopened.state(), "open");
        assert_eq!(reopened.revision(), draft.revision() + 1);
        let restored = database
            .list_open_draft_orders(branch.branch_id())
            .expect("reopened draft");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].draft().revision(), reopened.revision());
        assert_eq!(restored[0].draft().state(), "open");
        assert_eq!(restored[0].lines(), lines.as_slice());

        let replacement_ticket = database
            .send_draft_to_kitchen(draft.draft_order_id(), reopened.revision(), &owner)
            .expect("replacement ticket");
        assert_ne!(replacement_ticket.ticket_id(), original_ticket.ticket_id());
        assert_eq!(replacement_ticket.draft_revision(), reopened.revision());
        let active_tickets = database
            .list_active_kitchen_tickets(branch.branch_id())
            .expect("old cancellation and replacement ticket");
        assert_eq!(active_tickets.len(), 2);
        assert!(active_tickets.iter().any(|ticket| {
            ticket.ticket_id() == original_ticket.ticket_id() && ticket.cancellation_pending()
        }));
        assert!(active_tickets.iter().any(|ticket| {
            ticket.ticket_id() == replacement_ticket.ticket_id() && !ticket.cancellation_pending()
        }));
        assert!(matches!(
            database.reopen_sent_draft_order(
                draft.draft_order_id(),
                draft.revision(),
                &MutationReason::new("Stale reopen request").expect("reason"),
                &owner,
            ),
            Err(StorageError::CatalogConflict)
        ));

        database.lock_local_staff().expect("owner lock");
        database
            .unlock_local_staff(kitchen_account.staff_id(), "654321")
            .expect("kitchen unlock");
        let kitchen = database
            .community_active_staff_context()
            .expect("active kitchen context");
        database
            .acknowledge_kitchen_ticket_cancellation(original_ticket.ticket_id(), &kitchen)
            .expect("old ticket acknowledgement");
        let remaining = database
            .list_active_kitchen_tickets(branch.branch_id())
            .expect("replacement ticket remains");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].ticket_id(), replacement_ticket.ticket_id());
        database
            .verify_device_audit_chain(owner.device_id())
            .expect("owner audit chain");
        database
            .verify_device_audit_chain(kitchen.device_id())
            .expect("kitchen audit chain");
    }

    #[test]
    fn sent_draft_cancellation_and_reopen_reject_completed_or_settled_orders() {
        let (_temp, database, _branch) = provisioned_database();
        let owner = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(&CreateCategory::new("Meals", 1).expect("category"), &owner)
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Dal makhani",
                    Some(category.category_id().clone()),
                    Money::new(16_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &owner,
            )
            .expect("product");
        let lines = vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")];
        let reason = MutationReason::new("Too late to cancel kitchen order").expect("reason");

        let completed_draft = database
            .save_draft_order(None, None, OrderFulfillment::Takeaway, None, &lines, &owner)
            .expect("completed draft");
        let completed_ticket = database
            .send_draft_to_kitchen(
                completed_draft.draft_order_id(),
                completed_draft.revision(),
                &owner,
            )
            .expect("ticket");
        let preparing = database
            .advance_kitchen_ticket(
                completed_ticket.ticket_id(),
                completed_ticket.revision(),
                "preparing",
                &owner,
            )
            .expect("preparing");
        let ready = database
            .advance_kitchen_ticket(preparing.ticket_id(), preparing.revision(), "ready", &owner)
            .expect("ready");
        database
            .advance_kitchen_ticket(ready.ticket_id(), ready.revision(), "completed", &owner)
            .expect("completed");
        assert!(matches!(
            database.cancel_sent_draft_order(
                completed_draft.draft_order_id(),
                completed_draft.revision(),
                &reason,
                &owner,
            ),
            Err(StorageError::CatalogConflict)
        ));
        assert!(matches!(
            database.reopen_sent_draft_order(
                completed_draft.draft_order_id(),
                completed_draft.revision(),
                &reason,
                &owner,
            ),
            Err(StorageError::CatalogConflict)
        ));

        let settled_draft = database
            .save_draft_order(None, None, OrderFulfillment::Takeaway, None, &lines, &owner)
            .expect("settled draft");
        database
            .send_draft_to_kitchen(
                settled_draft.draft_order_id(),
                settled_draft.revision(),
                &owner,
            )
            .expect("sent ticket");
        let sale = CompleteSale::new(OrderFulfillment::Takeaway, PaymentMethod::Cash, lines)
            .expect("settlement command");
        database
            .complete_draft_sale(
                &sale,
                settled_draft.draft_order_id(),
                settled_draft.revision(),
                &owner,
            )
            .expect("settled draft");
        assert!(matches!(
            database.cancel_sent_draft_order(
                settled_draft.draft_order_id(),
                settled_draft.revision(),
                &reason,
                &owner,
            ),
            Err(StorageError::CatalogConflict)
        ));
        assert!(matches!(
            database.reopen_sent_draft_order(
                settled_draft.draft_order_id(),
                settled_draft.revision(),
                &reason,
                &owner,
            ),
            Err(StorageError::CatalogConflict)
        ));
    }

    #[test]
    fn unsent_draft_cancellation_is_reasoned_and_preserved() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Meals", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Veg biryani",
                    Some(category.category_id().clone()),
                    Money::new(18_000, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        let draft = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::Takeaway,
                None,
                &[SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
                &context,
            )
            .expect("draft");
        database
            .cancel_open_draft_order(
                draft.draft_order_id(),
                draft.revision(),
                &MutationReason::new("Customer changed plans").expect("reason"),
                &context,
            )
            .expect("cancelled");
        assert!(
            database
                .list_open_draft_orders(branch.branch_id())
                .expect("open drafts")
                .is_empty()
        );
        assert!(matches!(
            database.cancel_open_draft_order(
                draft.draft_order_id(),
                draft.revision(),
                &MutationReason::new("Repeat cancellation").expect("reason"),
                &context,
            ),
            Err(StorageError::CatalogConflict)
        ));
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn inventory_control_movements_are_immutable_and_derive_balance() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Rice",
                    None,
                    Money::new(100, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        assert!(matches!(
            database.set_inventory_low_stock_threshold(
                product.product_id(),
                4,
                &MutationReason::new("Initial replenishment policy").expect("reason"),
                &context,
            ),
            Err(StorageError::InventoryNotTracked)
        ));
        database
            .record_inventory_opening(product.product_id(), 12, &context)
            .expect("opening stock");
        database
            .set_inventory_low_stock_threshold(
                product.product_id(),
                15,
                &MutationReason::new("Alert before next delivery").expect("reason"),
                &context,
            )
            .expect("low-stock policy");
        assert_eq!(
            database
                .inventory_low_stock_threshold(branch.branch_id(), product.product_id())
                .expect("threshold"),
            Some(15)
        );
        database
            .clear_inventory_low_stock_threshold(
                product.product_id(),
                &MutationReason::new("Seasonal menu does not need alert").expect("reason"),
                &context,
            )
            .expect("clear low-stock policy");
        assert_eq!(
            database
                .inventory_low_stock_threshold(branch.branch_id(), product.product_id())
                .expect("cleared threshold"),
            None
        );
        database
            .set_inventory_low_stock_threshold(
                product.product_id(),
                15,
                &MutationReason::new("Resume replenishment policy").expect("reason"),
                &context,
            )
            .expect("restore low-stock policy after clear");
        assert!(
            database
                .connection
                .execute(
                    "INSERT INTO inventory_movements (inventory_movement_id, branch_id, product_id, movement_type, quantity_delta, occurred_at_utc, occurred_by_actor_id) VALUES (?1, ?2, ?3, 'opening', 1, '2026-07-17T00:00:00.000Z', ?4)",
                    params![EntityId::new_v7().to_string(), branch.branch_id().to_string(), product.product_id().to_string(), context.actor_id().to_string()],
                )
                .is_err(),
            "SQLite itself must reject a second opening movement"
        );
        database
            .record_inventory_purchase(product.product_id(), 8, &context)
            .expect("purchase stock");
        assert!(matches!(
            database.record_inventory_opening(product.product_id(), 1, &context),
            Err(StorageError::CatalogConflict)
        ));
        database
            .record_inventory_waste(
                product.product_id(),
                3,
                &MutationReason::new("Spoiled during storage").expect("reason"),
                &context,
            )
            .expect("waste stock");
        database
            .record_inventory_adjustment(
                product.product_id(),
                -2,
                &MutationReason::new("Physical count correction").expect("reason"),
                &context,
            )
            .expect("adjustment stock");
        assert_eq!(
            database
                .inventory_balance(branch.branch_id(), product.product_id())
                .expect("balance"),
            15
        );
        assert!(matches!(
            database.record_inventory_waste(
                product.product_id(),
                16,
                &MutationReason::new("Invalid overdraw").expect("reason"),
                &context,
            ),
            Err(StorageError::Sql(_))
        ));
        assert!(
            database
                .connection
                .execute("DELETE FROM inventory_movements", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM inventory_low_stock_threshold_clear_events", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE inventory_low_stock_threshold_events SET threshold_quantity = 0",
                    [],
                )
                .is_err()
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn expenses_are_immutable_audited_and_available_for_reporting() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let expense_id = database
            .record_expense(
                "Supplies",
                &MutationReason::new("Kitchen cleaning supplies").expect("reason"),
                &Money::new(1_250, "INR").expect("amount"),
                PaymentMethod::Upi,
                &context,
            )
            .expect("expense");
        let expenses = database
            .list_recent_expenses(branch.branch_id(), 20)
            .expect("expenses");
        assert_eq!(expenses.len(), 1);
        assert_eq!(expenses[0].expense_id(), &expense_id);
        assert_eq!(expenses[0].category(), "Supplies");
        assert_eq!(expenses[0].amount_minor(), 1_250);
        assert_eq!(expenses[0].payment_method(), "upi");
        assert_eq!(
            database
                .local_sales_summary(
                    branch.branch_id(),
                    &database.current_accounting_date_utc().expect("today")
                )
                .expect("report summary")
                .expense_minor(),
            1_250
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM expenses", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("UPDATE expenses SET amount_minor = 1", [])
                .is_err()
        );
        assert_eq!(
            database
                .list_pending_sync_operations(branch.branch_id())
                .expect("outbox")
                .len(),
            1
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn cash_drawer_sessions_are_single_open_immutable_and_variance_audited() {
        let (_temp, database, _branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let session_id = database
            .open_cash_drawer(1_000, &context)
            .expect("open drawer");
        assert_eq!(
            database
                .current_open_cash_drawer(context.branch_id())
                .expect("open drawer read")
                .expect("open session")
                .session_id,
            session_id
        );
        assert!(matches!(
            database.open_cash_drawer(0, &context),
            Err(StorageError::CatalogConflict)
        ));
        let closure = database
            .close_cash_drawer(&session_id, 900, &context)
            .expect("close drawer");
        assert_eq!(closure.expected_cash_minor, 1_000);
        assert_eq!(closure.variance_minor, -100);
        assert!(matches!(
            database.close_cash_drawer(&session_id, 900, &context),
            Err(StorageError::CatalogConflict)
        ));
        assert!(
            database
                .connection
                .execute("DELETE FROM cash_drawer_closures", [])
                .is_err()
        );
        assert!(
            database
                .current_open_cash_drawer(context.branch_id())
                .expect("closed drawer read")
                .is_none()
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn tracked_stock_is_deducted_atomically_with_sales() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Tracked rice",
                    None,
                    Money::new(100, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        database
            .record_inventory_opening(product.product_id(), 2, &context)
            .expect("opening stock");
        let sell_two = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 2).expect("line")],
        )
        .expect("sale");
        database
            .complete_sale(&sell_two, &context)
            .expect("stock-backed sale");
        assert_eq!(
            database
                .inventory_balance(branch.branch_id(), product.product_id())
                .expect("balance"),
            0
        );

        let sell_one = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale");
        assert!(matches!(
            database.complete_sale(&sell_one, &context),
            Err(StorageError::Sql(_))
        ));
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM invoices", [], |row| row
                    .get::<_, i64>(0))
                .expect("invoice count"),
            1
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn encrypted_database_does_not_have_the_plain_sqlite_header() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([7; 32]);

        let database = LocalDatabase::open(&path, &key).expect("encrypted database");
        database.integrity_check().expect("cipher integrity check");
        assert!(
            database
                .cipher_version()
                .expect("cipher version")
                .starts_with("4.")
        );
        let cipher_status: Option<String> = database
            .connection
            .query_row("PRAGMA cipher_status", [], |row| row.get(0))
            .optional()
            .expect("cipher status query");
        assert_eq!(cipher_status.as_deref(), Some("1"));
        assert_current_migration_manifest(&database);
        drop(database);

        let bytes = fs::read(path).expect("database bytes");
        assert_ne!(&bytes[..16], b"SQLite format 3\0");
    }

    #[test]
    fn wrong_key_fails_closed() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");

        let valid_key = DatabaseKey::from_bytes([3; 32]);
        let wrong_key = DatabaseKey::from_bytes([4; 32]);
        let database = LocalDatabase::open(&path, &valid_key).expect("encrypted database");
        drop(database);

        assert!(LocalDatabase::open(&path, &wrong_key).is_err());
    }

    #[test]
    fn audit_events_are_inserted_and_duplicate_sequences_are_rejected() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([9; 32]);
        let database = LocalDatabase::open(&path, &key).expect("encrypted database");

        let first = NewAuditEvent {
            event_id: "evt-1",
            branch_id: "branch-1",
            actor_id: "cashier-1",
            device_id: "device-1",
            sequence: 1,
            event_type: "invoice.created",
            payload_json: "{\"invoice_id\":\"inv-1\"}",
            occurred_at_utc: "2026-07-16T10:00:00Z",
            previous_hash: None,
            event_hash: b"first",
        };

        database.append_audit_event(first).expect("first event");
        assert_eq!(database.audit_event_count().expect("event count"), 1);

        let duplicate_sequence = NewAuditEvent {
            event_id: "evt-2",
            branch_id: "branch-1",
            actor_id: "cashier-1",
            device_id: "device-1",
            sequence: 1,
            event_type: "invoice.voided",
            payload_json: "{\"invoice_id\":\"inv-1\"}",
            occurred_at_utc: "2026-07-16T10:01:00Z",
            previous_hash: Some(b"first"),
            event_hash: b"second",
        };

        assert!(database.append_audit_event(duplicate_sequence).is_err());
        assert_eq!(database.audit_event_count().expect("event count"), 1);
    }

    #[test]
    fn catalog_operations_are_audited_archived_and_never_hard_deleted() {
        let (_temp, database, branch) = provisioned_database();
        let context = database
            .community_owner_context()
            .expect("stable Community owner context");
        let repeated_context = database
            .community_owner_context()
            .expect("stable Community owner context");
        assert_eq!(context.device_id(), repeated_context.device_id());
        assert_eq!(context.actor_id(), repeated_context.actor_id());
        let category = database
            .create_category(
                &CreateCategory::new("Hot drinks", 10).expect("valid category"),
                &context,
            )
            .expect("category created");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("valid money"),
                    Some("TEA-001"),
                    Some("890100000001"),
                    20,
                )
                .expect("valid product"),
                &context,
            )
            .expect("product created");

        assert_eq!(
            database
                .list_active_categories(branch.branch_id())
                .expect("active categories")
                .len(),
            1
        );
        assert_eq!(
            database
                .list_sale_products(branch.branch_id())
                .expect("sale products")
                .len(),
            1
        );
        assert_eq!(database.audit_event_count().expect("audit count"), 2);
        database
            .verify_device_audit_chain(context.device_id())
            .expect("valid audit chain");

        let reason = MutationReason::new("Seasonal menu refresh").expect("valid reason");
        assert!(matches!(
            database.archive_category(
                category.category_id(),
                category.revision(),
                &reason,
                &context,
            ),
            Err(StorageError::CategoryHasActiveProducts)
        ));
        assert_eq!(database.audit_event_count().expect("audit count"), 2);

        assert!(matches!(
            database.archive_product(
                product.product_id(),
                product.revision() + 1,
                &reason,
                &context,
            ),
            Err(StorageError::CatalogConflict)
        ));
        assert_eq!(database.audit_event_count().expect("audit count"), 2);

        let archived_product = database
            .archive_product(product.product_id(), product.revision(), &reason, &context)
            .expect("product archived");
        assert!(archived_product.archived());
        assert!(!archived_product.is_available());
        assert!(
            database
                .list_sale_products(branch.branch_id())
                .expect("sale products")
                .is_empty()
        );

        let archived_category = database
            .archive_category(
                category.category_id(),
                category.revision(),
                &reason,
                &context,
            )
            .expect("category archived");
        assert!(archived_category.archived());
        assert!(
            database
                .list_active_categories(branch.branch_id())
                .expect("active categories")
                .is_empty()
        );

        let reused_name = database
            .create_category(
                &CreateCategory::new(" hot drinks ", 30).expect("valid category"),
                &context,
            )
            .expect("archived category name can be reused");
        assert_ne!(reused_name.category_id(), category.category_id());
        assert_eq!(database.audit_event_count().expect("audit count"), 5);
        database
            .verify_device_audit_chain(context.device_id())
            .expect("valid audit chain");

        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM products WHERE product_id = ?1",
                    [product.product_id().to_string()],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("UPDATE audit_events SET event_type = 'tampered'", [])
                .is_err()
        );
    }

    #[test]
    fn menu_image_versions_are_encrypted_audited_and_append_only() {
        let (_temp, database, branch) = provisioned_database();
        let context = database
            .community_owner_context()
            .expect("stable Community owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Mains", 0).expect("valid category"),
                &context,
            )
            .expect("category created");
        let first_image = ProductImageContent::built_in("biryani");
        let product = database
            .create_product_with_image(
                &CreateProduct::new(
                    "House biryani",
                    Some(category.category_id().clone()),
                    Money::new(24_000, "INR").expect("valid money"),
                    None,
                    None,
                    0,
                )
                .expect("valid product"),
                Some(&first_image),
                &context,
            )
            .expect("product and image created atomically");

        let images = database
            .list_sale_product_images(branch.branch_id())
            .expect("current product image");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].product_id(), product.product_id());
        assert_eq!(images[0].source_kind(), ProductImageSource::BuiltIn);
        assert_eq!(images[0].asset_key(), Some("biryani"));
        assert_eq!(images[0].image_bytes(), None);
        assert_eq!(database.audit_event_count().expect("audit count"), 3);

        let replacement = ProductImageContent::built_in("chai");
        let assigned = database
            .replace_product_image(product.product_id(), &replacement, &context)
            .expect("new immutable image version");
        assert_eq!(assigned.asset_key(), Some("chai"));
        assert_eq!(database.audit_event_count().expect("audit count"), 4);
        database
            .verify_device_audit_chain(context.device_id())
            .expect("image events preserve the audit chain");

        let version_count: i64 = database
            .connection
            .query_row("SELECT COUNT(*) FROM product_image_versions", [], |row| {
                row.get(0)
            })
            .expect("image version count");
        assert_eq!(version_count, 2);
        assert!(
            database
                .connection
                .execute("DELETE FROM product_image_versions", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM product_image_assignments", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "INSERT OR REPLACE INTO product_image_assignments SELECT * FROM product_image_assignments LIMIT 1",
                    [],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE product_image_assignments SET revision = revision",
                    [],
                )
                .is_err()
        );
        let reason = MutationReason::new("Dish photo has been retired").expect("valid reason");
        assert!(matches!(
            database.delete_unused_product(
                product.product_id(),
                product.revision(),
                &reason,
                &context
            ),
            Err(StorageError::ProductHasRetainedHistory)
        ));

        let invalid_image = ProductImageContent::built_in("not-an-app-asset");
        assert!(
            database
                .create_product_with_image(
                    &CreateProduct::new(
                        "Invalid image item",
                        Some(category.category_id().clone()),
                        Money::new(1_000, "INR").expect("valid money"),
                        None,
                        None,
                        1,
                    )
                    .expect("valid product"),
                    Some(&invalid_image),
                    &context,
                )
                .is_err()
        );
        assert_eq!(
            database
                .list_sale_products(branch.branch_id())
                .expect("sale products")
                .len(),
            1
        );
    }

    #[test]
    fn gotigin_catalog_image_provenance_is_atomic_audited_and_append_only() {
        let (_temp, database, branch) = provisioned_database();
        let context = mutation_context(&database);
        let category = database
            .create_category(
                &CreateCategory::new("Catalogue dishes", 0).expect("category"),
                &context,
            )
            .expect("category created");
        let provenance = ProductImageCatalogProvenance::new(
            "01JTEST.CATALOGUE:IMAGE-1",
            vec![0x2a; 32],
            "Pexels License",
            "https://www.pexels.com/legal-pages/license/",
            GOTIGIN_CATALOG_SERVICE_ORIGIN,
            GOTIGIN_CATALOG_SERVICE_SCHEMA_VERSION,
        )
        .expect("valid catalogue provenance");
        let image = ProductImageContent::gotigin_catalog(
            vec![0xff, 0xd8, 0x01, 0x02, 0xff, 0xd9],
            2,
            1,
            provenance.clone(),
        );
        let product = database
            .create_product_with_image(
                &CreateProduct::new(
                    "Verified catalogue dish",
                    Some(category.category_id().clone()),
                    Money::new(12_500, "INR").expect("price"),
                    None,
                    None,
                    0,
                )
                .expect("product"),
                Some(&image),
                &context,
            )
            .expect("product, image, provenance, and audit commit together");

        let images = database
            .list_catalog_product_images(branch.branch_id())
            .expect("catalogue images");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].product_id(), product.product_id());
        assert_eq!(images[0].source_kind(), ProductImageSource::GotiginCatalog);
        assert_eq!(images[0].catalog_provenance(), Some(&provenance));

        let persisted: (String, Vec<u8>, String, String, String, i64, String) = database
            .connection
            .query_row(
                "SELECT catalog_image_id, original_content_sha256, licence_label, licence_url, service_origin, service_schema_version, audit_event_id FROM product_image_catalog_provenance",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("persisted provenance");
        assert_eq!(persisted.0, provenance.catalog_image_id());
        assert_eq!(persisted.1, provenance.original_content_sha256());
        assert_eq!(persisted.2, provenance.licence_label());
        assert_eq!(persisted.3, provenance.licence_url());
        assert_eq!(persisted.4, provenance.service_origin());
        assert_eq!(persisted.5, provenance.service_schema_version());

        let audit_payload: String = database
            .connection
            .query_row(
                "SELECT payload_json FROM audit_events WHERE event_id = ?1",
                [&persisted.6],
                |row| row.get(0),
            )
            .expect("linked image audit");
        let audit_payload: serde_json::Value =
            serde_json::from_str(&audit_payload).expect("valid audit JSON");
        assert_eq!(audit_payload["after"]["source_kind"], "gotigin_catalog");
        assert_eq!(
            audit_payload["after"]["catalog"]["image_id"],
            provenance.catalog_image_id()
        );
        assert_eq!(
            audit_payload["after"]["catalog"]["original_content_sha256"],
            "2a".repeat(32)
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("catalogue image preserves audit chain");

        assert!(
            database
                .connection
                .execute(
                    "UPDATE product_image_catalog_provenance SET licence_label = licence_label",
                    [],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM product_image_catalog_provenance", [])
                .is_err()
        );
    }

    #[test]
    fn product_price_changes_are_syncable_and_only_history_free_products_can_be_deleted() {
        let (_temp, database, branch) = provisioned_database();
        let context = database
            .community_owner_context()
            .expect("stable Community owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Quick menu", 0).expect("valid category"),
                &context,
            )
            .expect("category created");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Lemon soda",
                    Some(category.category_id().clone()),
                    Money::new(4_000, "INR").expect("valid money"),
                    None,
                    None,
                    0,
                )
                .expect("valid product"),
                &context,
            )
            .expect("product created");
        let reason = MutationReason::new("Seasonal price review").expect("valid reason");
        let repriced = database
            .update_product_price(
                product.product_id(),
                product.revision(),
                &Money::new(4_500, "INR").expect("valid money"),
                &reason,
                &context,
            )
            .expect("price updated");
        assert_eq!(repriced.unit_price().minor_units(), 4_500);
        assert_eq!(repriced.revision(), product.revision() + 1);
        assert_eq!(database.audit_event_count().expect("audit count"), 3);

        let delete_reason = MutationReason::new("Incorrect test item").expect("valid reason");
        assert!(matches!(
            database.delete_unused_product(
                repriced.product_id(),
                repriced.revision(),
                &delete_reason,
                &context,
            ),
            Err(StorageError::ProductHasRetainedHistory)
        ));
        let unused_product = database
            .create_product(
                &CreateProduct::new(
                    "Incorrect test item",
                    Some(category.category_id().clone()),
                    Money::new(1, "INR").expect("valid money"),
                    None,
                    None,
                    1,
                )
                .expect("valid product"),
                &context,
            )
            .expect("unused product created");
        database
            .delete_unused_product(
                unused_product.product_id(),
                unused_product.revision(),
                &delete_reason,
                &context,
            )
            .expect("unused product deleted");
        assert_eq!(
            database
                .list_sale_products(branch.branch_id())
                .expect("sale products")
                .len(),
            1
        );
        assert_eq!(database.audit_event_count().expect("audit count"), 5);
        database
            .verify_device_audit_chain(context.device_id())
            .expect("deletion remains in audit chain");
    }

    #[test]
    fn product_availability_is_revisioned_audited_and_blocks_sales_until_resumed() {
        let (_temp, database, _branch) = provisioned_database();
        let context = mutation_context(&database);
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 0).expect("category"),
                &context,
            )
            .expect("category created");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Soup",
                    Some(category.category_id().clone()),
                    Money::new(500, "INR").expect("price"),
                    None,
                    None,
                    0,
                )
                .expect("product"),
                &context,
            )
            .expect("product created");
        let reason = MutationReason::new("Temporarily sold out").expect("reason");
        let paused = database
            .set_product_availability(
                product.product_id(),
                product.revision(),
                false,
                &reason,
                &context,
            )
            .expect("paused");
        assert!(!paused.is_available());
        assert!(!paused.archived());
        let sale = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale");
        assert!(matches!(
            database.complete_sale(&sale, &context),
            Err(StorageError::ProductUnavailable)
        ));
        let repriced_while_sold_out = database
            .update_product_price(
                paused.product_id(),
                paused.revision(),
                &Money::new(4_500, "INR").expect("valid money"),
                &reason,
                &context,
            )
            .expect("price updated while sold out");
        assert!(!repriced_while_sold_out.is_available());
        assert!(matches!(
            database.set_product_availability(
                product.product_id(),
                paused.revision(),
                true,
                &reason,
                &context
            ),
            Err(StorageError::CatalogConflict)
        ));
        let resumed = database
            .set_product_availability(
                product.product_id(),
                repriced_while_sold_out.revision(),
                true,
                &reason,
                &context,
            )
            .expect("resumed");
        assert!(resumed.is_available());
        database
            .complete_sale(&sale, &context)
            .expect("sale after resume");
        database
            .verify_device_audit_chain(context.device_id())
            .expect("availability audit chain");
    }

    #[test]
    fn catalog_validation_rejects_currency_and_normalized_name_conflicts_atomically() {
        let (_temp, database, branch) = provisioned_database();
        let context = mutation_context(&database);
        let category = database
            .create_category(
                &CreateCategory::new("Café", 0).expect("valid category"),
                &context,
            )
            .expect("category created");

        assert!(
            database
                .create_category(
                    &CreateCategory::new(" Cafe\u{301} ", 1).expect("normalized category"),
                    &context,
                )
                .is_err()
        );
        assert_eq!(database.audit_event_count().expect("audit count"), 1);

        assert!(matches!(
            database.create_product(
                &CreateProduct::new(
                    "Imported coffee",
                    Some(category.category_id().clone()),
                    Money::new(500, "USD").expect("valid money"),
                    None,
                    None,
                    0,
                )
                .expect("valid product"),
                &context,
            ),
            Err(StorageError::CurrencyMismatch { .. })
        ));
        assert_eq!(database.audit_event_count().expect("audit count"), 1);
        assert!(
            database
                .list_sale_products(branch.branch_id())
                .expect("sale products")
                .is_empty()
        );
    }

    #[test]
    fn product_modifier_options_are_immutable_snapshotted_and_audited() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Build-your-own bowl",
                    None,
                    Money::new(1_000, "INR").expect("base price"),
                    None,
                    None,
                    0,
                )
                .expect("product"),
                &context,
            )
            .expect("product created");
        let extra_cheese = database
            .create_product_modifier_option(
                product.product_id(),
                &CreateModifierOption::new(
                    "Extra cheese",
                    Money::new(250, "INR").expect("positive delta"),
                )
                .expect("modifier"),
                &context,
            )
            .expect("option created");
        let avocado = database
            .create_product_modifier_option(
                product.product_id(),
                &CreateModifierOption::new(
                    "Avocado",
                    Money::new(400, "INR").expect("positive delta"),
                )
                .expect("modifier"),
                &context,
            )
            .expect("option created");

        assert!(
            CreateModifierOption::new("Invalid discount", Money::new(-1, "INR").expect("money"))
                .is_err()
        );
        let options = database
            .list_catalog_product_modifier_options(branch.branch_id())
            .expect("catalog modifiers");
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].display_name().display(), "Avocado");
        assert!(!options[0].archived());

        let plain_line = SaleLineInput::new(product.product_id().clone(), 1).expect("plain line");
        let customized_line = SaleLineInput::new(product.product_id().clone(), 2)
            .expect("custom line")
            .with_modifier_options(vec![
                extra_cheese.modifier_option_id().clone(),
                avocado.modifier_option_id().clone(),
            ])
            .expect("modifier selections");
        let sale = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![plain_line, customized_line],
        )
        .expect("same product with distinct modifier configurations is valid");
        let completed = database
            .complete_sale(&sale, &context)
            .expect("sale committed");
        assert_eq!(completed.total().minor_units(), 4_300);

        let receipt = database
            .load_invoice_detail(completed.invoice_id(), &context)
            .expect("receipt");
        assert_eq!(receipt.lines().len(), 2);
        assert!(receipt.lines().iter().any(|line| {
            line.quantity() == 2
                && line.unit_price_minor() == 1_650
                && line.line_total_minor() == 3_300
                && line.modifier_names().iter().any(|name| name == "Avocado")
                && line
                    .modifier_names()
                    .iter()
                    .any(|name| name == "Extra cheese")
        }));
        let modifier_snapshot: String = database
            .connection
            .query_row(
                "SELECT modifier_snapshot_json FROM order_lines WHERE order_id = ?1 AND quantity = 2",
                [completed.order_id().to_string()],
                |row| row.get(0),
            )
            .expect("immutable modifier snapshot");
        assert!(modifier_snapshot.contains("Extra cheese"));
        assert!(modifier_snapshot.contains("Avocado"));

        // The catalogue row cannot be rewritten directly, and the option
        // cannot be silently removed after it has appeared in a receipt.
        assert!(database
            .connection
            .execute(
                "UPDATE product_modifier_options SET price_delta_minor = 1 WHERE modifier_option_id = ?1",
                [extra_cheese.modifier_option_id().to_string()],
            )
            .is_err());
        let reason = MutationReason::new("No longer offered").expect("reason");
        let archived = database
            .archive_product_modifier_option(
                extra_cheese.modifier_option_id(),
                extra_cheese.revision(),
                &reason,
                &context,
            )
            .expect("option archived");
        assert!(archived.archived());
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM product_modifier_options WHERE modifier_option_id = ?1",
                    [extra_cheese.modifier_option_id().to_string()],
                )
                .is_err()
        );

        let unavailable_sale = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![
                SaleLineInput::new(product.product_id().clone(), 1)
                    .expect("line")
                    .with_modifier_options(vec![extra_cheese.modifier_option_id().clone()])
                    .expect("selection"),
            ],
        )
        .expect("command");
        assert!(matches!(
            database.complete_sale(&unavailable_sale, &context),
            Err(StorageError::ModifierOptionUnavailable)
        ));

        let draft = database
            .save_draft_order(
                None,
                None,
                OrderFulfillment::Takeaway,
                None,
                &[SaleLineInput::new(product.product_id().clone(), 1)
                    .expect("line")
                    .with_modifier_options(vec![avocado.modifier_option_id().clone()])
                    .expect("selection")],
                &context,
            )
            .expect("draft saved");
        let ticket = database
            .send_draft_to_kitchen(draft.draft_order_id(), draft.revision(), &context)
            .expect("ticket sent");
        assert!(ticket.line_snapshot_json().contains("Avocado"));
        assert!(
            database
                .list_recent_audit_events(50, &context)
                .expect("audit timeline")
                .iter()
                .any(|event| event.event_type() == "catalog.product_modifier_option.archived")
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn immediate_sale_is_atomic_immutable_and_queued_for_future_sync() {
        let (_temp, database, branch) = provisioned_database();
        let context = database
            .community_owner_context()
            .expect("stable Community owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 0).expect("valid category"),
                &context,
            )
            .expect("category created");
        let chai = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("valid price"),
                    None,
                    None,
                    0,
                )
                .expect("valid product"),
                &context,
            )
            .expect("chai created");
        let samosa = database
            .create_product(
                &CreateProduct::new(
                    "Samosa",
                    Some(category.category_id().clone()),
                    Money::new(1_000, "INR").expect("valid price"),
                    None,
                    None,
                    1,
                )
                .expect("valid product"),
                &context,
            )
            .expect("samosa created");

        let sale = CompleteSale::new(
            ros_core::OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![
                ros_core::SaleLineInput::new(chai.product_id().clone(), 1)
                    .expect("positive chai quantity"),
                ros_core::SaleLineInput::new(samosa.product_id().clone(), 2)
                    .expect("positive samosa quantity"),
            ],
        )
        .expect("valid immediate sale");
        let completed = database
            .complete_sale(&sale, &context)
            .expect("durable sale committed");

        assert_eq!(completed.invoice_number(), 1);
        assert_eq!(completed.total().minor_units(), 4_500);
        assert_eq!(completed.total().currency(), "INR");
        assert_eq!(completed.payment_method(), PaymentMethod::Cash);
        let recent_invoices = database
            .list_recent_invoices(
                branch.branch_id(),
                20,
                &database.current_accounting_date_utc().expect("today"),
            )
            .expect("recent invoices");
        assert_eq!(recent_invoices.len(), 1);
        assert_eq!(recent_invoices[0].invoice_number(), 1);
        assert_eq!(recent_invoices[0].total_minor(), 4_500);
        assert_eq!(recent_invoices[0].payment_method(), "cash");
        let top_items = database
            .list_top_selling_items(
                branch.branch_id(),
                20,
                &database.current_accounting_date_utc().expect("today"),
            )
            .expect("item sales");
        assert_eq!(top_items.len(), 2);
        assert_eq!(top_items[0].display_name(), "Masala chai");
        assert_eq!(top_items[0].quantity(), 1);
        assert_eq!(top_items[0].gross_total_minor(), 2_500);
        let receipt = database
            .load_invoice_detail(completed.invoice_id(), &context)
            .expect("immutable receipt detail");
        assert_eq!(receipt.invoice_number(), 1);
        assert_eq!(receipt.fulfillment(), "takeaway");
        assert_eq!(receipt.total_minor(), 4_500);
        assert_eq!(receipt.refunded_minor(), 0);
        assert_eq!(receipt.lines().len(), 2);
        assert_eq!(receipt.lines()[0].display_name(), "Masala chai");
        assert_eq!(receipt.lines()[1].quantity(), 2);
        assert_eq!(receipt.payments().len(), 1);
        assert_eq!(receipt.payments()[0].payment_method(), "cash");
        assert_eq!(receipt.payments()[0].amount_minor(), 4_500);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM orders", [], |row| row
                    .get::<_, i64>(0))
                .expect("order count"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM order_lines", [], |row| row
                    .get::<_, i64>(0))
                .expect("order line count"),
            2
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM invoices", [], |row| row
                    .get::<_, i64>(0))
                .expect("invoice count"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM payments", [], |row| row
                    .get::<_, i64>(0))
                .expect("payment count"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM sync_outbox_events", [], |row| row
                    .get::<_, i64>(0))
                .expect("outbox count"),
            3
        );
        assert_eq!(database.audit_event_count().expect("audit count"), 6);
        let audit_events = database
            .list_recent_audit_events(20, &context)
            .expect("owner audit timeline");
        assert_eq!(audit_events.len(), 6);
        assert!(
            audit_events
                .iter()
                .any(|event| event.event_type() == "sales.order.finalized")
        );
        assert!(audit_events[0].sequence() > 0);
        assert!(!audit_events[0].occurred_at_utc().is_empty());
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain remains valid");

        assert!(
            database
                .connection
                .execute("UPDATE invoices SET total_minor = 1", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM payments", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM sync_outbox_events", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM orders", [])
                .is_err()
        );
        assert_eq!(
            database
                .connection
                .query_row("PRAGMA recursive_triggers", [], |row| row.get::<_, i64>(0))
                .expect("recursive trigger policy"),
            1
        );
        for replace_statement in [
            "INSERT OR REPLACE INTO audit_events SELECT * FROM audit_events LIMIT 1",
            "INSERT OR REPLACE INTO orders SELECT * FROM orders LIMIT 1",
            "INSERT OR REPLACE INTO order_lines SELECT * FROM order_lines LIMIT 1",
            "INSERT OR REPLACE INTO invoices SELECT * FROM invoices LIMIT 1",
            "INSERT OR REPLACE INTO payments SELECT * FROM payments LIMIT 1",
            "INSERT OR REPLACE INTO sync_outbox_events SELECT * FROM sync_outbox_events LIMIT 1",
            "INSERT OR REPLACE INTO branch_document_sequences SELECT * FROM branch_document_sequences LIMIT 1",
        ] {
            assert!(
                database.connection.execute(replace_statement, []).is_err(),
                "immutable records must reject REPLACE: {replace_statement}"
            );
        }
        assert!(
            database
                .connection
                .execute(
                    "UPDATE branch_document_sequences SET next_invoice_number = 1",
                    [],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM branch_document_sequences", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "
                    INSERT INTO order_lines (
                        order_line_id, order_id, product_id, line_number,
                        product_name_snapshot, quantity, unit_price_minor,
                        line_total_minor, currency_code, created_at_utc
                    ) VALUES (?1, ?2, ?3, 99, 'Tampered chai', 1, 1, 1, 'INR', ?4)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        completed.order_id().to_string(),
                        chai.product_id().to_string(),
                        "2026-07-16T00:00:00Z",
                    ],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "
                    INSERT INTO payments (
                        payment_id, branch_id, invoice_id, payment_method,
                        payment_state, amount_minor, currency_code,
                        recorded_at_utc, recorded_by_actor_id
                    ) VALUES (?1, ?2, ?3, 'cash', 'recorded', 1, 'INR', ?4, ?5)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        context.branch_id().to_string(),
                        completed.invoice_id().to_string(),
                        "2026-07-16T00:00:00Z",
                        context.actor_id().to_string(),
                    ],
                )
                .is_err()
        );
        let first_audit_id: String = database
            .connection
            .query_row(
                "SELECT event_id FROM audit_events ORDER BY sequence LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("first audit identifier");
        assert!(
            database
                .connection
                .execute(
                    "
                    INSERT INTO sync_outbox_events (
                        operation_id, audit_event_id, branch_id, device_id,
                        entity_type, entity_id, event_type, correlation_id,
                        created_at_utc
                    ) VALUES (?1, ?2, ?3, ?4, 'audit', ?2, 'tampered', ?5, ?6)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        first_audit_id,
                        context.branch_id().to_string(),
                        EntityId::new_v7().to_string(),
                        context.correlation_id().to_string(),
                        "2026-07-16T00:00:00Z",
                    ],
                )
                .is_err()
        );

        let second_sale = CompleteSale::new(
            ros_core::OrderFulfillment::DineIn,
            PaymentMethod::Upi,
            vec![
                ros_core::SaleLineInput::new(chai.product_id().clone(), 1)
                    .expect("positive chai quantity"),
            ],
        )
        .expect("valid second sale");
        let second_completed = database
            .complete_sale(&second_sale, &context)
            .expect("second durable sale committed");
        assert_eq!(second_completed.invoice_number(), 2);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM sync_outbox_events", [], |row| row
                    .get::<_, i64>(0))
                .expect("outbox count"),
            6
        );
    }

    #[test]
    fn split_payments_match_the_invoice_and_refund_in_original_tender_order() {
        let (_temp, database, branch) = provisioned_database();
        let context = mutation_context(&database);
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 0).expect("category"),
                &context,
            )
            .expect("category created");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Thali",
                    Some(category.category_id().clone()),
                    Money::new(10_000, "INR").expect("price"),
                    None,
                    None,
                    0,
                )
                .expect("product"),
                &context,
            )
            .expect("product created");
        let split_sale = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale")
        .with_payment_allocations(vec![
            ros_core::PaymentAllocationInput::new(PaymentMethod::Cash, 4_000)
                .expect("cash allocation"),
            ros_core::PaymentAllocationInput::new(PaymentMethod::Upi, 6_000)
                .expect("upi allocation"),
        ])
        .expect("split allocation structure");
        let completed = database
            .complete_sale(&split_sale, &context)
            .expect("split sale committed");
        let allocations: Vec<(i64, String, i64)> = database
            .connection
            .prepare("SELECT payment_sequence, payment_method, amount_minor FROM payments WHERE invoice_id = ?1 ORDER BY payment_sequence")
            .expect("payment query")
            .query_map([completed.invoice_id().to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("allocation rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("allocations");
        assert_eq!(
            allocations,
            vec![(1, "cash".to_owned(), 4_000), (2, "upi".to_owned(), 6_000)]
        );
        assert_eq!(
            database
                .list_recent_invoices(
                    branch.branch_id(),
                    10,
                    &database.current_accounting_date_utc().expect("today")
                )
                .expect("invoices")[0]
                .payment_method(),
            "split"
        );
        let refund_reason = MutationReason::new("Customer returned order").expect("reason");
        let (_approver_id, approval) = dual_approver(&database, &context, "654321");
        database
            .refund_invoice(
                completed.invoice_id(),
                5_000,
                &refund_reason,
                &context,
                &approval,
            )
            .expect("split refund");
        let refunds: Vec<(String, i64)> = database
            .connection
            .prepare("SELECT payment_method_snapshot, amount_minor FROM invoice_refunds WHERE invoice_id = ?1 ORDER BY rowid")
            .expect("refund query")
            .query_map([completed.invoice_id().to_string()], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("refund rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("refunds");
        assert_eq!(
            refunds,
            vec![("cash".to_owned(), 4_000), ("upi".to_owned(), 1_000)]
        );
        let unequal_sale = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale")
        .with_payment_allocations(vec![
            ros_core::PaymentAllocationInput::new(PaymentMethod::Cash, 9_999).expect("allocation"),
        ])
        .expect("allocation structure");
        assert!(database.complete_sale(&unequal_sale, &context).is_err());
        database
            .verify_device_audit_chain(context.device_id())
            .expect("split payment audit chain");
    }

    #[test]
    fn sync_acknowledgements_are_idempotent_and_cannot_be_rewritten() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        database
            .complete_sale(
                &CompleteSale::new(
                    OrderFulfillment::Takeaway,
                    PaymentMethod::Cash,
                    vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
                )
                .expect("sale"),
                &context,
            )
            .expect("sale saved");
        let pending = database
            .list_pending_sync_operations(branch.branch_id())
            .expect("pending operations");
        assert_eq!(pending.len(), 3);
        assert_eq!(pending[0].event_type(), "sales.order.finalized");
        assert_eq!(pending[0].branch_id(), branch.branch_id());
        assert_eq!(pending[0].actor_id(), context.actor_id());
        for operation in &pending {
            let audit_actor_id: String = database
                .connection
                .query_row(
                    "SELECT actor_id FROM audit_events WHERE event_id = ?1",
                    [operation.audit_event_id().to_string()],
                    |row| row.get(0),
                )
                .expect("source audit actor");
            assert_eq!(operation.actor_id().to_string(), audit_actor_id);
        }
        assert!(!pending[0].payload_json().is_empty());
        let operation_id = pending[0].operation_id().clone();
        database
            .acknowledge_sync_operation(&operation_id, "server-event-1")
            .expect("acknowledged");
        database
            .acknowledge_sync_operation(&operation_id, "server-event-1")
            .expect("identical retry is safe");
        assert!(matches!(
            database.acknowledge_sync_operation(&operation_id, "server-event-2"),
            Err(StorageError::CatalogConflict)
        ));
        for operation in pending.iter().skip(1) {
            database
                .acknowledge_sync_operation(operation.operation_id(), "server-event-batch")
                .expect("acknowledged");
        }
        assert!(
            database
                .list_pending_sync_operations(branch.branch_id())
                .expect("pending operations")
                .is_empty()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE sync_acknowledgements SET server_event_id = 'rewritten'",
                    [],
                )
                .is_err()
        );
    }

    #[test]
    fn refunds_are_immutable_and_reduce_only_net_reporting() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        let sale = database
            .complete_sale(
                &CompleteSale::new(
                    OrderFulfillment::Takeaway,
                    PaymentMethod::Cash,
                    vec![SaleLineInput::new(product.product_id().clone(), 2).expect("line")],
                )
                .expect("sale"),
                &context,
            )
            .expect("sale saved");
        let (_approver_id, approval) = dual_approver(&database, &context, "654321");
        database
            .refund_invoice(
                sale.invoice_id(),
                2_000,
                &MutationReason::new("Customer return").expect("reason"),
                &context,
                &approval,
            )
            .expect("partial refund");
        let summary = database
            .local_sales_summary(
                branch.branch_id(),
                &database.current_accounting_date_utc().expect("today"),
            )
            .expect("summary");
        assert_eq!(summary.total_minor(), 3_000);
        assert_eq!(summary.cash_minor(), 3_000);
        assert_eq!(summary.refund_minor(), 2_000);
        assert!(matches!(
            database.refund_invoice(
                sale.invoice_id(),
                3_001,
                &MutationReason::new("Excess refund").expect("reason"),
                &context,
                &approval,
            ),
            Err(StorageError::CatalogConflict)
        ));
        assert!(
            database
                .connection
                .execute("DELETE FROM invoice_refunds", [])
                .is_err()
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn day_scoped_sales_summary_excludes_other_utc_days() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        database
            .complete_sale(
                &CompleteSale::new(
                    OrderFulfillment::Takeaway,
                    PaymentMethod::Cash,
                    vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
                )
                .expect("sale"),
                &context,
            )
            .expect("sale saved");
        let today = database.current_accounting_date_utc().expect("today");
        let today_summary = database
            .local_sales_summary(branch.branch_id(), &today)
            .expect("today summary");
        assert_eq!(today_summary.invoice_count(), 1);
        assert_eq!(today_summary.total_minor(), 2_500);
        let empty = database
            .local_sales_summary(branch.branch_id(), "2099-01-01")
            .expect("future day");
        assert_eq!(empty.invoice_count(), 0);
        assert_eq!(empty.total_minor(), 0);
        assert!(
            database
                .list_recent_invoices(branch.branch_id(), 20, "2099-01-01")
                .expect("future invoices")
                .is_empty()
        );
        assert!(
            database
                .list_top_selling_items(branch.branch_id(), 20, "2099-01-01")
                .expect("future items")
                .is_empty()
        );
    }

    #[test]
    fn day_scoped_summary_includes_discount_and_tax_totals() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Taxed menu", 0).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(10_000, "INR").expect("money"),
                    None,
                    None,
                    0,
                )
                .expect("product"),
                &context,
            )
            .expect("created");
        database
            .create_branch_tax_rate("Service tax", 1_000, &context)
            .expect("tax rate");
        database
            .set_product_tax_treatment(
                product.product_id(),
                "exclusive",
                product.revision(),
                &context,
            )
            .expect("exclusive");
        let sale = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale")
        .with_discount(OrderDiscount::fixed(1_000, "Regular guest courtesy").expect("discount"));
        database
            .complete_sale(&sale, &context)
            .expect("discounted taxed sale");
        let today = database.current_accounting_date_utc().expect("today");
        let summary = database
            .local_sales_summary(branch.branch_id(), &today)
            .expect("summary");
        assert_eq!(summary.discount_minor(), 1_000);
        assert_eq!(summary.tax_minor(), 900);
        assert_eq!(summary.total_minor(), 9_900);
    }

    #[test]
    fn accounting_day_close_is_append_only_and_cannot_reopen() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let today = database.current_accounting_date_utc().expect("today");
        assert!(
            database
                .accounting_day_close(branch.branch_id(), &today)
                .expect("lookup")
                .is_none()
        );
        let closed = database
            .close_accounting_day(
                &today,
                &MutationReason::new("End of service reconciliation").expect("reason"),
                &context,
            )
            .expect("closed");
        assert_eq!(closed.accounting_date_utc(), today);
        assert_eq!(closed.invoice_count(), 0);
        let loaded = database
            .accounting_day_close(branch.branch_id(), &today)
            .expect("lookup")
            .expect("closed day");
        assert_eq!(loaded.close_id(), closed.close_id());
        assert!(matches!(
            database.close_accounting_day(
                &today,
                &MutationReason::new("Second close attempt").expect("reason"),
                &context,
            ),
            Err(StorageError::CatalogConflict)
        ));
        assert!(
            database
                .connection
                .execute("DELETE FROM accounting_day_closes", [])
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("UPDATE accounting_day_closes SET reason = 'tampered'", [],)
                .is_err()
        );
        database
            .verify_device_audit_chain(context.device_id())
            .expect("audit chain");
    }

    #[test]
    fn voids_exclude_invoices_from_day_scoped_reporting() {
        let (_temp, database, branch) = provisioned_database();
        let context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 1).expect("category"),
                &context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &context,
            )
            .expect("product");
        let sale = database
            .complete_sale(
                &CompleteSale::new(
                    OrderFulfillment::Takeaway,
                    PaymentMethod::Cash,
                    vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
                )
                .expect("sale"),
                &context,
            )
            .expect("sale saved");
        let (_approver_id, approval) = dual_approver(&database, &context, "654321");
        database
            .void_invoice(
                sale.invoice_id(),
                &MutationReason::new("Entered on wrong register").expect("reason"),
                &context,
                &approval,
            )
            .expect("voided");
        let today = database.current_accounting_date_utc().expect("today");
        let summary = database
            .local_sales_summary(branch.branch_id(), &today)
            .expect("summary");
        assert_eq!(summary.invoice_count(), 0);
        assert_eq!(summary.total_minor(), 0);
        assert!(matches!(
            database.void_invoice(
                sale.invoice_id(),
                &MutationReason::new("Duplicate void").expect("reason"),
                &context,
                &approval,
            ),
            Err(StorageError::CatalogConflict)
        ));
        assert!(
            database
                .connection
                .execute("DELETE FROM invoice_voids", [])
                .is_err()
        );
    }

    #[test]
    fn same_installation_backup_can_be_verified_and_restored_without_overwrite() {
        let (temp, database, _branch) = provisioned_database();
        let owner = database.community_owner_context().expect("owner");
        let backup_path = temp
            .path()
            .join("verified-backups")
            .join("restore-source.db");
        database
            .create_verified_local_backup(&backup_path, &owner)
            .expect("backup");
        let verified = database.verify_local_backup(&backup_path).expect("verify");
        assert_eq!(verified.schema_version(), SCHEMA_VERSION);
        let restored_path = temp.path().join("restaurant-os.restored.db");
        database
            .restore_verified_local_backup(&backup_path, &restored_path, &owner, false)
            .expect("restore");
        assert!(restored_path.exists());
        assert!(matches!(
            database.restore_verified_local_backup(&backup_path, &restored_path, &owner, false,),
            Err(StorageError::InvalidPersistedData(_))
        ));
    }

    #[test]
    fn verified_financial_csv_requires_an_active_owner_and_excludes_sensitive_fields() {
        let (_temp, database, branch) = fresh_provisioned_database();
        let owner = database
            .list_local_staff()
            .expect("owner account")
            .into_iter()
            .find(|staff| staff.role() == ActorRole::Owner)
            .expect("owner account exists");
        database
            .set_initial_owner_pin("123456")
            .expect("owner pin configured");
        database
            .unlock_local_staff(owner.staff_id(), "123456")
            .expect("owner unlocked");
        let owner_context = database
            .community_active_staff_context()
            .expect("active owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Counter", 1).expect("category"),
                &owner_context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Private menu item",
                    Some(category.category_id().clone()),
                    Money::new(2_500, "INR").expect("money"),
                    None,
                    None,
                    1,
                )
                .expect("product"),
                &owner_context,
            )
            .expect("product");
        let sale = database
            .complete_sale(
                &CompleteSale::new(
                    OrderFulfillment::Takeaway,
                    PaymentMethod::Cash,
                    vec![SaleLineInput::new(product.product_id().clone(), 2).expect("line")],
                )
                .expect("sale"),
                &owner_context,
            )
            .expect("sale saved");
        let (_approver_id, approval) = dual_approver(&database, &owner_context, "654321");
        database
            .refund_invoice(
                sale.invoice_id(),
                2_000,
                &MutationReason::new("Customer alice@example.com correction")
                    .expect("refund reason"),
                &owner_context,
                &approval,
            )
            .expect("refund saved");
        database
            .record_expense(
                "Supplies",
                &MutationReason::new("Vendor bob@example.com cleaning supplies")
                    .expect("expense description"),
                &Money::new(1_250, "INR").expect("expense amount"),
                PaymentMethod::Upi,
                &owner_context,
            )
            .expect("expense saved");

        database.lock_local_staff().expect("owner lock");
        assert!(matches!(
            database.export_verified_community_financial_csv(),
            Err(StorageError::StaffSessionRequired)
        ));

        database
            .unlock_local_staff(owner.staff_id(), "123456")
            .expect("owner unlocked");

        let export = database
            .export_verified_community_financial_csv()
            .expect("owner export");
        let csv = String::from_utf8(export.csv_bytes().to_vec()).expect("UTF-8 CSV");
        assert_eq!(export.record_count(), 4);
        assert_eq!(export.byte_length().expect("byte length"), csv.len() as i64);
        assert!(csv.starts_with(
            "record_type,accounting_date_utc,payment_method,gross_sales_minor,refund_minor,net_sales_minor,expense_minor,currency_code\r\n"
        ));
        assert!(csv.contains(",cash,5000,,,,INR\r\n"));
        assert!(csv.contains(",cash,,2000,,,INR\r\n"));
        assert!(csv.contains(",upi,,,,1250,INR\r\n"));
        assert!(csv.contains("summary,,all,5000,2000,3000,1250,INR\r\n"));
        assert!(!csv.contains("Private menu item"));
        assert!(!csv.contains("alice@example.com"));
        assert!(!csv.contains("bob@example.com"));
        assert!(!csv.contains(&sale.invoice_id().to_string()));
        assert!(!csv.replace("\r\n", "").contains('\n'));

        let owner_context = database
            .community_active_staff_context()
            .expect("active owner context");
        let manager = database
            .create_local_staff("Manager", ActorRole::Manager, "654321", &owner_context)
            .expect("manager account");
        database.lock_local_staff().expect("owner lock");
        database
            .unlock_local_staff(manager.staff_id(), "654321")
            .expect("manager unlocked");
        assert!(matches!(
            database.export_verified_community_financial_csv(),
            Err(StorageError::PermissionDenied)
        ));

        assert_eq!(
            database
                .local_sales_summary(
                    branch.branch_id(),
                    &database.current_accounting_date_utc().expect("today")
                )
                .expect("summary")
                .total_minor(),
            3_000
        );
    }

    #[test]
    fn verified_financial_csv_refuses_a_failed_local_integrity_check() {
        let (_temp, database, branch) = fresh_provisioned_database();
        let owner_context = database.community_owner_context().expect("owner context");
        let owner = database
            .list_local_staff()
            .expect("owner account")
            .into_iter()
            .find(|staff| staff.role() == ActorRole::Owner)
            .expect("owner account exists");
        database
            .set_initial_owner_pin("123456")
            .expect("owner pin configured");
        database
            .unlock_local_staff(owner.staff_id(), "123456")
            .expect("owner unlocked");

        let event_id = EntityId::new_v7().to_string();
        let device_id = EntityId::new_v7().to_string();
        database
            .append_audit_event(NewAuditEvent {
                event_id: &event_id,
                branch_id: &branch.branch_id().to_string(),
                actor_id: &owner_context.actor_id().to_string(),
                device_id: &device_id,
                sequence: 1,
                event_type: "test.financial_export_tamper",
                payload_json: "{}",
                occurred_at_utc: "2026-07-17T00:00:00.000Z",
                previous_hash: None,
                event_hash: b"not-a-valid-audit-hash",
            })
            .expect("test event inserted");

        assert!(matches!(
            database.export_verified_community_financial_csv(),
            Err(StorageError::AuditChainInvalid(_))
        ));
    }

    #[test]
    fn financial_csv_writer_is_rfc4180_safe_and_bounded_before_writing() {
        let mut writer = BoundedCsvWriter::with_maximum_bytes(128);
        writer
            .append_row(&[
                "plain",
                "comma,value",
                "quote\"value",
                "line one\r\nline two",
            ])
            .expect("escaped row");
        assert_eq!(
            String::from_utf8(writer.into_bytes()).expect("UTF-8 row"),
            "plain,\"comma,value\",\"quote\"\"value\",\"line one\r\nline two\"\r\n"
        );

        let mut bounded = BoundedCsvWriter::with_maximum_bytes(8);
        assert!(matches!(
            bounded.append_row(&["too-long"]),
            Err(StorageError::FinancialExportTooLarge)
        ));
        assert!(bounded.into_bytes().is_empty());

        assert!(matches!(
            validate_financial_csv_date("=HYPERLINK(\"https://example.invalid\")"),
            Err(StorageError::FinancialExportIntegrityMismatch)
        ));
        assert!(matches!(
            validate_financial_csv_payment_method("=SUM(A1:A2)"),
            Err(StorageError::FinancialExportIntegrityMismatch)
        ));
        assert!(matches!(
            validate_financial_csv_currency_code("=X"),
            Err(StorageError::FinancialExportIntegrityMismatch)
        ));
    }

    #[test]
    fn online_backup_is_encrypted_verified_and_never_overwrites() {
        let (temp, database, branch) = provisioned_database();
        let owner = database.community_owner_context().expect("owner context");
        let destination = temp.path().join("verified-backups").join("backup.db");
        let backup = database
            .create_verified_local_backup(&destination, &owner)
            .expect("online backup");
        assert_eq!(backup.schema_version(), SCHEMA_VERSION);
        assert!(backup.byte_length() > 0);
        assert_eq!(backup.sha256().len(), 64);
        let bytes = fs::read(&destination).expect("backup bytes");
        assert!(!bytes.starts_with(b"SQLite format 3\0"));
        assert_no_backup_staging_artifacts(&destination);
        assert!(matches!(
            database.create_verified_local_backup(&destination, &owner),
            Err(StorageError::InvalidPersistedData(_))
        ));
        assert_eq!(
            fs::read(&destination).expect("existing backup bytes"),
            bytes,
            "a failed duplicate backup request must not replace its target"
        );
        assert_no_backup_staging_artifacts(&destination);

        let manager = MutationContext::new(
            branch.branch_id().clone(),
            EntityId::new_v7(),
            owner.device_id().clone(),
            EntityId::new_v7(),
            ActorRole::Manager,
        );
        let denied_destination = temp.path().join("verified-backups").join("manager.db");
        assert!(matches!(
            database.create_verified_local_backup(&denied_destination, &manager),
            Err(StorageError::PermissionDenied)
        ));
        assert!(!denied_destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn online_backup_refuses_symlink_destination_without_touching_its_target() {
        let (temp, database, _branch) = provisioned_database();
        let owner = database.community_owner_context().expect("owner context");
        let backup_directory = temp.path().join("verified-backups");
        fs::create_dir_all(&backup_directory).expect("backup directory");
        let protected_target = temp.path().join("protected-target.db");
        let protected_bytes = b"do not replace this file";
        fs::write(&protected_target, protected_bytes).expect("protected target");
        let destination = backup_directory.join("backup.db");
        std::os::unix::fs::symlink(&protected_target, &destination)
            .expect("symlinked backup destination");

        assert!(matches!(
            database.create_verified_local_backup(&destination, &owner),
            Err(StorageError::InvalidPersistedData(_))
        ));
        assert_eq!(
            fs::read(&protected_target).expect("protected target bytes"),
            protected_bytes
        );
        assert!(
            fs::symlink_metadata(&destination)
                .expect("backup destination metadata")
                .file_type()
                .is_symlink(),
            "the caller's existing symlink must remain untouched"
        );
        assert_no_backup_staging_artifacts(&destination);
    }

    #[cfg(unix)]
    #[test]
    fn backup_staging_path_replacement_is_detected_without_deleting_the_replacement() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let destination = temp.path().join("verified-backups").join("backup.db");
        let reservation =
            ReservedLocalBackupDestination::reserve(&destination).expect("staging reservation");
        let staging_path = reservation.path().to_owned();
        let protected_target = temp.path().join("protected-target.db");
        let protected_bytes = b"do not open through a replacement path";
        fs::write(&protected_target, protected_bytes).expect("protected target");
        fs::remove_file(&staging_path).expect("remove original staging path");
        std::os::unix::fs::symlink(&protected_target, &staging_path)
            .expect("replace staging path with symlink");

        assert!(
            Connection::open_with_flags(
                &staging_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )
            .is_err(),
            "SQLite must refuse a symlinked staging path"
        );
        assert!(matches!(
            reservation.ensure_path_still_references_reservation(),
            Err(StorageError::InvalidPersistedData(_))
        ));
        drop(reservation);

        assert_eq!(
            fs::read(&protected_target).expect("protected target bytes"),
            protected_bytes
        );
        assert!(
            fs::symlink_metadata(&staging_path)
                .expect("replacement metadata")
                .file_type()
                .is_symlink(),
            "cleanup must not delete a staging path replaced by another actor"
        );
    }

    #[test]
    fn financial_schema_rejects_cross_branch_facts_even_from_direct_sql() {
        let (_temp, database, branch) = provisioned_database();
        let context = mutation_context(&database);
        let created_at = "2026-07-16T00:00:00Z";
        let organization_id: String = database
            .connection
            .query_row(
                "SELECT organization_id FROM branches WHERE branch_id = ?1",
                [branch.branch_id().to_string()],
                |row| row.get(0),
            )
            .expect("Community organization");
        let other_branch_id = EntityId::new_v7();
        let other_product_id = EntityId::new_v7();
        let primary_order_id = EntityId::new_v7();
        let other_order_id = EntityId::new_v7();
        let other_invoice_id = EntityId::new_v7();

        database
            .connection
            .execute(
                "
                INSERT INTO branches (
                    branch_id, organization_id, display_name, currency_code,
                    time_zone, created_at_utc
                ) VALUES (?1, ?2, 'Second branch', 'INR', 'Asia/Kolkata', ?3)
                ",
                params![other_branch_id.to_string(), organization_id, created_at],
            )
            .expect("second branch");
        database
            .connection
            .execute(
                "
                INSERT INTO products (
                    product_id, branch_id, category_id, display_name, name_key,
                    sku, barcode, unit_price_minor, currency_code,
                    is_available, sort_order, revision, created_at_utc,
                    created_by_actor_id, updated_at_utc, updated_by_actor_id
                ) VALUES (
                    ?1, ?2, NULL, 'Other chai', 'other chai', NULL, NULL,
                    2500, 'INR', 1, 0, 1, ?3, ?4, ?3, ?4
                )
                ",
                params![
                    other_product_id.to_string(),
                    other_branch_id.to_string(),
                    created_at,
                    context.actor_id().to_string(),
                ],
            )
            .expect("second branch product");
        database
            .connection
            .execute(
                "
                INSERT INTO orders (
                    order_id, branch_id, order_type, order_state, currency_code,
                    subtotal_minor, created_at_utc, created_by_actor_id,
                    finalized_at_utc, finalized_by_actor_id
                ) VALUES (?1, ?2, 'takeaway', 'finalized', 'INR', 2500, ?3, ?4, ?3, ?4)
                ",
                params![
                    primary_order_id.to_string(),
                    branch.branch_id().to_string(),
                    created_at,
                    context.actor_id().to_string(),
                ],
            )
            .expect("primary order");
        assert!(
            database
                .connection
                .execute(
                    "
                    INSERT INTO order_lines (
                        order_line_id, order_id, product_id, line_number,
                        product_name_snapshot, quantity, unit_price_minor,
                        line_total_minor, currency_code, created_at_utc
                    ) VALUES (?1, ?2, ?3, 1, 'Other chai', 1, 2500, 2500, 'INR', ?4)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        primary_order_id.to_string(),
                        other_product_id.to_string(),
                        created_at,
                    ],
                )
                .is_err()
        );

        database
            .connection
            .execute(
                "
                INSERT INTO orders (
                    order_id, branch_id, order_type, order_state, currency_code,
                    subtotal_minor, created_at_utc, created_by_actor_id,
                    finalized_at_utc, finalized_by_actor_id
                ) VALUES (?1, ?2, 'takeaway', 'finalized', 'INR', 2500, ?3, ?4, ?3, ?4)
                ",
                params![
                    other_order_id.to_string(),
                    other_branch_id.to_string(),
                    created_at,
                    context.actor_id().to_string(),
                ],
            )
            .expect("second branch order");
        database
            .connection
            .execute(
                "
                INSERT INTO branch_document_sequences (branch_id, next_invoice_number)
                VALUES (?1, 1)
                ",
                [other_branch_id.to_string()],
            )
            .expect("second branch invoice sequence");
        database
            .connection
            .execute(
                "
                INSERT INTO branch_document_sequences (branch_id, next_invoice_number)
                VALUES (?1, 1)
                ",
                [branch.branch_id().to_string()],
            )
            .expect("primary branch invoice sequence");
        assert!(
            database
                .connection
                .execute(
                    "
                    INSERT INTO invoices (
                        invoice_id, branch_id, order_id, invoice_number,
                        invoice_state, subtotal_minor, total_minor, currency_code,
                        finalized_at_utc, finalized_by_actor_id
                    ) VALUES (?1, ?2, ?3, 1, 'finalized', 2500, 2500, 'INR', ?4, ?5)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        branch.branch_id().to_string(),
                        other_order_id.to_string(),
                        created_at,
                        context.actor_id().to_string(),
                    ],
                )
                .is_err()
        );
        database
            .connection
            .execute(
                "
                INSERT INTO invoices (
                    invoice_id, branch_id, order_id, invoice_number,
                    invoice_state, subtotal_minor, total_minor, currency_code,
                    finalized_at_utc, finalized_by_actor_id
                ) VALUES (?1, ?2, ?3, 1, 'finalized', 2500, 2500, 'INR', ?4, ?5)
                ",
                params![
                    other_invoice_id.to_string(),
                    other_branch_id.to_string(),
                    other_order_id.to_string(),
                    created_at,
                    context.actor_id().to_string(),
                ],
            )
            .expect("valid other-branch invoice");
        assert!(
            database
                .connection
                .execute(
                    "
                    INSERT INTO payments (
                        payment_id, branch_id, invoice_id, payment_method,
                        payment_state, amount_minor, currency_code,
                        recorded_at_utc, recorded_by_actor_id
                    ) VALUES (?1, ?2, ?3, 'cash', 'recorded', 2500, 'INR', ?4, ?5)
                    ",
                    params![
                        EntityId::new_v7().to_string(),
                        branch.branch_id().to_string(),
                        other_invoice_id.to_string(),
                        created_at,
                        context.actor_id().to_string(),
                    ],
                )
                .is_err()
        );
    }

    #[test]
    fn a_v1_database_upgrades_without_losing_its_audit_history() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([12; 32]);
        let connection = Connection::open(&path).expect("SQLite connection");
        configure_connection(&connection, &key).expect("configured SQLCipher connection");
        let transaction = begin_immediate_transaction(&connection).expect("migration transaction");
        transaction
            .execute_batch(LOCAL_SCHEMA_V1)
            .expect("v1 schema applied");
        transaction
            .execute(
                "
                INSERT INTO schema_migrations (version, applied_at_utc, checksum)
                VALUES (1, '2026-07-16T00:00:00Z', ?1)
                ",
                [LOCAL_SCHEMA_V1_CHECKSUM],
            )
            .expect("v1 migration history");
        transaction
            .execute(
                "
                INSERT INTO audit_events (
                    event_id, branch_id, actor_id, device_id, sequence, event_type,
                    payload_json, occurred_at_utc, event_hash
                ) VALUES ('legacy-event', 'legacy-branch', 'legacy-actor', 'legacy-device', 1,
                    'legacy.imported', '{}', '2026-07-16T00:00:00Z', X'01')
                ",
                [],
            )
            .expect("legacy audit event");
        transaction
            .execute_batch("PRAGMA user_version = 1;")
            .expect("v1 user version");
        transaction.commit().expect("v1 database committed");
        drop(connection);

        let upgraded = LocalDatabase::open(&path, &key).expect("latest upgrade");
        assert_current_migration_manifest(&upgraded);
        assert_eq!(upgraded.audit_event_count().expect("audit count"), 1);
    }

    #[test]
    fn a_preexisting_partial_v4_table_cannot_be_stamped_as_a_valid_migration() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([14; 32]);
        let connection = Connection::open(&path).expect("SQLite connection");
        configure_connection(&connection, &key).expect("configured SQLCipher connection");
        let transaction = begin_immediate_transaction(&connection).expect("migration transaction");

        for (version, sql, checksum) in [
            (1, LOCAL_SCHEMA_V1, LOCAL_SCHEMA_V1_CHECKSUM),
            (2, LOCAL_SCHEMA_V2, LOCAL_SCHEMA_V2_CHECKSUM),
            (3, LOCAL_SCHEMA_V3, LOCAL_SCHEMA_V3_CHECKSUM),
        ] {
            transaction.execute_batch(sql).expect("historical schema");
            transaction
                .execute(
                    "
                    INSERT INTO schema_migrations (version, applied_at_utc, checksum)
                    VALUES (?1, '2026-07-16T00:00:00Z', ?2)
                    ",
                    params![version, checksum],
                )
                .expect("historical migration record");
            transaction
                .execute_batch(&format!("PRAGMA user_version = {version};"))
                .expect("historical user version");
        }

        // It has every column migration 4 needs, so `CREATE TABLE IF NOT
        // EXISTS` alone would not expose the problem. It deliberately omits
        // STRICT and the financial check constraints.
        transaction
            .execute_batch(
                "
                CREATE TABLE orders (
                    order_id TEXT PRIMARY KEY,
                    branch_id TEXT,
                    order_type TEXT,
                    order_state TEXT,
                    currency_code TEXT,
                    subtotal_minor INTEGER,
                    created_at_utc TEXT,
                    created_by_actor_id TEXT,
                    finalized_at_utc TEXT,
                    finalized_by_actor_id TEXT
                );
                ",
            )
            .expect("partial attacker-controlled orders table");
        transaction.commit().expect("partial database committed");
        drop(connection);

        assert!(matches!(
            LocalDatabase::open(&path, &key),
            Err(StorageError::SchemaContractRejected(
                "orders table contract"
            ))
        ));
    }

    #[test]
    fn migration_checksum_tampering_fails_closed_on_next_open() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([13; 32]);
        let database = LocalDatabase::open(&path, &key).expect("encrypted database");
        database
            .connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'sha256:tampered' WHERE version = 2",
                [],
            )
            .expect("test-only migration mutation");
        drop(database);

        assert!(matches!(
            LocalDatabase::open(&path, &key),
            Err(StorageError::MigrationChecksumMismatch { version: 2, .. })
        ));
    }

    #[test]
    fn rolled_back_migration_write_leaves_database_upgradeable() {
        // Migrations apply inside one immediate transaction. An interrupted
        // write that never commits must leave user_version unchanged so the
        // next open can finish upgrading cleanly.
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([17; 32]);
        {
            let connection = Connection::open(&path).expect("SQLite connection");
            configure_connection(&connection, &key).expect("configured SQLCipher connection");
            let transaction =
                begin_immediate_transaction(&connection).expect("migration transaction");
            transaction
                .execute_batch(LOCAL_SCHEMA_V1)
                .expect("v1 schema applied");
            transaction
                .execute(
                    "
                    INSERT INTO schema_migrations (version, applied_at_utc, checksum)
                    VALUES (1, '2026-07-16T00:00:00Z', ?1)
                    ",
                    [LOCAL_SCHEMA_V1_CHECKSUM],
                )
                .expect("v1 migration history");
            transaction
                .execute_batch("PRAGMA user_version = 1;")
                .expect("v1 user version");
            transaction.commit().expect("v1 database committed");
        }
        {
            let connection = Connection::open(&path).expect("SQLite connection");
            configure_connection(&connection, &key).expect("configured SQLCipher connection");
            let transaction =
                begin_immediate_transaction(&connection).expect("interrupted migration");
            transaction
                .execute_batch(LOCAL_SCHEMA_V2)
                .expect("v2 schema applied inside uncommitted txn");
            transaction
                .execute(
                    "
                    INSERT INTO schema_migrations (version, applied_at_utc, checksum)
                    VALUES (2, '2026-07-16T00:00:00Z', ?1)
                    ",
                    [LOCAL_SCHEMA_V2_CHECKSUM],
                )
                .expect("v2 migration history inside uncommitted txn");
            transaction
                .execute_batch("PRAGMA user_version = 2;")
                .expect("v2 user version inside uncommitted txn");
            // Crash before commit: drop the connection without committing.
            drop(transaction);
            drop(connection);
        }
        let upgraded = LocalDatabase::open(&path, &key).expect("upgrade after interrupted write");
        assert_eq!(
            upgraded.schema_version().expect("schema version"),
            SCHEMA_VERSION
        );
        assert_current_migration_manifest(&upgraded);
    }

    #[test]
    fn orphan_migration_history_ahead_of_user_version_fails_closed() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let key = DatabaseKey::from_bytes([18; 32]);
        let connection = Connection::open(&path).expect("SQLite connection");
        configure_connection(&connection, &key).expect("configured SQLCipher connection");
        let transaction = begin_immediate_transaction(&connection).expect("migration transaction");
        transaction
            .execute_batch(LOCAL_SCHEMA_V1)
            .expect("v1 schema applied");
        transaction
            .execute(
                "
                INSERT INTO schema_migrations (version, applied_at_utc, checksum)
                VALUES (1, '2026-07-16T00:00:00Z', ?1)
                ",
                [LOCAL_SCHEMA_V1_CHECKSUM],
            )
            .expect("v1 migration history");
        transaction
            .execute(
                "
                INSERT INTO schema_migrations (version, applied_at_utc, checksum)
                VALUES (2, '2026-07-16T00:00:00Z', ?1)
                ",
                [LOCAL_SCHEMA_V2_CHECKSUM],
            )
            .expect("orphaned future migration history");
        transaction
            .execute_batch("PRAGMA user_version = 1;")
            .expect("stale user version");
        transaction
            .commit()
            .expect("inconsistent history committed");
        drop(connection);

        assert!(matches!(
            LocalDatabase::open(&path, &key),
            Err(StorageError::MigrationHistoryInvalid(2))
        ));
    }

    #[test]
    fn embedded_migration_sources_match_the_reviewed_checksum_manifest() {
        verify_local_migration_manifest().expect("embedded migration source checksums");
        assert_eq!(
            LOCAL_MIGRATIONS.len(),
            usize::try_from(SCHEMA_VERSION).expect("version")
        );
    }

    #[test]
    fn changed_migration_source_without_a_new_checksum_fails_closed() {
        let tampered = LocalMigration {
            version: 99,
            sql: "-- altered after review\nSELECT 1;\n",
            checksum: LOCAL_SCHEMA_V1_CHECKSUM,
        };
        assert!(matches!(
            verify_migration_source_checksum(&tampered),
            Err(StorageError::MigrationSourceChecksumMismatch { version: 99, .. })
        ));
    }

    #[test]
    fn desktop_keyring_service_ids_use_ros_with_legacy_restaurantos_fallback() {
        assert!(DATABASE_KEYRING_SERVICE.starts_with("com.gotigin.ros."));
        assert!(LEGACY_DATABASE_KEYRING_SERVICE.starts_with("com.gotigin.restaurantos."));
        assert_ne!(DATABASE_KEYRING_SERVICE, LEGACY_DATABASE_KEYRING_SERVICE);
        assert_eq!(
            DatabaseKeySlot::community_default().service,
            DATABASE_KEYRING_SERVICE
        );
    }

    #[test]
    fn key_store_bootstrap_requires_a_consistent_database_and_key_pair() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("restaurant.db");
        let store = MemoryKeyStore::default();
        let slot = DatabaseKeySlot::community_default();

        let database = open_or_create_with_key_store(&path, &store, slot)
            .expect("fresh database is provisioned with a generated key");
        assert_current_migration_manifest(&database);
        assert!(
            store
                .stored_key
                .lock()
                .expect("test key-store lock")
                .is_some()
        );
        drop(database);

        open_or_create_with_key_store(&path, &store, slot)
            .expect("existing database reopens with its stored key");

        let orphaned_database_path = temp.path().join("orphaned.db");
        let direct_key = DatabaseKey::from_bytes([42; 32]);
        LocalDatabase::open(&orphaned_database_path, &direct_key)
            .expect("encrypted orphaned database");
        let empty_store = MemoryKeyStore::default();
        assert!(matches!(
            open_or_create_with_key_store(&orphaned_database_path, &empty_store, slot),
            Err(DatabaseBootstrapError::DatabaseKeyMissing)
        ));

        let key_without_database_path = temp.path().join("missing.db");
        let key_only_store = MemoryKeyStore::default();
        key_only_store
            .store_new(slot, &DatabaseKey::from_bytes([99; 32]))
            .expect("test key stored");
        assert!(matches!(
            open_or_create_with_key_store(&key_without_database_path, &key_only_store, slot),
            Err(DatabaseBootstrapError::ExistingKeyWithoutDatabase)
        ));
    }

    #[test]
    fn exclusive_tax_and_manager_discount_are_priced_and_snapshotted() {
        let (_temp, database, branch) = provisioned_database();
        let owner_context = database.community_owner_context().expect("owner context");
        let category = database
            .create_category(
                &CreateCategory::new("Taxed menu", 0).expect("category"),
                &owner_context,
            )
            .expect("category");
        let product = database
            .create_product(
                &CreateProduct::new(
                    "Masala chai",
                    Some(category.category_id().clone()),
                    Money::new(10_000, "INR").expect("money"),
                    None,
                    None,
                    0,
                )
                .expect("product"),
                &owner_context,
            )
            .expect("created");
        database
            .create_branch_tax_rate("Service tax", 1_000, &owner_context)
            .expect("10% tax rate");
        let taxed = database
            .set_product_tax_treatment(
                product.product_id(),
                "exclusive",
                product.revision(),
                &owner_context,
            )
            .expect("exclusive treatment");
        assert_eq!(taxed.revision(), product.revision() + 1);

        let undiscouned = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale");
        let full_price = database
            .complete_sale(&undiscouned, &owner_context)
            .expect("taxed sale");
        // 100.00 net + 10% exclusive tax = 110.00 payable
        assert_eq!(full_price.total().minor_units(), 11_000);
        let receipt = database
            .load_invoice_detail(full_price.invoice_id(), &owner_context)
            .expect("receipt");
        assert_eq!(receipt.subtotal_minor(), 10_000);
        assert_eq!(receipt.discount_minor(), 0);
        assert_eq!(receipt.tax_minor(), 1_000);
        assert_eq!(receipt.total_minor(), 11_000);

        let cashier_context = MutationContext::new(
            branch.branch_id().clone(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            ActorRole::Cashier,
        );
        let cashier_discounted = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Cash,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale")
        .with_discount(OrderDiscount::fixed(1_000, "Regular guest courtesy").expect("discount"));
        assert!(matches!(
            database.complete_sale(&cashier_discounted, &cashier_context),
            Err(StorageError::PermissionDenied)
        ));

        let discounted = CompleteSale::new(
            OrderFulfillment::Takeaway,
            PaymentMethod::Upi,
            vec![SaleLineInput::new(product.product_id().clone(), 1).expect("line")],
        )
        .expect("sale")
        .with_discount(OrderDiscount::fixed(1_000, "Regular guest courtesy").expect("discount"));
        // Net 100 - 10 discount = 90; exclusive 10% of 90 = 9; payable = 99
        let discounted_sale = database
            .complete_sale(&discounted, &owner_context)
            .expect("manager discount");
        assert_eq!(discounted_sale.total().minor_units(), 9_900);
        let discounted_receipt = database
            .load_invoice_detail(discounted_sale.invoice_id(), &owner_context)
            .expect("discounted receipt");
        assert_eq!(discounted_receipt.subtotal_minor(), 10_000);
        assert_eq!(discounted_receipt.discount_minor(), 1_000);
        assert_eq!(discounted_receipt.tax_minor(), 900);
        assert_eq!(discounted_receipt.total_minor(), 9_900);
    }
}
