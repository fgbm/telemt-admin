use teloxide::prelude::*;
use teloxide::types::MessageId;

use crate::bot::handlers::format::{format_bytes_human, format_timestamp};
use crate::bot::handlers::state::BotState;
use super::{service_status_label, admin_activity_summary, upsert_screen};
use crate::bot::handlers::shared::HandlerResult;

pub async fn admin_show_stats(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let stats = state.db.admin_stats().await?;
    let caps = state.telemt_runtime.capabilities();
    let summary = state.telemt_runtime.summary().await;
    let admin_events = state.db.list_recent_admin_activities(4).await?;
    let telemt_stats = state.telemt_backend.stats_summary().await.ok().flatten();
    let connections_summary = state.telemt_backend.connections_summary(3).await.ok().flatten();
    let status_label = if caps.shows_systemd_unit {
        service_status_label(&summary.active_state, &summary.sub_state)
    } else {
        format!("{} · {}", summary.active_state, summary.sub_state)
    };

    let mut lines = vec![
        "📊 Сводка состояния".to_string(),
        String::new(),
        format!("Сервис: {}", state.telemt_runtime.display_label()),
        format!("Статус: {}", status_label),
        format!(
            "{}: {}",
            if caps.shows_systemd_unit {
                "Проверка systemd"
            } else {
                "Host-runtime"
            },
            if summary.success {
                "OK"
            } else {
                "Ошибка"
            }
        ),
    ];
    if caps.shows_systemd_unit {
        lines.push(format!(
            "Unit: {} | PID: {}",
            summary.unit_file_state,
            summary
                .main_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "—".to_string())
        ));
    } else {
        lines.push("Unit/PID: не применимо (runtime external/none)".to_string());
    }

    if let Some(exec_status) = summary.exec_main_status {
        lines.push(format!("Код процесса: {}", exec_status));
    }
    if let Some(error) = &summary.error {
        lines.push(format!("Ошибка статуса: {}", super::compact_line(error, 90)));
    }

    lines.push(String::new());
    lines.push("Доступ и заявки:".to_string());
    lines.push(format!("• Активные пользователи: {}", stats.approved));
    lines.push(format!("• Заявки в ожидании: {}", stats.pending));
    lines.push(format!("• Отклонённые заявки: {}", stats.rejected));
    lines.push(format!("• Отозванные доступы: {}", stats.deleted));
    lines.push(format!("• Всего записей: {}", stats.total));

    lines.push(String::new());
    lines.push("Invite-токены:".to_string());
    lines.push(format!("• Активные: {}", stats.tokens_active));
    lines.push(format!(
        "• Активные ручные / авто: {} / {}",
        stats.tokens_manual_active, stats.tokens_auto_active
    ));
    lines.push(format!("• Отозванные: {}", stats.tokens_revoked));
    lines.push(format!("• Истёкшие: {}", stats.tokens_expired));
    lines.push(format!("• Исчерпанные: {}", stats.tokens_exhausted));
    lines.push(format!("• Всего создано: {}", stats.tokens_total));

    lines.push(String::new());
    lines.push("Live telemt:".to_string());
    if let Some(stats_summary) = telemt_stats.as_ref() {
        lines.push(format!(
            "• Uptime: {:.0} s | configured users: {}",
            stats_summary.uptime_seconds, stats_summary.configured_users
        ));
        lines.push(format!(
            "• Connections total / bad: {} / {}",
            stats_summary.connections_total, stats_summary.connections_bad_total
        ));
        lines.push(format!(
            "• Handshake timeouts: {}",
            stats_summary.handshake_timeouts_total
        ));
    } else {
        lines.push("• stats summary: нет данных".to_string());
    }
    if let Some(live) = connections_summary.as_ref() {
        lines.push(format!(
            "• Live connections: {} | ME: {} | Direct: {} | active users: {}",
            live.current_connections,
            live.current_connections_me,
            live.current_connections_direct,
            live.active_users
        ));
        if let Some(top) = live.top_by_connections.first() {
            lines.push(format!(
                "• Top TCP: {} ({} conn, {})",
                top.username,
                top.current_connections,
                format_bytes_human(top.total_octets)
            ));
        }
        if let Some(top) = live.top_by_throughput.first() {
            lines.push(format!(
                "• Top traffic: {} ({})",
                top.username,
                format_bytes_human(top.total_octets)
            ));
        }
        let mut alerts = Vec::new();
        if !live.top_by_connections.is_empty() {
            for user in &live.top_by_connections {
                if user.current_connections >= 10 {
                    alerts.push(format!("TCP spike: {} ({})", user.username, user.current_connections));
                }
            }
        }
        if !live.top_by_throughput.is_empty() {
            for user in &live.top_by_throughput {
                if user.total_octets >= 1024_u64.pow(3) {
                    alerts.push(format!(
                        "traffic spike: {} ({})",
                        user.username,
                        format_bytes_human(user.total_octets)
                    ));
                }
            }
        }
        if alerts.is_empty() {
            lines.push("• Аномалии: не обнаружены".to_string());
        } else {
            lines.push(format!("• Аномалии: {}", alerts.join("; ")));
        }
    } else {
        lines.push("• connections summary: нет данных".to_string());
    }

    lines.push(String::new());
    lines.push("Недавняя активность:".to_string());
    if admin_events.is_empty() {
        lines.push("• пока нет событий".to_string());
    } else {
        for item in admin_events.iter().take(4) {
            lines.push(format!(
                "• {} · {}",
                format_timestamp(item.timestamp),
                super::compact_line(&admin_activity_summary(item), 70)
            ));
        }
    }

    let text = lines.join("\n");
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::stats_keyboard(),
    )
    .await
}
