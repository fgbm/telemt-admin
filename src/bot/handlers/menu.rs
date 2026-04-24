use super::actions::{
    apply_user_limit_from_input, broadcast_to_approved_users, handle_token_create_from_text,
    import_remote_user_by_tg_id, open_token_from_lookup_input, open_user_from_lookup_input,
    process_invite_token, prompt_delete_confirmation,
};
use super::shared::{HandlerResult, send_admin_backend_error};
use super::state::{
    BotState, WizardState, clear_wizard_state, is_admin_message, sender_display_name,
    sender_user_id, wizard_state,
};
use chrono::{NaiveDate, Utc};
use teloxide::prelude::*;

fn parse_group_expiration_input(value: &str) -> Result<Option<i64>, anyhow::Error> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("no")
        || trimmed.eq_ignore_ascii_case("off")
        || trimmed.eq_ignore_ascii_case("clear")
        || trimmed.eq_ignore_ascii_case("без даты")
    {
        return Ok(None);
    }

    if let Ok(date_time) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Ok(Some(date_time.timestamp()));
    }

    if let Some(days) = trimmed.strip_prefix('+') {
        let days = days.trim_end_matches('d').trim();
        let days = days
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("Количество дней должно быть положительным целым числом"))?;
        if days <= 0 {
            return Err(anyhow::anyhow!("Количество дней должно быть больше нуля"));
        }
        return Ok(Some((Utc::now() + chrono::Duration::days(days)).timestamp()));
    }

    let date = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Используйте RFC3339, YYYY-MM-DD, +30d или `none`"))?;
    let date_time = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| anyhow::anyhow!("Не удалось собрать дату истечения"))?;
    Ok(Some(
        chrono::DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc).timestamp(),
    ))
}

pub async fn handle_menu_buttons(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let is_admin = is_admin_message(&msg, &state);
    let chat_id = msg.chat.id;
    let result = handle_menu_buttons_inner(bot.clone(), msg, state).await;
    if let Err(error) = result {
        tracing::error!(error = %error, "Ошибка выполнения текстового сценария");
        if is_admin {
            send_admin_backend_error(&bot, chat_id, "текстовый шаг сценария", error.as_ref()).await;
        }
    }
    Ok(())
}

async fn handle_menu_buttons_inner(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };

    match wizard_state(&state, user_id).await? {
        Some(WizardState::AdminBroadcastAwaitingMessage) => {
            if !is_admin_message(&msg, &state) {
                clear_wizard_state(&state, user_id).await?;
                return Ok(());
            }
            broadcast_to_approved_users(&bot, &msg, &state, user_id, text).await?;
        }
        Some(WizardState::AdminGroupAwaitingName) => {
            if !is_admin_message(&msg, &state) {
                clear_wizard_state(&state, user_id).await?;
                return Ok(());
            }
            let name = text.trim();
            if name.is_empty() {
                clear_wizard_state(&state, user_id).await?;
                bot.send_message(msg.chat.id, "Создание группы отменено (пустое имя).")
                    .await?;
                return Ok(());
            }
            match state.db.create_user_group(name, None).await {
                Ok(g) => {
                    clear_wizard_state(&state, user_id).await?;
                    bot.send_message(
                        msg.chat.id,
                        format!("Группа «{}» создана (id={}).", g.name, g.id),
                    )
                    .await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Не удалось создать группу: {}", error),
                    )
                    .await?;
                }
            }
        }
        Some(WizardState::AdminGroupExpiryAwaitingValue { group_id }) => {
            if !is_admin_message(&msg, &state) {
                clear_wizard_state(&state, user_id).await?;
                return Ok(());
            }
            let expires_at = match parse_group_expiration_input(text) {
                Ok(value) => value,
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Не удалось разобрать срок группы: {}", error),
                    )
                    .await?;
                    return Ok(());
                }
            };
            match state.db.set_user_group_expiry(group_id, expires_at).await {
                Ok(true) => {
                    clear_wizard_state(&state, user_id).await?;
                    let result_text = match expires_at {
                        Some(ts) => format!(
                            "Общий срок группы обновлён.\nUnix timestamp: {}\nОткройте карточку группы и примените срок к участникам.",
                            ts
                        ),
                        None => "Общий срок группы снят.".to_string(),
                    };
                    bot.send_message(msg.chat.id, result_text).await?;
                }
                Ok(false) => {
                    clear_wizard_state(&state, user_id).await?;
                    bot.send_message(msg.chat.id, "Группа не найдена.")
                        .await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Не удалось сохранить срок группы: {}", error),
                    )
                    .await?;
                }
            }
        }
        Some(WizardState::AdminImportAwaitingTgId) => {
            if !is_admin_message(&msg, &state) {
                clear_wizard_state(&state, user_id).await?;
                return Ok(());
            }
            let trimmed = text.trim();
            let tg_target = match trimmed.parse::<i64>() {
                Ok(v) => v,
                Err(_) => {
                    bot.send_message(
                        msg.chat.id,
                        "Нужен числовой Telegram user id (например 123456789).",
                    )
                    .await?;
                    return Ok(());
                }
            };
            match import_remote_user_by_tg_id(&state, tg_target).await {
                Ok(message) => {
                    clear_wizard_state(&state, user_id).await?;
                    bot.send_message(msg.chat.id, message).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("Импорт не выполнен: {}", error))
                        .await?;
                }
            }
        }
        Some(WizardState::AwaitingInviteToken) => {
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
        }
        Some(WizardState::AdminDeleteAwaitingTarget) => {
            let prompted =
                prompt_delete_confirmation(&bot, msg.chat.id, &state, text.trim()).await?;
            if prompted {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminFindUserAwaitingTarget { page }) => {
            let opened =
                open_user_from_lookup_input(&bot, msg.chat.id, &state, text.trim(), page).await?;
            if opened {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminSetUserLimitAwaitingValue {
            tg_user_id,
            page: _,
            field,
        }) => {
            let updated =
                apply_user_limit_from_input(&bot, msg.chat.id, &state, tg_user_id, field, text.trim())
                    .await?;
            if updated {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminFindTokenAwaitingCode { page }) => {
            let opened =
                open_token_from_lookup_input(&bot, msg.chat.id, &state, text.trim(), page).await?;
            if opened {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminTokenCreateAwaitingParams { auto_approve }) => {
            let created = handle_token_create_from_text(
                &bot,
                msg.chat.id,
                &state,
                auto_approve,
                text.trim(),
                Some(user_id),
            )
            .await?;
            if created {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        None => {
            bot.send_message(
                msg.chat.id,
                state.config.bot_messages.fallback_unknown_request_or_default(),
            )
            .await?;
        }
    }
    Ok(())
}
