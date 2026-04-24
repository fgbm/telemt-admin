use super::common::{ack_callback, replace_wizard_state};
use crate::bot::handlers::actions::{send_user_link, try_auto_import_remote_user_by_tg_id};
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{show_admin_home, show_usage_guide, show_user_home};
use crate::bot::handlers::shared::callback_message_target;
use crate::bot::handlers::state::{BotState, WizardState, clear_wizard_state};
use teloxide::prelude::{Bot, CallbackQuery, Requester};

pub async fn handle_user_action(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match action {
        CallbackAction::ShowUserHome => {
            let user_id = q.from.id.0 as i64;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                clear_wizard_state(state, user_id).await?;
                let username = q.from.username.as_deref();
                let display_name = q.from.full_name();
                let _ = try_auto_import_remote_user_by_tg_id(
                    state,
                    user_id,
                    username,
                    Some(display_name.as_str()),
                    None,
                )
                .await?;
                ack_callback(bot, q.id.clone(), None, false).await?;
                show_user_home(bot, chat_id, Some(message_id), state, user_id).await?;
            }
            Ok(true)
        }
        CallbackAction::ShowUserLink => {
            let user_id = q.from.id.0 as i64;
            let username = q.from.username.as_deref();
            let display_name = Some(q.from.full_name());
            ack_callback(bot, q.id.clone(), Some("Отправляю ссылку"), false).await?;
            if let Some((chat_id, _)) = callback_message_target(q) {
                send_user_link(
                    bot,
                    chat_id,
                    user_id,
                    username,
                    display_name.as_deref(),
                    state,
                )
                .await?;
            }
            Ok(true)
        }
        CallbackAction::ShowUsageGuide => {
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                ack_callback(bot, q.id.clone(), None, false).await?;
                show_usage_guide(bot, chat_id, Some(message_id), state).await?;
            }
            Ok(true)
        }
        CallbackAction::PromptInviteToken => {
            let user_id = q.from.id.0 as i64;
            replace_wizard_state(state, user_id, WizardState::AwaitingInviteToken).await?;
            ack_callback(
                bot,
                q.id.clone(),
                Some("Жду invite-токен следующим сообщением"),
                false,
            )
            .await?;
            if let Some((chat_id, _)) = callback_message_target(q) {
                bot.send_message(
                    chat_id,
                    state.config.bot_messages.invite_followup_prompt_or_default(),
                )
                .await?;
            }
            Ok(true)
        }
        CallbackAction::CancelWizard => {
            let user_id = q.from.id.0 as i64;
            clear_wizard_state(state, user_id).await?;
            ack_callback(bot, q.id.clone(), Some("Сценарий отменён"), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                if state.config.is_admin(user_id) {
                    show_admin_home(bot, chat_id, Some(message_id), state).await?;
                } else {
                    let username = q.from.username.as_deref();
                    let display_name = q.from.full_name();
                    let _ = try_auto_import_remote_user_by_tg_id(
                        state,
                        user_id,
                        username,
                        Some(display_name.as_str()),
                        None,
                    )
                    .await?;
                    show_user_home(bot, chat_id, Some(message_id), state, user_id).await?;
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
