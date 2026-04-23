use teloxide::prelude::*;
use teloxide::types::{KeyboardRemove, MessageId};

use crate::bot::handlers::state::BotState;
use super::upsert_screen;
use crate::bot::handlers::format::usage_guide_text;
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
) -> HandlerResult {
    let text = "Панель администратора\n\nВыберите раздел ниже.";
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
    let text = if let Some(existing) = state.db.get_request_by_tg_user(user_id).await? {
        match existing.status {
            RequestStatus::Approved => {
                "Доступ уже открыт.\n\nНажмите «Получить ссылку».".to_string()
            }
            RequestStatus::Pending => {
                "Заявка уже на рассмотрении.\n\nДождитесь решения администратора.".to_string()
            }
            RequestStatus::Rejected => {
                "Заявка отклонена.\n\nЕсли есть новый invite-токен, отправьте /start и введите его заново.".to_string()
            }
            RequestStatus::Deleted => {
                "Доступ был отозван.\n\nДля новой регистрации отправьте /start и введите invite-токен заново.".to_string()
            }
        }
    } else {
        "Чтобы получить доступ, отправьте /start и введите invite-токен.\n\nЕсли токен уже есть, нажмите кнопку ниже."
            .to_string()
    };

    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::user_home_keyboard(),
    )
    .await
}

pub async fn show_usage_guide(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        usage_guide_text().to_string(),
        crate::bot::keyboards::guide_keyboard(),
    )
    .await
}
