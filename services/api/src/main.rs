use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Request, State},
    http::{
        HeaderMap, HeaderValue, Method, StatusCode,
        header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use sqlx_postgres::{PgConnectOptions, PgSslMode};
use url::Url;
use uuid::Uuid;

pub mod auth;
pub mod owner;
pub mod sync;
pub mod sync_store;

use auth::{DeviceAuthError, DeviceTokenVerifier};
use sync::SyncBatchRequest;
use sync_store::{PostgresSyncStore, SyncBatchAcknowledgements, SyncStoreError};

const MAX_SYNC_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEVICE_TOKEN_PUBLIC_KEY_BYTES: usize = 16 * 1024;
const DEFAULT_SYNC_REQUESTS_PER_MINUTE: u32 = 120;
const MAX_SYNC_REQUESTS_PER_MINUTE: u32 = 10_000;
const MAX_RATE_LIMIT_BUCKETS: usize = 100_000;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const DEFAULT_API_REQUEST_TIMEOUT_MS: u32 = 15_000;
const MIN_API_REQUEST_TIMEOUT_MS: u32 = 100;
const MAX_API_REQUEST_TIMEOUT_MS: u32 = 120_000;
const DEFAULT_API_QUEUE_TIMEOUT_MS: u32 = 100;
const MIN_API_QUEUE_TIMEOUT_MS: u32 = 1;
const MAX_API_QUEUE_TIMEOUT_MS: u32 = 5_000;
const DEFAULT_API_MAX_IN_FLIGHT: u32 = 256;
const MAX_API_MAX_IN_FLIGHT: u32 = 4_096;
const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    deployment_environment: String,
    professional_sync: &'static str,
}

#[derive(Clone)]
struct ApiConfig {
    listen_address: SocketAddr,
    deployment_environment: String,
    operational: OperationalConfig,
    sync: Option<SyncConfig>,
}

#[derive(Clone, Copy)]
struct OperationalConfig {
    request_timeout: Duration,
    queue_timeout: Duration,
    max_in_flight: usize,
}

impl OperationalConfig {
    fn from_environment() -> Result<Self, String> {
        let request_timeout_ms = bounded_u32_environment(
            "ROS_API_REQUEST_TIMEOUT_MS",
            DEFAULT_API_REQUEST_TIMEOUT_MS,
            MIN_API_REQUEST_TIMEOUT_MS,
            MAX_API_REQUEST_TIMEOUT_MS,
        )?;
        let queue_timeout_ms = bounded_u32_environment(
            "ROS_API_QUEUE_TIMEOUT_MS",
            DEFAULT_API_QUEUE_TIMEOUT_MS,
            MIN_API_QUEUE_TIMEOUT_MS,
            MAX_API_QUEUE_TIMEOUT_MS,
        )?;
        if queue_timeout_ms > request_timeout_ms {
            return Err(
                "ROS_API_QUEUE_TIMEOUT_MS cannot exceed ROS_API_REQUEST_TIMEOUT_MS".to_owned(),
            );
        }
        let max_in_flight = bounded_u32_environment(
            "ROS_API_MAX_IN_FLIGHT",
            DEFAULT_API_MAX_IN_FLIGHT,
            1,
            MAX_API_MAX_IN_FLIGHT,
        )?;
        Ok(Self {
            request_timeout: Duration::from_millis(u64::from(request_timeout_ms)),
            queue_timeout: Duration::from_millis(u64::from(queue_timeout_ms)),
            max_in_flight: max_in_flight as usize,
        })
    }

    #[cfg(test)]
    fn test_default() -> Self {
        Self {
            request_timeout: Duration::from_millis(u64::from(DEFAULT_API_REQUEST_TIMEOUT_MS)),
            queue_timeout: Duration::from_millis(u64::from(DEFAULT_API_QUEUE_TIMEOUT_MS)),
            max_in_flight: DEFAULT_API_MAX_IN_FLIGHT as usize,
        }
    }
}

#[derive(Clone)]
struct SyncConfig {
    database_url: String,
    token_public_key_file: String,
    token_issuer: String,
    token_audience: String,
    requests_per_minute: u32,
}

impl ApiConfig {
    fn from_environment() -> Result<Self, String> {
        let listen_address = env::var("ROS_API_LISTEN_ADDRESS")
            .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
            .parse()
            .map_err(|_| "ROS_API_LISTEN_ADDRESS must be an IP address and port".to_owned())?;
        let deployment_environment =
            env::var("ROS_DEPLOYMENT_ENV").unwrap_or_else(|_| "development".to_owned());
        validate_deployment_environment(&deployment_environment)?;
        let operational = OperationalConfig::from_environment()?;
        let sync_enabled = parse_boolean_environment("ROS_ENABLE_PROFESSIONAL_SYNC", false)?;
        let sync = if sync_enabled {
            let sync = SyncConfig {
                database_url: required_environment("ROS_DATABASE_URL")?,
                token_public_key_file: required_environment("ROS_DEVICE_TOKEN_PUBLIC_KEY_FILE")?,
                token_issuer: required_environment("ROS_DEVICE_TOKEN_ISSUER")?,
                token_audience: required_environment("ROS_DEVICE_TOKEN_AUDIENCE")?,
                requests_per_minute: bounded_u32_environment(
                    "ROS_SYNC_REQUESTS_PER_MINUTE",
                    DEFAULT_SYNC_REQUESTS_PER_MINUTE,
                    1,
                    MAX_SYNC_REQUESTS_PER_MINUTE,
                )?,
            };
            validate_sync_transport(&deployment_environment, &sync)?;
            Some(sync)
        } else {
            None
        };
        Ok(Self {
            listen_address,
            deployment_environment,
            operational,
            sync,
        })
    }
}

fn validate_sync_transport(environment: &str, sync: &SyncConfig) -> Result<(), String> {
    let database = PgConnectOptions::from_str(&sync.database_url)
        .map_err(|_| "ROS_DATABASE_URL must be a valid PostgreSQL connection URL".to_owned())?;
    if environment != "development" && !matches!(database.get_ssl_mode(), PgSslMode::VerifyFull) {
        return Err("ROS_DATABASE_URL must set sslmode=verify-full outside development".to_owned());
    }

    let issuer = Url::parse(&sync.token_issuer)
        .map_err(|_| "ROS_DEVICE_TOKEN_ISSUER must be an absolute URL".to_owned())?;
    if issuer.host_str().is_none() || (environment != "development" && issuer.scheme() != "https") {
        return Err("ROS_DEVICE_TOKEN_ISSUER must use HTTPS outside development".to_owned());
    }
    Ok(())
}

fn validate_deployment_environment(value: &str) -> Result<(), String> {
    if matches!(value, "development" | "staging" | "production") {
        Ok(())
    } else {
        Err("ROS_DEPLOYMENT_ENV must be development, staging, or production".to_owned())
    }
}

fn parse_boolean_environment(name: &str, default: bool) -> Result<bool, String> {
    match env::var(name) {
        Ok(value) => parse_boolean_value(name, &value),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} must contain valid UTF-8")),
    }
}

fn parse_boolean_value(name: &str, value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{name} must be true or false")),
    }
}

fn required_environment(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} is required when sync is enabled"))?;
    if value.trim().is_empty() {
        Err(format!("{name} cannot be blank"))
    } else {
        Ok(value)
    }
}

fn bounded_u32_environment(
    name: &str,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> Result<u32, String> {
    let raw = match env::var(name) {
        Ok(raw) => raw,
        Err(env::VarError::NotPresent) => return Ok(default),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{name} must contain valid UTF-8"));
        }
    };
    parse_bounded_u32_value(name, &raw, minimum, maximum)
}

fn parse_bounded_u32_value(
    name: &str,
    raw: &str,
    minimum: u32,
    maximum: u32,
) -> Result<u32, String> {
    if raw.is_empty() || raw.trim() != raw || raw.starts_with('+') {
        return Err(format!("{name} must be a canonical positive integer"));
    }
    let value = raw
        .parse::<u32>()
        .map_err(|_| format!("{name} must be a canonical positive integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name} must be between {minimum} and {maximum}"));
    }
    Ok(value)
}

#[derive(Clone)]
struct SyncRateLimiter {
    requests_per_window: u32,
    buckets: Arc<Mutex<HashMap<(Uuid, Uuid), RateLimitWindow>>>,
}

#[derive(Clone, Copy)]
struct RateLimitWindow {
    started_at: Instant,
    requests: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RateLimitDecision {
    Allowed,
    Limited { retry_after_seconds: u64 },
    CapacityUnavailable,
}

impl SyncRateLimiter {
    fn new(requests_per_window: u32) -> Self {
        Self {
            requests_per_window: requests_per_window.max(1),
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn check(&self, claims: &auth::DeviceClaims) -> Result<(), ApiProblem> {
        match self.check_at(
            claims.organization_id().as_uuid(),
            claims.device_id().as_uuid(),
            Instant::now(),
        ) {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::Limited {
                retry_after_seconds,
            } => Err(ApiProblem::rate_limited(retry_after_seconds)),
            RateLimitDecision::CapacityUnavailable => Err(ApiProblem::sync_unavailable()),
        }
    }

    fn check_at(&self, organization_id: Uuid, device_id: Uuid, now: Instant) -> RateLimitDecision {
        let Ok(mut buckets) = self.buckets.lock() else {
            return RateLimitDecision::CapacityUnavailable;
        };
        let key = (organization_id, device_id);
        if let Some(window) = buckets.get_mut(&key) {
            let elapsed = now.saturating_duration_since(window.started_at);
            if elapsed >= RATE_LIMIT_WINDOW {
                *window = RateLimitWindow {
                    started_at: now,
                    requests: 1,
                };
                return RateLimitDecision::Allowed;
            }
            if window.requests >= self.requests_per_window {
                let remaining = RATE_LIMIT_WINDOW.saturating_sub(elapsed);
                let retry_after_seconds = remaining
                    .as_secs()
                    .saturating_add(u64::from(remaining.subsec_nanos() != 0))
                    .max(1);
                return RateLimitDecision::Limited {
                    retry_after_seconds,
                };
            }
            window.requests = window.requests.saturating_add(1);
            return RateLimitDecision::Allowed;
        }

        if buckets.len() >= MAX_RATE_LIMIT_BUCKETS {
            buckets.retain(|_, window| {
                now.saturating_duration_since(window.started_at) < RATE_LIMIT_WINDOW
            });
            if buckets.len() >= MAX_RATE_LIMIT_BUCKETS {
                return RateLimitDecision::CapacityUnavailable;
            }
        }
        buckets.insert(
            key,
            RateLimitWindow {
                started_at: now,
                requests: 1,
            },
        );
        RateLimitDecision::Allowed
    }
}

#[derive(Clone)]
pub(crate) struct SyncRuntime {
    verifier: DeviceTokenVerifier,
    store: PostgresSyncStore,
    rate_limiter: SyncRateLimiter,
}

impl SyncRuntime {
    async fn initialize(
        verifier: DeviceTokenVerifier,
        store: PostgresSyncStore,
        requests_per_minute: u32,
    ) -> Result<Self, String> {
        store
            .readiness()
            .await
            .map_err(|_| "Professional sync schema is unavailable or incomplete".to_owned())?;
        Ok(Self {
            verifier,
            store,
            rate_limiter: SyncRateLimiter::new(requests_per_minute),
        })
    }

    pub(crate) async fn require_ready(&self) -> Result<(), ApiProblem> {
        // Schema probes against an unreachable database must fail closed as
        // 503 quickly instead of waiting for the full request deadline (504).
        match tokio::time::timeout(Duration::from_secs(2), self.store.readiness()).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(ApiProblem::from_store(error)),
            Err(_) => Err(ApiProblem::sync_unavailable()),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ApiState {
    config: ApiConfig,
    pub(crate) sync: Option<SyncRuntime>,
}

impl ApiState {
    async fn initialize(config: ApiConfig) -> Result<Self, String> {
        let sync = match &config.sync {
            Some(sync) => {
                let public_key_metadata = tokio::fs::metadata(&sync.token_public_key_file)
                    .await
                    .map_err(|_| "device-token public key could not be read".to_owned())?;
                if !public_key_metadata.is_file()
                    || public_key_metadata.len() == 0
                    || public_key_metadata.len() > MAX_DEVICE_TOKEN_PUBLIC_KEY_BYTES as u64
                {
                    return Err("device-token public key has an invalid size".to_owned());
                }
                let public_key = tokio::fs::read(&sync.token_public_key_file)
                    .await
                    .map_err(|_| "device-token public key could not be read".to_owned())?;
                if public_key.is_empty() || public_key.len() > MAX_DEVICE_TOKEN_PUBLIC_KEY_BYTES {
                    return Err("device-token public key has an invalid size".to_owned());
                }
                let verifier = DeviceTokenVerifier::from_ed25519_pem(
                    &public_key,
                    &sync.token_issuer,
                    &sync.token_audience,
                )
                .map_err(|_| "device-token verifier configuration was rejected".to_owned())?;
                let store = PostgresSyncStore::connect(&sync.database_url)
                    .await
                    .map_err(|_| "Professional sync database is unavailable".to_owned())?;
                Some(SyncRuntime::initialize(verifier, store, sync.requests_per_minute).await?)
            }
            None => None,
        };
        Ok(Self { config, sync })
    }
}

async fn ready(State(state): State<ApiState>) -> Response {
    let (status, sync_status) = match &state.sync {
        Some(sync) => match sync.store.readiness().await {
            Ok(()) => (StatusCode::OK, "ready"),
            Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        },
        None => (StatusCode::OK, "disabled"),
    };
    (
        status,
        Json(HealthResponse {
            status: if status == StatusCode::OK {
                "ok"
            } else {
                "unavailable"
            },
            service: "gotigin-restaurant-os-professional-api",
            version: env!("CARGO_PKG_VERSION"),
            deployment_environment: state.config.deployment_environment,
            professional_sync: sync_status,
        }),
    )
        .into_response()
}

async fn live(State(state): State<ApiState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "gotigin-restaurant-os-professional-api",
        version: env!("CARGO_PKG_VERSION"),
        deployment_environment: state.config.deployment_environment,
        professional_sync: if state.sync.is_some() {
            "configured-not-probed"
        } else {
            "disabled"
        },
    })
}

async fn submit_sync_batch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SyncBatchAcknowledgements>, ApiProblem> {
    let runtime = state
        .sync
        .as_ref()
        .ok_or_else(ApiProblem::sync_unavailable)?;
    let authorization = unique_authorization(&headers).map_err(ApiProblem::from_auth)?;
    let claims = runtime
        .verifier
        .verify_bearer(authorization)
        .map_err(ApiProblem::from_auth)?;
    runtime.rate_limiter.check(&claims)?;
    runtime.require_ready().await?;
    if !has_json_content_type(&headers) {
        return Err(ApiProblem::unsupported_media_type());
    }
    let request = serde_json::from_slice::<SyncBatchRequest>(&body)
        .map_err(|_| ApiProblem::request_invalid())?;
    let acknowledgements = runtime
        .store
        .accept_batch(&claims, request)
        .await
        .map_err(ApiProblem::from_store)?;
    Ok(Json(acknowledgements))
}

pub(crate) fn unique_authorization(headers: &HeaderMap) -> Result<Option<&str>, DeviceAuthError> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(DeviceAuthError::Invalid);
    }
    value
        .to_str()
        .map(Some)
        .map_err(|_| DeviceAuthError::Invalid)
}

fn has_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

async fn reject_sync_batch() -> ApiProblem {
    ApiProblem::sync_unavailable()
}

#[derive(Clone)]
struct OperationalRuntime {
    request_timeout: Duration,
    queue_timeout: Duration,
    in_flight: Arc<tokio::sync::Semaphore>,
}

impl OperationalRuntime {
    fn new(config: OperationalConfig) -> Self {
        Self {
            request_timeout: config.request_timeout,
            queue_timeout: config.queue_timeout,
            in_flight: Arc::new(tokio::sync::Semaphore::new(config.max_in_flight)),
        }
    }
}

async fn operational_boundary(
    State(runtime): State<OperationalRuntime>,
    mut request: Request,
    next: Next,
) -> Response {
    let started_at = Instant::now();
    let request_id = canonical_or_generated_request_id(request.headers());
    let request_id_value = HeaderValue::from_str(&request_id.hyphenated().to_string())
        .expect("a UUID is always a valid header value");
    request
        .headers_mut()
        .insert(REQUEST_ID_HEADER, request_id_value.clone());

    let method = request.method().clone();
    let route = privacy_safe_route_label(&method, request.uri().path());
    let method_label = privacy_safe_method_label(&method);
    let bypass_admission = method == Method::GET && request.uri().path() == "/healthz";

    let (mut response, outcome) = if bypass_admission {
        match tokio::time::timeout(runtime.request_timeout, next.run(request)).await {
            Ok(response) => (response, "completed"),
            Err(_) => (ApiProblem::request_timeout().into_response(), "timed_out"),
        }
    } else {
        let queue_budget = runtime.queue_timeout.min(runtime.request_timeout);
        match tokio::time::timeout(queue_budget, runtime.in_flight.clone().acquire_owned()).await {
            Ok(Ok(permit)) => {
                let remaining = runtime.request_timeout.saturating_sub(started_at.elapsed());
                let result = tokio::time::timeout(remaining, next.run(request)).await;
                drop(permit);
                match result {
                    Ok(response) => (response, "completed"),
                    Err(_) => (ApiProblem::request_timeout().into_response(), "timed_out"),
                }
            }
            Ok(Err(_)) | Err(_) => (ApiProblem::server_busy().into_response(), "backpressured"),
        }
    };

    apply_operational_response_headers(&mut response, request_id_value);
    let duration_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        target: "ros_api::http",
        request_id = %request_id.hyphenated(),
        route,
        method = method_label,
        status = response.status().as_u16(),
        duration_ms,
        outcome,
        "http_request_completed"
    );
    response
}

fn canonical_or_generated_request_id(headers: &HeaderMap) -> Uuid {
    let mut values = headers.get_all(REQUEST_ID_HEADER).iter();
    let Some(value) = values.next() else {
        return Uuid::now_v7();
    };
    if values.next().is_some() {
        return Uuid::now_v7();
    }
    let Some(value) = value.to_str().ok().filter(|value| value.len() == 36) else {
        return Uuid::now_v7();
    };
    let Some(parsed) = Uuid::parse_str(value).ok().filter(|parsed| {
        matches!(parsed.get_version_num(), 4 | 7)
            && value.as_bytes() == parsed.hyphenated().to_string().as_bytes()
    }) else {
        return Uuid::now_v7();
    };
    parsed
}

fn privacy_safe_route_label(method: &Method, path: &str) -> &'static str {
    match (method, path) {
        (&Method::GET, "/healthz") => "healthz",
        (&Method::GET, "/readyz") => "readyz",
        (&Method::POST, "/v1/sync/operations:batch") => "sync_batch",
        (&Method::GET, "/v1/owner/organization") => "owner_organization",
        (&Method::GET, "/v1/owner/branches") => "owner_branches",
        (&Method::GET, "/v1/owner/devices") => "owner_devices",
        (&Method::POST, path) if path.starts_with("/v1/owner/devices/") => "owner_device_revoke",
        (&Method::GET, "/v1/owner/sync-health") => "owner_sync_health",
        (&Method::GET, "/v1/owner/sales/summary") => "owner_sales_summary",
        (&Method::GET, "/v1/owner/backups") => "owner_backups",
        _ => "unmatched",
    }
}

fn privacy_safe_method_label(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        _ => "OTHER",
    }
}

fn apply_operational_response_headers(response: &mut Response, request_id: HeaderValue) {
    let headers = response.headers_mut();
    headers.insert(REQUEST_ID_HEADER, request_id);
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    headers.insert("pragma", HeaderValue::from_static("no-cache"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'; base-uri 'none'"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), geolocation=(), microphone=()"),
    );
}

fn app(state: ApiState) -> Router {
    let operational = OperationalRuntime::new(state.config.operational);
    let sync_route = if state.sync.is_some() {
        post(submit_sync_batch)
    } else {
        post(reject_sync_batch)
    };
    Router::new()
        .route("/healthz", get(live))
        .route("/readyz", get(ready))
        .route(
            "/v1/sync/operations:batch",
            sync_route.layer(DefaultBodyLimit::max(MAX_SYNC_REQUEST_BYTES)),
        )
        .route("/v1/owner/organization", get(owner::organization))
        .route("/v1/owner/branches", get(owner::branches))
        .route("/v1/owner/devices", get(owner::devices))
        .route(
            "/v1/owner/devices/{device_id}/revocations",
            post(owner::revoke_device),
        )
        .route("/v1/owner/sync-health", get(owner::sync_health))
        .route("/v1/owner/sales/summary", get(owner::sales_summary))
        .route("/v1/owner/backups", get(owner::backups))
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            operational,
            operational_boundary,
        ))
}

#[derive(Serialize)]
struct ProblemBody {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
}

pub(crate) struct ApiProblem {
    status: StatusCode,
    body: ProblemBody,
    retry_after_seconds: Option<u64>,
}

impl ApiProblem {
    pub(crate) fn new(
        status: StatusCode,
        r#type: &'static str,
        title: &'static str,
        code: &'static str,
    ) -> Self {
        Self {
            status,
            body: ProblemBody {
                r#type,
                title,
                status: status.as_u16(),
                code,
            },
            retry_after_seconds: None,
        }
    }

    pub(crate) fn sync_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "https://ros.gotigin.com/problems/sync-unavailable",
            "Professional sync is unavailable",
            "sync_unavailable",
        )
    }

    pub(crate) fn request_invalid() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "https://ros.gotigin.com/problems/sync-request-invalid",
            "Sync request rejected",
            "sync_request_invalid",
        )
    }

    fn unsupported_media_type() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "https://ros.gotigin.com/problems/content-type-unsupported",
            "JSON content type required",
            "content_type_unsupported",
        )
    }

    fn rate_limited(retry_after_seconds: u64) -> Self {
        let mut problem = Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "https://ros.gotigin.com/problems/sync-rate-limited",
            "Sync request rate exceeded",
            "sync_rate_limited",
        );
        problem.retry_after_seconds = Some(retry_after_seconds.max(1));
        problem
    }

    fn request_timeout() -> Self {
        Self::new(
            StatusCode::GATEWAY_TIMEOUT,
            "https://ros.gotigin.com/problems/request-timeout",
            "Request deadline exceeded",
            "request_timeout",
        )
    }

    fn server_busy() -> Self {
        let mut problem = Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "https://ros.gotigin.com/problems/server-busy",
            "Server is busy",
            "server_busy",
        );
        problem.retry_after_seconds = Some(1);
        problem
    }

    pub(crate) fn from_auth(error: DeviceAuthError) -> Self {
        match error {
            DeviceAuthError::Forbidden => Self::new(
                StatusCode::FORBIDDEN,
                "https://ros.gotigin.com/problems/sync-forbidden",
                "Sync permission denied",
                "sync_forbidden",
            ),
            DeviceAuthError::ConfigurationInvalid => Self::sync_unavailable(),
            DeviceAuthError::Missing | DeviceAuthError::Invalid => Self::new(
                StatusCode::UNAUTHORIZED,
                "https://ros.gotigin.com/problems/device-token-invalid",
                "Device authentication required",
                "device_token_invalid",
            ),
        }
    }

    fn from_store(error: SyncStoreError) -> Self {
        match error {
            SyncStoreError::Forbidden => Self::new(
                StatusCode::FORBIDDEN,
                "https://ros.gotigin.com/problems/sync-forbidden",
                "Sync permission denied",
                "sync_forbidden",
            ),
            SyncStoreError::Invalid(_) => Self::new(
                StatusCode::BAD_REQUEST,
                "https://ros.gotigin.com/problems/sync-envelope-invalid",
                "Sync envelope rejected",
                "sync_envelope_invalid",
            ),
            SyncStoreError::Conflict => Self::new(
                StatusCode::CONFLICT,
                "https://ros.gotigin.com/problems/sync-conflict",
                "Sync fact conflicts with durable history",
                "sync_conflict",
            ),
            SyncStoreError::Unavailable => Self::sync_unavailable(),
        }
    }
}

impl IntoResponse for ApiProblem {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self.body)).into_response();
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Bearer"),
            );
        }
        if let Some(retry_after_seconds) = self.retry_after_seconds
            && let Ok(value) = axum::http::HeaderValue::from_str(&retry_after_seconds.to_string())
        {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, value);
        }
        response
    }
}

async fn shutdown_signal() {
    let control_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = control_c => {},
        () = terminate => {},
    }
}

fn privacy_trace_filter() -> tracing_subscriber::filter::Targets {
    tracing_subscriber::filter::Targets::new()
        .with_target("ros_api::http", tracing::Level::INFO)
        .with_target("ros_api::lifecycle", tracing::Level::INFO)
}

fn initialize_observability() {
    use tracing_subscriber::{Layer, layer::SubscriberExt};

    let format = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(false)
        .with_span_list(false)
        .with_target(true)
        .with_writer(std::io::stderr)
        .with_filter(privacy_trace_filter());
    let subscriber = tracing_subscriber::registry().with(format);
    tracing::subscriber::set_global_default(subscriber)
        .expect("structured tracing subscriber must initialize exactly once");
}

#[tokio::main]
async fn main() {
    initialize_observability();
    let config = ApiConfig::from_environment()
        .unwrap_or_else(|message| panic!("API configuration rejected: {message}"));
    let listen_address = config.listen_address;
    let state = ApiState::initialize(config)
        .await
        .unwrap_or_else(|message| panic!("API initialization rejected: {message}"));
    let listener = tokio::net::TcpListener::bind(listen_address)
        .await
        .expect("local API listener must bind");
    tracing::info!(target: "ros_api::lifecycle", "api_listener_ready");
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("local API server must not fail");
}

#[cfg(test)]
mod tests {
    use std::{env, sync::Arc};

    use super::*;
    use axum::{body::Body, http::Request};
    use ros_core::EntityId;
    use sqlx::{AssertSqlSafe, PgPool, postgres::PgPoolOptions};
    use tokio::sync::Notify;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::auth::{
        SYNC_WRITE_PERMISSION, test_support::test_ed25519_keys, tests::signed_token,
    };

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

    async fn create_test_schema(pool: &PgPool, schema: &str) {
        assert!(
            schema
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
            "generated schema identifier must stay within a safe alphabet"
        );
        sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA \"{schema}\"")))
            .execute(pool)
            .await
            .expect("isolated readiness-test schema creates");
    }

    async fn apply_migration(pool: &PgPool, schema: &str, migration: &'static str) {
        let mut connection = pool
            .acquire()
            .await
            .expect("readiness-test database connection acquires");
        sqlx::query(AssertSqlSafe(format!("SET search_path TO \"{schema}\"")))
            .execute(&mut *connection)
            .await
            .expect("readiness-test search path sets");
        sqlx::raw_sql(migration)
            .execute(&mut *connection)
            .await
            .expect("cloud migration applies to isolated readiness-test schema");
    }

    fn disabled_state() -> ApiState {
        ApiState {
            config: ApiConfig {
                listen_address: "127.0.0.1:3000".parse().expect("valid address"),
                deployment_environment: "development".to_owned(),
                operational: OperationalConfig::test_default(),
                sync: None,
            },
            sync: None,
        }
    }

    fn enabled_state_without_database_connection() -> ApiState {
        let sync_config = SyncConfig {
            database_url: "postgresql://unused:unused@127.0.0.1:9/unused".to_owned(),
            token_public_key_file: "unused-in-test".to_owned(),
            token_issuer: "https://identity.test.gotigin.invalid".to_owned(),
            token_audience: "restaurant-os-professional-api".to_owned(),
            requests_per_minute: DEFAULT_SYNC_REQUESTS_PER_MINUTE,
        };
        let verifier = DeviceTokenVerifier::from_ed25519_pem(
            test_ed25519_keys().public_key_pem(),
            &sync_config.token_issuer,
            &sync_config.token_audience,
        )
        .expect("test verifier");
        let pool = PgPoolOptions::new()
            .connect_lazy(&sync_config.database_url)
            .expect("lazy test pool");
        ApiState {
            config: ApiConfig {
                listen_address: "127.0.0.1:3000".parse().expect("valid address"),
                deployment_environment: "development".to_owned(),
                operational: OperationalConfig::test_default(),
                sync: Some(sync_config),
            },
            sync: Some(SyncRuntime {
                verifier,
                store: PostgresSyncStore::from_pool(pool),
                rate_limiter: SyncRateLimiter::new(DEFAULT_SYNC_REQUESTS_PER_MINUTE),
            }),
        }
    }

    #[tokio::test]
    async fn health_endpoint_is_available_without_cloud_configuration() {
        let response = app(disabled_state())
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store, max-age=0"))
        );
        assert_eq!(
            response.headers().get("x-content-type-options"),
            Some(&HeaderValue::from_static("nosniff"))
        );
        let request_id = response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            .expect("the edge generates a parseable request ID");
        assert_eq!(request_id.get_version_num(), 7);
    }

    #[tokio::test]
    async fn canonical_request_id_is_echoed_and_untrusted_value_is_replaced() {
        let accepted = "9c62c2ab-0d8e-4a68-bf78-19c6638be14d";
        let accepted_response = app(disabled_state())
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .header(REQUEST_ID_HEADER, accepted)
                    .header(AUTHORIZATION, "Bearer must-not-be-echoed")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            accepted_response
                .headers()
                .get(REQUEST_ID_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some(accepted)
        );
        assert!(accepted_response.headers().get(AUTHORIZATION).is_none());

        let untrusted = "not-a-request-id-with-user-controlled-data".repeat(8);
        let replaced_response = app(disabled_state())
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .header(REQUEST_ID_HEADER, &untrusted)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let replacement = replaced_response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .expect("replacement request ID");
        assert_ne!(replacement, untrusted);
        assert_eq!(
            Uuid::parse_str(replacement)
                .expect("replacement is a UUID")
                .get_version_num(),
            7
        );
    }

    #[tokio::test(start_paused = true)]
    async fn request_deadline_returns_a_bounded_problem_response() {
        let operational = OperationalRuntime::new(OperationalConfig {
            request_timeout: Duration::from_secs(5),
            queue_timeout: Duration::from_secs(1),
            max_in_flight: 1,
        });
        let service = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    StatusCode::OK
                }),
            )
            .layer(middleware::from_fn_with_state(
                operational,
                operational_boundary,
            ));
        let response = service
            .oneshot(
                Request::builder()
                    .uri("/slow")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            response.headers().get(CONTENT_TYPE),
            Some(&HeaderValue::from_static("application/problem+json"))
        );
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    #[tokio::test(start_paused = true)]
    async fn concurrency_limit_backpressures_work_without_blocking_liveness() {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let entered_for_handler = entered.clone();
        let release_for_handler = release.clone();
        let operational = OperationalRuntime::new(OperationalConfig {
            request_timeout: Duration::from_secs(30),
            queue_timeout: Duration::from_secs(1),
            max_in_flight: 1,
        });
        let service = Router::new()
            .route(
                "/work",
                get(move || {
                    let entered = entered_for_handler.clone();
                    let release = release_for_handler.clone();
                    async move {
                        entered.notify_one();
                        release.notified().await;
                        StatusCode::OK
                    }
                }),
            )
            .route("/healthz", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn_with_state(
                operational,
                operational_boundary,
            ));

        let first_service = service.clone();
        let first = tokio::spawn(async move {
            first_service
                .oneshot(
                    Request::builder()
                        .uri("/work")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("first response")
        });
        entered.notified().await;

        let liveness = service
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("liveness response");
        assert_eq!(liveness.status(), StatusCode::OK);

        let backpressured = service
            .oneshot(
                Request::builder()
                    .uri("/work")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("backpressure response");
        assert_eq!(backpressured.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            backpressured.headers().get(axum::http::header::RETRY_AFTER),
            Some(&HeaderValue::from_static("1"))
        );

        release.notify_one();
        assert_eq!(
            first.await.expect("first request task joins").status(),
            StatusCode::OK
        );
    }

    #[test]
    fn trace_route_labels_never_include_dynamic_or_unknown_paths() {
        assert_eq!(
            privacy_safe_route_label(&Method::POST, "/v1/sync/operations:batch"),
            "sync_batch"
        );
        assert_eq!(
            privacy_safe_route_label(
                &Method::GET,
                "/restaurants/018f7df0-52ba-7cc6-9f4b-12f7df3e58af?secret=value"
            ),
            "unmatched"
        );
        assert_eq!(
            privacy_safe_method_label(&Method::from_bytes(b"PRIVATE-DATA").expect("test method")),
            "OTHER"
        );

        let trace_filter = privacy_trace_filter();
        assert!(trace_filter.would_enable("ros_api::http", &tracing::Level::INFO));
        assert!(!trace_filter.would_enable("ros_api::http", &tracing::Level::DEBUG));
        assert!(!trace_filter.would_enable("sqlx::query", &tracing::Level::INFO));
    }

    #[tokio::test]
    async fn sync_route_fails_closed_when_professional_runtime_is_disabled() {
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let body = serde_json::json!({
            "branch_id": branch_id.to_string(),
            "device_id": device_id.to_string(),
            "operations": [],
        })
        .to_string();
        let response = app(disabled_state())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sync/operations:batch")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn disabled_sync_route_rejects_before_parsing_the_request() {
        let response = app(disabled_state())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sync/operations:batch")
                    .header("content-type", "application/json")
                    .body(Body::from("not-json"))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers().get(axum::http::header::CONTENT_TYPE),
            Some(&axum::http::HeaderValue::from_static(
                "application/problem+json"
            ))
        );
    }

    #[tokio::test]
    async fn enabled_sync_route_authenticates_before_probing_schema() {
        let response = app(enabled_state_without_database_connection())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sync/operations:batch")
                    .header("content-type", "application/json")
                    .body(Body::from("not-json"))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(axum::http::header::CONTENT_TYPE),
            Some(&axum::http::HeaderValue::from_static(
                "application/problem+json"
            ))
        );
    }

    #[tokio::test]
    async fn authenticated_sync_route_rejects_when_schema_cannot_be_probed() {
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let token = signed_token(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        let response = app(enabled_state_without_database_connection())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sync/operations:batch")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from("not-json"))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers().get(axum::http::header::CONTENT_TYPE),
            Some(&axum::http::HeaderValue::from_static(
                "application/problem+json"
            ))
        );
    }

    #[tokio::test]
    #[ignore = "requires a disposable PostgreSQL database in ROS_TEST_DATABASE_URL"]
    async fn sync_startup_and_admission_reject_incomplete_or_tampered_schema() {
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .connect(&required_test_database_url())
            .await
            .expect("test PostgreSQL connects");
        let schema = format!("ros_api_readiness_{}", Uuid::now_v7().simple());
        create_test_schema(&pool, &schema).await;

        apply_migration(
            &pool,
            &schema,
            include_str!("../../../database/cloud-migrations/0001_tenant_event_log.sql"),
        )
        .await;
        let incomplete_store =
            PostgresSyncStore::from_pool_for_readiness_schema(pool.clone(), &schema);
        let incomplete_verifier = DeviceTokenVerifier::from_ed25519_pem(
            test_ed25519_keys().public_key_pem(),
            "https://identity.test.gotigin.invalid",
            "restaurant-os-professional-api",
        )
        .expect("runtime-generated test verifier");
        assert!(
            SyncRuntime::initialize(
                incomplete_verifier,
                incomplete_store,
                DEFAULT_SYNC_REQUESTS_PER_MINUTE,
            )
            .await
            .is_err(),
            "startup must refuse a partially migrated schema"
        );

        for migration in [
            include_str!(
                "../../../database/cloud-migrations/0002_tenant_integrity_and_forced_rls.sql"
            ),
            include_str!(
                "../../../database/cloud-migrations/0003_sync_event_actor_and_immutability.sql"
            ),
            include_str!(
                "../../../database/cloud-migrations/0004_sync_acknowledgements_and_device_grants.sql"
            ),
        ] {
            apply_migration(&pool, &schema, migration).await;
        }
        let store = PostgresSyncStore::from_pool_for_readiness_schema(pool.clone(), &schema);
        let verifier = DeviceTokenVerifier::from_ed25519_pem(
            test_ed25519_keys().public_key_pem(),
            "https://identity.test.gotigin.invalid",
            "restaurant-os-professional-api",
        )
        .expect("runtime-generated test verifier");
        let runtime = SyncRuntime::initialize(verifier, store, DEFAULT_SYNC_REQUESTS_PER_MINUTE)
            .await
            .expect("fully migrated schema passes the startup gate");

        sqlx::query(AssertSqlSafe(format!(
            "ALTER TABLE \"{schema}\".sync_events DISABLE TRIGGER sync_events_reject_delete"
        )))
        .execute(&pool)
        .await
        .expect("isolated test deliberately tampers with an immutability trigger");
        let state = ApiState {
            config: ApiConfig {
                listen_address: "127.0.0.1:3000".parse().expect("valid address"),
                deployment_environment: "development".to_owned(),
                operational: OperationalConfig::test_default(),
                sync: Some(SyncConfig {
                    database_url: required_test_database_url(),
                    token_public_key_file: "runtime-generated-in-test".to_owned(),
                    token_issuer: "https://identity.test.gotigin.invalid".to_owned(),
                    token_audience: "restaurant-os-professional-api".to_owned(),
                    requests_per_minute: DEFAULT_SYNC_REQUESTS_PER_MINUTE,
                }),
            },
            sync: Some(runtime),
        };
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let token = signed_token(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sync/operations:batch")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from("not-json"))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            response
                .headers()
                .get(axum::http::header::WWW_AUTHENTICATE)
                .is_none(),
            "schema failure must not be misreported as an authentication failure"
        );

        sqlx::query(AssertSqlSafe(format!("DROP SCHEMA \"{schema}\" CASCADE")))
            .execute(&pool)
            .await
            .expect("isolated readiness-test schema drops");
    }

    #[test]
    fn authorization_header_must_be_unique_and_utf8() {
        let mut headers = HeaderMap::new();
        headers.append(
            AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer one"),
        );
        headers.append(
            AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer two"),
        );
        assert_eq!(
            unique_authorization(&headers),
            Err(DeviceAuthError::Invalid)
        );
    }

    #[test]
    fn deployed_sync_requires_verified_database_tls_and_https_issuer() {
        let secure = SyncConfig {
            database_url:
                "postgresql://service:secret@database.test.invalid/ros?sslmode=verify-full"
                    .to_owned(),
            token_public_key_file: "unused".to_owned(),
            token_issuer: "https://identity.test.gotigin.invalid".to_owned(),
            token_audience: "restaurant-os-professional-api".to_owned(),
            requests_per_minute: DEFAULT_SYNC_REQUESTS_PER_MINUTE,
        };
        assert!(validate_sync_transport("staging", &secure).is_ok());
        assert!(validate_sync_transport("production", &secure).is_ok());

        let mut insecure_database = secure.clone();
        insecure_database.database_url =
            "postgresql://service:secret@database.test.invalid/ros?sslmode=require".to_owned();
        assert!(validate_sync_transport("production", &insecure_database).is_err());
        assert!(validate_sync_transport("development", &insecure_database).is_ok());

        let mut insecure_issuer = secure;
        insecure_issuer.token_issuer = "http://identity.test.gotigin.invalid".to_owned();
        assert!(validate_sync_transport("production", &insecure_issuer).is_err());
        assert!(validate_sync_transport("development", &insecure_issuer).is_ok());
    }

    #[test]
    fn deployment_configuration_rejects_unknown_environment() {
        assert!(validate_deployment_environment("production").is_ok());
        assert!(validate_deployment_environment("invalid").is_err());
    }

    #[test]
    fn boolean_configuration_is_deliberately_strict() {
        assert_eq!(parse_boolean_value("TEST", "true"), Ok(true));
        assert_eq!(parse_boolean_value("TEST", "false"), Ok(false));
        assert!(parse_boolean_value("TEST", "TRUE").is_err());
        assert!(parse_boolean_value("TEST", "1").is_err());
    }

    #[test]
    fn sync_rate_limit_is_monotonic_and_scoped_to_one_device() {
        let limiter = SyncRateLimiter::new(2);
        let organization = Uuid::now_v7();
        let first_device = Uuid::now_v7();
        let second_device = Uuid::now_v7();
        let started_at = Instant::now();

        assert_eq!(
            limiter.check_at(organization, first_device, started_at),
            RateLimitDecision::Allowed
        );
        assert_eq!(
            limiter.check_at(
                organization,
                first_device,
                started_at + Duration::from_secs(1),
            ),
            RateLimitDecision::Allowed
        );
        assert_eq!(
            limiter.check_at(
                organization,
                first_device,
                started_at + Duration::from_secs(2),
            ),
            RateLimitDecision::Limited {
                retry_after_seconds: 58,
            }
        );
        assert_eq!(
            limiter.check_at(
                organization,
                second_device,
                started_at + Duration::from_secs(2),
            ),
            RateLimitDecision::Allowed,
            "one device must not consume another device's local allowance"
        );
        assert_eq!(
            limiter.check_at(organization, first_device, started_at + RATE_LIMIT_WINDOW,),
            RateLimitDecision::Allowed,
            "the monotonic window must reset exactly at its boundary"
        );
    }

    #[test]
    fn rate_limit_problem_has_machine_readable_retry_contract() {
        let response = ApiProblem::rate_limited(17).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get(axum::http::header::RETRY_AFTER),
            Some(&axum::http::HeaderValue::from_static("17"))
        );
        assert_eq!(
            response.headers().get(axum::http::header::CONTENT_TYPE),
            Some(&axum::http::HeaderValue::from_static(
                "application/problem+json"
            ))
        );
    }

    #[test]
    fn rate_limit_configuration_is_canonical_and_bounded() {
        assert_eq!(parse_bounded_u32_value("TEST", "120", 1, 10_000), Ok(120));
        for invalid in ["", "0", "+1", " 1", "1 ", "10001", "1.0"] {
            assert!(
                parse_bounded_u32_value("TEST", invalid, 1, 10_000).is_err(),
                "{invalid:?} must not be accepted"
            );
        }
    }
}
