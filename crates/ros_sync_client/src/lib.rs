//! Professional sync upload client (Stage 4).
//!
//! Builds bounded sync batch requests and posts them to the Professional API
//! with retry/backoff. Acknowledgement persistence is delegated to the
//! caller via [`AcknowledgementSink`].

#![forbid(unsafe_code)]

use std::thread;
use std::time::Duration;

use ros_core::EntityId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;
pub const DEFAULT_INITIAL_BACKOFF_MS: u64 = 250;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncOperationEnvelope {
    pub operation_id: String,
    pub audit_event_id: String,
    pub branch_id: String,
    pub device_id: String,
    pub actor_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub correlation_id: String,
    pub created_at_utc: String,
    pub payload_canonical: String,
    pub event_hash_hex: String,
    pub previous_hash_hex: Option<String>,
    pub sequence: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncBatchRequest {
    pub organization_id: String,
    pub branch_id: String,
    pub device_id: String,
    pub operations: Vec<SyncOperationEnvelope>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncAcknowledgement {
    pub operation_id: String,
    pub server_event_id: String,
    pub accepted_at_utc: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncBatchAcknowledgements {
    pub acknowledgements: Vec<SyncAcknowledgement>,
}

pub trait AcknowledgementSink {
    fn apply(&mut self, acknowledgements: &[SyncAcknowledgement]) -> Result<(), SyncClientError>;
}

#[derive(Debug)]
pub enum SyncClientError {
    Http(String),
    Unavailable(String),
    InvalidResponse(String),
    Sink(String),
}

impl std::fmt::Display for SyncClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(message)
            | Self::Unavailable(message)
            | Self::InvalidResponse(message)
            | Self::Sink(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SyncClientError {}

#[derive(Clone, Debug)]
pub struct SyncClientConfig {
    pub base_url: String,
    pub bearer_token: String,
    pub max_attempts: u32,
    pub initial_backoff: Duration,
}

impl SyncClientConfig {
    pub fn new(base_url: impl Into<String>, bearer_token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            bearer_token: bearer_token.into(),
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            initial_backoff: Duration::from_millis(DEFAULT_INITIAL_BACKOFF_MS),
        }
    }
}

pub fn build_batch_request(
    organization_id: &EntityId,
    branch_id: &EntityId,
    device_id: &EntityId,
    operations: Vec<SyncOperationEnvelope>,
) -> SyncBatchRequest {
    SyncBatchRequest {
        organization_id: organization_id.to_string(),
        branch_id: branch_id.to_string(),
        device_id: device_id.to_string(),
        operations,
    }
}

/// Posts a sync batch with exponential backoff. Retries on 503/504/429.
pub fn upload_batch_with_retry<S: AcknowledgementSink>(
    config: &SyncClientConfig,
    request: &SyncBatchRequest,
    sink: &mut S,
) -> Result<SyncBatchAcknowledgements, SyncClientError> {
    let url = format!(
        "{}/v1/sync/operations:batch",
        config.base_url.trim_end_matches('/')
    );
    let body = serde_json::to_string(request)
        .map_err(|error| SyncClientError::InvalidResponse(error.to_string()))?;
    let mut attempt = 0_u32;
    let mut backoff = config.initial_backoff;
    loop {
        attempt += 1;
        match post_batch(&url, &config.bearer_token, &body) {
            Ok(acknowledgements) => {
                sink.apply(&acknowledgements.acknowledgements)
                    .map_err(|error| SyncClientError::Sink(error.to_string()))?;
                return Ok(acknowledgements);
            }
            Err(error) if attempt < config.max_attempts && is_retryable(&error) => {
                thread::sleep(backoff);
                backoff = backoff.saturating_mul(2);
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_retryable(error: &SyncClientError) -> bool {
    matches!(error, SyncClientError::Unavailable(_))
}

fn post_batch(
    url: &str,
    bearer_token: &str,
    body: &str,
) -> Result<SyncBatchAcknowledgements, SyncClientError> {
    let response = ureq::post(url)
        .header("authorization", format!("Bearer {bearer_token}"))
        .header("content-type", "application/json")
        .send(body)
        .map_err(|error| SyncClientError::Http(error.to_string()))?;
    let status = response.status();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|error| SyncClientError::InvalidResponse(error.to_string()))?;
    if status == 503 || status == 504 || status == 429 {
        return Err(SyncClientError::Unavailable(format!(
            "sync temporarily unavailable ({status})"
        )));
    }
    if status != 200 {
        return Err(SyncClientError::Http(format!(
            "sync rejected with status {status}: {text}"
        )));
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| SyncClientError::InvalidResponse(error.to_string()))?;
    serde_json::from_value(value)
        .map_err(|error| SyncClientError::InvalidResponse(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_batch_request_binds_tenant_scope() {
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let request = build_batch_request(&organization_id, &branch_id, &device_id, Vec::new());
        assert_eq!(request.organization_id, organization_id.to_string());
        assert_eq!(request.branch_id, branch_id.to_string());
        assert_eq!(request.device_id, device_id.to_string());
        assert!(request.operations.is_empty());
    }

    #[test]
    fn retryable_classifier_accepts_unavailable_only() {
        assert!(is_retryable(&SyncClientError::Unavailable("busy".into())));
        assert!(!is_retryable(&SyncClientError::Http("no".into())));
    }
}
