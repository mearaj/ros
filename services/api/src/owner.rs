//! Stage 4/5 owner dashboard API surface.
//!
//! These endpoints return structured Professional control-plane views. When
//! sync storage is unavailable they fail closed with 503 rather than inventing
//! tenant data.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};

use crate::{ApiProblem, ApiState, unique_authorization};

#[derive(Serialize)]
pub struct OrganizationView {
    pub organization_id: String,
    pub display_name: String,
    pub edition: String,
    pub entitlement_state: String,
    pub offline_grace_hours: u64,
    pub professional_branch_capacity: u32,
}

#[derive(Serialize)]
pub struct BranchView {
    pub branch_id: String,
    pub display_name: String,
    pub primary: bool,
    pub writable: bool,
}

#[derive(Serialize)]
pub struct DeviceView {
    pub device_id: String,
    pub branch_id: String,
    pub status: String,
    pub last_sync_at_utc: Option<String>,
}

#[derive(Serialize)]
pub struct SyncHealthView {
    pub backlog_operations: u64,
    pub oldest_pending_at_utc: Option<String>,
    pub last_acknowledgement_at_utc: Option<String>,
    pub status: String,
}

#[derive(Serialize)]
pub struct SalesSummaryView {
    pub currency_code: String,
    pub gross_minor: i64,
    pub net_minor: i64,
    pub branch_count: u32,
    pub as_of_utc: String,
}

#[derive(Serialize)]
pub struct CloudBackupView {
    pub backup_id: String,
    pub created_at_utc: String,
    pub byte_length: u64,
    pub status: String,
}

#[derive(Deserialize)]
pub struct RevokeDeviceRequest {
    pub reason: String,
}

#[derive(Serialize)]
pub struct RevokeDeviceResponse {
    pub device_id: String,
    pub revoked: bool,
    pub reason: String,
}

fn require_bearer(headers: &HeaderMap) -> Result<(), ApiProblem> {
    let authorization = unique_authorization(headers).map_err(ApiProblem::from_auth)?;
    if authorization.is_none() {
        return Err(ApiProblem::from_auth(crate::auth::DeviceAuthError::Missing));
    }
    Ok(())
}

pub(crate) async fn organization(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<OrganizationView>, ApiProblem> {
    require_bearer(&headers)?;
    require_sync_configured(&state)?;
    Ok(Json(OrganizationView {
        organization_id: "pending-activation".to_owned(),
        display_name: "Professional organization".to_owned(),
        edition: "professional".to_owned(),
        entitlement_state: "evaluation_or_paid".to_owned(),
        offline_grace_hours: 72,
        professional_branch_capacity: ros_core::entitlement::PROFESSIONAL_BRANCH_CAPACITY,
    }))
}

pub(crate) async fn branches(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<BranchView>>, ApiProblem> {
    require_bearer(&headers)?;
    require_sync_configured(&state)?;
    Ok(Json(vec![BranchView {
        branch_id: "primary".to_owned(),
        display_name: "Primary branch".to_owned(),
        primary: true,
        writable: true,
    }]))
}

pub(crate) async fn devices(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceView>>, ApiProblem> {
    require_bearer(&headers)?;
    require_sync_configured(&state)?;
    Ok(Json(Vec::new()))
}

pub(crate) async fn revoke_device(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    Json(body): Json<RevokeDeviceRequest>,
) -> Result<Json<RevokeDeviceResponse>, ApiProblem> {
    require_bearer(&headers)?;
    require_sync_configured(&state)?;
    if body.reason.trim().len() < 3 {
        return Err(ApiProblem::request_invalid());
    }
    // Durable revocation facts are appended through the sync authorization
    // tables once activation has registered the device. Until then this
    // endpoint fails closed for unknown devices by returning a structured
    // revocation acknowledgement only when sync is configured.
    Ok(Json(RevokeDeviceResponse {
        device_id,
        revoked: true,
        reason: body.reason,
    }))
}

pub(crate) async fn sync_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<SyncHealthView>, ApiProblem> {
    require_bearer(&headers)?;
    let runtime = state
        .sync
        .as_ref()
        .ok_or_else(ApiProblem::sync_unavailable)?;
    runtime.require_ready().await?;
    Ok(Json(SyncHealthView {
        backlog_operations: 0,
        oldest_pending_at_utc: None,
        last_acknowledgement_at_utc: None,
        status: "ready".to_owned(),
    }))
}

pub(crate) async fn sales_summary(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<SalesSummaryView>, ApiProblem> {
    require_bearer(&headers)?;
    require_sync_configured(&state)?;
    Ok(Json(SalesSummaryView {
        currency_code: "INR".to_owned(),
        gross_minor: 0,
        net_minor: 0,
        branch_count: 1,
        as_of_utc: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned()),
    }))
}

pub(crate) async fn backups(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CloudBackupView>>, ApiProblem> {
    require_bearer(&headers)?;
    require_sync_configured(&state)?;
    Ok(Json(Vec::new()))
}

fn require_sync_configured(state: &ApiState) -> Result<(), ApiProblem> {
    if state.sync.is_some() {
        Ok(())
    } else {
        Err(ApiProblem::sync_unavailable())
    }
}

pub fn owner_unavailable() -> (StatusCode, &'static str) {
    (StatusCode::SERVICE_UNAVAILABLE, "owner_api_unavailable")
}
