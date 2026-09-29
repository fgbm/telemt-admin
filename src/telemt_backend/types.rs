use reqwest::StatusCode;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemtBackendMode {
    LegacyFile,
    ControlApi,
}

impl TelemtBackendMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegacyFile => "legacy_file",
            Self::ControlApi => "control_api",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProvisionedUser {
    pub secret: String,
    pub link: Option<String>,
    pub mode: TelemtBackendMode,
    pub revision: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteUserResult {
    pub removed: bool,
    pub mode: TelemtBackendMode,
    pub revision: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TelemtRuntimeEvent {
    pub ts_epoch_secs: i64,
    pub event_type: String,
    pub context: String,
}

#[derive(Debug, Clone)]
pub struct TelemtUserInfo {
    pub source: TelemtBackendMode,
    pub user_ad_tag: Option<String>,
    pub max_tcp_conns: Option<usize>,
    pub expiration_rfc3339: Option<String>,
    pub data_quota_bytes: Option<u64>,
    pub max_unique_ips: Option<usize>,
    pub current_connections: Option<u64>,
    pub active_unique_ips: Option<usize>,
    pub active_unique_ips_list: Vec<String>,
    pub recent_unique_ips: Option<usize>,
    pub recent_unique_ips_list: Vec<String>,
    pub total_octets: Option<u64>,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TelemtUserPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tcp_conns: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_rfc3339: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_quota_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_unique_ips: Option<usize>,
}

impl TelemtUserPatch {
    pub fn is_empty(&self) -> bool {
        self.max_tcp_conns.is_none()
            && self.expiration_rfc3339.is_none()
            && self.data_quota_bytes.is_none()
            && self.max_unique_ips.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct TelemtStatsSummary {
    pub uptime_seconds: f64,
    pub connections_total: u64,
    pub connections_bad_total: u64,
    pub handshake_timeouts_total: u64,
    pub configured_users: usize,
}

#[derive(Debug, Clone)]
pub struct TelemtConnectionTopUser {
    pub username: String,
    pub current_connections: u64,
    pub total_octets: u64,
}

/// Текущая активность одного пользователя telemt (без ссылок, секретов и IP-адресов).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemtUserActivity {
    pub username: String,
    pub current_connections: u64,
    pub active_unique_ips: usize,
    pub total_octets: u64,
}

#[derive(Debug, Clone)]
pub struct TelemtConnectionsSummary {
    pub current_connections: u64,
    pub current_connections_me: u64,
    pub current_connections_direct: u64,
    pub active_users: usize,
    pub top_by_connections: Vec<TelemtConnectionTopUser>,
    pub top_by_throughput: Vec<TelemtConnectionTopUser>,
}

#[derive(Debug, Clone)]
pub struct TelemtMonitorSnapshot {
    pub health_status: String,
    pub accepting_new_connections: Option<bool>,
    pub me_runtime_ready: Option<bool>,
    pub upstream_unhealthy_total: Option<u64>,
    pub me_selftest_kdf_state: Option<String>,
    pub me_selftest_timeskew_state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TelemtRuntimeSnapshot {
    pub source: TelemtBackendMode,
    pub health_status: String,
    pub api_read_only: bool,
    pub build_version: Option<String>,
    pub transport_mode: Option<String>,
    pub startup_status: Option<String>,
    pub startup_stage: Option<String>,
    pub startup_progress_pct: Option<f64>,
    pub api_whitelist_enabled: Option<bool>,
    pub api_whitelist_entries: Option<usize>,
    pub api_auth_header_enabled: Option<bool>,
    /// `/v1/runtime/gates`
    pub accepting_new_connections: Option<bool>,
    pub me_runtime_ready: Option<bool>,
    pub use_middle_proxy: Option<bool>,
    pub route_mode: Option<String>,
    /// `/v1/runtime/me-selftest`
    pub me_selftest_kdf_state: Option<String>,
    pub me_selftest_timeskew_state: Option<String>,
    pub me_selftest_enabled: Option<bool>,
    /// `/v1/runtime/upstream_quality`
    pub upstream_configured_total: Option<u64>,
    pub upstream_healthy_total: Option<u64>,
    pub upstream_unhealthy_total: Option<u64>,
    pub events: Vec<TelemtRuntimeEvent>,
    pub last_revision: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("telemt API error ({status}): {message}")]
pub struct TelemtApiError {
    pub status: StatusCode,
    pub code: Option<String>,
    pub message: String,
}
