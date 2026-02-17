//! Обработчики команд пользователя и админа.

use crate::config::Config;
use crate::db::{
    ConsumedInviteToken, Db, InviteToken, RegisterResult, RegistrationRequest, TokenConsumeError,
    TokenMode,
};
use crate::link::{build_proxy_link, generate_user_secret};
use crate::service::ServiceController;
use crate::telemt_cfg::TelemtConfig;
use chrono::{DateTime, Local, Utc};
use std::collections::HashSet;
use std::sync::Arc;
use teloxide::dispatching::DpHandlerDescription;
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::utils::command::BotCommands;
use tokio::sync::Mutex;

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
pub struct BotState {
    pub config: Arc<Config>,
    pub db: Arc<Db>,
    pub telemt_cfg: Arc<TelemtConfig>,
    pub service: ServiceController,
    pub bot_username: Option<String>,
    pub awaiting_invite_users: Arc<Mutex<HashSet<i64>>>,
}

fn telemt_username(tg_user_id: i64) -> String {
    format!("tg_{}", tg_user_id)
}

fn sender_user_id(msg: &Message) -> Option<i64> {
    msg.from.as_ref().map(|user| user.id.0 as i64)
}

fn sender_display_name(msg: &Message) -> Option<String> {
    msg.from.as_ref().map(|user| {
        let mut full_name = user.first_name.clone();
        if let Some(last_name) = user.last_name.as_deref()
            && !last_name.trim().is_empty()
        {
            full_name.push(' ');
            full_name.push_str(last_name);
        }
        full_name
    })
}

enum CreateTarget {
    UserId(i64),
    Username(String),
}

fn parse_create_target(arg: &str) -> Option<CreateTarget> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(user_id) = trimmed.parse::<i64>() {
        return Some(CreateTarget::UserId(user_id));
    }

    let username = trimmed.strip_prefix('@')?.trim();
    if username.is_empty() {
        return None;
    }

    Some(CreateTarget::Username(username.to_string()))
}

fn parse_start_token(text: &str) -> Option<String> {
    let mut parts = text.split_whitespace();
    let command = parts.next()?;
    if !command.starts_with("/start") {
        return None;
    }
    let token = parts.next()?.trim();
    if token.is_empty() {
        return None;
    }

    let decoded = match urlencoding::decode(token) {
        Ok(value) => value.into_owned(),
        Err(_) => token.to_string(),
    };
    let normalized = decoded.trim().trim_matches('`').trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn format_date(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&Local).format("%d.%m.%Y").to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn format_mode(auto_approve: bool) -> &'static str {
    if auto_approve {
        "АВТОПОДТВЕРЖДЕНИЕ 🚀"
    } else {
        "Ручной ✅"
    }
}

fn build_bot_start_link(bot_username: &str, token: &str) -> String {
    let normalized = bot_username.trim_start_matches('@');
    format!("https://t.me/{}?start={}", normalized, token)
}

async fn mark_user_waiting_for_invite(state: &BotState, tg_user_id: i64) {
    state.awaiting_invite_users.lock().await.insert(tg_user_id);
}

async fn unmark_user_waiting_for_invite(state: &BotState, tg_user_id: i64) {
    state.awaiting_invite_users.lock().await.remove(&tg_user_id);
}

async fn is_user_waiting_for_invite(state: &BotState, tg_user_id: i64) -> bool {
    state
        .awaiting_invite_users
        .lock()
        .await
        .contains(&tg_user_id)
}

async fn notify_auto_approve(
    bot: &Bot,
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    token: &ConsumedInviteToken,
) {
    let mode_label = match token.mode {
        TokenMode::AutoApprove => "auto",
        TokenMode::Manual => "manual",
    };
    let text = format!(
        "✅ Автоподключение по токену\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Token: {}\n\
         Token ID: {}\n\
         Mode: {}\n\
         Expires: {}\n\
         Usage: {}/{}\n\
         Created by: {}",
        tg_user_id,
        tg_username.unwrap_or("—"),
        tg_display_name.unwrap_or("—"),
        token.token,
        token.id,
        mode_label,
        format_timestamp(token.expires_at),
        token.usage_count,
        token
            .max_usage
            .map(|value| value.to_string())
            .unwrap_or_else(|| "∞".to_string()),
        token
            .created_by
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_string())
    );

    for admin_id in &state.config.admin_ids {
        if let Err(error) = bot.send_message(ChatId(*admin_id), text.clone()).await {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось отправить аудит автоподключения"
            );
        }
    }
}

fn is_admin_message(msg: &Message, state: &BotState) -> bool {
    sender_user_id(msg).is_some_and(|user_id| state.config.is_admin(user_id))
}

fn parse_callback_request_id(data: &str, prefix: &str) -> Result<i64, anyhow::Error> {
    data.strip_prefix(prefix)
        .ok_or_else(|| anyhow::anyhow!("Некорректный callback payload"))?
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("Некорректный request_id"))
}

fn callback_message_target(q: &CallbackQuery) -> Option<(ChatId, teloxide::types::MessageId)> {
    q.message.as_ref().map(|msg| (msg.chat().id, msg.id()))
}

async fn approve_request_and_build_link(
    state: &BotState,
    request_id: i64,
) -> Result<Option<(RegistrationRequest, String)>, anyhow::Error> {
    let request = match state.db.get_pending_by_id(request_id).await? {
        Some(request) => request,
        None => return Ok(None),
    };

    let telemt_user = telemt_username(request.tg_user_id);
    let user_secret = generate_user_secret();

    state.telemt_cfg.upsert_user(&telemt_user, &user_secret)?;
    if state
        .db
        .approve(request_id, &telemt_user, &user_secret)
        .await?
        .is_none()
    {
        return Ok(None);
    }

    // telemt не поддерживает hot reload — перезапуск обязателен после изменения конфига
    let restart_result = state.service.restart();
    if !restart_result.success {
        tracing::warn!(
            stderr = %restart_result.stderr,
            "Не удалось перезапустить telemt после одобрения заявки"
        );
    }

    let link_params = state.telemt_cfg.read_link_params()?;
    let proxy_link = build_proxy_link(&link_params, &user_secret)?;
    Ok(Some((request, proxy_link)))
}

async fn approve_user_direct_and_build_link(
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
) -> Result<String, anyhow::Error> {
    let telemt_user = telemt_username(tg_user_id);
    let secret = generate_user_secret();
    state.telemt_cfg.upsert_user(&telemt_user, &secret)?;
    state
        .db
        .set_approved(
            tg_user_id,
            tg_username,
            tg_display_name,
            &telemt_user,
            &secret,
        )
        .await?;

    // telemt не поддерживает hot reload — перезапуск обязателен после изменения конфига
    let restart_result = state.service.restart();
    if !restart_result.success {
        tracing::warn!(
            stderr = %restart_result.stderr,
            tg_user_id = tg_user_id,
            "Не удалось перезапустить telemt после выдачи доступа"
        );
    }

    let params = state.telemt_cfg.read_link_params()?;
    build_proxy_link(&params, &secret).map_err(anyhow::Error::from)
}

async fn process_invite_token(
    bot: &Bot,
    msg: &Message,
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    token: &str,
) -> HandlerResult {
    let consumed = match state.db.consume_invite_token(token).await {
        Ok(token_payload) => token_payload,
        Err(TokenConsumeError::NotFound) => {
            bot.send_message(
                msg.chat.id,
                "Токен не найден. Проверьте код и попробуйте снова.",
            )
            .await?;
            return Ok(());
        }
        Err(TokenConsumeError::Revoked) => {
            bot.send_message(msg.chat.id, "Этот токен отозван администратором.")
                .await?;
            return Ok(());
        }
        Err(TokenConsumeError::Expired) => {
            bot.send_message(msg.chat.id, "Срок действия токена истёк.")
                .await?;
            return Ok(());
        }
        Err(TokenConsumeError::UsageLimitReached) => {
            bot.send_message(msg.chat.id, "Лимит использований токена исчерпан.")
                .await?;
            return Ok(());
        }
    };

    tracing::info!(
        tg_user_id = tg_user_id,
        token = %consumed.token,
        token_id = consumed.id,
        mode = ?consumed.mode,
        usage_count = consumed.usage_count,
        max_usage = ?consumed.max_usage,
        expires_at = consumed.expires_at,
        "Токен успешно применён"
    );

    match consumed.mode {
        TokenMode::Manual => {
            let result = state
                .db
                .register_or_get(tg_user_id, tg_username, tg_display_name)
                .await?;
            match result {
                RegisterResult::Approved(secret) => {
                    let params = state.telemt_cfg.read_link_params()?;
                    let link = build_proxy_link(&params, &secret)?;
                    bot.send_message(msg.chat.id, format!("Ваша ссылка на прокси:\n\n{}", link))
                        .reply_markup(crate::bot::keyboards::user_menu())
                        .await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
                RegisterResult::Rejected => {
                    bot.send_message(
                        msg.chat.id,
                        "Ваша заявка на регистрацию отклонена администратором.",
                    )
                    .reply_markup(crate::bot::keyboards::user_menu())
                    .await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
                RegisterResult::AlreadyPending => {
                    bot.send_message(
                        msg.chat.id,
                        "Ваша заявка уже на рассмотрении. Ожидайте подтверждения администратора.",
                    )
                    .reply_markup(crate::bot::keyboards::user_menu())
                    .await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
                RegisterResult::NewPending(ref req) => {
                    bot.send_message(msg.chat.id, "Заявка отправлена. Ожидайте подтверждения.")
                        .reply_markup(crate::bot::keyboards::user_menu())
                        .await?;
                    notify_admins(bot, state, req).await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
            }
        }
        TokenMode::AutoApprove => {
            let link =
                approve_user_direct_and_build_link(state, tg_user_id, tg_username, tg_display_name)
                    .await?;
            bot.send_message(
                msg.chat.id,
                format!("Доступ одобрен! Ваша ссылка для подключения:\n\n{}", link),
            )
            .reply_markup(crate::bot::keyboards::user_menu())
            .await?;
            notify_auto_approve(
                bot,
                state,
                tg_user_id,
                tg_username,
                tg_display_name,
                &consumed,
            )
            .await;
            unmark_user_waiting_for_invite(state, tg_user_id).await;
        }
    }

    Ok(())
}

async fn start_cmd(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let user_id = sender_user_id(&msg).unwrap_or_default();
    let username = msg.from.as_ref().and_then(|u| u.username.clone());
    let display_name = sender_display_name(&msg);
    tracing::info!(
        user_id = user_id,
        username = ?username,
        display_name = ?display_name,
        "Received /start command"
    );

    if state.config.is_admin(user_id) {
        bot.send_message(
            msg.chat.id,
            "Добро пожаловать в панель администратора. Используйте кнопки ниже.",
        )
        .reply_markup(crate::bot::keyboards::admin_menu())
        .await?;
        return Ok(());
    }

    if let Some(existing) = state.db.get_request_by_tg_user(user_id).await? {
        match existing.status.as_str() {
            "approved" => {
                if let Some(secret) = existing.secret {
                    let params = state.telemt_cfg.read_link_params()?;
                    let link = build_proxy_link(&params, &secret)?;
                    bot.send_message(msg.chat.id, format!("Ваша ссылка на прокси:\n\n{}", link))
                        .reply_markup(crate::bot::keyboards::user_menu())
                        .await?;
                    unmark_user_waiting_for_invite(&state, user_id).await;
                    return Ok(());
                }
            }
            "pending" => {
                bot.send_message(
                    msg.chat.id,
                    "Ваша заявка уже на рассмотрении. Ожидайте подтверждения администратора.",
                )
                .reply_markup(crate::bot::keyboards::user_menu())
                .await?;
                unmark_user_waiting_for_invite(&state, user_id).await;
                return Ok(());
            }
            "rejected" => {
                bot.send_message(
                    msg.chat.id,
                    "Ваша заявка на регистрацию отклонена администратором.",
                )
                .reply_markup(crate::bot::keyboards::user_menu())
                .await?;
                unmark_user_waiting_for_invite(&state, user_id).await;
                return Ok(());
            }
            _ => {}
        }
    }

    let text = msg.text().unwrap_or("");
    if let Some(token) = parse_start_token(text) {
        process_invite_token(
            &bot,
            &msg,
            &state,
            user_id,
            username.as_deref(),
            display_name.as_deref(),
            &token,
        )
        .await?;
        return Ok(());
    }

    mark_user_waiting_for_invite(&state, user_id).await;
    bot.send_message(
        msg.chat.id,
        "Введите пригласительный токен для подачи заявки на доступ.",
    )
    .reply_markup(crate::bot::keyboards::user_menu())
    .await?;
    Ok(())
}

async fn notify_admins(bot: &Bot, state: &BotState, req: &RegistrationRequest) -> HandlerResult {
    let text = format!(
        "📋 Новая заявка #{}:\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Время: {}",
        req.id,
        req.tg_user_id,
        req.tg_username.as_deref().unwrap_or("—"),
        req.tg_display_name.as_deref().unwrap_or("—"),
        format_timestamp(req.created_at),
    );

    let kb = crate::bot::keyboards::approve_reject_buttons(req.id);

    for admin_id in &state.config.admin_ids {
        if let Err(e) = bot
            .send_message(ChatId(*admin_id), text.clone())
            .reply_markup(kb.clone())
            .await
        {
            tracing::warn!(
                "Не удалось отправить уведомление админу {}: {}",
                admin_id,
                e
            );
        }
    }
    Ok(())
}

fn format_timestamp(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|| format!("Некорректный timestamp: {}", ts))
}

async fn callback_approve(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    let callback_id = q.id.clone();
    let admin_id = q.from.id.0 as i64;
    if !state.config.is_admin(admin_id) {
        bot.answer_callback_query(callback_id)
            .text("Недостаточно прав")
            .show_alert(true)
            .await?;
        return Ok(());
    }

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
            bot.answer_callback_query(callback_id)
                .text("Заявка уже обработана или не найдена")
                .await?;
            return Ok(());
        }
    };

    bot.answer_callback_query(q.id).text("Одобрено").await?;

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
    let callback_id = q.id.clone();
    let admin_id = q.from.id.0 as i64;
    if !state.config.is_admin(admin_id) {
        bot.answer_callback_query(callback_id)
            .text("Недостаточно прав")
            .show_alert(true)
            .await?;
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let request_id = parse_callback_request_id(data, "reject:")?;
    tracing::info!(
        admin_id = admin_id,
        request_id = request_id,
        "Reject callback received"
    );
    let message_target = callback_message_target(&q);
    let request = state.db.reject(request_id).await?;

    bot.answer_callback_query(q.id).text("Отклонено").await?;

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

async fn cmd_approve(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let request_id: i64 = match text.split_whitespace().nth(1).unwrap_or("").parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "Использование: /approve <request_id>")
                .await?;
            return Ok(());
        }
    };
    tracing::info!(request_id = request_id, "Admin command /approve");

    let (request, link) = match approve_request_and_build_link(&state, request_id).await? {
        Some(payload) => payload,
        None => {
            bot.send_message(msg.chat.id, "Заявка не найдена или уже обработана")
                .await?;
            return Ok(());
        }
    };

    bot.send_message(
        msg.chat.id,
        format!("Одобрено. Ссылка отправлена пользователю.\n{}", link),
    )
    .await?;
    bot.send_message(
        ChatId(request.tg_user_id),
        format!("Ваша ссылка на прокси:\n\n{}", link),
    )
    .await?;
    Ok(())
}

async fn cmd_reject(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let request_id: i64 = match text.split_whitespace().nth(1).unwrap_or("").parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "Использование: /reject <request_id>")
                .await?;
            return Ok(());
        }
    };
    tracing::info!(request_id = request_id, "Admin command /reject");

    let req = state.db.reject(request_id).await?;
    if let Some(r) = req {
        bot.send_message(msg.chat.id, "Заявка отклонена").await?;
        bot.send_message(
            ChatId(r.tg_user_id),
            "Ваша заявка на регистрацию отклонена администратором.",
        )
        .await?;
    } else {
        bot.send_message(msg.chat.id, "Заявка не найдена или уже обработана")
            .await?;
    }
    Ok(())
}

async fn cmd_create(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let arg = text.split_whitespace().nth(1).unwrap_or("");
    let tg_user_id: i64 = match parse_create_target(arg) {
        Some(CreateTarget::UserId(id)) => id,
        Some(CreateTarget::Username(username)) => {
            match state.db.find_tg_user_id_by_username(&username).await? {
                Some(user_id) => user_id,
                None => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "Пользователь @{} не найден в базе.\n\
                             Он должен хотя бы раз отправить боту /start.",
                            username
                        ),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
        None => {
            bot.send_message(
                msg.chat.id,
                "Использование: /create <telegram_user_id | @username>",
            )
            .await?;
            return Ok(());
        }
    };
    tracing::info!(tg_user_id = tg_user_id, "Admin command /create");

    let telemt_user = telemt_username(tg_user_id);
    let link = approve_user_direct_and_build_link(&state, tg_user_id, None, None).await?;

    bot.send_message(
        msg.chat.id,
        format!("Пользователь {} создан.\nСсылка:\n{}", telemt_user, link),
    )
    .await?;
    Ok(())
}

async fn cmd_delete(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let tg_user_id: i64 = match text.split_whitespace().nth(1).unwrap_or("").parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "Использование: /delete <telegram_user_id>")
                .await?;
            return Ok(());
        }
    };
    tracing::info!(tg_user_id = tg_user_id, "Admin command /delete");

    let telemt_user = telemt_username(tg_user_id);
    let removed = state.telemt_cfg.remove_user(&telemt_user)?;
    let _ = state.db.deactivate_user(tg_user_id).await;

    if removed {
        // telemt не поддерживает hot reload — перезапуск обязателен после изменения конфига
        let restart_result = state.service.restart();
        if !restart_result.success {
            tracing::warn!(
                stderr = %restart_result.stderr,
                "Не удалось перезапустить telemt после удаления пользователя"
            );
        }
        bot.send_message(msg.chat.id, format!("Пользователь {} удалён", telemt_user))
            .await?;
    } else {
        bot.send_message(
            msg.chat.id,
            format!("Пользователь {} не найден в конфиге", telemt_user),
        )
        .await?;
    }
    Ok(())
}

async fn cmd_service(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let args: Vec<&str> = text.split_whitespace().collect();
    let action = args.get(1).copied().unwrap_or("status");
    tracing::info!(action = action, "Admin command /service");

    let (action_name, result) = match action {
        "start" => ("start", state.service.start()),
        "stop" => ("stop", state.service.stop()),
        "restart" => ("restart", state.service.restart()),
        "reload" => ("reload", state.service.reload()),
        "status" => ("status", state.service.status()),
        _ => {
            bot.send_message(
                msg.chat.id,
                "Использование: /service <start|stop|restart|reload|status>",
            )
            .await?;
            return Ok(());
        }
    };

    let reply = state.service.format_result(action_name, &result);
    bot.send_message(msg.chat.id, reply).await?;
    Ok(())
}

async fn cmd_token(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let args: Vec<&str> = text.split_whitespace().collect();
    let Some(subcommand) = args.get(1).copied() else {
        bot.send_message(
            msg.chat.id,
            "Использование:\n/token create [days] [--auto|-a] [--max-uses N]\n/token list\n/token revoke <token>",
        )
        .await?;
        return Ok(());
    };

    match subcommand {
        "create" => {
            let mut days: Option<i64> = None;
            let mut auto_approve = false;
            let mut max_uses: Option<i64> = None;
            let mut index = 2;

            while index < args.len() {
                match args[index] {
                    "--auto" | "-a" => {
                        auto_approve = true;
                        index += 1;
                    }
                    "--max-uses" => {
                        let Some(value) = args.get(index + 1) else {
                            bot.send_message(
                                msg.chat.id,
                                "Использование: /token create [days] [--auto|-a] [--max-uses N]",
                            )
                            .await?;
                            return Ok(());
                        };
                        let parsed = match value.parse::<i64>() {
                            Ok(parsed) if parsed >= 1 => parsed,
                            _ => {
                                bot.send_message(
                                    msg.chat.id,
                                    "Параметр --max-uses должен быть целым числом >= 1.",
                                )
                                .await?;
                                return Ok(());
                            }
                        };
                        max_uses = Some(parsed);
                        index += 2;
                    }
                    value => {
                        if let Ok(parsed_days) = value.parse::<i64>() {
                            if days.is_some() {
                                bot.send_message(
                                    msg.chat.id,
                                    "Использование: /token create [days] [--auto|-a] [--max-uses N]",
                                )
                                .await?;
                                return Ok(());
                            }
                            days = Some(parsed_days);
                            index += 1;
                            continue;
                        }
                        bot.send_message(
                            msg.chat.id,
                            "Использование: /token create [days] [--auto|-a] [--max-uses N]",
                        )
                        .await?;
                        return Ok(());
                    }
                }
            }

            let security = &state.config.security;
            let days = days.unwrap_or(security.default_token_days);
            if days < 1 {
                bot.send_message(msg.chat.id, "Срок действия должен быть не меньше 1 дня.")
                    .await?;
                return Ok(());
            }
            if days > security.max_token_days {
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Нельзя создать токен на срок больше {} дней.",
                        security.max_token_days
                    ),
                )
                .await?;
                return Ok(());
            }
            if auto_approve && !security.allow_auto_approve_tokens {
                bot.send_message(
                    msg.chat.id,
                    "Автоподтверждение токенов запрещено в конфигурации.",
                )
                .await?;
                return Ok(());
            }

            let created_by = sender_user_id(&msg);
            let token = state
                .db
                .create_invite_token(days, auto_approve, max_uses, created_by)
                .await?;

            let link_line = state
                .bot_username
                .as_deref()
                .map(|bot_username| {
                    let invite_link = build_bot_start_link(bot_username, &token.token);
                    format!("Ссылка: {}\n", invite_link)
                })
                .unwrap_or_else(|| {
                    "Ссылка: недоступна (у бота не задан username в Telegram).\n".to_string()
                });

            let response = format!(
                "✅ Токен создан:\n\
                 Код: <code>{}</code>\n\
                 {}\
                 Режим: {}\n\
                 Действует до: {}\n\
                 Лимит использований: {}\n\
                 Используйте команду <code>/token revoke {}</code> для отзыва.",
                token.token,
                link_line,
                format_mode(token.auto_approve),
                format_date(token.expires_at),
                token
                    .max_usage
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "без лимита".to_string()),
                token.token
            );
            bot.send_message(msg.chat.id, response)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        "list" => {
            let tokens = state.db.list_active_invite_tokens(50).await?;
            if tokens.is_empty() {
                bot.send_message(msg.chat.id, "Активных invite-токенов нет.")
                    .await?;
                return Ok(());
            }

            let mut lines: Vec<String> = Vec::with_capacity(tokens.len());
            for token in tokens {
                lines.push(render_invite_token_line(&token));
            }
            let text = format!("Активные токены:\n\n{}", lines.join("\n"));
            bot.send_message(msg.chat.id, text).await?;
        }
        "revoke" => {
            let Some(token_value) = args.get(2).copied() else {
                bot.send_message(msg.chat.id, "Использование: /token revoke <token>")
                    .await?;
                return Ok(());
            };
            let revoked = state.db.revoke_invite_token(token_value).await?;
            if revoked {
                bot.send_message(msg.chat.id, format!("Токен {} отозван.", token_value))
                    .await?;
            } else {
                bot.send_message(msg.chat.id, "Токен не найден или уже отозван.")
                    .await?;
            }
        }
        _ => {
            bot.send_message(
                msg.chat.id,
                "Использование:\n/token create [days] [--auto|-a] [--max-uses N]\n/token list\n/token revoke <token>",
            )
            .await?;
        }
    }

    Ok(())
}

fn render_invite_token_line(token: &InviteToken) -> String {
    let mode = if token.auto_approve { "AUTO" } else { "MANUAL" };
    let usage = token
        .max_usage
        .map(|max| format!("{}/{}", token.usage_count, max))
        .unwrap_or_else(|| format!("{}/∞", token.usage_count));
    let created_by = token
        .created_by
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_string());
    format!(
        "• {} | {} | до {} | usage {} | creator {} | создан {}",
        token.token,
        mode,
        format_date(token.expires_at),
        usage,
        created_by,
        format_date(token.created_at)
    )
}

async fn cmd_link(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let user_id = sender_user_id(&msg).unwrap_or_default();
    tracing::info!(user_id = user_id, "Received /link command");

    send_user_link(&bot, msg.chat.id, user_id, &state).await
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum BotCommand {
    #[command(description = "Зарегистрироваться")]
    Start,
    #[command(description = "Получить ссылку на прокси")]
    Link,
    #[command(description = "Справка")]
    Help,
    #[command(description = "Одобрить заявку (админ)")]
    Approve,
    #[command(description = "Отклонить заявку (админ)")]
    Reject,
    #[command(description = "Создать пользователя (админ)")]
    Create,
    #[command(description = "Удалить пользователя (админ)")]
    Delete,
    #[command(description = "Управление сервисом (админ)")]
    Service,
    #[command(description = "Управление invite-токенами (админ)")]
    Token,
}

async fn cmd_help(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let user_id = sender_user_id(&msg).unwrap_or_default();
    let is_admin = state.config.is_admin(user_id);
    let text = r#"Команды:
/start — зарегистрироваться (заявка на подтверждение админу)
/link — получить ссылку на прокси (если уже одобрены)

Для администраторов:
/approve <id> — одобрить заявку
/reject <id> — отклонить заявку
/create <tg_user_id | @username> — создать пользователя
/delete <tg_user_id> — удалить пользователя
/service <start|stop|restart|reload|status> — управление telemt.service
/token create [days] [--auto|-a] [--max-uses N] — создать invite-токен
/token list — список активных invite-токенов
/token revoke <token> — отозвать invite-токен"#;
    let reply_markup = if is_admin {
        crate::bot::keyboards::admin_menu()
    } else {
        crate::bot::keyboards::user_menu()
    };
    bot.send_message(msg.chat.id, text)
        .reply_markup(reply_markup)
        .await?;
    Ok(())
}

async fn send_user_link(
    bot: &Bot,
    chat_id: ChatId,
    tg_user_id: i64,
    state: &BotState,
) -> HandlerResult {
    let maybe = state.db.get_approved(tg_user_id).await?;
    match maybe {
        Some((_, secret)) => {
            let params = state.telemt_cfg.read_link_params()?;
            let link = build_proxy_link(&params, &secret)?;
            bot.send_message(chat_id, format!("Ваша ссылка на прокси:\n\n{}", link))
                .reply_markup(crate::bot::keyboards::user_menu())
                .await?;
        }
        None => {
            bot.send_message(
                chat_id,
                "У вас нет доступа к прокси. Отправьте /start для регистрации.",
            )
            .reply_markup(crate::bot::keyboards::user_menu())
            .await?;
        }
    }
    Ok(())
}

fn usage_guide_text() -> &'static str {
    r#"Как подключиться к прокси:

1) Нажмите «🔗 Моя ссылка» — бот отправит вам ссылку.
2) Нажмите на ссылку — Telegram автоматически предложит добавить прокси.
3) Подтвердите добавление.

Если не получается, обратитесь к администратору."#
}

async fn admin_show_pending(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let pending = state.db.list_pending_requests(10).await?;
    if pending.is_empty() {
        bot.send_message(chat_id, "Новых заявок нет.")
            .reply_markup(crate::bot::keyboards::admin_menu())
            .await?;
        return Ok(());
    }

    bot.send_message(chat_id, format!("Найдено новых заявок: {}", pending.len()))
        .reply_markup(crate::bot::keyboards::admin_menu())
        .await?;

    for req in pending {
        let text = format!(
            "📋 Заявка #{}:\n\
             User ID: {}\n\
             Username: @{}\n\
             Имя: {}\n\
             Время: {}",
            req.id,
            req.tg_user_id,
            req.tg_username.as_deref().unwrap_or("—"),
            req.tg_display_name.as_deref().unwrap_or("—"),
            format_timestamp(req.created_at),
        );
        bot.send_message(chat_id, text)
            .reply_markup(crate::bot::keyboards::approve_reject_buttons(req.id))
            .await?;
    }
    Ok(())
}

async fn admin_show_users(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let users = state.db.list_active_users(20).await?;
    if users.is_empty() {
        bot.send_message(chat_id, "Активных пользователей нет.")
            .reply_markup(crate::bot::keyboards::admin_menu())
            .await?;
        return Ok(());
    }

    bot.send_message(
        chat_id,
        format!(
            "Активные пользователи: {} (показаны последние {})",
            users.len(),
            users.len()
        ),
    )
    .reply_markup(crate::bot::keyboards::admin_menu())
    .await?;

    for user in users {
        let display_name = user
            .tg_display_name
            .clone()
            .or_else(|| {
                user.tg_username
                    .as_ref()
                    .map(|username| format!("@{}", username))
            })
            .or_else(|| user.telemt_username.clone())
            .unwrap_or_else(|| format!("tg_{}", user.tg_user_id));

        let text = format!(
            "👤 {} (tg id: {})\nUsername: @{}\nИмя: {}\nСоздано: {}",
            display_name,
            user.tg_user_id,
            user.tg_username.as_deref().unwrap_or("—"),
            user.tg_display_name.as_deref().unwrap_or("—"),
            format_timestamp(user.created_at),
        );
        bot.send_message(chat_id, text)
            .reply_markup(crate::bot::keyboards::delete_user_button(user.tg_user_id))
            .await?;
    }
    Ok(())
}

async fn admin_show_stats(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let stats = state.db.admin_stats().await?;
    let text = format!(
        "📊 Статистика:\n\
         Всего записей: {}\n\
         Ожидают: {}\n\
         Активные: {}\n\
         Отклонённые: {}\n\
         Удалённые: {}",
        stats.total, stats.pending, stats.approved, stats.rejected, stats.deleted
    );
    bot.send_message(chat_id, text)
        .reply_markup(crate::bot::keyboards::admin_menu())
        .await?;
    Ok(())
}

async fn admin_show_service_panel(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let result = state.service.status();
    let text = format!(
        "⚙️ Сервис telemt\n\n{}",
        state.service.format_result("status", &result)
    );
    bot.send_message(chat_id, text)
        .reply_markup(crate::bot::keyboards::service_control_buttons())
        .await?;
    Ok(())
}

async fn handle_menu_buttons(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let user_id = sender_user_id(&msg).unwrap_or_default();
    let is_admin = state.config.is_admin(user_id);

    if !is_admin && !text.starts_with('/') && is_user_waiting_for_invite(&state, user_id).await {
        let username = msg.from.as_ref().and_then(|u| u.username.clone());
        let display_name = sender_display_name(&msg);
        process_invite_token(
            &bot,
            &msg,
            &state,
            user_id,
            username.as_deref(),
            display_name.as_deref(),
            text.trim(),
        )
        .await?;
        return Ok(());
    }

    match text {
        crate::bot::keyboards::BTN_USER_LINK => {
            send_user_link(&bot, msg.chat.id, user_id, &state).await?;
        }
        crate::bot::keyboards::BTN_USER_GUIDE => {
            bot.send_message(msg.chat.id, usage_guide_text())
                .reply_markup(crate::bot::keyboards::user_menu())
                .await?;
        }
        crate::bot::keyboards::BTN_ADMIN_PENDING if is_admin => {
            admin_show_pending(&bot, msg.chat.id, &state).await?;
        }
        crate::bot::keyboards::BTN_ADMIN_USERS if is_admin => {
            admin_show_users(&bot, msg.chat.id, &state).await?;
        }
        crate::bot::keyboards::BTN_ADMIN_SERVICE if is_admin => {
            admin_show_service_panel(&bot, msg.chat.id, &state).await?;
        }
        crate::bot::keyboards::BTN_ADMIN_STATS if is_admin => {
            admin_show_stats(&bot, msg.chat.id, &state).await?;
        }
        crate::bot::keyboards::BTN_ADMIN_CREATE_HINT if is_admin => {
            bot.send_message(
                msg.chat.id,
                "Создание пользователя:\n\
                 /create <tg_user_id>\n\
                 /create @username\n\n\
                 Для варианта с @username пользователь должен ранее отправить боту /start.",
            )
            .reply_markup(crate::bot::keyboards::admin_menu())
            .await?;
        }
        crate::bot::keyboards::BTN_ADMIN_HELP if is_admin => {
            cmd_help(bot, msg, state).await?;
        }
        _ => {
            let text = if is_admin {
                "Не понял команду. Используйте кнопки админ-меню ниже."
            } else {
                "Не понял запрос. Используйте кнопки меню ниже."
            };
            let reply_markup = if is_admin {
                crate::bot::keyboards::admin_menu()
            } else {
                crate::bot::keyboards::user_menu()
            };
            bot.send_message(msg.chat.id, text)
                .reply_markup(reply_markup)
                .await?;
        }
    }
    Ok(())
}

async fn callback_delete_user(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    let callback_id = q.id.clone();
    let admin_id = q.from.id.0 as i64;
    if !state.config.is_admin(admin_id) {
        bot.answer_callback_query(callback_id)
            .text("Недостаточно прав")
            .show_alert(true)
            .await?;
        return Ok(());
    }

    let data = q.data.as_deref().unwrap_or("");
    let tg_user_id = parse_callback_request_id(data, "delete_user:")?;
    let telemt_user = telemt_username(tg_user_id);
    let removed_from_cfg = state.telemt_cfg.remove_user(&telemt_user)?;
    let removed_from_db = state.db.deactivate_user(tg_user_id).await?;

    if removed_from_cfg {
        // telemt не поддерживает hot reload — перезапуск обязателен после изменения конфига
        let restart_result = state.service.restart();
        if !restart_result.success {
            tracing::warn!(
                stderr = %restart_result.stderr,
                "Не удалось перезапустить telemt после удаления пользователя"
            );
        }
    }

    let status_text = if removed_from_cfg || removed_from_db {
        format!("Пользователь {} удалён", telemt_user)
    } else {
        format!("Пользователь {} не найден", telemt_user)
    };

    bot.answer_callback_query(callback_id)
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
    let callback_id = q.id.clone();
    let admin_id = q.from.id.0 as i64;
    if !state.config.is_admin(admin_id) {
        bot.answer_callback_query(callback_id)
            .text("Недостаточно прав")
            .show_alert(true)
            .await?;
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

    bot.answer_callback_query(callback_id)
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

pub fn schema() -> dptree::Handler<
    'static,
    Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>,
    DpHandlerDescription,
> {
    let command_handler = teloxide::filter_command::<BotCommand, _>()
        .branch(dptree::case![BotCommand::Start].endpoint(start_cmd))
        .branch(dptree::case![BotCommand::Link].endpoint(cmd_link))
        .branch(dptree::case![BotCommand::Help].endpoint(cmd_help))
        .branch(dptree::case![BotCommand::Approve].endpoint(cmd_approve))
        .branch(dptree::case![BotCommand::Reject].endpoint(cmd_reject))
        .branch(dptree::case![BotCommand::Create].endpoint(cmd_create))
        .branch(dptree::case![BotCommand::Delete].endpoint(cmd_delete))
        .branch(dptree::case![BotCommand::Service].endpoint(cmd_service))
        .branch(dptree::case![BotCommand::Token].endpoint(cmd_token));

    let callback_handler = Update::filter_callback_query()
        .branch(
            dptree::filter_map(|q: CallbackQuery| {
                if q.data
                    .as_deref()
                    .is_some_and(|payload| payload.starts_with("approve:"))
                {
                    Some(q)
                } else {
                    None
                }
            })
            .endpoint(callback_approve),
        )
        .branch(
            dptree::filter_map(|q: CallbackQuery| {
                if q.data
                    .as_deref()
                    .is_some_and(|payload| payload.starts_with("reject:"))
                {
                    Some(q)
                } else {
                    None
                }
            })
            .endpoint(callback_reject),
        )
        .branch(
            dptree::filter_map(|q: CallbackQuery| {
                if q.data
                    .as_deref()
                    .is_some_and(|payload| payload.starts_with("delete_user:"))
                {
                    Some(q)
                } else {
                    None
                }
            })
            .endpoint(callback_delete_user),
        )
        .branch(
            dptree::filter_map(|q: CallbackQuery| {
                if q.data
                    .as_deref()
                    .is_some_and(|payload| payload.starts_with("service:"))
                {
                    Some(q)
                } else {
                    None
                }
            })
            .endpoint(callback_service_action),
        );

    let message_handler = Update::filter_message()
        .branch(command_handler)
        .endpoint(handle_menu_buttons);

    dptree::entry()
        .branch(message_handler)
        .branch(callback_handler)
}
