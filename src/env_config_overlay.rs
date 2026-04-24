//! Переменные окружения `TELEMT_ADMIN__*` как overlay поверх TOML (см. ADR 004).

use std::path::PathBuf;

use crate::config::Config;
use crate::runtime::RuntimeMode;

const PREFIX: &str = "TELEMT_ADMIN__";

/// Применить whitelist env-overrides. Возвращает список имён установленных ключей (без значений).
pub fn apply(config: &mut Config) -> Result<Vec<String>, anyhow::Error> {
    let mut applied = Vec::new();

    if let Some(v) = read_nonempty("BOT_TOKEN") {
        config.bot_token = Some(v);
        applied.push("TELEMT_ADMIN__BOT_TOKEN".to_string());
    }

    if let Some(v) = read_nonempty("BOT_USERNAME") {
        config.bot_username = Some(v);
        applied.push("TELEMT_ADMIN__BOT_USERNAME".to_string());
    }

    if let Some(v) = read_nonempty("ADMIN_IDS") {
        config.admin_ids = parse_admin_ids(&v)?;
        applied.push("TELEMT_ADMIN__ADMIN_IDS".to_string());
    }

    if let Some(v) = read_nonempty("TELEMT_CONFIG_PATH") {
        config.telemt_config_path = PathBuf::from(v);
        applied.push("TELEMT_ADMIN__TELEMT_CONFIG_PATH".to_string());
    }

    if let Some(v) = read_nonempty("DB_PATH") {
        config.db_path = PathBuf::from(v);
        applied.push("TELEMT_ADMIN__DB_PATH".to_string());
    }

    if let Some(v) = read_nonempty("SERVICE_NAME") {
        config.service_name = v;
        applied.push("TELEMT_ADMIN__SERVICE_NAME".to_string());
    }

    if let Some(v) = read_nonempty("RUNTIME__MODE") {
        config.runtime.get_or_insert_with(default_runtime_section);
        if let Some(r) = config.runtime.as_mut() {
            r.mode = parse_runtime_mode(&v)?;
        }
        applied.push("TELEMT_ADMIN__RUNTIME__MODE".to_string());
    }

    if let Some(v) = read_nonempty("RUNTIME__SERVICE_NAME") {
        config.runtime.get_or_insert_with(default_runtime_section);
        if let Some(r) = config.runtime.as_mut() {
            r.service_name = Some(v);
        }
        applied.push("TELEMT_ADMIN__RUNTIME__SERVICE_NAME".to_string());
    }

    if let Some(v) = read_nonempty("RUNTIME__LABEL") {
        config.runtime.get_or_insert_with(default_runtime_section);
        if let Some(r) = config.runtime.as_mut() {
            r.label = Some(v);
        }
        applied.push("TELEMT_ADMIN__RUNTIME__LABEL".to_string());
    }

    if let Some(v) = read_nonempty("TELEMT_API__ENABLED") {
        config.telemt_api.enabled = parse_bool(&v)?;
        applied.push("TELEMT_ADMIN__TELEMT_API__ENABLED".to_string());
    }

    if let Some(v) = read_nonempty("TELEMT_API__BASE_URL") {
        config.telemt_api.base_url = v;
        applied.push("TELEMT_ADMIN__TELEMT_API__BASE_URL".to_string());
    }

    if let Some(v) = read_nonempty("TELEMT_API__AUTH_HEADER") {
        config.telemt_api.auth_header = Some(v);
        applied.push("TELEMT_ADMIN__TELEMT_API__AUTH_HEADER".to_string());
    }

    if let Some(v) = read_nonempty("TELEMT_API__TIMEOUT_MS") {
        config.telemt_api.timeout_ms = v.trim().parse::<u64>().map_err(|_| {
            anyhow::anyhow!("TELEMT_ADMIN__TELEMT_API__TIMEOUT_MS: ожидается положительное целое")
        })?;
        if config.telemt_api.timeout_ms == 0 {
            return Err(anyhow::anyhow!(
                "TELEMT_ADMIN__TELEMT_API__TIMEOUT_MS должен быть > 0"
            ));
        }
        applied.push("TELEMT_ADMIN__TELEMT_API__TIMEOUT_MS".to_string());
    }

    if let Some(v) = read_nonempty("TELEMT_API__ALLOW_FILE_FALLBACK") {
        config.telemt_api.allow_file_fallback = parse_bool(&v)?;
        applied.push("TELEMT_ADMIN__TELEMT_API__ALLOW_FILE_FALLBACK".to_string());
    }

    if let Some(v) = read_nonempty("TELEMT_API__PREFER_API_LINKS") {
        config.telemt_api.prefer_api_links = parse_bool(&v)?;
        applied.push("TELEMT_ADMIN__TELEMT_API__PREFER_API_LINKS".to_string());
    }

    if let Some(v) = read_nonempty("NOTIFICATIONS__ENABLED") {
        config.notifications.enabled = parse_bool(&v)?;
        applied.push("TELEMT_ADMIN__NOTIFICATIONS__ENABLED".to_string());
    }

    if let Some(v) = read_nonempty("NOTIFICATIONS__HEALTH_CHECK_INTERVAL_SECS") {
        let n = v.trim().parse::<u64>().map_err(|_| {
            anyhow::anyhow!(
                "TELEMT_ADMIN__NOTIFICATIONS__HEALTH_CHECK_INTERVAL_SECS: ожидается положительное целое"
            )
        })?;
        if n == 0 {
            return Err(anyhow::anyhow!(
                "TELEMT_ADMIN__NOTIFICATIONS__HEALTH_CHECK_INTERVAL_SECS должен быть > 0"
            ));
        }
        config.notifications.health_check_interval_secs = n;
        applied.push("TELEMT_ADMIN__NOTIFICATIONS__HEALTH_CHECK_INTERVAL_SECS".to_string());
    }

    if let Some(v) = read_nonempty("NOTIFICATIONS__NOTIFY_ON_HEALTH_CHANGE") {
        config.notifications.notify_on_health_change = parse_bool(&v)?;
        applied.push("TELEMT_ADMIN__NOTIFICATIONS__NOTIFY_ON_HEALTH_CHANGE".to_string());
    }

    if let Some(v) = read_nonempty("NOTIFICATIONS__NOTIFY_ON_RUNTIME_ALERTS") {
        config.notifications.notify_on_runtime_alerts = parse_bool(&v)?;
        applied.push("TELEMT_ADMIN__NOTIFICATIONS__NOTIFY_ON_RUNTIME_ALERTS".to_string());
    }

    if let Some(v) = read_nonempty("NOTIFICATIONS__NOTIFY_ON_NEW_REQUEST") {
        config.notifications.notify_on_new_request = parse_bool(&v)?;
        applied.push("TELEMT_ADMIN__NOTIFICATIONS__NOTIFY_ON_NEW_REQUEST".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__START_WITHOUT_INVITE") {
        config.bot_messages.start_without_invite = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__START_WITHOUT_INVITE".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__INVITE_MANUAL_PROMPT") {
        config.bot_messages.invite_manual_prompt = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__INVITE_MANUAL_PROMPT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__INVITE_FOLLOWUP_PROMPT") {
        config.bot_messages.invite_followup_prompt = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__INVITE_FOLLOWUP_PROMPT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__USER_LINK_TEMPLATE") {
        config.bot_messages.user_link_template = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__USER_LINK_TEMPLATE".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__ACCESS_APPROVED_TEMPLATE") {
        config.bot_messages.access_approved_template = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__ACCESS_APPROVED_TEMPLATE".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__REQUEST_SUBMITTED") {
        config.bot_messages.request_submitted = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__REQUEST_SUBMITTED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__REQUEST_PENDING") {
        config.bot_messages.request_pending = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__REQUEST_PENDING".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__REQUEST_REJECTED") {
        config.bot_messages.request_rejected = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__REQUEST_REJECTED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__BROADCAST_PROMPT") {
        config.bot_messages.broadcast_prompt = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__BROADCAST_PROMPT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__BROADCAST_CANCELLED") {
        config.bot_messages.broadcast_cancelled = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__BROADCAST_CANCELLED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__BROADCAST_SUMMARY_TEMPLATE") {
        config.bot_messages.broadcast_summary_template = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__BROADCAST_SUMMARY_TEMPLATE".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__NO_ACCESS_STATUS_TEXT") {
        config.bot_messages.no_access_status_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__NO_ACCESS_STATUS_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__NO_ACCESS_LINK_TEXT") {
        config.bot_messages.no_access_link_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__NO_ACCESS_LINK_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__ADMIN_HOME_TEXT") {
        config.bot_messages.admin_home_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__ADMIN_HOME_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__USER_HOME_APPROVED") {
        config.bot_messages.user_home_approved = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__USER_HOME_APPROVED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__USER_HOME_PENDING") {
        config.bot_messages.user_home_pending = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__USER_HOME_PENDING".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__USER_HOME_REJECTED") {
        config.bot_messages.user_home_rejected = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__USER_HOME_REJECTED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__USER_HOME_DELETED") {
        config.bot_messages.user_home_deleted = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__USER_HOME_DELETED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__USAGE_GUIDE_TEXT") {
        config.bot_messages.usage_guide_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__USAGE_GUIDE_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__HELP_ADMIN_TEXT") {
        config.bot_messages.help_admin_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__HELP_ADMIN_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__HELP_USER_TEXT") {
        config.bot_messages.help_user_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__HELP_USER_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__UNKNOWN_COMMAND_TEXT") {
        config.bot_messages.unknown_command_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__UNKNOWN_COMMAND_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__ADMIN_ONLY_DEEP_LINK_TEXT") {
        config.bot_messages.admin_only_deep_link_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__ADMIN_ONLY_DEEP_LINK_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__USER_NOT_FOUND_TEXT") {
        config.bot_messages.user_not_found_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__USER_NOT_FOUND_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__TOKEN_NOT_FOUND_TEXT") {
        config.bot_messages.token_not_found_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__TOKEN_NOT_FOUND_TEXT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__SERVICE_ACTION_CONFIRM_TEMPLATE") {
        config.bot_messages.service_action_confirm_template = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__SERVICE_ACTION_CONFIRM_TEMPLATE".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__TOKEN_ERROR_NOT_FOUND") {
        config.bot_messages.token_error_not_found = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__TOKEN_ERROR_NOT_FOUND".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__TOKEN_ERROR_REVOKED") {
        config.bot_messages.token_error_revoked = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__TOKEN_ERROR_REVOKED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__TOKEN_ERROR_EXPIRED") {
        config.bot_messages.token_error_expired = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__TOKEN_ERROR_EXPIRED".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__TOKEN_ERROR_USAGE_LIMIT") {
        config.bot_messages.token_error_usage_limit = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__TOKEN_ERROR_USAGE_LIMIT".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__ADMIN_NOTIFY_NEW_REQUEST_TEMPLATE") {
        config.bot_messages.admin_notify_new_request_template = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__ADMIN_NOTIFY_NEW_REQUEST_TEMPLATE".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__ADMIN_NOTIFY_AUTO_APPROVE_TEMPLATE") {
        config.bot_messages.admin_notify_auto_approve_template = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__ADMIN_NOTIFY_AUTO_APPROVE_TEMPLATE".to_string());
    }

    if let Some(v) = read_nonempty("BOT_MESSAGES__FALLBACK_UNKNOWN_REQUEST_TEXT") {
        config.bot_messages.fallback_unknown_request_text = Some(v);
        applied.push("TELEMT_ADMIN__BOT_MESSAGES__FALLBACK_UNKNOWN_REQUEST_TEXT".to_string());
    }

    Ok(applied)
}

fn read_nonempty(suffix: &str) -> Option<String> {
    let key = format!("{PREFIX}{suffix}");
    std::env::var(&key).ok().and_then(|v| {
        let t = v.trim();
        if t.is_empty() { None } else { Some(v) }
    })
}

fn parse_bool(s: &str) -> Result<bool, anyhow::Error> {
    match s.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(anyhow::anyhow!("ожидается true/false (или 1/0)")),
    }
}

fn parse_admin_ids(s: &str) -> Result<Vec<i64>, anyhow::Error> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        let id: i64 = t
            .parse()
            .map_err(|_| anyhow::anyhow!("TELEMT_ADMIN__ADMIN_IDS: неверный id «{}»", t))?;
        out.push(id);
    }
    if out.is_empty() {
        return Err(anyhow::anyhow!(
            "TELEMT_ADMIN__ADMIN_IDS: нужен хотя бы один admin id"
        ));
    }
    Ok(out)
}

fn parse_runtime_mode(s: &str) -> Result<RuntimeMode, anyhow::Error> {
    match s.trim().to_lowercase().as_str() {
        "systemd" => Ok(RuntimeMode::Systemd),
        "external" => Ok(RuntimeMode::External),
        "none" => Ok(RuntimeMode::None),
        _ => Err(anyhow::anyhow!(
            "TELEMT_ADMIN__RUNTIME__MODE: ожидается systemd|external|none"
        )),
    }
}

fn default_runtime_section() -> crate::config::RuntimeSection {
    crate::config::RuntimeSection {
        mode: RuntimeMode::Systemd,
        service_name: None,
        label: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_admin_ids, parse_bool, parse_runtime_mode};
    use crate::runtime::RuntimeMode;

    #[test]
    fn parse_bool_accepts_common_truthy_and_falsy_values() {
        assert_eq!(parse_bool("true").ok(), Some(true));
        assert_eq!(parse_bool("YES").ok(), Some(true));
        assert_eq!(parse_bool("0").ok(), Some(false));
        assert_eq!(parse_bool("off").ok(), Some(false));
        assert!(parse_bool("maybe").is_err());
    }

    #[test]
    fn parse_admin_ids_trims_and_requires_at_least_one_value() {
        assert_eq!(parse_admin_ids("1, 2,3").ok(), Some(vec![1, 2, 3]));
        assert!(parse_admin_ids(" , ").is_err());
        assert!(parse_admin_ids("1, nope").is_err());
    }

    #[test]
    fn parse_runtime_mode_accepts_known_variants() {
        assert_eq!(parse_runtime_mode("systemd").ok(), Some(RuntimeMode::Systemd));
        assert_eq!(parse_runtime_mode("external").ok(), Some(RuntimeMode::External));
        assert_eq!(parse_runtime_mode("none").ok(), Some(RuntimeMode::None));
        assert!(parse_runtime_mode("docker").is_err());
    }
}
