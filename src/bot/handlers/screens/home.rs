use teloxide::prelude::*;
use teloxide::types::{KeyboardRemove, MessageId};

use crate::bot::handlers::state::BotState;
use super::upsert_screen;
use crate::bot::handlers::shared::HandlerResult;
use crate::db::RequestStatus;

pub async fn send_text_with_keyboard_removed(
    bot: &Bot,
    chat_id: ChatId,
    text: impl Into<String>,
) -> HandlerResult {
    bot.send_message(chat_id, text.into())
        .reply_markup(KeyboardRemove::new())
        .await?;
    Ok(())
}

pub async fn show_admin_home(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
) -> HandlerResult {
    let text = state.config.bot_messages.admin_home_or_default();
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text.to_string(),
        crate::bot::keyboards::admin_home_keyboard(),
    )
    .await
}

pub async fn show_user_home(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
    user_id: i64,
) -> HandlerResult {
    let (text, has_access) = if let Some(existing) = state.db.get_request_by_tg_user(user_id).await? {
        let has_access = existing.status == RequestStatus::Approved;
        let text = match existing.status {
            RequestStatus::Approved => {
                state.config.bot_messages.user_home_approved_text("").to_string()
            }
            RequestStatus::Pending => {
                state.config.bot_messages.user_home_pending_or_default().to_string()
            }
            RequestStatus::Rejected => {
                state.config.bot_messages.user_home_rejected_or_default().to_string()
            }
            RequestStatus::Deleted => {
                state.config.bot_messages.user_home_deleted_or_default().to_string()
            }
        };
        (text, has_access)
    } else {
        (
            state.config.bot_messages.no_access_status_or_default().to_string(),
            false,
        )
    };

    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::user_home_keyboard(has_access),
    )
    .await
}

pub async fn show_usage_guide(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        state.config.bot_messages.usage_guide_or_default().to_string(),
        crate::bot::keyboards::guide_keyboard(),
    )
    .await
}
