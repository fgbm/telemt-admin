use super::super::common::{ack_callback, admin_callback_target};
use super::AdminActionResult;
use crate::bot::handlers::actions::{
    admin_show_connections_summary, admin_show_service_panel,
    admin_show_service_panel_with_notice, execute_service_action,
};
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::show_service_action_confirm;
use crate::bot::handlers::state::BotState;
use teloxide::prelude::{Bot, CallbackQuery};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowServicePanel => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_service_panel(bot, chat_id, state, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowConnectionsSummary => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_connections_summary(bot, chat_id, state, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ConfirmServiceAction { action } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_service_action_confirm(bot, chat_id, message_id, action, state).await?;
            Ok(true)
        }
        CallbackAction::ExecuteServiceAction { action } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = execute_service_action(state, action).await;
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            admin_show_service_panel_with_notice(
                bot,
                chat_id,
                state,
                Some(message_id),
                Some(&status_text),
            )
            .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
