use crate::db::{InviteToken, RegistrationRequest};
use crate::telemt_backend::TelemtUserInfo;
use chrono::{DateTime, Local, Utc};

pub fn format_date(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&Local).format("%d.%m.%Y").to_string())
        .unwrap_or_else(|| "—".to_string())
}

pub fn format_mode(auto_approve: bool) -> &'static str {
    if auto_approve {
        "АВТОПОДТВЕРЖДЕНИЕ 🚀"
    } else {
        "Ручной ✅"
    }
}

pub fn format_timestamp(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|| format!("Некорректный timestamp: {}", ts))
}

pub fn user_display_name(user: &RegistrationRequest) -> String {
    user.tg_display_name
        .clone()
        .or_else(|| {
            user.tg_username
                .as_ref()
                .map(|username| format!("@{}", username))
        })
        .or_else(|| user.telemt_username.clone())
        .unwrap_or_else(|| format!("tg_{}", user.tg_user_id))
}

pub fn render_invite_token_button_title(token: &InviteToken) -> String {
    let mode = if token.auto_approve { "AUTO" } else { "MANUAL" };
    format!(
        "{} · {} · invite до {}",
        token.token,
        mode,
        format_date(token.expires_at)
    )
}

pub fn render_invite_token_card_text(token: &InviteToken) -> String {
    let mode = if token.auto_approve { "AUTO" } else { "MANUAL" };
    let usage = token
        .max_usage
        .map(|max| format!("{}/{}", token.usage_count, max))
        .unwrap_or_else(|| format!("{}/∞", token.usage_count));
    let created_by = token
        .created_by
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let expires_at = format_date(token.expires_at);

    format!(
        "🎟 Invite-токен\n\n\
         🔑 {}\n\
         ⚙️ {}\n\
         ⏳ Срок действия ссылки (invite): до {}\n\
         📊 Активаций по ссылке: {}\n\
         👤 {}\n\
         📅 создан {}\n\n\
         Срок доступа пользователя в telemt задаётся отдельно (карточка пользователя).",
        token.token,
        mode,
        expires_at,
        usage,
        created_by,
        format_date(token.created_at),
    )
}

pub fn format_bytes_human(value: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let value_f = value as f64;
    if value_f >= TB {
        format!("{:.2} TB", value_f / TB)
    } else if value_f >= GB {
        format!("{:.2} GB", value_f / GB)
    } else if value_f >= MB {
        format!("{:.2} MB", value_f / MB)
    } else if value_f >= KB {
        format!("{:.2} KB", value_f / KB)
    } else {
        format!("{value} B")
    }
}

fn format_optional_bytes(value: Option<u64>) -> String {
    value
        .map(format_bytes_human)
        .unwrap_or_else(|| "—".to_string())
}

fn format_optional_usize(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn format_ip_list(values: &[String]) -> String {
    if values.is_empty() {
        "—".to_string()
    } else {
        values.join(", ")
    }
}

fn render_runtime_alerts(runtime_info: Option<&TelemtUserInfo>) -> String {
    let Some(info) = runtime_info else {
        return "нет live-данных".to_string();
    };

    let mut alerts = Vec::new();
    if let (Some(current), Some(limit)) = (info.current_connections, info.max_tcp_conns)
        && current > limit as u64
    {
        alerts.push(format!("TCP {}>{}", current, limit));
    }
    if let (Some(current), Some(limit)) = (info.active_unique_ips, info.max_unique_ips)
        && current > limit
    {
        alerts.push(format!("active IPs {}>{}", current, limit));
    }
    if let (Some(current), Some(limit)) = (info.recent_unique_ips, info.max_unique_ips)
        && current > limit
    {
        alerts.push(format!("recent IPs {}>{}", current, limit));
    }
    if let (Some(total), Some(quota)) = (info.total_octets, info.data_quota_bytes)
        && total >= quota
    {
        alerts.push(format!(
            "traffic {} / {}",
            format_bytes_human(total),
            format_bytes_human(quota)
        ));
    }

    if alerts.is_empty() {
        "не обнаружены".to_string()
    } else {
        alerts.join(", ")
    }
}

pub fn render_user_card_text(
    user: &RegistrationRequest,
    runtime_info: Option<&TelemtUserInfo>,
) -> String {
    let username = user
        .tg_username
        .as_deref()
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| "—".to_string());
    let telemt = user.telemt_username.as_deref().unwrap_or("—");
    let backend_mode = user.backend_mode.as_deref().unwrap_or("—");
    let last_sync = user
        .last_synced_at
        .map(format_timestamp)
        .unwrap_or_else(|| "—".to_string());
    let last_revision = user
        .last_seen_revision
        .as_deref()
        .map(|value| {
            if value.chars().count() > 24 {
                format!("{}...", value.chars().take(21).collect::<String>())
            } else {
                value.to_string()
            }
        })
        .unwrap_or_else(|| "—".to_string());
    let sync_error = user.last_sync_error.as_deref().unwrap_or("нет");
    let runtime_source = runtime_info
        .map(|info| info.source.as_str())
        .unwrap_or("нет данных");
    let current_connections = runtime_info
        .and_then(|info| info.current_connections)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let active_unique_ips = runtime_info
        .and_then(|info| info.active_unique_ips)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let recent_unique_ips = runtime_info
        .and_then(|info| info.recent_unique_ips)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let active_ip_list = runtime_info
        .map(|info| format_ip_list(&info.active_unique_ips_list))
        .unwrap_or_else(|| "—".to_string());
    let recent_ip_list = runtime_info
        .map(|info| format_ip_list(&info.recent_unique_ips_list))
        .unwrap_or_else(|| "—".to_string());
    let total_octets = runtime_info
        .map(|info| format_optional_bytes(info.total_octets))
        .unwrap_or_else(|| "—".to_string());
    let user_ad_tag = runtime_info
        .and_then(|info| info.user_ad_tag.as_deref())
        .unwrap_or("—");
    let max_tcp_conns = runtime_info
        .map(|info| format_optional_usize(info.max_tcp_conns))
        .unwrap_or_else(|| "—".to_string());
    let data_quota = runtime_info
        .map(|info| format_optional_bytes(info.data_quota_bytes))
        .unwrap_or_else(|| "—".to_string());
    let max_unique_ips = runtime_info
        .map(|info| format_optional_usize(info.max_unique_ips))
        .unwrap_or_else(|| "—".to_string());
    let expiration = runtime_info
        .and_then(|info| info.expiration_rfc3339.as_deref())
        .unwrap_or("—");
    let links_count = runtime_info
        .map(|info| info.links.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    let runtime_alerts = render_runtime_alerts(runtime_info);
    let invite_token_id = user
        .invite_token_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "—".to_string());

    format!(
        "👤 {}\n\n\
         🆔 {}\n\
         📱 {}\n\
         📋 статус: {}\n\
         🎟 ID ссылки (invite): {}\n\
         🔗 telemt: {}\n\
         🧩 backend: {}\n\
         🔁 sync: {}\n\
         🪪 revision: {}\n\
         ⚠️ sync error: {}\n\
         \n\
         📡 runtime source: {}\n\
         🔌 live connections: {}\n\
         🌐 active unique IPs: {}\n\
         🕘 recent unique IPs: {}\n\
         📋 active IP list: {}\n\
         📋 recent IP list: {}\n\
         📦 traffic: {}\n\
         🏷 ad tag: {}\n\
         🛡 max TCP: {}\n\
         🧮 quota: {}\n\
         🌍 max unique IPs: {}\n\
         🚨 anomalies: {}\n\
         ⏳ expires: {}\n\
         🔗 links: {}\n\
         📅 {}",
        user_display_name(user),
        user.tg_user_id,
        username,
        user.status,
        invite_token_id,
        telemt,
        backend_mode,
        last_sync,
        last_revision,
        sync_error,
        runtime_source,
        current_connections,
        active_unique_ips,
        recent_unique_ips,
        active_ip_list,
        recent_ip_list,
        total_octets,
        user_ad_tag,
        max_tcp_conns,
        data_quota,
        max_unique_ips,
        runtime_alerts,
        expiration,
        links_count,
        format_timestamp(user.created_at),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        format_bytes_human, render_invite_token_button_title, render_user_card_text,
        user_display_name,
    };
    use crate::db::{InviteToken, RegistrationRequest, RequestStatus};
    use crate::telemt_backend::{TelemtBackendMode, TelemtUserInfo};

    fn sample_request() -> RegistrationRequest {
        RegistrationRequest {
            id: 1,
            tg_user_id: 42,
            tg_username: Some("alice".to_string()),
            tg_display_name: Some("Alice".to_string()),
            status: RequestStatus::Approved,
            telemt_username: Some("tg_42".to_string()),
            secret: Some("secret".to_string()),
            created_at: 1_700_000_000,
            backend_mode: Some("control_api".to_string()),
            last_sync_error: None,
            last_seen_revision: Some("rev-123".to_string()),
            last_synced_at: Some(1_700_000_100),
            invite_token_id: Some(7),
        }
    }

    fn sample_runtime_info() -> TelemtUserInfo {
        TelemtUserInfo {
            source: TelemtBackendMode::ControlApi,
            user_ad_tag: Some("promo".to_string()),
            max_tcp_conns: Some(10),
            expiration_rfc3339: Some("2026-04-01T00:00:00Z".to_string()),
            data_quota_bytes: Some(4096),
            max_unique_ips: Some(3),
            current_connections: Some(2),
            active_unique_ips: Some(1),
            active_unique_ips_list: vec!["1.1.1.1".to_string()],
            recent_unique_ips: Some(2),
            recent_unique_ips_list: vec!["1.1.1.1".to_string(), "2.2.2.2".to_string()],
            total_octets: Some(8192),
            links: vec!["link-1".to_string(), "link-2".to_string()],
        }
    }

    #[test]
    fn user_display_name_prefers_display_name_then_username_then_telemt() {
        let request = sample_request();
        assert_eq!(user_display_name(&request), "Alice");

        let mut no_display = sample_request();
        no_display.tg_display_name = None;
        assert_eq!(user_display_name(&no_display), "@alice");

        let mut no_username = sample_request();
        no_username.tg_display_name = None;
        no_username.tg_username = None;
        assert_eq!(user_display_name(&no_username), "tg_42");
    }

    #[test]
    fn render_invite_token_button_title_contains_mode_and_date() {
        let token = InviteToken {
            id: 1,
            token: "TOKEN123".to_string(),
            created_at: 1_700_000_000,
            expires_at: 1_800_000_000,
            auto_approve: true,
            created_by: Some(7),
            usage_count: 1,
            max_usage: Some(5),
            is_active: true,
        };

        let title = render_invite_token_button_title(&token);

        assert!(title.contains("TOKEN123"));
        assert!(title.contains("AUTO"));
        assert!(title.contains("invite до "));
    }

    #[test]
    fn format_bytes_human_formats_thresholds() {
        assert_eq!(format_bytes_human(512), "512 B");
        assert_eq!(format_bytes_human(2048), "2.00 KB");
        assert_eq!(format_bytes_human(5 * 1024 * 1024), "5.00 MB");
    }

    #[test]
    fn render_user_card_text_includes_runtime_and_sync_data() {
        let text = render_user_card_text(&sample_request(), Some(&sample_runtime_info()));

        assert!(text.contains("Alice"));
        assert!(text.contains("control_api"));
        assert!(text.contains("runtime source: control_api"));
        assert!(text.contains("live connections: 2"));
        assert!(text.contains("links: 2"));
        assert!(text.contains("sync error: нет"));
        assert!(text.contains("ID ссылки (invite): 7"));
    }

    #[test]
    fn render_user_card_text_shows_dash_when_no_invite_token_id() {
        let mut req = sample_request();
        req.invite_token_id = None;
        let text = render_user_card_text(&req, None);
        assert!(text.contains("ID ссылки (invite): —"));
    }

}
