use super::super::common::{ack_callback, admin_callback_target};
use super::AdminActionResult;
use crate::bot::handlers::actions::approve_request_and_build_link;
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{admin_show_pending_requests_page, show_pending_request_card};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use crate::bot::handlers::state::BotState;
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Bot, CallbackQuery, ChatId, Requester};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowPendingRequests => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_pending_requests_page(bot, chat_id, state, 1, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowPendingRequestsPage { page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_pending_requests_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::OpenPendingRequest { request_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(request) = state.db.get_pending_by_id(request_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Заявка уже обработана"), false).await?;
                admin_show_pending_requests_page(bot, chat_id, state, page, Some(message_id))
                    .await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Открыта заявка"), false).await?;
            show_pending_request_card(bot, chat_id, message_id, &request, page).await?;
            Ok(true)
        }
        CallbackAction::ApproveRequest { request_id, page } => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let message_target = callback_message_target(q);
            let (request, link) = match approve_request_and_build_link(state, request_id).await? {
                Some(payload) => payload,
                None => {
                    ack_callback(
                        bot,
                        q.id.clone(),
                        Some("Заявка уже обработана или не найдена"),
                        false,
                    )
                    .await?;
                    return Ok(true);
                }
            };
            ack_callback(bot, q.id.clone(), Some("Одобрено"), false).await?;
            if let Some((chat_id, message_id)) = message_target {
                bot.edit_message_text(chat_id, message_id, "✅ Заявка одобрена")
                    .reply_markup(crate::bot::keyboards::pending_result_keyboard(page))
                    .await?;
            }
            bot.send_message(
                ChatId(request.tg_user_id),
                state.config.user_link_message(&link),
            )
            .await?;
            tracing::info!("Admin {} approved request #{}", admin_id, request_id);
            Ok(true)
        }
        CallbackAction::RejectRequest { request_id, page } => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let message_target = callback_message_target(q);
            let request = state.db.reject(request_id).await?;
            ack_callback(bot, q.id.clone(), Some("Отклонено"), false).await?;
            if let Some(request) = request {
                if let Some((chat_id, message_id)) = message_target {
                    bot.edit_message_text(chat_id, message_id, "❌ Заявка отклонена")
                        .reply_markup(crate::bot::keyboards::pending_result_keyboard(page))
                        .await?;
                }
                bot.send_message(
                    ChatId(request.tg_user_id),
                    state.config.bot_messages.request_rejected_or_default(),
                )
                .await?;
            }
            tracing::info!("Admin {} rejected request #{}", admin_id, request_id);
            Ok(true)
        }
        _ => Ok(false),
    }
}
