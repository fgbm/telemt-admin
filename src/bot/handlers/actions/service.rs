use crate::bot::handlers::callback_data::ServiceAction;
use crate::bot::handlers::screens::{
    ServicePanelData, admin_show_connections_summary_screen, admin_show_service_panel_screen,
};
use crate::bot::handlers::shared::HandlerResult;
use crate::bot::handlers::state::BotState;
use teloxide::prelude::{Bot, ChatId};
use teloxide::types::MessageId;

pub async fn execute_service_action(state: &BotState, action: ServiceAction) -> String {
    let result = match action {
        ServiceAction::Start => state.telemt_runtime.start().await,
        ServiceAction::Stop => state.telemt_runtime.stop().await,
        ServiceAction::Restart => state.telemt_runtime.restart().await,
        ServiceAction::Reload => state.telemt_runtime.reload().await,
        ServiceAction::Status => state.telemt_runtime.status().await,
    };

    if result.success {
        format!("{}: OK", action.as_str())
    } else {
        format!("{}: {}", action.as_str(), result.stderr)
    }
}

async fn load_service_panel_data(
    state: &BotState,
    notice: Option<&str>,
) -> Result<ServicePanelData, anyhow::Error> {
    let caps = state.telemt_runtime.capabilities();
    let summary = state.telemt_runtime.summary().await;
    let service_events = state.telemt_runtime.recent_events(3).await;
    let admin_events = state.db.list_recent_admin_activities(4).await?;
    let stats = state.db.admin_stats().await?;
    let active_tokens = state.db.count_active_invite_tokens().await?;
    let sync_health = state.db.sync_health_summary(3).await?;
    let (telemt_stats, telemt_stats_error) = match state.telemt_backend.stats_summary().await {
        Ok(value) => (value, None),
        Err(error) => {
            tracing::warn!(error = %error, "Не удалось получить stats summary telemt");
            (None, Some(error.to_string()))
        }
    };
    let (connections_summary, connections_summary_error) =
        match state.telemt_backend.connections_summary(5).await {
            Ok(value) => (value, None),
            Err(error) => {
                tracing::warn!(error = %error, "Не удалось получить connections summary telemt");
                (None, Some(error.to_string()))
            }
        };
    let (runtime_snapshot, runtime_snapshot_error) =
        match state.telemt_backend.runtime_snapshot(6).await {
            Ok(value) => (value, None),
            Err(error) => {
                tracing::warn!(error = %error, "Не удалось получить runtime snapshot telemt");
                (None, Some(error.to_string()))
            }
        };

    Ok(ServicePanelData {
        notice: notice.map(str::to_string),
        caps,
        runtime_label: state.telemt_runtime.display_label().to_string(),
        backend_mode: state.telemt_backend.mode(),
        summary,
        service_events,
        admin_events,
        stats,
        active_tokens,
        sync_health,
        telemt_stats,
        telemt_stats_error,
        connections_summary,
        connections_summary_error,
        runtime_snapshot,
        runtime_snapshot_error,
    })
}

pub async fn admin_show_service_panel(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
) -> HandlerResult {
    admin_show_service_panel_with_notice(bot, chat_id, state, message_id, None).await
}

pub async fn admin_show_service_panel_with_notice(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
    notice: Option<&str>,
) -> HandlerResult {
    let data = load_service_panel_data(state, notice).await?;
    admin_show_service_panel_screen(bot, chat_id, message_id, data).await
}

pub async fn admin_show_connections_summary(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let (summary, summary_error) = match state.telemt_backend.connections_summary(10).await {
        Ok(value) => (value, None),
        Err(error) => {
            tracing::warn!(error = %error, "Не удалось получить connections summary telemt");
            (None, Some(error.to_string()))
        }
    };
    admin_show_connections_summary_screen(bot, chat_id, message_id, summary, summary_error).await
}
