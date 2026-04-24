use super::actions::{
    admin_show_connections_summary, admin_show_service_panel, process_invite_token,
    send_user_link, show_user_card, try_auto_import_remote_user_by_tg_id,
};
use super::callback_data::CallbackAction;
use super::screens::{
    admin_show_pending_requests_page, admin_show_stats, admin_show_users_page,
    send_text_with_keyboard_removed, show_admin_home, show_token_card, show_token_menu,
    show_user_home,
};
use super::shared::{
    AdminStartScreen, HandlerResult, StartPayload, parse_start_payload, send_admin_backend_error,
    user_id_or_reply,
};
use super::state::{
    BotState, WizardState, clear_wizard_state, is_admin_message, sender_display_name,
    sender_user_id, set_wizard_state,
};
use crate::db::RequestStatus;
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::BotCommand;
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum PublicBotCommand {
    #[command(description = "🏠 Главный экран")]
    Start,
    #[command(description = "❓ Справка")]
    Help,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum AdminBotCommand {
    #[command(description = "🏠 Главный экран")]
    Start,
    #[command(description = "👥 Пользователи")]
    User,
    #[command(description = "🎟 Управление токенами")]
    Token,
    #[command(description = "⚙️ Управление сервисом")]
    Service,
    #[command(description = "🔗 Получить ссылку")]
    Link,
    #[command(description = "❓ Справка")]
    Help,
}

pub fn public_telegram_commands() -> Vec<BotCommand> {
    PublicBotCommand::bot_commands()
}

pub fn admin_telegram_commands() -> Vec<BotCommand> {
    AdminBotCommand::bot_commands()
}

pub fn handler()
-> teloxide::dispatching::UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    dptree::filter(|msg: Message| {
        msg.text()
            .is_some_and(|text| text.trim_start().starts_with('/'))
    })
    .endpoint(handle_command_message)
}

fn extract_command_name<'a>(text: &'a str, bot_username: Option<&str>) -> Option<&'a str> {
    let command = text.split_whitespace().next()?.strip_prefix('/')?;
    let (name, mentioned_bot) = command
        .split_once('@')
        .map_or((command, None), |(name, bot)| (name, Some(bot)));
    if let (Some(mentioned_bot), Some(bot_username)) = (mentioned_bot, bot_username) {
        let bot_username = bot_username.trim_start_matches('@');
        if !mentioned_bot.eq_ignore_ascii_case(bot_username) {
            return None;
        }
    }
    Some(name)
}

async fn handle_command_message(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let command_label = msg
        .text()
        .and_then(|text| extract_command_name(text, state.bot_username.as_deref()))
        .map(|name| format!("/{}", name))
        .unwrap_or_else(|| "неизвестная команда".to_string());
    let is_admin = is_admin_message(&msg, &state);
    let chat_id = msg.chat.id;
    let result = handle_command_message_inner(bot.clone(), msg, state).await;
    if let Err(error) = result {
        tracing::error!(command = %command_label, error = %error, "Ошибка выполнения команды");
        if is_admin {
            send_admin_backend_error(&bot, chat_id, &command_label, error.as_ref()).await;
        }
    }
    Ok(())
}

async fn handle_command_message_inner(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let Some(command_name) = extract_command_name(text, state.bot_username.as_deref()) else {
        return Ok(());
    };

    match command_name {
        "start" => start_cmd(bot, msg, state).await,
        "link" => cmd_link(bot, msg, state).await,
        "help" => cmd_help(bot, msg, state).await,
        "user" => cmd_user(bot, msg, state).await,
        "service" => cmd_service(bot, msg, state).await,
        "token" => cmd_token(bot, msg, state).await,
        _ => {
            bot.send_message(msg.chat.id, state.config.bot_messages.unknown_command_or_default())
                .await?;
            Ok(())
        }
    }
}

pub async fn cmd_help(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };
    let is_admin = state.config.is_admin(user_id);
    let text = if is_admin {
        state.config.bot_messages.help_admin_or_default()
    } else {
        state.config.bot_messages.help_user_or_default()
    };
    send_text_with_keyboard_removed(&bot, msg.chat.id, text).await?;
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

    let text = msg.text().unwrap_or("");
    if let Some(payload) = parse_start_payload(text) {
        match payload {
            StartPayload::InviteToken(token) => {
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
            StartPayload::AdminUser(target_user_id) => {
                if !state.config.is_admin(user_id) {
                    bot.send_message(
                        msg.chat.id,
                        state.config.bot_messages.admin_only_deep_link_or_default(),
                    )
                    .await?;
                    return Ok(());
                }
                clear_wizard_state(&state, user_id).await?;
                if let Some(user) = state.db.get_active_user_by_tg_user(target_user_id).await? {
                    show_user_card(&bot, msg.chat.id, None, &user, 1, &state).await?;
                } else {
                    bot.send_message(msg.chat.id, state.config.bot_messages.user_not_found_or_default())
                        .await?;
                }
                return Ok(());
            }
            StartPayload::AdminToken(token_id) => {
                if !state.config.is_admin(user_id) {
                    bot.send_message(
                        msg.chat.id,
                        state.config.bot_messages.admin_only_deep_link_or_default(),
                    )
                    .await?;
                    return Ok(());
                }
                clear_wizard_state(&state, user_id).await?;
                if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                    show_token_card(&bot, msg.chat.id, None, &token, 1).await?;
                } else {
                    bot.send_message(msg.chat.id, state.config.bot_messages.token_not_found_or_default())
                        .await?;
                }
                return Ok(());
            }
            StartPayload::AdminScreen(screen) => {
                if !state.config.is_admin(user_id) {
                    bot.send_message(
                        msg.chat.id,
                        state.config.bot_messages.admin_only_deep_link_or_default(),
                    )
                    .await?;
                    return Ok(());
                }
                clear_wizard_state(&state, user_id).await?;
                match screen {
                    AdminStartScreen::Home => show_admin_home(&bot, msg.chat.id, None, &state).await?,
                    AdminStartScreen::Users => {
                        admin_show_users_page(&bot, msg.chat.id, &state, 1, None).await?
                    }
                    AdminStartScreen::Tokens => show_token_menu(&bot, msg.chat.id, None, &state).await?,
                    AdminStartScreen::Service => {
                        admin_show_service_panel(&bot, msg.chat.id, &state, None).await?
                    }
                    AdminStartScreen::Stats => {
                        admin_show_stats(&bot, msg.chat.id, &state, None).await?
                    }
                    AdminStartScreen::Pending => {
                        admin_show_pending_requests_page(&bot, msg.chat.id, &state, 1, None)
                            .await?
                    }
                    AdminStartScreen::Connections => {
                        admin_show_connections_summary(&bot, msg.chat.id, &state, None).await?
                    }
                }
                return Ok(());
            }
        }
    }

    if state.config.is_admin(user_id) {
        clear_wizard_state(&state, user_id).await?;
        show_admin_home(&bot, msg.chat.id, None, &state).await?;
        return Ok(());
    }

    if let Some(existing) = state.db.get_request_by_tg_user(user_id).await? {
        clear_wizard_state(&state, user_id).await?;
        match existing.status {
            RequestStatus::Approved | RequestStatus::Pending | RequestStatus::Rejected => {
                show_user_home(&bot, msg.chat.id, None, &state, user_id).await?;
                return Ok(());
            }
            RequestStatus::Deleted => {}
        }
    }

    if try_auto_import_remote_user_by_tg_id(
        &state,
        user_id,
        username.as_deref(),
        display_name.as_deref(),
        None,
    )
    .await?
    {
        clear_wizard_state(&state, user_id).await?;
        show_user_home(&bot, msg.chat.id, None, &state, user_id).await?;
        return Ok(());
    }

    if let Some(stub) = state
        .config
        .bot_messages
        .start_without_invite
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        clear_wizard_state(&state, user_id).await?;
        bot.send_message(msg.chat.id, stub)
            .reply_markup(crate::bot::keyboards::user_home_keyboard(false))
            .await?;
        return Ok(());
    }

    set_wizard_state(&state, user_id, WizardState::AwaitingInviteToken).await?;
    bot.send_message(
        msg.chat.id,
        state.config.bot_messages.invite_manual_prompt_or_default(),
    )
    .reply_markup(crate::bot::keyboards::cancel_keyboard(
        CallbackAction::ShowUserHome,
    ))
    .await?;
    Ok(())
}

async fn cmd_link(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };
    let username = msg.from.as_ref().and_then(|u| u.username.as_deref());
    let display_name = sender_display_name(&msg);
    tracing::info!(user_id = user_id, "Received /link command");

    send_user_link(
        &bot,
        msg.chat.id,
        user_id,
        username,
        display_name.as_deref(),
        &state,
    )
    .await
}

async fn cmd_user(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }
    admin_show_users_page(&bot, msg.chat.id, &state, 1, None).await?;
    Ok(())
}

async fn cmd_service(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    admin_show_service_panel(&bot, msg.chat.id, &state, None).await?;
    Ok(())
}

async fn cmd_token(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    show_token_menu(&bot, msg.chat.id, None, &state).await?;
    Ok(())
}
