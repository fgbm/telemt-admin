use super::format::{format_date, format_mode, render_invite_token_line};
use super::shared::{
    admin_show_pending, admin_show_service_panel, admin_show_stats, admin_show_users_page,
    approve_request_and_build_link, approve_user_direct_and_build_link, build_bot_start_link,
    is_user_waiting_for_invite, mark_user_waiting_for_invite, parse_create_target, parse_start_token,
    perform_hard_ban, process_invite_token, send_user_link, unmark_user_waiting_for_invite,
    user_id_or_reply, CreateTarget, HandlerResult,
};
use super::state::{is_admin_message, sender_display_name, sender_user_id, telemt_username, BotState};
use crate::db::RequestStatus;
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum BotCommand {
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

pub fn handler() -> teloxide::dispatching::UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    teloxide::filter_command::<BotCommand, _>()
        .branch(dptree::case![BotCommand::Start].endpoint(start_cmd))
        .branch(dptree::case![BotCommand::Link].endpoint(cmd_link))
        .branch(dptree::case![BotCommand::Help].endpoint(cmd_help))
        .branch(dptree::case![BotCommand::Approve].endpoint(cmd_approve))
        .branch(dptree::case![BotCommand::Reject].endpoint(cmd_reject))
        .branch(dptree::case![BotCommand::Create].endpoint(cmd_create))
        .branch(dptree::case![BotCommand::Delete].endpoint(cmd_delete))
        .branch(dptree::case![BotCommand::Service].endpoint(cmd_service))
        .branch(dptree::case![BotCommand::Token].endpoint(cmd_token))
}

pub async fn cmd_help(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };
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

async fn start_cmd(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let user_id = match user_id_or_reply(&msg) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(error = %error, "Received /start without sender");
            return Ok(());
        }
    };
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
        match existing.status {
            RequestStatus::Approved => {
                if let Some(secret) = existing.secret {
                    let params = state.telemt_cfg.read_link_params()?;
                    let link = crate::link::build_proxy_link(&params, &secret)?;
                    bot.send_message(msg.chat.id, format!("Ваша ссылка на прокси:\n\n{}", link))
                        .reply_markup(crate::bot::keyboards::user_menu())
                        .await?;
                    unmark_user_waiting_for_invite(&state, user_id).await;
                    return Ok(());
                }
            }
            RequestStatus::Pending => {
                bot.send_message(
                    msg.chat.id,
                    "Ваша заявка уже на рассмотрении. Ожидайте подтверждения администратора.",
                )
                .reply_markup(crate::bot::keyboards::user_menu())
                .await?;
                unmark_user_waiting_for_invite(&state, user_id).await;
                return Ok(());
            }
            RequestStatus::Rejected => {
                bot.send_message(
                    msg.chat.id,
                    "Ваша заявка на регистрацию отклонена администратором.",
                )
                .reply_markup(crate::bot::keyboards::user_menu())
                .await?;
                unmark_user_waiting_for_invite(&state, user_id).await;
                return Ok(());
            }
            RequestStatus::Deleted => {}
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

async fn cmd_link(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };
    tracing::info!(user_id = user_id, "Received /link command");

    send_user_link(&bot, msg.chat.id, user_id, &state).await
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

    let status_text = perform_hard_ban(&state, tg_user_id).await?;
    bot.send_message(msg.chat.id, status_text).await?;
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

pub async fn admin_show_pending_cmd(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    admin_show_pending(bot, chat_id, state).await
}

pub async fn admin_show_users_cmd(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    admin_show_users_page(bot, chat_id, state, 1, None).await
}

pub async fn admin_show_service_cmd(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    admin_show_service_panel(bot, chat_id, state).await
}

pub async fn admin_show_stats_cmd(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    admin_show_stats(bot, chat_id, state).await
}

pub async fn try_process_waiting_invite(
    bot: &Bot,
    msg: &Message,
    state: &BotState,
    user_id: i64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if !state.config.is_admin(user_id)
        && !msg.text().unwrap_or("").starts_with('/')
        && is_user_waiting_for_invite(state, user_id).await
    {
        let username = msg.from.as_ref().and_then(|u| u.username.clone());
        let display_name = sender_display_name(msg);
        process_invite_token(
            bot,
            msg,
            state,
            user_id,
            username.as_deref(),
            display_name.as_deref(),
            msg.text().unwrap_or("").trim(),
        )
        .await?;
        return Ok(true);
    }
    Ok(false)
}
