//! Durable PostgreSQL transaction for authenticated Professional sync.
//!
//! The device row is locked before its audit anchor is read, making validation,
//! idempotency resolution, inserts, and acknowledgements one serial operation
//! per device. No acknowledgement escapes before the transaction commits.

use std::{collections::HashSet, fmt, sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ros_core::EntityId;
use serde::Serialize;
use sqlx::{PgPool, Row, postgres::PgPoolOptions, types::Json};
use time::{OffsetDateTime, UtcOffset};
use uuid::Uuid;

use crate::{
    auth::DeviceClaims,
    sync::{
        MAX_SYNC_BATCH_OPERATIONS, SyncBatchRequest, SyncChainAnchor, SyncOperationRequest,
        SyncValidationError, ValidatedSyncOperation, validate_sync_batch,
    },
};

const MAX_DATABASE_CONNECTIONS: u32 = 20;
const DATABASE_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct PostgresSyncStore {
    pool: PgPool,
    readiness_schema: Arc<str>,
}

impl PostgresSyncStore {
    pub async fn connect(database_url: &str) -> Result<Self, SyncStoreError> {
        if database_url.trim().is_empty() {
            return Err(SyncStoreError::Unavailable);
        }
        let pool = PgPoolOptions::new()
            .max_connections(MAX_DATABASE_CONNECTIONS)
            .acquire_timeout(DATABASE_ACQUIRE_TIMEOUT)
            .connect(database_url)
            .await
            .map_err(|_| SyncStoreError::Unavailable)?;
        Ok(Self {
            pool,
            readiness_schema: Arc::from("public"),
        })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self {
            pool,
            readiness_schema: Arc::from("public"),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_pool_for_readiness_schema(pool: PgPool, schema: &str) -> Self {
        Self {
            pool,
            readiness_schema: Arc::from(schema),
        }
    }

    pub async fn readiness(&self) -> Result<(), SyncStoreError> {
        let ready: bool = sqlx::query_scalar(
            r#"
            WITH required_tables(table_name) AS (
                VALUES
                    ('organizations'),
                    ('branches'),
                    ('devices'),
                    ('sync_events'),
                    ('device_branch_grants'),
                    ('device_branch_revocations'),
                    ('branch_actors'),
                    ('branch_actor_revocations')
            ),
            required_sync_columns(column_name) AS (
                VALUES
                    ('actor_id'),
                    ('payload_canonical'),
                    ('server_event_id'),
                    ('accepted_at_utc')
            ),
            required_constraints(table_name, constraint_name) AS (
                VALUES
                    ('branches', 'branches_organization_branch_unique'),
                    ('devices', 'devices_organization_device_unique'),
                    ('sync_events', 'sync_events_branch_same_organization'),
                    ('sync_events', 'sync_events_device_same_organization'),
                    ('sync_events', 'sync_events_server_event_id_unique'),
                    ('sync_events', 'sync_events_server_event_id_canonical_v7'),
                    ('sync_events', 'sync_events_event_hash_length'),
                    ('sync_events', 'sync_events_previous_hash_length'),
                    ('sync_events', 'sync_events_event_type_canonical'),
                    ('sync_events', 'sync_events_payload_canonical_matches_json'),
                    ('sync_events', 'sync_events_payload_canonical_size'),
                    ('sync_events', 'sync_events_actor_authorized_for_branch')
            ),
            required_policies(table_name, policy_name) AS (
                VALUES
                    ('organizations', 'tenant_organizations'),
                    ('branches', 'tenant_branches'),
                    ('devices', 'tenant_devices'),
                    ('sync_events', 'tenant_events'),
                    ('device_branch_grants', 'tenant_device_branch_grants'),
                    ('device_branch_revocations', 'tenant_device_branch_revocations'),
                    ('branch_actors', 'tenant_branch_actors'),
                    ('branch_actor_revocations', 'tenant_branch_actor_revocations')
            ),
            required_triggers(table_name, trigger_name) AS (
                VALUES
                    ('sync_events', 'sync_events_reject_update'),
                    ('sync_events', 'sync_events_reject_delete'),
                    ('sync_events', 'sync_events_reject_truncate'),
                    ('device_branch_grants', 'device_branch_grants_reject_update'),
                    ('device_branch_grants', 'device_branch_grants_reject_delete'),
                    ('device_branch_grants', 'device_branch_grants_reject_truncate'),
                    ('device_branch_revocations', 'device_branch_revocations_reject_update'),
                    ('device_branch_revocations', 'device_branch_revocations_reject_delete'),
                    ('device_branch_revocations', 'device_branch_revocations_reject_truncate'),
                    ('branch_actors', 'branch_actors_reject_update'),
                    ('branch_actors', 'branch_actors_reject_delete'),
                    ('branch_actors', 'branch_actors_reject_truncate'),
                    ('branch_actor_revocations', 'branch_actor_revocations_reject_update'),
                    ('branch_actor_revocations', 'branch_actor_revocations_reject_delete'),
                    ('branch_actor_revocations', 'branch_actor_revocations_reject_truncate')
            )
            SELECT
                NOT EXISTS (
                    SELECT 1
                    FROM required_tables required
                    LEFT JOIN pg_namespace namespace
                        ON namespace.nspname = $1
                    LEFT JOIN pg_class relation
                        ON relation.relnamespace = namespace.oid
                       AND relation.relname = required.table_name
                       AND relation.relkind = 'r'
                    WHERE relation.oid IS NULL
                       OR NOT relation.relrowsecurity
                       OR NOT relation.relforcerowsecurity
                )
                AND NOT EXISTS (
                    SELECT 1
                    FROM required_sync_columns required
                    LEFT JOIN information_schema.columns column_info
                        ON column_info.table_schema = $1
                       AND column_info.table_name = 'sync_events'
                       AND column_info.column_name = required.column_name
                    WHERE column_info.column_name IS NULL
                )
                AND NOT EXISTS (
                    SELECT 1
                    FROM required_constraints required
                    LEFT JOIN pg_namespace namespace
                        ON namespace.nspname = $1
                    LEFT JOIN pg_class relation
                        ON relation.relnamespace = namespace.oid
                       AND relation.relname = required.table_name
                    LEFT JOIN pg_constraint constraint_info
                        ON constraint_info.conrelid = relation.oid
                       AND constraint_info.conname = required.constraint_name
                       AND constraint_info.convalidated
                    WHERE constraint_info.oid IS NULL
                )
                AND NOT EXISTS (
                    SELECT 1
                    FROM required_policies required
                    LEFT JOIN pg_namespace namespace
                        ON namespace.nspname = $1
                    LEFT JOIN pg_class relation
                        ON relation.relnamespace = namespace.oid
                       AND relation.relname = required.table_name
                    LEFT JOIN pg_policy policy_info
                        ON policy_info.polrelid = relation.oid
                       AND policy_info.polname = required.policy_name
                       AND policy_info.polqual IS NOT NULL
                       AND policy_info.polwithcheck IS NOT NULL
                    WHERE policy_info.oid IS NULL
                )
                AND NOT EXISTS (
                    SELECT 1
                    FROM required_triggers required
                    LEFT JOIN pg_namespace namespace
                        ON namespace.nspname = $1
                    LEFT JOIN pg_class relation
                        ON relation.relnamespace = namespace.oid
                       AND relation.relname = required.table_name
                    LEFT JOIN pg_trigger trigger_info
                        ON trigger_info.tgrelid = relation.oid
                       AND trigger_info.tgname = required.trigger_name
                       AND NOT trigger_info.tgisinternal
                       AND trigger_info.tgenabled = 'O'
                    WHERE trigger_info.oid IS NULL
                )
            "#,
        )
        .bind(self.readiness_schema.as_ref())
        .fetch_one(&self.pool)
        .await
        .map_err(|_| SyncStoreError::Unavailable)?;
        if ready {
            Ok(())
        } else {
            Err(SyncStoreError::Unavailable)
        }
    }

    pub async fn accept_batch(
        &self,
        claims: &DeviceClaims,
        request: SyncBatchRequest,
    ) -> Result<SyncBatchAcknowledgements, SyncStoreError> {
        authorize_request_identity(claims, &request)?;
        let operation_identities = validate_batch_identity_shape(&request)?;
        let organization_id = claims.organization_id().as_uuid();
        let branch_id = claims.branch_id().as_uuid();
        let device_id = claims.device_id().as_uuid();
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| SyncStoreError::Unavailable)?;

        // Authorization uses a second statement after each parent-row lock to
        // observe a revocation that committed while the lock was being
        // acquired. Pinning READ COMMITTED makes that visibility guarantee
        // independent of a database-level default isolation override.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *transaction)
            .await
            .map_err(|_| SyncStoreError::Unavailable)?;

        sqlx::query("SELECT set_config('app.organization_id', $1, true)")
            .bind(organization_id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(|_| SyncStoreError::Unavailable)?;

        let authorized_device = sqlx::query(
            "SELECT d.device_id FROM devices d JOIN device_branch_grants branch_grant ON branch_grant.organization_id = d.organization_id AND branch_grant.device_id = d.device_id AND branch_grant.branch_id = $3 WHERE d.organization_id = $1 AND d.device_id = $2 AND d.revoked_at IS NULL FOR UPDATE OF d, branch_grant",
        )
        .bind(organization_id)
        .bind(device_id)
        .bind(branch_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| SyncStoreError::Unavailable)?;
        if authorized_device.is_none() {
            return Err(SyncStoreError::Forbidden);
        }
        let device_grant_revoked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM device_branch_revocations WHERE organization_id = $1 AND device_id = $2 AND branch_id = $3)",
        )
        .bind(organization_id)
        .bind(device_id)
        .bind(branch_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| SyncStoreError::Unavailable)?;
        if device_grant_revoked {
            return Err(SyncStoreError::Forbidden);
        }

        let anchor = load_anchor(&mut transaction, organization_id, device_id).await?;
        let mut replayed = Vec::new();
        let mut encountered_new_operation = false;

        for (operation, (operation_id, audit_event_id)) in
            request.operations.iter().zip(operation_identities)
        {
            let existing = load_existing_identity_matches(
                &mut transaction,
                organization_id,
                operation_id,
                audit_event_id,
            )
            .await?;
            let mut existing = existing.into_iter();
            match (existing.next(), existing.next()) {
                (None, None) => {
                    encountered_new_operation = true;
                }
                (Some(record), None) if !encountered_new_operation => {
                    if !record.matches(organization_id, branch_id, device_id, operation)? {
                        return Err(SyncStoreError::Conflict);
                    }
                    replayed.push(record);
                }
                _ => return Err(SyncStoreError::Conflict),
            }
        }

        // A replay prefix is validated from its own durable predecessor, while
        // an entirely new batch starts at the current durable anchor. This
        // validates the complete submitted chain without misclassifying old
        // replayed events as new work against a newer anchor.
        let validation_anchor = replayed
            .first()
            .map(PersistedSyncEvent::predecessor_anchor)
            .transpose()?
            .unwrap_or_else(|| anchor.clone());
        let validated =
            validate_sync_batch(request, &validation_anchor).map_err(SyncStoreError::Invalid)?;
        let replay_count = replayed.len();

        if replay_count > 0 && replay_count < validated.operations().len() {
            let replay_reaches_anchor = replayed
                .last()
                .expect("a non-empty replay prefix has a final event")
                .is_anchor(&anchor)?;
            if !replay_reaches_anchor {
                return Err(SyncStoreError::Conflict);
            }
        }

        let mut acknowledgements = Vec::with_capacity(validated.operations().len());
        for event in &replayed {
            acknowledgements.push(event.acknowledgement("already_accepted")?);
        }

        let new_operations = &validated.operations()[replay_count..];
        if !new_operations.is_empty() {
            authorize_actors(&mut transaction, organization_id, branch_id, new_operations).await?;

            for operation in new_operations {
                let server_event_id = Uuid::now_v7();
                let payload: serde_json::Value = serde_json::from_str(operation.payload_json())
                    .map_err(|_| SyncStoreError::Invalid(SyncValidationError::PayloadInvalid))?;
                let inserted = sqlx::query(
                    "INSERT INTO sync_events (operation_id, organization_id, branch_id, device_id, audit_event_id, actor_id, sequence, event_type, payload_json, payload_canonical, occurred_at_utc, previous_hash, event_hash, server_event_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12, $13, $14) RETURNING accepted_at_utc",
                )
                .bind(operation.operation_id().as_uuid())
                .bind(organization_id)
                .bind(branch_id)
                .bind(device_id)
                .bind(operation.audit_event_id().as_uuid())
                .bind(operation.actor_id().as_uuid())
                .bind(operation.sequence())
                .bind(operation.event_type())
                .bind(Json(payload))
                .bind(operation.payload_json())
                .bind(operation.occurred_at_utc())
                .bind(operation.previous_hash().map(|hash| hash.to_vec()))
                .bind(operation.event_hash().to_vec())
                .bind(server_event_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_insert_error)?;
                let accepted_at: OffsetDateTime = inserted
                    .try_get("accepted_at_utc")
                    .map_err(|_| SyncStoreError::Unavailable)?;
                acknowledgements.push(SyncAcknowledgement {
                    operation_id: operation.operation_id().to_string(),
                    audit_event_id: operation.audit_event_id().to_string(),
                    server_event_id: server_event_id.hyphenated().to_string(),
                    accepted_at_utc: canonical_millisecond_timestamp(accepted_at)?,
                    disposition: "accepted",
                });
            }
        }

        transaction
            .commit()
            .await
            .map_err(|_| SyncStoreError::Unavailable)?;
        Ok(SyncBatchAcknowledgements { acknowledgements })
    }
}

async fn load_anchor(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    device_id: Uuid,
) -> Result<SyncChainAnchor, SyncStoreError> {
    let row = sqlx::query(
        "SELECT sequence, event_hash FROM sync_events WHERE organization_id = $1 AND device_id = $2 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(organization_id)
    .bind(device_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| SyncStoreError::Unavailable)?;
    let Some(row) = row else {
        return Ok(SyncChainAnchor::empty());
    };
    let sequence: i64 = row
        .try_get("sequence")
        .map_err(|_| SyncStoreError::Unavailable)?;
    if sequence < 1 || sequence == i64::MAX {
        return Err(SyncStoreError::Unavailable);
    }
    let event_hash: Vec<u8> = row
        .try_get("event_hash")
        .map_err(|_| SyncStoreError::Unavailable)?;
    let event_hash: [u8; 32] = event_hash
        .try_into()
        .map_err(|_| SyncStoreError::Unavailable)?;
    Ok(SyncChainAnchor::from_durable_event(sequence, event_hash))
}

async fn load_existing_identity_matches(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    operation_id: Uuid,
    audit_event_id: Uuid,
) -> Result<Vec<PersistedSyncEvent>, SyncStoreError> {
    let rows = sqlx::query(
        "SELECT operation_id, organization_id, branch_id, device_id, audit_event_id, actor_id, sequence, event_type, payload_canonical, occurred_at_utc, previous_hash, event_hash, server_event_id, accepted_at_utc FROM sync_events WHERE organization_id = $1 AND (operation_id = $2 OR audit_event_id = $3)",
    )
    .bind(organization_id)
    .bind(operation_id)
    .bind(audit_event_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| SyncStoreError::Unavailable)?;
    rows.into_iter().map(PersistedSyncEvent::from_row).collect()
}

async fn authorize_actors(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    branch_id: Uuid,
    operations: &[ValidatedSyncOperation],
) -> Result<(), SyncStoreError> {
    let mut actors = operations
        .iter()
        .map(|operation| operation.actor_id().as_uuid())
        .collect::<Vec<_>>();
    actors.sort_unstable();
    actors.dedup();
    for actor_id in actors {
        let authorized_actor = sqlx::query(
            "SELECT actor_id FROM branch_actors WHERE organization_id = $1 AND branch_id = $2 AND actor_id = $3 FOR UPDATE",
        )
        .bind(organization_id)
        .bind(branch_id)
        .bind(actor_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| SyncStoreError::Unavailable)?;
        if authorized_actor.is_none() {
            return Err(SyncStoreError::Forbidden);
        }
        let actor_revoked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM branch_actor_revocations WHERE organization_id = $1 AND branch_id = $2 AND actor_id = $3)",
        )
        .bind(organization_id)
        .bind(branch_id)
        .bind(actor_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| SyncStoreError::Unavailable)?;
        if actor_revoked {
            return Err(SyncStoreError::Forbidden);
        }
    }
    Ok(())
}

fn authorize_request_identity(
    claims: &DeviceClaims,
    request: &SyncBatchRequest,
) -> Result<(), SyncStoreError> {
    let branch_id = parse_canonical_entity_id(&request.branch_id)?;
    let device_id = parse_canonical_entity_id(&request.device_id)?;
    if &branch_id != claims.branch_id() || &device_id != claims.device_id() {
        return Err(SyncStoreError::Forbidden);
    }
    Ok(())
}

fn validate_batch_identity_shape(
    request: &SyncBatchRequest,
) -> Result<Vec<(Uuid, Uuid)>, SyncStoreError> {
    if request.operations.is_empty() {
        return Err(SyncStoreError::Invalid(SyncValidationError::BatchEmpty));
    }
    if request.operations.len() > MAX_SYNC_BATCH_OPERATIONS {
        return Err(SyncStoreError::Invalid(SyncValidationError::BatchTooLarge));
    }

    let mut operation_ids = HashSet::with_capacity(request.operations.len());
    let mut audit_event_ids = HashSet::with_capacity(request.operations.len());
    let mut identities = Vec::with_capacity(request.operations.len());
    for operation in &request.operations {
        let operation_id = canonical_uuid(&operation.operation_id)?;
        let audit_event_id = canonical_uuid(&operation.audit_event_id)?;
        if !operation_ids.insert(operation_id) {
            return Err(SyncStoreError::Invalid(
                SyncValidationError::DuplicateOperationId,
            ));
        }
        if !audit_event_ids.insert(audit_event_id) {
            return Err(SyncStoreError::Invalid(
                SyncValidationError::DuplicateAuditEventId,
            ));
        }
        identities.push((operation_id, audit_event_id));
    }
    Ok(identities)
}

fn parse_canonical_entity_id(value: &str) -> Result<EntityId, SyncStoreError> {
    if value.len() != 36 {
        return Err(SyncStoreError::Invalid(
            SyncValidationError::IdentifierInvalid,
        ));
    }
    EntityId::parse(value)
        .map_err(|_| SyncStoreError::Invalid(SyncValidationError::IdentifierInvalid))
}

fn canonical_uuid(value: &str) -> Result<Uuid, SyncStoreError> {
    Ok(parse_canonical_entity_id(value)?.as_uuid())
}

fn map_insert_error(error: sqlx::Error) -> SyncStoreError {
    if error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "23505")
    {
        SyncStoreError::Conflict
    } else {
        SyncStoreError::Unavailable
    }
}

#[derive(Debug)]
struct PersistedSyncEvent {
    operation_id: Uuid,
    organization_id: Uuid,
    branch_id: Uuid,
    device_id: Uuid,
    audit_event_id: Uuid,
    actor_id: Uuid,
    sequence: i64,
    event_type: String,
    payload_canonical: String,
    occurred_at_utc: OffsetDateTime,
    previous_hash: Option<Vec<u8>>,
    event_hash: Vec<u8>,
    server_event_id: Uuid,
    accepted_at_utc: OffsetDateTime,
}

impl PersistedSyncEvent {
    fn from_row(row: sqlx::postgres::PgRow) -> Result<Self, SyncStoreError> {
        Ok(Self {
            operation_id: row
                .try_get("operation_id")
                .map_err(|_| SyncStoreError::Unavailable)?,
            organization_id: row
                .try_get("organization_id")
                .map_err(|_| SyncStoreError::Unavailable)?,
            branch_id: row
                .try_get("branch_id")
                .map_err(|_| SyncStoreError::Unavailable)?,
            device_id: row
                .try_get("device_id")
                .map_err(|_| SyncStoreError::Unavailable)?,
            audit_event_id: row
                .try_get("audit_event_id")
                .map_err(|_| SyncStoreError::Unavailable)?,
            actor_id: row
                .try_get("actor_id")
                .map_err(|_| SyncStoreError::Unavailable)?,
            sequence: row
                .try_get("sequence")
                .map_err(|_| SyncStoreError::Unavailable)?,
            event_type: row
                .try_get("event_type")
                .map_err(|_| SyncStoreError::Unavailable)?,
            payload_canonical: row
                .try_get("payload_canonical")
                .map_err(|_| SyncStoreError::Unavailable)?,
            occurred_at_utc: row
                .try_get("occurred_at_utc")
                .map_err(|_| SyncStoreError::Unavailable)?,
            previous_hash: row
                .try_get("previous_hash")
                .map_err(|_| SyncStoreError::Unavailable)?,
            event_hash: row
                .try_get("event_hash")
                .map_err(|_| SyncStoreError::Unavailable)?,
            server_event_id: row
                .try_get("server_event_id")
                .map_err(|_| SyncStoreError::Unavailable)?,
            accepted_at_utc: row
                .try_get("accepted_at_utc")
                .map_err(|_| SyncStoreError::Unavailable)?,
        })
    }

    fn matches(
        &self,
        organization_id: Uuid,
        branch_id: Uuid,
        device_id: Uuid,
        operation: &SyncOperationRequest,
    ) -> Result<bool, SyncStoreError> {
        let previous_hash = match self.previous_hash.as_deref() {
            Some(value) if value.len() == 32 => Some(STANDARD.encode(value)),
            None => None,
            Some(_) => return Err(SyncStoreError::Unavailable),
        };
        if self.event_hash.len() != 32 {
            return Err(SyncStoreError::Unavailable);
        }
        let occurred_at_utc = canonical_millisecond_timestamp(self.occurred_at_utc)?;
        Ok(self.organization_id == organization_id
            && self.branch_id == branch_id
            && self.device_id == device_id
            && self.operation_id.hyphenated().to_string() == operation.operation_id
            && self.audit_event_id.hyphenated().to_string() == operation.audit_event_id
            && self.actor_id.hyphenated().to_string() == operation.actor_id
            && self.sequence == operation.sequence
            && self.event_type == operation.event_type
            && self.payload_canonical == operation.payload_json
            && occurred_at_utc == operation.occurred_at_utc
            && previous_hash.as_deref() == operation.previous_hash.as_deref()
            && STANDARD.encode(&self.event_hash) == operation.event_hash)
    }

    fn predecessor_anchor(&self) -> Result<SyncChainAnchor, SyncStoreError> {
        match (self.sequence, self.previous_hash.as_deref()) {
            (1, None) => Ok(SyncChainAnchor::empty()),
            (sequence, Some(previous_hash)) if sequence > 1 && sequence < i64::MAX => {
                let previous_hash: [u8; 32] = previous_hash
                    .try_into()
                    .map_err(|_| SyncStoreError::Unavailable)?;
                Ok(SyncChainAnchor::from_durable_event(
                    sequence - 1,
                    previous_hash,
                ))
            }
            _ => Err(SyncStoreError::Unavailable),
        }
    }

    fn is_anchor(&self, anchor: &SyncChainAnchor) -> Result<bool, SyncStoreError> {
        let event_hash: [u8; 32] = self
            .event_hash
            .as_slice()
            .try_into()
            .map_err(|_| SyncStoreError::Unavailable)?;
        Ok(self.sequence == anchor.sequence() && anchor.event_hash() == Some(&event_hash))
    }

    fn acknowledgement(
        &self,
        disposition: &'static str,
    ) -> Result<SyncAcknowledgement, SyncStoreError> {
        Ok(SyncAcknowledgement {
            operation_id: self.operation_id.hyphenated().to_string(),
            audit_event_id: self.audit_event_id.hyphenated().to_string(),
            server_event_id: self.server_event_id.hyphenated().to_string(),
            accepted_at_utc: canonical_millisecond_timestamp(self.accepted_at_utc)?,
            disposition,
        })
    }
}

fn canonical_millisecond_timestamp(value: OffsetDateTime) -> Result<String, SyncStoreError> {
    let value = value.to_offset(UtcOffset::UTC);
    let year = value.year();
    if !(0..=9999).contains(&year) {
        return Err(SyncStoreError::Unavailable);
    }
    Ok(format!(
        "{year:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        u8::from(value.month()),
        value.day(),
        value.hour(),
        value.minute(),
        value.second(),
        value.millisecond(),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SyncBatchAcknowledgements {
    pub acknowledgements: Vec<SyncAcknowledgement>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SyncAcknowledgement {
    pub operation_id: String,
    pub audit_event_id: String,
    pub server_event_id: String,
    pub accepted_at_utc: String,
    pub disposition: &'static str,
}

#[derive(Debug)]
pub enum SyncStoreError {
    Forbidden,
    Invalid(SyncValidationError),
    Conflict,
    Unavailable,
}

impl fmt::Display for SyncStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Forbidden => "the device is not authorized for this sync scope",
            Self::Invalid(_) => "the sync envelope is invalid",
            Self::Conflict => "the sync envelope conflicts with an accepted fact",
            Self::Unavailable => "durable sync storage is unavailable",
        })
    }
}

impl std::error::Error for SyncStoreError {}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;
    use jsonwebtoken::{Algorithm, Header, encode};
    use ros_core::{AuditEventHashInput, audit_event_hash};
    use serde::Serialize;

    use crate::auth::{
        DeviceTokenVerifier, SYNC_WRITE_PERMISSION, test_support::test_ed25519_keys,
    };
    const TEST_TOKEN_ISSUER: &str = "https://identity.test.gotigin.invalid";
    const TEST_TOKEN_AUDIENCE: &str = "restaurant-os-professional-api";

    #[derive(Serialize)]
    struct TestDeviceTokenClaims {
        sub: String,
        organization_id: String,
        branch_id: String,
        device_id: String,
        jti: String,
        permissions: Vec<String>,
        token_version: u8,
        iss: &'static str,
        aud: &'static str,
        iat: i64,
        nbf: i64,
        exp: i64,
    }

    struct TestScope {
        store: PostgresSyncStore,
        pool: PgPool,
        organization_id: EntityId,
        branch_id: EntityId,
        device_id: EntityId,
        actor_id: EntityId,
        claims: DeviceClaims,
    }

    fn operation_with_identities(operation_id: Uuid, audit_event_id: Uuid) -> SyncOperationRequest {
        SyncOperationRequest {
            operation_id: operation_id.hyphenated().to_string(),
            audit_event_id: audit_event_id.hyphenated().to_string(),
            actor_id: Uuid::now_v7().hyphenated().to_string(),
            sequence: 1,
            event_type: "sale.created".to_owned(),
            payload_json: "{}".to_owned(),
            occurred_at_utc: "2025-07-17T00:00:00.000Z".to_owned(),
            previous_hash: None,
            event_hash: STANDARD.encode([7_u8; 32]),
        }
    }

    fn persisted_event(sequence: i64, previous_hash: Option<Vec<u8>>) -> PersistedSyncEvent {
        PersistedSyncEvent {
            operation_id: Uuid::now_v7(),
            organization_id: Uuid::now_v7(),
            branch_id: Uuid::now_v7(),
            device_id: Uuid::now_v7(),
            audit_event_id: Uuid::now_v7(),
            actor_id: Uuid::now_v7(),
            sequence,
            event_type: "sale.created".to_owned(),
            payload_canonical: "{}".to_owned(),
            occurred_at_utc: OffsetDateTime::from_unix_timestamp(1_752_710_400)
                .expect("valid test instant"),
            previous_hash,
            event_hash: vec![7_u8; 32],
            server_event_id: Uuid::now_v7(),
            accepted_at_utc: OffsetDateTime::from_unix_timestamp(1_752_710_401)
                .expect("valid test instant"),
        }
    }

    fn matching_request(event: &PersistedSyncEvent) -> SyncOperationRequest {
        SyncOperationRequest {
            operation_id: event.operation_id.hyphenated().to_string(),
            audit_event_id: event.audit_event_id.hyphenated().to_string(),
            actor_id: event.actor_id.hyphenated().to_string(),
            sequence: event.sequence,
            event_type: event.event_type.clone(),
            payload_json: event.payload_canonical.clone(),
            occurred_at_utc: canonical_millisecond_timestamp(event.occurred_at_utc)
                .expect("canonical test timestamp"),
            previous_hash: event
                .previous_hash
                .as_deref()
                .map(|hash| STANDARD.encode(hash)),
            event_hash: STANDARD.encode(&event.event_hash),
        }
    }

    fn required_test_database_url() -> String {
        let database_url = env::var("ROS_TEST_DATABASE_URL").expect(
            "ROS_TEST_DATABASE_URL is required when running ignored PostgreSQL integration tests",
        );
        assert!(
            database_url.starts_with("postgresql://") || database_url.starts_with("postgres://"),
            "ROS_TEST_DATABASE_URL must be a PostgreSQL connection URL"
        );
        database_url
    }

    fn verified_test_claims(
        organization_id: &EntityId,
        branch_id: &EntityId,
        device_id: &EntityId,
    ) -> DeviceClaims {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let claims = TestDeviceTokenClaims {
            sub: device_id.to_string(),
            organization_id: organization_id.to_string(),
            branch_id: branch_id.to_string(),
            device_id: device_id.to_string(),
            jti: EntityId::new_v7().to_string(),
            permissions: vec![SYNC_WRITE_PERMISSION.to_owned()],
            token_version: 1,
            iss: TEST_TOKEN_ISSUER,
            aud: TEST_TOKEN_AUDIENCE,
            iat: now.saturating_sub(1),
            nbf: now.saturating_sub(1),
            exp: now.saturating_add(60),
        };
        let token = encode(
            &Header::new(Algorithm::EdDSA),
            &claims,
            test_ed25519_keys().encoding_key(),
        )
        .expect("test device token encodes");
        DeviceTokenVerifier::from_ed25519_pem(
            test_ed25519_keys().public_key_pem(),
            TEST_TOKEN_ISSUER,
            TEST_TOKEN_AUDIENCE,
        )
        .expect("valid test verifier")
        .verify_bearer(Some(&format!("Bearer {token}")))
        .expect("test token verifies")
    }

    async fn provision_test_scope() -> TestScope {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&required_test_database_url())
            .await
            .expect("test PostgreSQL connects");
        let store = PostgresSyncStore::from_pool(pool.clone());
        store
            .readiness()
            .await
            .expect("all cloud migrations must be applied before the integration suite runs");

        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let provisioning_actor_id = EntityId::new_v7();

        sqlx::query("INSERT INTO organizations (organization_id, display_name) VALUES ($1, $2)")
            .bind(organization_id.as_uuid())
            .bind("PostgreSQL integration restaurant")
            .execute(&pool)
            .await
            .expect("test organization inserts");
        sqlx::query(
            "INSERT INTO branches (branch_id, organization_id, display_name) VALUES ($1, $2, $3)",
        )
        .bind(branch_id.as_uuid())
        .bind(organization_id.as_uuid())
        .bind("PostgreSQL integration branch")
        .execute(&pool)
        .await
        .expect("test branch inserts");
        sqlx::query(
            "INSERT INTO devices (device_id, organization_id, revoked_at) VALUES ($1, $2, NULL)",
        )
        .bind(device_id.as_uuid())
        .bind(organization_id.as_uuid())
        .execute(&pool)
        .await
        .expect("test device inserts");
        sqlx::query(
            "INSERT INTO device_branch_grants (organization_id, device_id, branch_id, granted_by_actor_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(organization_id.as_uuid())
        .bind(device_id.as_uuid())
        .bind(branch_id.as_uuid())
        .bind(provisioning_actor_id.as_uuid())
        .execute(&pool)
        .await
        .expect("test device grant inserts");
        sqlx::query(
            "INSERT INTO branch_actors (organization_id, branch_id, actor_id, enrolled_by_actor_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(organization_id.as_uuid())
        .bind(branch_id.as_uuid())
        .bind(actor_id.as_uuid())
        .bind(provisioning_actor_id.as_uuid())
        .execute(&pool)
        .await
        .expect("test actor grant inserts");

        let claims = verified_test_claims(&organization_id, &branch_id, &device_id);
        TestScope {
            store,
            pool,
            organization_id,
            branch_id,
            device_id,
            actor_id,
            claims,
        }
    }

    fn valid_sync_operation(
        scope: &TestScope,
        actor_id: &EntityId,
        operation_id: EntityId,
        audit_event_id: EntityId,
        sequence: i64,
        previous_hash: Option<[u8; 32]>,
        payload_json: &str,
    ) -> SyncOperationRequest {
        let event_type = "sales.order.finalized".to_owned();
        let occurred_at_utc = "2026-07-17T00:00:00.000Z".to_owned();
        let event_hash = audit_event_hash(AuditEventHashInput {
            event_id: &audit_event_id.to_string(),
            branch_id: &scope.branch_id.to_string(),
            actor_id: &actor_id.to_string(),
            device_id: &scope.device_id.to_string(),
            sequence,
            event_type: &event_type,
            payload_json,
            occurred_at_utc: &occurred_at_utc,
            previous_hash: previous_hash.as_ref().map(|value| value.as_slice()),
        });
        SyncOperationRequest {
            operation_id: operation_id.to_string(),
            audit_event_id: audit_event_id.to_string(),
            actor_id: actor_id.to_string(),
            sequence,
            event_type,
            payload_json: payload_json.to_owned(),
            occurred_at_utc,
            previous_hash: previous_hash.map(|hash| STANDARD.encode(hash)),
            event_hash: STANDARD.encode(event_hash),
        }
    }

    fn decoded_request_hash(operation: &SyncOperationRequest) -> [u8; 32] {
        STANDARD
            .decode(&operation.event_hash)
            .expect("test operation hash is standard Base64")
            .try_into()
            .expect("test operation hash is 32 bytes")
    }

    fn batch_for(scope: &TestScope, operations: Vec<SyncOperationRequest>) -> SyncBatchRequest {
        SyncBatchRequest {
            branch_id: scope.branch_id.to_string(),
            device_id: scope.device_id.to_string(),
            operations,
        }
    }

    async fn accepted_event_count(scope: &TestScope) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM sync_events WHERE organization_id = $1")
            .bind(scope.organization_id.as_uuid())
            .fetch_one(&scope.pool)
            .await
            .expect("test event count reads")
    }

    #[tokio::test]
    #[ignore = "requires a migrated disposable PostgreSQL database in ROS_TEST_DATABASE_URL"]
    async fn postgresql_acceptance_replay_and_revocation_are_atomic() {
        let scope = provision_test_scope().await;
        let first = valid_sync_operation(
            &scope,
            &scope.actor_id,
            EntityId::new_v7(),
            EntityId::new_v7(),
            1,
            None,
            r#"{"order_id":"first"}"#,
        );
        let first_hash = decoded_request_hash(&first);
        let first_batch = batch_for(&scope, vec![first.clone()]);

        let first_acceptance = scope
            .store
            .accept_batch(&scope.claims, first_batch.clone())
            .await
            .expect("first immutable fact accepts");
        assert_eq!(first_acceptance.acknowledgements.len(), 1);
        assert_eq!(first_acceptance.acknowledgements[0].disposition, "accepted");
        assert_eq!(accepted_event_count(&scope).await, 1);

        let replay = scope
            .store
            .accept_batch(&scope.claims, first_batch)
            .await
            .expect("exact replay returns the original acknowledgement");
        assert_eq!(replay.acknowledgements.len(), 1);
        assert_eq!(replay.acknowledgements[0].disposition, "already_accepted");
        assert_eq!(
            replay.acknowledgements[0].server_event_id,
            first_acceptance.acknowledgements[0].server_event_id
        );
        assert_eq!(
            replay.acknowledgements[0].accepted_at_utc,
            first_acceptance.acknowledgements[0].accepted_at_utc
        );
        assert_eq!(accepted_event_count(&scope).await, 1);

        let second = valid_sync_operation(
            &scope,
            &scope.actor_id,
            EntityId::new_v7(),
            EntityId::new_v7(),
            2,
            Some(first_hash),
            r#"{"order_id":"second"}"#,
        );
        let second_hash = decoded_request_hash(&second);
        let mixed_retry = scope
            .store
            .accept_batch(
                &scope.claims,
                batch_for(&scope, vec![first.clone(), second.clone()]),
            )
            .await
            .expect("a contiguous replay suffix followed by new work accepts atomically");
        assert_eq!(mixed_retry.acknowledgements.len(), 2);
        assert_eq!(
            mixed_retry.acknowledgements[0].disposition,
            "already_accepted"
        );
        assert_eq!(mixed_retry.acknowledgements[1].disposition, "accepted");
        assert_eq!(accepted_event_count(&scope).await, 2);

        let conflicting_identity = valid_sync_operation(
            &scope,
            &scope.actor_id,
            EntityId::parse(&second.operation_id).expect("canonical operation ID"),
            EntityId::new_v7(),
            2,
            Some(first_hash),
            r#"{"order_id":"tampered"}"#,
        );
        assert!(matches!(
            scope
                .store
                .accept_batch(&scope.claims, batch_for(&scope, vec![conflicting_identity]))
                .await,
            Err(SyncStoreError::Conflict)
        ));
        assert_eq!(accepted_event_count(&scope).await, 2);

        let unapproved_actor_operation = valid_sync_operation(
            &scope,
            &EntityId::new_v7(),
            EntityId::new_v7(),
            EntityId::new_v7(),
            3,
            Some(second_hash),
            r#"{"order_id":"unapproved-actor"}"#,
        );
        assert!(matches!(
            scope
                .store
                .accept_batch(
                    &scope.claims,
                    batch_for(&scope, vec![unapproved_actor_operation])
                )
                .await,
            Err(SyncStoreError::Forbidden)
        ));
        assert_eq!(accepted_event_count(&scope).await, 2);

        sqlx::query(
            "INSERT INTO device_branch_revocations (organization_id, device_id, branch_id, revoked_by_actor_id, reason) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(scope.organization_id.as_uuid())
        .bind(scope.device_id.as_uuid())
        .bind(scope.branch_id.as_uuid())
        .bind(EntityId::new_v7().as_uuid())
        .bind("integration test revocation")
        .execute(&scope.pool)
        .await
        .expect("test revocation inserts");

        let revoked_device_operation = valid_sync_operation(
            &scope,
            &scope.actor_id,
            EntityId::new_v7(),
            EntityId::new_v7(),
            3,
            Some(second_hash),
            r#"{"order_id":"revoked-device"}"#,
        );
        assert!(matches!(
            scope
                .store
                .accept_batch(
                    &scope.claims,
                    batch_for(&scope, vec![revoked_device_operation])
                )
                .await,
            Err(SyncStoreError::Forbidden)
        ));
        assert_eq!(accepted_event_count(&scope).await, 2);
    }

    #[tokio::test]
    #[ignore = "requires a migrated disposable PostgreSQL database in ROS_TEST_DATABASE_URL"]
    async fn postgresql_concurrent_device_retries_share_one_durable_fact() {
        let scope = provision_test_scope().await;
        let operation = valid_sync_operation(
            &scope,
            &scope.actor_id,
            EntityId::new_v7(),
            EntityId::new_v7(),
            1,
            None,
            r#"{"order_id":"concurrent-retry"}"#,
        );
        let first_store = scope.store.clone();
        let first_claims = scope.claims.clone();
        let first_batch = batch_for(&scope, vec![operation.clone()]);
        let second_store = scope.store.clone();
        let second_claims = scope.claims.clone();
        let second_batch = batch_for(&scope, vec![operation]);

        let first = async move { first_store.accept_batch(&first_claims, first_batch).await };
        let second = async move {
            second_store
                .accept_batch(&second_claims, second_batch)
                .await
        };
        let (first, second) = tokio::join!(first, second);
        let first = first.expect("one concurrent retry resolves");
        let second = second.expect("the other concurrent retry resolves");
        assert_eq!(first.acknowledgements.len(), 1);
        assert_eq!(second.acknowledgements.len(), 1);
        assert_eq!(accepted_event_count(&scope).await, 1);

        let first_acknowledgement = &first.acknowledgements[0];
        let second_acknowledgement = &second.acknowledgements[0];
        assert_ne!(
            first_acknowledgement.disposition, second_acknowledgement.disposition,
            "exactly one transaction creates the immutable fact"
        );
        assert!(
            matches!(
                first_acknowledgement.disposition,
                "accepted" | "already_accepted"
            ) && matches!(
                second_acknowledgement.disposition,
                "accepted" | "already_accepted"
            )
        );
        assert_eq!(
            first_acknowledgement.server_event_id,
            second_acknowledgement.server_event_id
        );
        assert_eq!(
            first_acknowledgement.accepted_at_utc,
            second_acknowledgement.accepted_at_utc
        );
    }

    #[test]
    fn canonical_acknowledgement_timestamp_always_has_milliseconds_and_z() {
        let instant = OffsetDateTime::from_unix_timestamp_nanos(1_752_710_400_123_987_654)
            .expect("valid instant");
        let formatted = canonical_millisecond_timestamp(instant).expect("canonical timestamp");
        assert_eq!(formatted, "2025-07-17T00:00:00.123Z");
        assert_eq!(formatted.len(), 24);
    }

    #[test]
    fn identity_preflight_rejects_empty_oversized_and_duplicate_batches() {
        let branch_id = Uuid::now_v7().hyphenated().to_string();
        let device_id = Uuid::now_v7().hyphenated().to_string();
        let empty = SyncBatchRequest {
            branch_id: branch_id.clone(),
            device_id: device_id.clone(),
            operations: Vec::new(),
        };
        assert!(matches!(
            validate_batch_identity_shape(&empty),
            Err(SyncStoreError::Invalid(SyncValidationError::BatchEmpty))
        ));

        let repeated = operation_with_identities(Uuid::now_v7(), Uuid::now_v7());
        let oversized = SyncBatchRequest {
            branch_id: branch_id.clone(),
            device_id: device_id.clone(),
            operations: vec![repeated; MAX_SYNC_BATCH_OPERATIONS + 1],
        };
        assert!(matches!(
            validate_batch_identity_shape(&oversized),
            Err(SyncStoreError::Invalid(SyncValidationError::BatchTooLarge))
        ));

        let duplicate_operation_id = Uuid::now_v7();
        let duplicate_operation = SyncBatchRequest {
            branch_id: branch_id.clone(),
            device_id: device_id.clone(),
            operations: vec![
                operation_with_identities(duplicate_operation_id, Uuid::now_v7()),
                operation_with_identities(duplicate_operation_id, Uuid::now_v7()),
            ],
        };
        assert!(matches!(
            validate_batch_identity_shape(&duplicate_operation),
            Err(SyncStoreError::Invalid(
                SyncValidationError::DuplicateOperationId
            ))
        ));

        let duplicate_audit_event_id = Uuid::now_v7();
        let duplicate_audit_event = SyncBatchRequest {
            branch_id,
            device_id,
            operations: vec![
                operation_with_identities(Uuid::now_v7(), duplicate_audit_event_id),
                operation_with_identities(Uuid::now_v7(), duplicate_audit_event_id),
            ],
        };
        assert!(matches!(
            validate_batch_identity_shape(&duplicate_audit_event),
            Err(SyncStoreError::Invalid(
                SyncValidationError::DuplicateAuditEventId
            ))
        ));
    }

    #[test]
    fn replay_match_requires_exact_hash_encoding_and_absence() {
        let genesis = persisted_event(1, None);
        let mut request = matching_request(&genesis);
        assert!(
            genesis
                .matches(
                    genesis.organization_id,
                    genesis.branch_id,
                    genesis.device_id,
                    &request
                )
                .expect("coherent durable event")
        );

        request.previous_hash = Some("not-base64".to_owned());
        assert!(
            !genesis
                .matches(
                    genesis.organization_id,
                    genesis.branch_id,
                    genesis.device_id,
                    &request
                )
                .expect("coherent durable event")
        );

        let linked = persisted_event(2, Some(vec![3_u8; 32]));
        let mut request = matching_request(&linked);
        request.previous_hash = request
            .previous_hash
            .as_deref()
            .map(|value| value.trim_end_matches('=').to_owned());
        assert!(
            !linked
                .matches(
                    linked.organization_id,
                    linked.branch_id,
                    linked.device_id,
                    &request
                )
                .expect("coherent durable event")
        );

        let malformed = persisted_event(2, Some(vec![3_u8; 31]));
        assert!(matches!(
            malformed.matches(
                malformed.organization_id,
                malformed.branch_id,
                malformed.device_id,
                &matching_request(&malformed)
            ),
            Err(SyncStoreError::Unavailable)
        ));
    }

    #[test]
    fn replay_anchor_helpers_fail_closed_on_incoherent_durable_rows() {
        let event = persisted_event(5, Some(vec![3_u8; 32]));
        let predecessor = event.predecessor_anchor().expect("valid predecessor");
        assert_eq!(predecessor.sequence(), 4);
        assert_eq!(predecessor.event_hash(), Some(&[3_u8; 32]));

        let current = SyncChainAnchor::from_durable_event(5, [7_u8; 32]);
        assert!(event.is_anchor(&current).expect("valid event hash"));
        assert!(
            !event
                .is_anchor(&SyncChainAnchor::from_durable_event(4, [7_u8; 32]))
                .expect("valid event hash")
        );

        assert!(matches!(
            persisted_event(1, Some(vec![3_u8; 32])).predecessor_anchor(),
            Err(SyncStoreError::Unavailable)
        ));
        assert!(matches!(
            persisted_event(2, None).predecessor_anchor(),
            Err(SyncStoreError::Unavailable)
        ));
        assert!(matches!(
            persisted_event(2, Some(vec![3_u8; 31])).predecessor_anchor(),
            Err(SyncStoreError::Unavailable)
        ));
        assert!(matches!(
            persisted_event(i64::MAX, Some(vec![3_u8; 32])).predecessor_anchor(),
            Err(SyncStoreError::Unavailable)
        ));
    }

    #[test]
    fn replay_acknowledgement_uses_the_public_contract_disposition() {
        let event = persisted_event(1, None);
        let acknowledgement = event
            .acknowledgement("already_accepted")
            .expect("valid acknowledgement");
        assert_eq!(acknowledgement.disposition, "already_accepted");
        assert_eq!(
            acknowledgement.server_event_id,
            event.server_event_id.to_string()
        );
    }
}
