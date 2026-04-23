//! Рассылка сообщений всем пользователям со статусом approved.

use crate::bot::handlers::shared::HandlerResult;
use crate::bot::handlers::state::{BotState, clear_wizard_state};
use std::time::Duration;
use teloxide::prelude::{Bot, ChatId, Message, Requester};
use teloxide::payloads::SendMessageSetters;
use teloxide::types::ParseMode;
use teloxide::utils::render::RenderMessageTextHelper;

pub async fn broadcast_to_approved_users(
    bot: &Bot,
    msg: &Message,
    state: &BotState,
    admin_tg_user_id: i64,
    text: &str,
) -> HandlerResult {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        clear_wizard_state(state, admin_tg_user_id).await?;
        bot.send_message(
            msg.chat.id,
            state.config.bot_messages.broadcast_cancelled_or_default(),
        )
            .await?;
        return Ok(());
    }

    let ids = state.db.list_approved_tg_user_ids().await?;
    let mut ok: u64 = 0;
    let mut failed: u64 = 0;

    for tg_user_id in ids {
        let send_result = if let Some(html) = msg.html_text() {
            let trimmed_html = html.trim();
            bot
                .send_message(ChatId(tg_user_id), trimmed_html)
                .parse_mode(ParseMode::Html)
                .await
        } else {
            bot
                .send_message(ChatId(tg_user_id), trimmed)
                .await
        };

        match send_result {
            Ok(_) => ok += 1,
            Err(error) => {
                failed += 1;
                tracing::warn!(
                    target = tg_user_id,
                    error = %error,
                    "Не удалось доставить сообщение рассылки"
                );
            }
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }

    clear_wizard_state(state, admin_tg_user_id).await?;

    bot.send_message(
        msg.chat.id,
        state
            .config
            .bot_messages
            .broadcast_summary_text(ok, failed, ok + failed),
    )
    .await?;
    Ok(())
}
