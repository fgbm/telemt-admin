use teloxide::prelude::*;
use teloxide::types::MessageId;

use crate::bot::handlers::format::{format_timestamp, user_display_name};
use crate::bot::handlers::state::BotState;
use crate::bot::handlers::shared::HandlerResult;

pub async fn admin_show_pending_requests_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_pending = state.db.count_pending_requests().await?;
    let requests_page_size = state.config.users_page_size.max(1);
    super::render_paged_selector_screen(
        bot,
        super::PagedSelectorConfig {
            chat_id,
            message_id,
            total_items: total_pending,
            page_size: requests_page_size,
            requested_page,
            empty_text: "📥 Заявки\n\nНовых заявок нет.".to_string(),
            empty_keyboard: crate::bot::keyboards::admin_home_keyboard(),
        },
        |limit, offset| state.db.list_pending_requests_page(limit, offset),
        |req| {
            (
                req.id,
                format!("📋 #{} · {}", req.id, user_display_name(req)),
            )
        },
        |total, page, total_pages| {
            format!(
                "📥 Заявки · {}\nСтраница: {}/{}\n\nВыберите заявку.",
                total, page, total_pages
            )
        },
        crate::bot::keyboards::pending_requests_keyboard,
    )
    .await
}

pub async fn show_pending_request_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    request: &crate::db::RegistrationRequest,
    page: i64,
) -> HandlerResult {
    let invite_line = request
        .invite_token_id
        .map(|id| format!("\n🎟 ID ссылки (invite): {}", id))
        .unwrap_or_default();
    let text = format!(
        "📋 Заявка #{}\n\n\
         👤 {}\n\
         🆔 {}\n\
         📱 {}\n\
         📅 {}{}",
        request.id,
        user_display_name(request),
        request.tg_user_id,
        request
            .tg_username
            .as_deref()
            .map(|username| format!("@{}", username))
            .unwrap_or_else(|| "—".to_string()),
        format_timestamp(request.created_at),
        invite_line,
    );
    bot.edit_message_text(chat_id, message_id, text)
        .reply_markup(crate::bot::keyboards::pending_request_card_keyboard(
            request.id, page,
        ))
        .await?;
    Ok(())
}
