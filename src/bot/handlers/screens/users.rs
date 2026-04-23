use teloxide::prelude::*;
use teloxide::types::{InputFile, MessageId};

use crate::bot::handlers::format::{render_user_card_text, user_display_name};
use crate::bot::handlers::shared::{HandlerResult, build_user_qr_png_bytes, callback_message_target};
use crate::bot::handlers::state::BotState;
use super::upsert_screen;

pub async fn show_delete_user_confirm(
    bot: &Bot,
    chat_id: ChatId,
    tg_user_id: i64,
) -> HandlerResult {
    bot.send_message(
        chat_id,
        format!(
            "Удалить пользователя с Telegram ID {}?\n\nДействие деактивирует пользователя в БД и удалит его из telemt-конфига.",
            tg_user_id
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_delete_keyboard(tg_user_id))
    .await?;
    Ok(())
}

pub async fn show_user_ban_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    tg_user_id: i64,
    page: i64,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Удалить пользователя {}?\n\nЭто действие уберёт запись из telemt и деактивирует доступ в БД.",
            tg_user_id
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_user_ban_keyboard(
        tg_user_id, page,
    ))
    .await?;
    Ok(())
}

pub async fn admin_show_users_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_users = state.db.count_active_users().await?;
    let users_page_size = state.config.users_page_size.max(1);
    super::render_paged_selector_screen(
        bot,
        super::PagedSelectorConfig {
            chat_id,
            message_id,
            total_items: total_users,
            page_size: users_page_size,
            requested_page,
            empty_text:
                "👥 Пользователи\n\nАктивных пользователей нет.\n\nМожно создать нового пользователя."
                    .to_string(),
            empty_keyboard: crate::bot::keyboards::users_page_keyboard(&[], 1, 1),
        },
        |limit, offset| state.db.list_active_users_page(limit, offset),
        |user| {
            let display_name = user_display_name(user);
            let short = if display_name.chars().count() > 40 {
                format!("{}...", display_name.chars().take(37).collect::<String>())
            } else {
                display_name
            };
            (user.tg_user_id, format!("{} (id {})", short, user.tg_user_id))
        },
        |total, page, total_pages| {
            format!(
                "👥 Пользователи · {}\nСтраница: {}/{}\n\nВыберите пользователя.",
                total, page, total_pages
            )
        },
        crate::bot::keyboards::users_page_keyboard,
    )
    .await
}

pub async fn send_user_qr_to_admin(
    bot: &Bot,
    q: &teloxide::types::CallbackQuery,
    user: &crate::db::RegistrationRequest,
    state: &BotState,
) -> Result<(), anyhow::Error> {
    let Some(telemt_username) = user.telemt_username.as_deref() else {
        return Err(anyhow::anyhow!("Не найден telemt username пользователя"));
    };

    let secret_opt = user
        .secret
        .as_deref()
        .filter(|s| !s.is_empty());
    let link = state
        .telemt_backend
        .build_user_link(telemt_username, secret_opt)
        .await?;
    let qr_png = build_user_qr_png_bytes(&link)?;
    let caption = format!(
        "👤 {} ({})\n\n🔗 {}",
        user_display_name(user),
        user.tg_user_id,
        link
    );

    if let Some((chat_id, _)) = callback_message_target(q) {
        bot.send_photo(
            chat_id,
            InputFile::memory(qr_png).file_name(format!("telemt-proxy-{}.png", user.tg_user_id)),
        )
        .caption(caption)
        .await?;
    }
    Ok(())
}

pub async fn show_user_card_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    user: &crate::db::RegistrationRequest,
    runtime_info: Option<crate::telemt_backend::TelemtUserInfo>,
    page: i64,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_user_card_text(user, runtime_info.as_ref()),
        crate::bot::keyboards::user_card_keyboard(user.tg_user_id, page),
    )
    .await
}
