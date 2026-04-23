use teloxide::prelude::*;
use teloxide::types::MessageId;

use crate::bot::handlers::format::format_bytes_human;
use super::upsert_screen;
use crate::bot::handlers::shared::HandlerResult;

fn render_connections_summary_text(
    summary: Option<&crate::telemt_backend::TelemtConnectionsSummary>,
    error: Option<&str>,
) -> String {
    match summary {
        Some(summary) => {
            let mut lines = vec![
                "📈 Top пользователей".to_string(),
                String::new(),
                format!(
                    "Live connections: {} | ME: {} | Direct: {} | active users: {}",
                    summary.current_connections,
                    summary.current_connections_me,
                    summary.current_connections_direct,
                    summary.active_users
                ),
                String::new(),
                "Топ по соединениям:".to_string(),
            ];
            if summary.top_by_connections.is_empty() {
                lines.push("• нет данных".to_string());
            } else {
                for user in summary.top_by_connections.iter().take(5) {
                    lines.push(format!(
                        "• {} · conns {} · traffic {}",
                        user.username,
                        user.current_connections,
                        format_bytes_human(user.total_octets)
                    ));
                }
            }
            lines.push(String::new());
            lines.push("Топ по трафику:".to_string());
            if summary.top_by_throughput.is_empty() {
                lines.push("• нет данных".to_string());
            } else {
                for user in summary.top_by_throughput.iter().take(5) {
                    lines.push(format!(
                        "• {} · traffic {} · conns {}",
                        user.username,
                        format_bytes_human(user.total_octets),
                        user.current_connections
                    ));
                }
            }
            lines.join("\n")
        }
        None => {
            let mut text =
                "📈 Top пользователей\n\nRuntime endpoint недоступен или выключен в telemt API."
                    .to_string();
            if let Some(error) = error {
                text.push_str("\n\nПричина: ");
                text.push_str(&super::compact_line(error, 90));
            }
            text
        }
    }
}

pub async fn admin_show_connections_summary_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    summary: Option<crate::telemt_backend::TelemtConnectionsSummary>,
    summary_error: Option<String>,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_connections_summary_text(summary.as_ref(), summary_error.as_deref()),
        crate::bot::keyboards::connections_summary_keyboard(),
    )
    .await
}
