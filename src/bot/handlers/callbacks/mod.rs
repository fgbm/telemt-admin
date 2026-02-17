use super::format::render_user_card_text;
use super::shared::{
    admin_show_users_page, approve_request_and_build_link, callback_message_target,
    callback_prefix_filter, parse_callback_page, parse_callback_request_id, parse_callback_user_action,
    perform_hard_ban, require_admin_callback, send_user_qr_to_admin, HandlerResult,
};
use super::state::BotState;
use teloxide::dptree;
use teloxide::prelude::*;

pub fn handler() -> teloxide::dispatching::UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    Update::filter_callback_query()
        .branch(
            dptree::filter_map(callback_prefix_filter("users_page:")).endpoint(callback_users_page),
        )
        .branch(dptree::filter_map(callback_prefix_filter("user_open:")).endpoint(callback_user_open))
        .branch(dptree::filter_map(callback_prefix_filter("user_view:")).endpoint(callback_user_view))
        .branch(dptree::filter_map(callback_prefix_filter("user_ban:")).endpoint(callback_user_ban))
        .branch(dptree::filter_map(callback_prefix_filter("approve:")).endpoint(callback_approve))
        .branch(dptree::filter_map(callback_prefix_filter("reject:")).endpoint(callback_reject))
        .branch(
            dptree::filter_map(callback_prefix_filter("delete_user:")).endpoint(callback_delete_user),
        )
        .branch(
            dptree::filter_map(callback_prefix_filter("service:")).endpoint(callback_service_action),
        )
}

async fn callback_approve(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
        return Ok(());
    };

    let data = q.data.as_deref().unwrap_or("");
    let request_id = parse_callback_request_id(data, "approve:")?;
    tracing::info!(
        admin_id = admin_id,
        request_id = request_id,
        "Approve callback received"
    );
    let message_target = callback_message_target(&q);

    let (request, link) = match approve_request_and_build_link(&state, request_id).await? {
        Some(payload) => payload,
        None => {
            bot.answer_callback_query(q.id.clone())
                .text("Заявка уже обработана или не найдена")
                .await?;
            return Ok(());
        }
    };

    bot.answer_callback_query(q.id.clone()).text("Одобрено").await?;

    if let Some((chat_id, message_id)) = message_target {
        bot.edit_message_text(chat_id, message_id, "✅ Заявка одобрена")
            .reply_markup(teloxide::types::InlineKeyboardMarkup::default())
            .await?;
    }

    bot.send_message(
        ChatId(request.tg_user_id),
        format!("Ваша ссылка на прокси:\n\n{}", link),
    )
    .await?;

    tracing::info!("Admin {} approved request #{}", admin_id, request_id);
    Ok(())
}

async fn callback_reject(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
        return Ok(());
    };

    let data = q.data.as_deref().unwrap_or("");
    let request_id = parse_callback_request_id(data, "reject:")?;
    tracing::info!(
        admin_id = admin_id,
        request_id = request_id,
        "Reject callback received"
    );
    let message_target = callback_message_target(&q);
    let request = state.db.reject(request_id).await?;

    bot.answer_callback_query(q.id.clone()).text("Отклонено").await?;

    if let Some(request) = request {
        if let Some((chat_id, message_id)) = message_target {
            bot.edit_message_text(chat_id, message_id, "❌ Заявка отклонена")
                .reply_markup(teloxide::types::InlineKeyboardMarkup::default())
                .await?;
        }
        bot.send_message(
            ChatId(request.tg_user_id),
            "Ваша заявка на регистрацию отклонена администратором.",
        )
        .await?;
    }

    tracing::info!("Admin {} rejected request #{}", admin_id, request_id);
    Ok(())
}

async fn callback_users_page(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    if require_admin_callback(&bot, &q, &state).await?.is_none() {
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let page = parse_callback_page(data, "users_page:")?;
    bot.answer_callback_query(q.id.clone()).await?;

    if let Some((chat_id, message_id)) = callback_message_target(&q) {
        admin_show_users_page(&bot, chat_id, &state, page, Some(message_id)).await?;
    }
    Ok(())
}

async fn callback_user_open(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    if require_admin_callback(&bot, &q, &state).await?.is_none() {
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let (tg_user_id, page) = parse_callback_user_action(data, "user_open:")?;
    let user = state.db.get_active_user_by_tg_user(tg_user_id).await?;
    let Some(user) = user else {
        bot.answer_callback_query(q.id.clone())
            .text("Пользователь уже неактивен")
            .show_alert(true)
            .await?;
        return Ok(());
    };

    bot.answer_callback_query(q.id.clone())
        .text("Открыта карточка")
        .await?;
    if let Some((chat_id, message_id)) = callback_message_target(&q) {
        bot.edit_message_text(chat_id, message_id, render_user_card_text(&user, page))
            .reply_markup(crate::bot::keyboards::user_card_keyboard(user.tg_user_id, page))
            .await?;
    }
    Ok(())
}

async fn callback_user_view(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    if require_admin_callback(&bot, &q, &state).await?.is_none() {
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let (tg_user_id, _) = parse_callback_user_action(data, "user_view:")?;
    let user = state.db.get_active_user_by_tg_user(tg_user_id).await?;
    let Some(user) = user else {
        bot.answer_callback_query(q.id.clone())
            .text("Пользователь уже неактивен")
            .show_alert(true)
            .await?;
        return Ok(());
    };

    bot.answer_callback_query(q.id.clone())
        .text("Отправляю ссылку и QR")
        .await?;
    send_user_qr_to_admin(&bot, &q, &user, &state).await?;
    Ok(())
}

async fn callback_user_ban(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    if require_admin_callback(&bot, &q, &state).await?.is_none() {
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let (tg_user_id, page) = parse_callback_user_action(data, "user_ban:")?;
    let status_text = perform_hard_ban(&state, tg_user_id).await?;
    bot.answer_callback_query(q.id.clone())
        .text(status_text.clone())
        .await?;

    if let Some((chat_id, message_id)) = callback_message_target(&q) {
        bot.send_message(chat_id, status_text).await?;
        admin_show_users_page(&bot, chat_id, &state, page, Some(message_id)).await?;
    }
    Ok(())
}

async fn callback_delete_user(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    if require_admin_callback(&bot, &q, &state).await?.is_none() {
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let tg_user_id = parse_callback_request_id(data, "delete_user:")?;
    let status_text = perform_hard_ban(&state, tg_user_id).await?;

    bot.answer_callback_query(q.id.clone())
        .text(status_text.clone())
        .await?;

    if let Some((chat_id, message_id)) = callback_message_target(&q) {
        bot.edit_message_reply_markup(chat_id, message_id)
            .reply_markup(teloxide::types::InlineKeyboardMarkup::default())
            .await?;
        bot.send_message(chat_id, status_text)
            .reply_markup(crate::bot::keyboards::admin_menu())
            .await?;
    }
    Ok(())
}

async fn callback_service_action(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    if require_admin_callback(&bot, &q, &state).await?.is_none() {
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let action = data.strip_prefix("service:").unwrap_or("status");
    let (action_name, result) = match action {
        "restart" => ("restart", state.service.restart()),
        "reload" => ("reload", state.service.reload()),
        "status" => ("status", state.service.status()),
        _ => ("status", state.service.status()),
    };

    bot.answer_callback_query(q.id.clone())
        .text(format!("Выполнено: {}", action_name))
        .await?;

    if let Some((chat_id, message_id)) = callback_message_target(&q) {
        let text = format!(
            "⚙️ Сервис telemt\n\n{}",
            state.service.format_result(action_name, &result)
        );
        bot.edit_message_text(chat_id, message_id, text)
            .reply_markup(crate::bot::keyboards::service_control_buttons())
            .await?;
    }
    Ok(())
}
