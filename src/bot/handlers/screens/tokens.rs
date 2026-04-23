use teloxide::prelude::*;
use teloxide::types::MessageId;

use crate::bot::handlers::format::{
    render_invite_token_button_title, render_invite_token_card_text,
};
use crate::bot::handlers::state::BotState;
use super::upsert_screen;
use crate::bot::handlers::shared::HandlerResult;
use crate::db::InviteToken;

pub async fn show_token_menu(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
) -> HandlerResult {
    let text = "Управление invite-токенами\n\n\
        Ссылка действует ограниченное время и ограничена числом активаций; это не срок подписки пользователя в telemt.\n\n\
        Выберите действие.";
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text.to_string(),
        crate::bot::keyboards::token_menu_keyboard(state.config.security.allow_auto_approve_tokens),
    )
    .await
}

pub async fn admin_show_token_list_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_tokens = state.db.count_active_invite_tokens().await?;
    let tokens_page_size = state.config.users_page_size.max(1);
    super::render_paged_selector_screen(
        bot,
        super::PagedSelectorConfig {
            chat_id,
            message_id,
            total_items: total_tokens,
            page_size: tokens_page_size,
            requested_page,
            empty_text: "🎟 Токены\n\nАктивных invite-токенов нет.\n\
                (Срок в параметрах токена — это срок ссылки, не пользователя в telemt.)"
                .to_string(),
            empty_keyboard: crate::bot::keyboards::token_menu_keyboard(
                state.config.security.allow_auto_approve_tokens,
            ),
        },
        |limit, offset| state.db.list_active_invite_tokens_page(limit, offset),
        |token| (token.id, render_invite_token_button_title(token)),
        |total, page, total_pages| {
            format!(
                "🎟 Токены · {}\nСтраница: {}/{}\n\n\
                 В списке — срок действия invite-ссылки, не подписки пользователя.\n\n\
                 Выберите токен.",
                total, page, total_pages
            )
        },
        crate::bot::keyboards::token_list_keyboard,
    )
    .await
}

pub async fn show_token_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    token: &InviteToken,
    page: i64,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_invite_token_card_text(token),
        crate::bot::keyboards::token_card_keyboard(token.id, page),
    )
    .await
}

pub async fn show_token_revoke_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    token: &InviteToken,
    page: i64,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Отозвать invite-токен {}?\n\nПосле этого его нельзя будет использовать для регистрации.",
            token.token
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_token_revoke_keyboard(
        token.id, page,
    ))
    .await?;
    Ok(())
}
