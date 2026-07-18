//! Privacy-allowlisted local diagnostics.
//!
//! Events are validated against a fixed field and code allow-list before they
//! are appended to a rotating JSONL file outside SQLCipher. See
//! `docs/contracts/local-diagnostics-v1.md`.

use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const SCHEMA_VERSION: u32 = 1;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RECENT_EVENTS: usize = 200;
const MAX_DETAIL_CODE_BYTES: usize = 64;
const MAX_PACK_EVENTS: usize = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticComponent {
    Bootstrap,
    Storage,
    Bridge,
    Ui,
    Share,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticOutcome {
    Ok,
    Denied,
    Failed,
    Busy,
    Throttled,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    pub schema_version: u32,
    pub occurred_at_utc: String,
    pub event_code: String,
    pub component: DiagnosticComponent,
    pub outcome: DiagnosticOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub platform: String,
    pub app_channel: String,
    pub process_correlation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticRecordInput<'a> {
    pub event_code: &'a str,
    pub component: DiagnosticComponent,
    pub outcome: DiagnosticOutcome,
    pub duration_ms: Option<u64>,
    pub platform: &'a str,
    pub app_channel: &'a str,
    pub detail_code: Option<&'a str>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DiagnosticsError {
    InvalidEventCode,
    InvalidDetailCode,
    InvalidPlatform,
    InvalidAppChannel,
    ForbiddenField,
    Io,
}

impl std::fmt::Display for DiagnosticsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEventCode => write!(f, "diagnostic event code is not allow-listed"),
            Self::InvalidDetailCode => write!(f, "diagnostic detail code is not allow-listed"),
            Self::InvalidPlatform => write!(f, "diagnostic platform label is invalid"),
            Self::InvalidAppChannel => write!(f, "diagnostic app channel is invalid"),
            Self::ForbiddenField => write!(f, "diagnostic payload contained a forbidden field"),
            Self::Io => write!(f, "diagnostic storage I/O failed"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticsPack {
    pub json_bytes: Vec<u8>,
    pub sha256: String,
    pub event_count: u64,
}

pub struct LocalDiagnosticsSink {
    directory: PathBuf,
    process_correlation: String,
    lock: Mutex<()>,
}

impl LocalDiagnosticsSink {
    pub fn open(application_support_directory: impl AsRef<Path>) -> Result<Self, DiagnosticsError> {
        let directory = application_support_directory
            .as_ref()
            .join("diagnostics");
        fs::create_dir_all(&directory).map_err(|_| DiagnosticsError::Io)?;
        Ok(Self {
            directory,
            process_correlation: Uuid::now_v7().to_string(),
            lock: Mutex::new(()),
        })
    }

    pub fn process_correlation(&self) -> &str {
        &self.process_correlation
    }

    pub fn events_path(&self) -> PathBuf {
        self.directory.join("events.jsonl")
    }

    pub fn record(&self, input: DiagnosticRecordInput<'_>) -> Result<(), DiagnosticsError> {
        let event = build_event(input, &self.process_correlation)?;
        let line = serde_json::to_string(&event).map_err(|_| DiagnosticsError::ForbiddenField)?;
        reject_forbidden_substrings(&line)?;
        let _guard = self.lock.lock().map_err(|_| DiagnosticsError::Io)?;
        self.rotate_if_needed()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events_path())
            .map_err(|_| DiagnosticsError::Io)?;
        writeln!(file, "{line}").map_err(|_| DiagnosticsError::Io)?;
        Ok(())
    }

    pub fn recent_events(&self, limit: usize) -> Result<Vec<DiagnosticEvent>, DiagnosticsError> {
        let limit = limit.clamp(1, MAX_RECENT_EVENTS);
        let _guard = self.lock.lock().map_err(|_| DiagnosticsError::Io)?;
        let mut events = read_events_file(&self.events_path())?;
        if let Ok(rotated) = read_events_file(&self.directory.join("events.jsonl.1")) {
            let mut combined = rotated;
            combined.append(&mut events);
            events = combined;
        }
        if events.len() > limit {
            events = events.split_off(events.len() - limit);
        }
        Ok(events)
    }

    pub fn export_pack(&self) -> Result<DiagnosticsPack, DiagnosticsError> {
        let _guard = self.lock.lock().map_err(|_| DiagnosticsError::Io)?;
        let mut events = read_events_file(&self.directory.join("events.jsonl.1")).unwrap_or_default();
        events.extend(read_events_file(&self.events_path())?);
        if events.len() > MAX_PACK_EVENTS {
            events = events.split_off(events.len() - MAX_PACK_EVENTS);
        }
        let pack = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "pack_kind": "ros_local_diagnostics_v1",
            "generated_at_utc": utc_now(),
            "process_correlation": self.process_correlation,
            "event_count": events.len(),
            "events": events,
        });
        let json_bytes = serde_json::to_vec_pretty(&pack).map_err(|_| DiagnosticsError::Io)?;
        reject_forbidden_substrings(std::str::from_utf8(&json_bytes).unwrap_or(""))?;
        let sha256 = format!("{:x}", Sha256::digest(&json_bytes));
        Ok(DiagnosticsPack {
            event_count: events.len() as u64,
            json_bytes,
            sha256,
        })
    }

    pub fn clear(&self) -> Result<(), DiagnosticsError> {
        let _guard = self.lock.lock().map_err(|_| DiagnosticsError::Io)?;
        let _ = fs::remove_file(self.events_path());
        let _ = fs::remove_file(self.directory.join("events.jsonl.1"));
        Ok(())
    }

    fn rotate_if_needed(&self) -> Result<(), DiagnosticsError> {
        let path = self.events_path();
        let Ok(metadata) = fs::metadata(&path) else {
            return Ok(());
        };
        if metadata.len() < MAX_FILE_BYTES {
            return Ok(());
        }
        let rotated = self.directory.join("events.jsonl.1");
        let _ = fs::remove_file(&rotated);
        fs::rename(&path, &rotated).map_err(|_| DiagnosticsError::Io)?;
        Ok(())
    }
}

fn build_event(
    input: DiagnosticRecordInput<'_>,
    process_correlation: &str,
) -> Result<DiagnosticEvent, DiagnosticsError> {
    if !is_allowed_event_code(input.event_code) {
        return Err(DiagnosticsError::InvalidEventCode);
    }
    if !is_allowed_platform(input.platform) {
        return Err(DiagnosticsError::InvalidPlatform);
    }
    if !is_allowed_app_channel(input.app_channel) {
        return Err(DiagnosticsError::InvalidAppChannel);
    }
    let detail_code = match input.detail_code {
        Some(code) => {
            if !is_allowed_detail_code(code) {
                return Err(DiagnosticsError::InvalidDetailCode);
            }
            Some(code.to_owned())
        }
        None => None,
    };
    Ok(DiagnosticEvent {
        schema_version: SCHEMA_VERSION,
        occurred_at_utc: utc_now(),
        event_code: input.event_code.to_owned(),
        component: input.component,
        outcome: input.outcome,
        duration_ms: input.duration_ms,
        platform: input.platform.to_owned(),
        app_channel: input.app_channel.to_owned(),
        process_correlation: process_correlation.to_owned(),
        detail_code,
    })
}

fn read_events_file(path: &Path) -> Result<Vec<DiagnosticEvent>, DiagnosticsError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path).map_err(|_| DiagnosticsError::Io)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|_| DiagnosticsError::Io)?;
        if line.trim().is_empty() {
            continue;
        }
        reject_forbidden_substrings(&line)?;
        let event: DiagnosticEvent =
            serde_json::from_str(&line).map_err(|_| DiagnosticsError::ForbiddenField)?;
        if !is_allowed_event_code(&event.event_code) {
            continue;
        }
        events.push(event);
    }
    Ok(events)
}

fn utc_now() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let secs = millis / 1_000;
    let rem = millis % 1_000;
    // Compact RFC3339 without chrono dependency: good enough for diagnostics.
    format_unix_millis(secs as u64, rem as u32)
}

fn format_unix_millis(secs: u64, millis: u32) -> String {
    // Use a simple UTC formatter via days since epoch.
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
    )
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    // Howard Hinnant civil_from_days algorithm.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn is_allowed_event_code(code: &str) -> bool {
    matches!(
        code,
        "bootstrap_open"
            | "storage_integrity"
            | "migration_apply"
            | "staff_unlock"
            | "staff_lock"
            | "owner_pin_configure"
            | "sale_complete"
            | "sale_preview"
            | "invoice_refund"
            | "invoice_void"
            | "accounting_day_close"
            | "backup_create"
            | "backup_verify"
            | "backup_restore"
            | "nav_overview"
            | "nav_pos"
            | "nav_kitchen"
            | "nav_inventory"
            | "nav_reports"
            | "action_backup_create"
            | "action_backup_verify"
            | "action_backup_restore"
            | "action_day_close"
            | "action_export_csv"
            | "action_diagnostics_open"
            | "ui_error"
            | "ui_zone_error"
            | "diagnostics_export"
            | "diagnostics_share_attempted"
            | "diagnostics_cleared"
    )
}

const FORBIDDEN_DETAIL_TOKENS: [&str; 12] = [
    "pin",
    "key",
    "password",
    "token",
    "email",
    "phone",
    "customer",
    "path",
    "invoice",
    "actor",
    "device",
    "hash",
];

fn is_allowed_detail_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_DETAIL_CODE_BYTES
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !code
            .split('_')
            .any(|part| FORBIDDEN_DETAIL_TOKENS.contains(&part))
}

fn is_allowed_platform(value: &str) -> bool {
    matches!(
        value,
        "linux" | "windows" | "macos" | "android" | "ios" | "unknown"
    )
}

fn is_allowed_app_channel(value: &str) -> bool {
    matches!(value, "development" | "production")
}

fn reject_forbidden_substrings(value: &str) -> Result<(), DiagnosticsError> {
    let lower = value.to_ascii_lowercase();
    // Hard reject common secret markers if they ever appear in serialized output.
    for needle in [
        "argon2",
        "sqlcipher",
        "bearer ",
        "password",
        "@",
        "-----begin",
    ] {
        if lower.contains(needle) {
            return Err(DiagnosticsError::ForbiddenField);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink() -> (tempfile::TempDir, LocalDiagnosticsSink) {
        let temp = tempfile::tempdir().expect("temp");
        let sink = LocalDiagnosticsSink::open(temp.path()).expect("sink");
        (temp, sink)
    }

    #[test]
    fn records_allowlisted_events_and_exports_pack() {
        let (_temp, sink) = sink();
        sink.record(DiagnosticRecordInput {
            event_code: "bootstrap_open",
            component: DiagnosticComponent::Bootstrap,
            outcome: DiagnosticOutcome::Ok,
            duration_ms: Some(12),
            platform: "linux",
            app_channel: "development",
            detail_code: Some("ready"),
        })
        .expect("record");
        let recent = sink.recent_events(10).expect("recent");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].event_code, "bootstrap_open");
        let pack = sink.export_pack().expect("pack");
        assert_eq!(pack.event_count, 1);
        assert!(!pack.sha256.is_empty());
        assert!(String::from_utf8_lossy(&pack.json_bytes).contains("ros_local_diagnostics_v1"));
    }

    #[test]
    fn rejects_unknown_event_codes() {
        let (_temp, sink) = sink();
        assert_eq!(
            sink.record(DiagnosticRecordInput {
                event_code: "customer_phone_lookup",
                component: DiagnosticComponent::Ui,
                outcome: DiagnosticOutcome::Ok,
                duration_ms: None,
                platform: "linux",
                app_channel: "development",
                detail_code: None,
            }),
            Err(DiagnosticsError::InvalidEventCode)
        );
    }

    #[test]
    fn rejects_detail_codes_with_forbidden_tokens() {
        let (_temp, sink) = sink();
        assert_eq!(
            sink.record(DiagnosticRecordInput {
                event_code: "staff_unlock",
                component: DiagnosticComponent::Bridge,
                outcome: DiagnosticOutcome::Denied,
                duration_ms: None,
                platform: "linux",
                app_channel: "development",
                detail_code: Some("bad_pin_attempt"),
            }),
            Err(DiagnosticsError::InvalidDetailCode)
        );
    }

    #[test]
    fn accepts_safe_bootstrap_detail_codes() {
        let (_temp, sink) = sink();
        sink.record(DiagnosticRecordInput {
            event_code: "bootstrap_open",
            component: DiagnosticComponent::Bootstrap,
            outcome: DiagnosticOutcome::Failed,
            duration_ms: Some(3),
            platform: "linux",
            app_channel: "development",
            detail_code: Some("secret_missing"),
        })
        .expect("secret_missing is allow-listed");
        sink.record(DiagnosticRecordInput {
            event_code: "bootstrap_open",
            component: DiagnosticComponent::Bootstrap,
            outcome: DiagnosticOutcome::Failed,
            duration_ms: Some(3),
            platform: "linux",
            app_channel: "development",
            detail_code: Some("secure_store_unavailable"),
        })
        .expect("secure_store_unavailable is allow-listed");
    }

    #[test]
    fn rotates_when_events_file_exceeds_cap() {
        let temp = tempfile::tempdir().expect("temp");
        let sink = LocalDiagnosticsSink::open(temp.path()).expect("sink");
        let path = sink.events_path();
        // Seed an oversized file so the next append rotates it.
        let oversized = vec![b'x'; MAX_FILE_BYTES as usize];
        std::fs::write(&path, &oversized).expect("seed oversized file");
        sink.record(DiagnosticRecordInput {
            event_code: "nav_reports",
            component: DiagnosticComponent::Ui,
            outcome: DiagnosticOutcome::Ok,
            duration_ms: None,
            platform: "linux",
            app_channel: "development",
            detail_code: None,
        })
        .expect("record after rotation");
        assert!(temp.path().join("diagnostics/events.jsonl.1").exists());
        let current = std::fs::read_to_string(sink.events_path()).expect("current");
        assert!(current.contains("nav_reports"));
        assert!(!current.starts_with("xxx"));
    }

    #[test]
    fn clear_removes_local_files() {
        let (_temp, sink) = sink();
        sink.record(DiagnosticRecordInput {
            event_code: "nav_reports",
            component: DiagnosticComponent::Ui,
            outcome: DiagnosticOutcome::Ok,
            duration_ms: None,
            platform: "linux",
            app_channel: "development",
            detail_code: None,
        })
        .expect("record");
        sink.clear().expect("clear");
        assert!(sink.recent_events(10).expect("recent").is_empty());
    }
}
