//! Provider-neutral validation for Professional immutable sync envelopes.
//!
//! HTTP authentication and PostgreSQL persistence deliberately sit outside
//! this module. The database transaction supplies the durable per-device
//! anchor, this module verifies exactly what will be committed, and only then
//! may the transport write an idempotency acknowledgement.

use std::{collections::HashSet, fmt};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ros_core::{AuditEventHashInput, EntityId, audit_event_hash};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub const MAX_SYNC_BATCH_OPERATIONS: usize = 200;
pub const MAX_SYNC_EVENT_TYPE_BYTES: usize = 200;
pub const MAX_SYNC_PAYLOAD_BYTES: usize = 1_048_576;
const AUDIT_HASH_BYTES: usize = 32;
const CANONICAL_UUID_V7_BYTES: usize = 36;
const CANONICAL_UTC_MILLISECOND_TIMESTAMP_BYTES: usize = 24;

/// Untrusted request shape received at the Professional API boundary. Each
/// batch belongs to one authenticated branch/device pair; the caller must
/// still compare these IDs with verified tenant/device claims before use.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SyncBatchRequest {
    pub branch_id: String,
    pub device_id: String,
    pub operations: Vec<SyncOperationRequest>,
}

/// The immutable audit envelope uploaded by a local client. `payload_json`
/// stays a string because it is covered byte-for-byte by the audit hash.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SyncOperationRequest {
    pub operation_id: String,
    pub audit_event_id: String,
    pub actor_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub payload_json: String,
    pub occurred_at_utc: String,
    pub previous_hash: Option<String>,
    pub event_hash: String,
}

/// The last durable event accepted for one device. The database must load it
/// while holding the same transaction that will insert a new batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncChainAnchor {
    sequence: i64,
    event_hash: Option<[u8; AUDIT_HASH_BYTES]>,
}

impl SyncChainAnchor {
    pub const fn empty() -> Self {
        Self {
            sequence: 0,
            event_hash: None,
        }
    }

    pub const fn from_durable_event(sequence: i64, event_hash: [u8; AUDIT_HASH_BYTES]) -> Self {
        Self {
            sequence,
            event_hash: Some(event_hash),
        }
    }

    pub const fn sequence(&self) -> i64 {
        self.sequence
    }

    pub const fn event_hash(&self) -> Option<&[u8; AUDIT_HASH_BYTES]> {
        self.event_hash.as_ref()
    }
}

/// Parsed, hash-verified values ready for a tenant-scoped database insert.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSyncBatch {
    branch_id: EntityId,
    device_id: EntityId,
    operations: Vec<ValidatedSyncOperation>,
}

impl ValidatedSyncBatch {
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    pub fn device_id(&self) -> &EntityId {
        &self.device_id
    }

    pub fn operations(&self) -> &[ValidatedSyncOperation] {
        &self.operations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSyncOperation {
    operation_id: EntityId,
    audit_event_id: EntityId,
    actor_id: EntityId,
    sequence: i64,
    event_type: String,
    payload_json: String,
    occurred_at_utc: String,
    previous_hash: Option<[u8; AUDIT_HASH_BYTES]>,
    event_hash: [u8; AUDIT_HASH_BYTES],
}

impl ValidatedSyncOperation {
    pub fn operation_id(&self) -> &EntityId {
        &self.operation_id
    }

    pub fn audit_event_id(&self) -> &EntityId {
        &self.audit_event_id
    }

    pub fn actor_id(&self) -> &EntityId {
        &self.actor_id
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

    pub fn previous_hash(&self) -> Option<&[u8; AUDIT_HASH_BYTES]> {
        self.previous_hash.as_ref()
    }

    pub fn event_hash(&self) -> &[u8; AUDIT_HASH_BYTES] {
        &self.event_hash
    }
}

/// Safe failure categories for a 400/409 API response. No variant stores or
/// formats raw event data, so validation errors cannot leak counter payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncValidationError {
    BatchEmpty,
    BatchTooLarge,
    IdentifierInvalid,
    DuplicateOperationId,
    DuplicateAuditEventId,
    AnchorInvalid,
    SequenceInvalid,
    SequenceDiscontinuity,
    EventTypeInvalid,
    PayloadInvalid,
    TimestampInvalid,
    HashEncodingInvalid,
    PreviousHashDiscontinuity,
    EventHashMismatch,
}

impl fmt::Display for SyncValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BatchEmpty => "a sync batch must contain at least one operation",
            Self::BatchTooLarge => "a sync batch exceeds the operation limit",
            Self::IdentifierInvalid => "an immutable envelope identifier is invalid",
            Self::DuplicateOperationId => "a sync batch repeats an operation identifier",
            Self::DuplicateAuditEventId => "a sync batch repeats an audit event identifier",
            Self::AnchorInvalid => "the durable device audit anchor is incoherent",
            Self::SequenceInvalid => {
                "an audit sequence must be positive and leave room for a successor"
            }
            Self::SequenceDiscontinuity => "the device audit sequence is not contiguous",
            Self::EventTypeInvalid => "the event type is invalid",
            Self::PayloadInvalid => "the event payload is invalid",
            Self::TimestampInvalid => "the event timestamp is invalid",
            Self::HashEncodingInvalid => "an audit hash encoding is invalid",
            Self::PreviousHashDiscontinuity => "the previous audit hash does not match",
            Self::EventHashMismatch => "the audit event hash does not verify",
        })
    }
}

impl std::error::Error for SyncValidationError {}

/// Validates a batch against the exact durable device anchor supplied by the
/// database layer. Successful validation does not itself acknowledge or write
/// anything; callers must do both atomically in their tenant-scoped SQL
/// transaction. This function rejects duplicate operation and audit-event
/// identities within the incoming batch. Persistence must additionally resolve
/// prior operation IDs idempotently in the same transaction before accepting a
/// newly validated batch.
pub fn validate_sync_batch(
    batch: SyncBatchRequest,
    anchor: &SyncChainAnchor,
) -> Result<ValidatedSyncBatch, SyncValidationError> {
    if batch.operations.is_empty() {
        return Err(SyncValidationError::BatchEmpty);
    }
    if batch.operations.len() > MAX_SYNC_BATCH_OPERATIONS {
        return Err(SyncValidationError::BatchTooLarge);
    }
    if !anchor.is_coherent() {
        return Err(SyncValidationError::AnchorInvalid);
    }
    let branch_id = parse_canonical_entity_id(&batch.branch_id)?;
    let device_id = parse_canonical_entity_id(&batch.device_id)?;
    let branch_id_text = branch_id.to_string();
    let device_id_text = device_id.to_string();
    let mut expected_sequence = anchor
        .sequence()
        .checked_add(1)
        .ok_or(SyncValidationError::SequenceInvalid)?;
    let mut expected_previous_hash = anchor.event_hash().copied();
    let mut validated = Vec::with_capacity(batch.operations.len());
    let mut seen_operation_ids = HashSet::with_capacity(batch.operations.len());
    let mut seen_audit_event_ids = HashSet::with_capacity(batch.operations.len());

    for operation in batch.operations {
        let operation_id = parse_canonical_entity_id(&operation.operation_id)?;
        let audit_event_id = parse_canonical_entity_id(&operation.audit_event_id)?;
        let actor_id = parse_canonical_entity_id(&operation.actor_id)?;
        if !seen_operation_ids.insert(operation_id.clone()) {
            return Err(SyncValidationError::DuplicateOperationId);
        }
        if !seen_audit_event_ids.insert(audit_event_id.clone()) {
            return Err(SyncValidationError::DuplicateAuditEventId);
        }
        if operation.sequence < 1 || operation.sequence == i64::MAX {
            return Err(SyncValidationError::SequenceInvalid);
        }
        if operation.sequence != expected_sequence {
            return Err(SyncValidationError::SequenceDiscontinuity);
        }
        if !is_canonical_event_type(&operation.event_type) {
            return Err(SyncValidationError::EventTypeInvalid);
        }
        if !is_canonical_json_payload(&operation.payload_json) {
            return Err(SyncValidationError::PayloadInvalid);
        }
        if !is_canonical_utc_millisecond_timestamp(&operation.occurred_at_utc) {
            return Err(SyncValidationError::TimestampInvalid);
        }
        let previous_hash = operation
            .previous_hash
            .as_deref()
            .map(decode_audit_hash)
            .transpose()?;
        if previous_hash != expected_previous_hash {
            return Err(SyncValidationError::PreviousHashDiscontinuity);
        }
        let event_hash = decode_audit_hash(&operation.event_hash)?;
        let audit_event_id_text = audit_event_id.to_string();
        let actor_id_text = actor_id.to_string();
        let expected_event_hash = audit_event_hash(AuditEventHashInput {
            event_id: &audit_event_id_text,
            branch_id: &branch_id_text,
            actor_id: &actor_id_text,
            device_id: &device_id_text,
            sequence: operation.sequence,
            event_type: &operation.event_type,
            payload_json: &operation.payload_json,
            occurred_at_utc: &operation.occurred_at_utc,
            previous_hash: previous_hash.as_ref().map(|value| value.as_slice()),
        });
        if event_hash != expected_event_hash {
            return Err(SyncValidationError::EventHashMismatch);
        }
        validated.push(ValidatedSyncOperation {
            operation_id,
            audit_event_id,
            actor_id,
            sequence: operation.sequence,
            event_type: operation.event_type,
            payload_json: operation.payload_json,
            occurred_at_utc: operation.occurred_at_utc,
            previous_hash,
            event_hash,
        });
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(SyncValidationError::SequenceInvalid)?;
        expected_previous_hash = Some(event_hash);
    }

    Ok(ValidatedSyncBatch {
        branch_id,
        device_id,
        operations: validated,
    })
}

impl SyncChainAnchor {
    fn is_coherent(&self) -> bool {
        (self.sequence == 0 && self.event_hash.is_none())
            || (self.sequence > 0 && self.event_hash.is_some())
    }
}

fn parse_canonical_entity_id(value: &str) -> Result<EntityId, SyncValidationError> {
    if value.len() != CANONICAL_UUID_V7_BYTES {
        return Err(SyncValidationError::IdentifierInvalid);
    }
    EntityId::parse(value).map_err(|_| SyncValidationError::IdentifierInvalid)
}

fn is_canonical_event_type(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() <= MAX_SYNC_EVENT_TYPE_BYTES
        && bytes.first().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b'-')
        })
}

fn is_canonical_json_payload(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_SYNC_PAYLOAD_BYTES {
        return false;
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(value) else {
        return false;
    };
    serde_json::to_string(&parsed).is_ok_and(|canonical| canonical == value)
}

fn is_canonical_utc_millisecond_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != CANONICAL_UTC_MILLISECOND_TIMESTAMP_BYTES {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let valid = match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'.',
            23 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        };
        if !valid {
            return false;
        }
    }
    OffsetDateTime::parse(value, &Rfc3339).is_ok()
}

fn decode_audit_hash(value: &str) -> Result<[u8; AUDIT_HASH_BYTES], SyncValidationError> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| SyncValidationError::HashEncodingInvalid)?;
    let hash: [u8; AUDIT_HASH_BYTES] = bytes
        .try_into()
        .map_err(|_| SyncValidationError::HashEncodingInvalid)?;
    if STANDARD.encode(hash) != value {
        return Err(SyncValidationError::HashEncodingInvalid);
    }
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(bytes: &[u8]) -> String {
        STANDARD.encode(bytes)
    }

    fn valid_operation(
        branch_id: &EntityId,
        device_id: &EntityId,
        actor_id: &EntityId,
        sequence: i64,
        previous_hash: Option<[u8; AUDIT_HASH_BYTES]>,
    ) -> SyncOperationRequest {
        let audit_event_id = EntityId::new_v7();
        let operation_id = EntityId::new_v7();
        let event_type = "sales.order.finalized".to_owned();
        let payload_json = r#"{"order_id":"example"}"#.to_owned();
        let occurred_at_utc = "2026-07-17T00:00:00.000Z".to_owned();
        let event_hash = audit_event_hash(AuditEventHashInput {
            event_id: &audit_event_id.to_string(),
            branch_id: &branch_id.to_string(),
            actor_id: &actor_id.to_string(),
            device_id: &device_id.to_string(),
            sequence,
            event_type: &event_type,
            payload_json: &payload_json,
            occurred_at_utc: &occurred_at_utc,
            previous_hash: previous_hash.as_ref().map(|value| value.as_slice()),
        });
        SyncOperationRequest {
            operation_id: operation_id.to_string(),
            audit_event_id: audit_event_id.to_string(),
            actor_id: actor_id.to_string(),
            sequence,
            event_type,
            payload_json,
            occurred_at_utc,
            previous_hash: previous_hash.as_ref().map(|value| encoded(value)),
            event_hash: encoded(&event_hash),
        }
    }

    #[test]
    fn validates_a_contiguous_chain_against_its_durable_anchor() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let first = valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        let first_hash = decode_audit_hash(&first.event_hash).expect("hash");
        let second = valid_operation(&branch_id, &device_id, &actor_id, 2, Some(first_hash));
        let result = validate_sync_batch(
            SyncBatchRequest {
                branch_id: branch_id.to_string(),
                device_id: device_id.to_string(),
                operations: vec![first, second],
            },
            &SyncChainAnchor::empty(),
        )
        .expect("valid chain");
        assert_eq!(result.operations().len(), 2);
        assert_eq!(result.operations()[1].sequence(), 2);
        assert_eq!(result.operations()[1].actor_id(), &actor_id);
    }

    #[test]
    fn rejects_actor_tampering_even_when_all_other_fields_are_valid() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let mut operation = valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        operation.actor_id = EntityId::new_v7().to_string();
        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![operation],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::EventHashMismatch)
        );
    }

    #[test]
    fn rejects_a_sequence_gap_before_any_cloud_write() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let operation = valid_operation(&branch_id, &device_id, &actor_id, 2, None);
        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![operation],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::SequenceDiscontinuity)
        );
    }

    #[test]
    fn rejects_duplicate_transport_operation_ids_within_one_batch() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let first = valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        let first_hash = decode_audit_hash(&first.event_hash).expect("hash");
        let mut second = valid_operation(&branch_id, &device_id, &actor_id, 2, Some(first_hash));
        second.operation_id = first.operation_id.clone();

        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![first, second],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::DuplicateOperationId)
        );
    }

    #[test]
    fn rejects_duplicate_audit_event_ids_within_one_batch() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let first = valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        let first_hash = decode_audit_hash(&first.event_hash).expect("hash");
        let mut second = valid_operation(&branch_id, &device_id, &actor_id, 2, Some(first_hash));
        second.audit_event_id = first.audit_event_id.clone();

        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![first, second],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::DuplicateAuditEventId)
        );
    }

    #[test]
    fn rejects_an_incoherent_durable_anchor_before_processing_operations() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let operation = valid_operation(&branch_id, &device_id, &actor_id, 1, None);

        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![operation],
                },
                &SyncChainAnchor::from_durable_event(0, [0; AUDIT_HASH_BYTES]),
            ),
            Err(SyncValidationError::AnchorInvalid)
        );
    }

    #[test]
    fn rejects_noncanonical_payload_timestamp_and_event_type_bytes() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();

        let mut noncanonical_payload = valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        noncanonical_payload.payload_json = "{\"order_id\": \"example\"}".to_owned();
        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![noncanonical_payload],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::PayloadInvalid)
        );

        let mut noncanonical_timestamp =
            valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        noncanonical_timestamp.occurred_at_utc = "2026-07-17T00:00:00Z".to_owned();
        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![noncanonical_timestamp],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::TimestampInvalid)
        );

        let mut noncanonical_event_type =
            valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        noncanonical_event_type.event_type = "sales.é".to_owned();
        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![noncanonical_event_type],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::EventTypeInvalid)
        );

        let mut noncanonical_actor_id = valid_operation(&branch_id, &device_id, &actor_id, 1, None);
        noncanonical_actor_id.actor_id = noncanonical_actor_id.actor_id.to_uppercase();
        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![noncanonical_actor_id],
                },
                &SyncChainAnchor::empty(),
            ),
            Err(SyncValidationError::IdentifierInvalid)
        );
    }

    #[test]
    fn rejects_a_noncanonical_base64_hash_encoding() {
        let mut noncanonical = encoded(&[0; AUDIT_HASH_BYTES]).into_bytes();
        noncanonical[42] = b'B';
        let noncanonical = String::from_utf8(noncanonical).expect("ASCII base64");

        assert_eq!(
            decode_audit_hash(&noncanonical),
            Err(SyncValidationError::HashEncodingInvalid)
        );
    }

    #[test]
    fn rejects_the_last_representable_sequence_before_it_can_exhaust_a_chain() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let actor_id = EntityId::new_v7();
        let anchor_hash = [7; AUDIT_HASH_BYTES];
        let operation = valid_operation(
            &branch_id,
            &device_id,
            &actor_id,
            i64::MAX,
            Some(anchor_hash),
        );

        assert_eq!(
            validate_sync_batch(
                SyncBatchRequest {
                    branch_id: branch_id.to_string(),
                    device_id: device_id.to_string(),
                    operations: vec![operation],
                },
                &SyncChainAnchor::from_durable_event(i64::MAX - 1, anchor_hash),
            ),
            Err(SyncValidationError::SequenceInvalid)
        );
    }
}
