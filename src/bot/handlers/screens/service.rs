use teloxide::prelude::*;
use teloxide::types::MessageId;

use crate::bot::handlers::callback_data::ServiceAction;
use crate::bot::handlers::format::format_timestamp;
use super::compact_line;
use super::upsert_screen;
use crate::bot::handlers::shared::HandlerResult;
use crate::bot::handlers::state::BotState;
use crate::db::{AdminActivity, AdminStats, SyncHealthSummary};
use crate::runtime::{RuntimeCapabilities, ServiceEvents, ServiceSummary};

pub struct ServicePanelData {
    pub notice: Option<String>,
    pub caps: RuntimeCapabilities,
    pub runtime_label: String,
    pub backend_mode: crate::telemt_backend::TelemtBackendMode,
    pub summary: ServiceSummary,
    pub service_events: ServiceEvents,
    pub admin_events: Vec<AdminActivity>,
    pub stats: AdminStats,
    pub active_tokens: i64,
    pub sync_health: SyncHealthSummary,
    pub telemt_stats: Option<crate::telemt_backend::TelemtStatsSummary>,
    pub telemt_stats_error: Option<String>,
    pub connections_summary: Option<crate::telemt_backend::TelemtConnectionsSummary>,
    pub connections_summary_error: Option<String>,
    pub runtime_snapshot: Option<crate::telemt_backend::TelemtRuntimeSnapshot>,
    pub runtime_snapshot_error: Option<String>,
}

fn service_action_title(action: ServiceAction) -> &'static str {
    match action {
        ServiceAction::Start => "запустить сервис",
        ServiceAction::Stop => "остановить сервис",
        ServiceAction::Restart => "перезапустить сервис",
        ServiceAction::Reload => "перечитать конфиг",
        ServiceAction::Status => "обновить статус",
    }
}

fn render_service_panel_text(data: &ServicePanelData) -> String {
    let status_label = if data.caps.shows_systemd_unit {
        super::service_status_label(&data.summary.active_state, &data.summary.sub_state)
    } else {
        format!("{} · {}", data.summary.active_state, data.summary.sub_state)
    };
    let admin_version = env!("CARGO_PKG_VERSION");

    let mut lines = vec![
        "⚙️ Статус".to_string(),
        String::new(),
        format!("telemt-admin: v{} · бот активен", admin_version),
    ];

    lines.push(String::new());
    if data.caps.shows_systemd_unit {
        lines.push(format!(
            "Юнит {}: {}",
            data.runtime_label,
            status_label
        ));
        lines.push(format!(
            "Проверка systemd: {}",
            if data.summary.success {
                "OK"
            } else {
                "ошибка"
            }
        ));
    } else {
        lines.push(format!(
            "Telemt ({}): {}",
            data.runtime_label,
            status_label
        ));
        lines.push(format!(
            "Статус host-runtime: {}",
            if data.summary.success {
                "OK"
            } else {
                "ошибка"
            }
        ));
    }

    if let Some(notice) = data.notice.as_deref() {
        lines.push(format!("Действие: {}", notice));
    }

    if data.caps.shows_systemd_unit {
        lines.push(format!(
            "Unit: {} | PID: {}",
            data.summary.unit_file_state,
            data.summary
                .main_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "—".to_string())
        ));
    } else {
        lines.push("Unit/PID: не применимо (runtime external/none)".to_string());
    }
    lines.push(format!(
        "Пользователи: {} | Заявки: {} | Токены: {}",
        data.stats.approved, data.stats.pending, data.active_tokens
    ));
    lines.push(format!(
        "Sync: degraded {} | API {} | legacy {}",
        data.sync_health.degraded_users,
        data.sync_health.approved_via_control_api,
        data.sync_health.approved_via_legacy
    ));
    lines.push(format!(
        "Режим провижининга: {}",
        match data.backend_mode {
            crate::telemt_backend::TelemtBackendMode::LegacyFile => "файл + systemd",
            crate::telemt_backend::TelemtBackendMode::ControlApi => "control API",
        }
    ));

    if let Some(exec_status) = data.summary.exec_main_status {
        lines.push(format!("Код процесса: {}", exec_status));
    }
    if let Some(error) = &data.summary.error {
        lines.push(format!("Ошибка статуса: {}", compact_line(error, 90)));
    }

    if let Some(snapshot) = data.runtime_snapshot.as_ref() {
        lines.push(String::new());
        lines.push("Демон (control API)".to_string());
        lines.push(format!("Профиль: {}", snapshot.source.as_str()));
        lines.push(format!(
            "Версия: {} | Health: {} | read-only: {}",
            snapshot
                .build_version
                .as_deref()
                .unwrap_or("—"),
            snapshot.health_status,
            if snapshot.api_read_only { "да" } else { "нет" }
        ));
        if let Some(mode) = &snapshot.transport_mode {
            lines.push(format!("Транспорт: {}", mode));
        }
        if let (Some(acc), Some(me_ready), Some(proxy), Some(route)) = (
            snapshot.accepting_new_connections,
            snapshot.me_runtime_ready,
            snapshot.use_middle_proxy,
            snapshot.route_mode.as_deref(),
        ) {
            lines.push(format!(
                "Маршрут: {} | middle proxy: {} | ME runtime: {} | приём соединений: {}",
                route,
                if proxy { "да" } else { "нет" },
                if me_ready { "да" } else { "нет" },
                if acc { "да" } else { "нет" }
            ));
        }
        if let (Some(cfg), Some(ok), Some(bad)) = (
            snapshot.upstream_configured_total,
            snapshot.upstream_healthy_total,
            snapshot.upstream_unhealthy_total,
        ) {
            lines.push(format!("Upstream: здоровых {} из {}", ok, cfg));
            if bad > 0 {
                lines.push(format!("⚠️ Нездоровых upstream: {}", bad));
            }
        }
        match snapshot.me_selftest_enabled {
            Some(true) => {
                let kdf = snapshot
                    .me_selftest_kdf_state
                    .as_deref()
                    .unwrap_or("—");
                let skew = snapshot
                    .me_selftest_timeskew_state
                    .as_deref()
                    .unwrap_or("—");
                lines.push(format!("ME self-test: KDF `{}` · время `{}`", kdf, skew));
            }
            Some(false) => {
                lines.push("ME self-test: данные пока недоступны (ME pool)".to_string());
            }
            None => {}
        }
        if let Some(startup_status) = snapshot.startup_status.as_deref() {
            let progress = snapshot
                .startup_progress_pct
                .map(|value| format!("{:.1}%", value))
                .unwrap_or_else(|| "—".to_string());
            lines.push(format!("Запуск: {} ({})", startup_status, progress));
        }
        if let Some(stage) = snapshot.startup_stage.as_deref() {
            lines.push(format!("Этап: {}", compact_line(stage, 60)));
        }
        if let Some(enabled) = snapshot.api_whitelist_enabled {
            let entries = snapshot
                .api_whitelist_entries
                .map(|value| value.to_string())
                .unwrap_or_else(|| "—".to_string());
            lines.push(format!(
                "API whitelist: {} ({})",
                if enabled {
                    "вкл"
                } else {
                    "выкл"
                },
                entries
            ));
        }
        if let Some(enabled) = snapshot.api_auth_header_enabled {
            lines.push(format!(
                "API auth header: {}",
                if enabled { "вкл" } else { "выкл" }
            ));
        }
        if let Some(revision) = snapshot.last_revision.as_deref() {
            lines.push(format!("Revision: {}", compact_line(revision, 24)));
        }
        lines.push(String::new());
        lines.push("События API:".to_string());
        if snapshot.events.is_empty() {
            lines.push("• нет данных".to_string());
        } else {
            for event in snapshot.events.iter().take(4) {
                lines.push(format!(
                    "• {} · {} · {}",
                    format_timestamp(event.ts_epoch_secs),
                    compact_line(&event.event_type, 28),
                    compact_line(&event.context, 42)
                ));
            }
        }
    } else if let Some(error) = data.runtime_snapshot_error.as_deref() {
        lines.push(String::new());
        lines.push("Демон (control API)".to_string());
        lines.push(format!(
            "Ошибка опроса runtime API: {}",
            compact_line(error, 90)
        ));
    }

    if let Some(summary) = data.telemt_stats.as_ref() {
        lines.push(String::new());
        lines.push("Нагрузка".to_string());
        lines.push(format!(
            "Uptime: {:.0} сек | users in config: {}",
            summary.uptime_seconds, summary.configured_users
        ));
        lines.push(format!(
            "Всего соединений: {} | bad: {} | handshake timeout: {}",
            summary.connections_total,
            summary.connections_bad_total,
            summary.handshake_timeouts_total
        ));
    } else if let Some(error) = data.telemt_stats_error.as_deref() {
        lines.push(String::new());
        lines.push(format!(
            "Нагрузка telemt: ошибка опроса ({})",
            compact_line(error, 90)
        ));
    }
    if let Some(connections) = data.connections_summary.as_ref() {
        lines.push(format!(
            "Live: {} | ME: {} | Direct: {} | active users: {}",
            connections.current_connections,
            connections.current_connections_me,
            connections.current_connections_direct,
            connections.active_users
        ));
    } else if let Some(error) = data.connections_summary_error.as_deref() {
        lines.push(format!(
            "Live connections: ошибка опроса ({})",
            compact_line(error, 90)
        ));
    }

    if !data.sync_health.top_sync_errors.is_empty() {
        lines.push(String::new());
        lines.push("Sync ошибки:".to_string());
        for item in data.sync_health.top_sync_errors.iter().take(3) {
            lines.push(format!("• {} · {}", item.code, item.affected_users));
        }
    }

    lines.push(String::new());
    lines.push(
        if data.caps.shows_journal_tail {
            "События сервиса:"
        } else {
            "События сервиса (journal недоступен в этом runtime):"
        }
        .to_string(),
    );
    if data.service_events.lines.is_empty() {
        lines.push(
            data.service_events
                .error
                .as_deref()
                .map(|error| format!("• {}", compact_line(error, 90)))
                .unwrap_or_else(|| "• нет данных".to_string()),
        );
    } else {
        if !data.service_events.success {
            lines.push("• журнал прочитан частично".to_string());
        }
        for line in data.service_events.lines.iter().take(3) {
            lines.push(format!("• {}", compact_line(line, 90)));
        }
    }

    lines.push(String::new());
    lines.push("Действия админа:".to_string());
    if data.admin_events.is_empty() {
        lines.push("• пока нет событий".to_string());
    } else {
        for item in data.admin_events.iter().take(4) {
            lines.push(format!(
                "• {} · {}",
                format_timestamp(item.timestamp),
                compact_line(&super::admin_activity_summary(item), 70)
            ));
        }
    }

    lines.join("\n")
}

pub async fn admin_show_service_panel_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    data: ServicePanelData,
) -> HandlerResult {
    let text = render_service_panel_text(&data);
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::service_control_buttons(&data.caps),
    )
    .await
}

pub async fn show_service_action_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    action: ServiceAction,
    state: &BotState,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        state.config.bot_messages.service_action_confirm_text(service_action_title(action)),
    )
    .reply_markup(crate::bot::keyboards::confirm_service_action_keyboard(
        action,
    ))
    .await?;
    Ok(())
}
