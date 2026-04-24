use super::super::common::{ack_callback, admin_callback_target};
use super::AdminActionResult;
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{admin_show_stats, show_admin_home};
use crate::bot::handlers::state::{BotState, WizardState, clear_wizard_state, set_wizard_state};
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Bot, CallbackQuery, Requester};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowAdminHome => {
            let Some((admin_id, chat_id, message_id)) =
                admin_callback_target(bot, q, state).await?
            else {
                return Ok(true);
            };
            clear_wizard_state(state, admin_id).await?;
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_admin_home(bot, chat_id, Some(message_id), state).await?;
            Ok(true)
        }
        CallbackAction::ShowStats => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_stats(bot, chat_id, state, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::PromptBroadcastApproved => {
            let Some((admin_id, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            set_wizard_state(state, admin_id, WizardState::AdminBroadcastAwaitingMessage).await?;
            ack_callback(
                bot,
                q.id.clone(),
                Some("Отправьте текст следующим сообщением"),
                false,
            )
            .await?;
            bot.send_message(
                chat_id,
                state
                    .config
                    .bot_messages
                    .broadcast_prompt_text("всем пользователям со статусом «доступ открыт»"),
            )
            .reply_markup(crate::bot::keyboards::cancel_keyboard(
                CallbackAction::ShowAdminHome,
            ))
            .await?;
            Ok(true)
        }
        CallbackAction::PromptImportUser => {
            let Some((admin_id, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            set_wizard_state(state, admin_id, WizardState::AdminImportAwaitingTgId).await?;
            ack_callback(
                bot,
                q.id.clone(),
                Some("Жду Telegram user id"),
                false,
            )
            .await?;
            bot.send_message(
                chat_id,
                "Импорт пользователя из telemt по известному Telegram user id.\n\n\
                 Отправьте числовой id (например 123456789). Пользователь должен существовать в telemt как `tg_<id>`.\n\n\
                 Требуется `telemt_api.enabled = true`.",
            )
            .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
