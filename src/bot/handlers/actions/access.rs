use super::users::try_auto_import_remote_user_by_tg_id;
use crate::bot::handlers::format::format_timestamp;
use crate::bot::handlers::shared::HandlerResult;
use crate::bot::handlers::state::{BotState, clear_wizard_state, telemt_username};
use crate::db::{
    ConsumedInviteToken, Db, RegisterResult, RegistrationRequest, TokenConsumeError, TokenMode,
};
use crate::link::generate_user_secret;
use crate::telemt_backend::{DeleteUserResult, ProvisionedUser, TelemtBackendMode};
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Bot, ChatId, Message, Requester};

const SYNC_ERROR_USER_NOT_FOUND_IN_BACKEND: &str = "user_not_found_in_backend";
const SYNC_ERROR_DEGRADED_LEGACY_FALLBACK: &str = "degraded_legacy_fallback";

#[derive(Debug, Clone)]
struct SyncStateUpdate {
    backend_mode: String,
    last_seen_revision: Option<String>,
    last_sync_error: Option<&'static str>,
}

impl SyncStateUpdate {
    fn new(
        backend_mode: TelemtBackendMode,
        last_seen_revision: Option<String>,
        last_sync_error: Option<&'static str>,
    ) -> Self {
        Self {
            backend_mode: backend_mode.as_str().to_string(),
            last_seen_revision,
            last_sync_error,
        }
    }

    async fn persist(&self, db: &Db, tg_user_id: i64) -> Result<(), anyhow::Error> {
        db.mark_sync_state(
            tg_user_id,
            &self.backend_mode,
            self.last_seen_revision.as_deref(),
            self.last_sync_error,
        )
        .await
    }
}

fn sync_state_for_provision(
    configured_mode: TelemtBackendMode,
    provisioned: &ProvisionedUser,
) -> SyncStateUpdate {
    let last_sync_error =
        if configured_mode == TelemtBackendMode::ControlApi
            && provisioned.mode == TelemtBackendMode::LegacyFile
        {
            Some(SYNC_ERROR_DEGRADED_LEGACY_FALLBACK)
        } else {
            None
        };

    SyncStateUpdate::new(
        provisioned.mode,
        provisioned.revision.clone(),
        last_sync_error,
    )
}

fn sync_state_for_delete(
    configured_mode: TelemtBackendMode,
    delete_result: &DeleteUserResult,
) -> SyncStateUpdate {
    let last_sync_error = if !delete_result.removed {
        Some(SYNC_ERROR_USER_NOT_FOUND_IN_BACKEND)
    } else if configured_mode == TelemtBackendMode::ControlApi
        && delete_result.mode == TelemtBackendMode::LegacyFile
    {
        Some(SYNC_ERROR_DEGRADED_LEGACY_FALLBACK)
    } else {
        None
    };

    SyncStateUpdate::new(
        delete_result.mode,
        delete_result.revision.clone(),
        last_sync_error,
    )
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
        "✅ Автоподключение по invite-токену\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Invite token: {}\n\
         Token ID: {}\n\
         Mode: {}\n\
         Срок действия ссылки (invite), не пользователя: {}\n\
         Активаций по токену: {}/{}\n\
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

async fn notify_admins(bot: &Bot, state: &BotState, req: &RegistrationRequest) -> HandlerResult {
    if !state.config.notifications.notify_on_new_request {
        tracing::debug!(
            request_id = req.id,
            "Уведомления о новых заявках отключены конфигом"
        );
        return Ok(());
    }
    let invite_line = req
        .invite_token_id
        .map(|id| format!("\n🎟 ID ссылки (invite): {}", id))
        .unwrap_or_default();
    let text = format!(
        "📋 Новая заявка #{}:\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Время: {}{}",
        req.id,
        req.tg_user_id,
        req.tg_username.as_deref().unwrap_or("—"),
        req.tg_display_name.as_deref().unwrap_or("—"),
        format_timestamp(req.created_at),
        invite_line,
    );

    let kb = crate::bot::keyboards::approve_reject_buttons(req.id);
    for admin_id in &state.config.admin_ids {
        if let Err(error) = bot
            .send_message(ChatId(*admin_id), text.clone())
            .reply_markup(kb.clone())
            .await
        {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось отправить уведомление админу"
            );
        }
    }
    Ok(())
}

async fn send_invite_token_error_message(
    bot: &Bot,
    chat_id: ChatId,
    error: TokenConsumeError,
) -> HandlerResult {
    match error {
        TokenConsumeError::NotFound => {
            bot.send_message(chat_id, "Токен не найден. Проверьте код и попробуйте снова.")
                .await?;
        }
        TokenConsumeError::Revoked => {
            bot.send_message(chat_id, "Этот токен отозван администратором.")
                .await?;
        }
        TokenConsumeError::Expired => {
            bot.send_message(chat_id, "Срок действия invite-токена (ссылки) истёк.")
                .await?;
        }
        TokenConsumeError::UsageLimitReached => {
            bot.send_message(chat_id, "Лимит активаций invite-токена исчерпан.")
                .await?;
        }
        TokenConsumeError::Internal(error) => return Err(error.into()),
    }
    Ok(())
}

async fn send_existing_user_link_message(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    telemt_user: &str,
    secret: &str,
) -> HandlerResult {
    let secret_opt = (!secret.is_empty()).then_some(secret);
    let link = state
        .telemt_backend
        .build_user_link(telemt_user, secret_opt)
        .await?;
    bot.send_message(chat_id, state.config.bot_messages.user_link_text(&link))
        .await?;
    Ok(())
}

pub async fn approve_request_and_build_link(
    state: &BotState,
    request_id: i64,
) -> Result<Option<(RegistrationRequest, String)>, anyhow::Error> {
    let request = match state.db.get_pending_by_id(request_id).await? {
        Some(request) => request,
        None => return Ok(None),
    };

    let telemt_user = telemt_username(request.tg_user_id);
    let user_secret = generate_user_secret();
    let provisioned = state
        .telemt_backend
        .provision_user(&telemt_user, &user_secret)
        .await?;
    if state
        .db
        .approve(request_id, &telemt_user, &provisioned.secret)
        .await?
        .is_none()
    {
        return Ok(None);
    }
    sync_state_for_provision(state.telemt_backend.mode(), &provisioned)
        .persist(&state.db, request.tg_user_id)
        .await?;
    let proxy_link = if let Some(link) = provisioned.link {
        link
    } else {
        state
            .telemt_backend
            .build_user_link(&telemt_user, Some(&provisioned.secret))
            .await?
    };
    Ok(Some((request, proxy_link)))
}

pub async fn approve_user_direct_and_build_link(
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    invite_token_id: Option<i64>,
) -> Result<String, anyhow::Error> {
    let telemt_user = telemt_username(tg_user_id);
    let secret = generate_user_secret();
    let provisioned = state
        .telemt_backend
        .provision_user(&telemt_user, &secret)
        .await?;
    state
        .db
        .set_approved(
            tg_user_id,
            tg_username,
            tg_display_name,
            &telemt_user,
            &provisioned.secret,
            invite_token_id,
        )
        .await?;
    sync_state_for_provision(state.telemt_backend.mode(), &provisioned)
        .persist(&state.db, tg_user_id)
        .await?;
    if let Some(link) = provisioned.link {
        Ok(link)
    } else {
        state
            .telemt_backend
            .build_user_link(&telemt_user, Some(&provisioned.secret))
            .await
    }
}

pub async fn process_invite_token(
    bot: &Bot,
    msg: &Message,
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    token: &str,
) -> HandlerResult {
    if let Some((telemt_user, secret)) = state.db.get_approved(tg_user_id).await? {
        match state.db.peek_invite_token_for_existing_user(token).await {
            Ok(_) => {}
            Err(TokenConsumeError::UsageLimitReached) => {
                return Err(anyhow::anyhow!(
                    "peek_invite_token_for_existing_user не должен возвращать UsageLimitReached"
                )
                .into());
            }
            Err(error) => {
                send_invite_token_error_message(bot, msg.chat.id, error).await?;
                return Ok(());
            }
        }
        send_existing_user_link_message(bot, msg.chat.id, state, &telemt_user, &secret).await?;
        clear_wizard_state(state, tg_user_id).await?;
        tracing::info!(
            tg_user_id = tg_user_id,
            token = %token,
            "Повторный переход по invite для уже одобренного пользователя (usage не увеличивается)"
        );
        return Ok(());
    }

    if state.db.get_request_by_tg_user(tg_user_id).await?.is_none() {
        match state.db.peek_invite_token_for_existing_user(token).await {
            Ok(existing_token) => {
                if try_auto_import_remote_user_by_tg_id(
                    state,
                    tg_user_id,
                    tg_username,
                    tg_display_name,
                    Some(existing_token.id),
                )
                .await?
                {
                    let (telemt_user, secret) = state
                        .db
                        .get_approved(tg_user_id)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("Пользователь импортирован, но запись approved не найдена"))?;
                    send_existing_user_link_message(bot, msg.chat.id, state, &telemt_user, &secret)
                        .await?;
                    clear_wizard_state(state, tg_user_id).await?;
                    tracing::info!(
                        tg_user_id = tg_user_id,
                        token = %token,
                        token_id = existing_token.id,
                        "Автоподхватили существующего пользователя из telemt по invite без увеличения usage"
                    );
                    return Ok(());
                }
            }
            Err(TokenConsumeError::UsageLimitReached) => {}
            Err(error) => {
                send_invite_token_error_message(bot, msg.chat.id, error).await?;
                return Ok(());
            }
        }
    }

    let consumed = match state.db.consume_invite_token(token).await {
        Ok(token_payload) => token_payload,
        Err(error) => {
            send_invite_token_error_message(bot, msg.chat.id, error).await?;
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
                .register_or_get(
                    tg_user_id,
                    tg_username,
                    tg_display_name,
                    Some(consumed.id),
                )
                .await?;
            match result {
                RegisterResult::Approved(secret) => {
                    let sec = (!secret.is_empty()).then_some(secret.as_str());
                    let link = state
                        .telemt_backend
                        .build_user_link(&telemt_username(tg_user_id), sec)
                        .await?;
                    bot.send_message(msg.chat.id, state.config.bot_messages.user_link_text(&link))
                        .await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
                RegisterResult::Rejected => {
                    bot.send_message(
                        msg.chat.id,
                        state.config.bot_messages.request_rejected_or_default(),
                    )
                    .await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
                RegisterResult::AlreadyPending => {
                    bot.send_message(
                        msg.chat.id,
                        state.config.bot_messages.request_pending_or_default(),
                    )
                    .await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
                RegisterResult::NewPending(ref req) => {
                    bot.send_message(
                        msg.chat.id,
                        state.config.bot_messages.request_submitted_or_default(),
                    )
                        .await?;
                    notify_admins(bot, state, req).await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
            }
        }
        TokenMode::AutoApprove => {
            let link = approve_user_direct_and_build_link(
                state,
                tg_user_id,
                tg_username,
                tg_display_name,
                Some(consumed.id),
            )
            .await?;
            bot.send_message(
                msg.chat.id,
                state.config.bot_messages.access_approved_text(&link),
            )
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
            clear_wizard_state(state, tg_user_id).await?;
        }
    }

    Ok(())
}

pub async fn send_user_link(
    bot: &Bot,
    chat_id: ChatId,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    state: &BotState,
) -> HandlerResult {
    let maybe = state.db.get_approved(tg_user_id).await?;
    match maybe {
        Some((telemt_user, secret)) => {
            send_existing_user_link_message(bot, chat_id, state, &telemt_user, &secret).await?;
        }
        None => {
            if state.config.is_admin(tg_user_id) {
                tracing::info!(
                    tg_user_id = tg_user_id,
                    "Администратор запросил ссылку без существующей учётной записи, создаём доступ автоматически"
                );
                let link = approve_user_direct_and_build_link(
                    state,
                    tg_user_id,
                    tg_username,
                    tg_display_name,
                    None,
                )
                .await?;
                bot.send_message(chat_id, state.config.bot_messages.user_link_text(&link))
                    .await?;
            } else if try_auto_import_remote_user_by_tg_id(
                state,
                tg_user_id,
                tg_username,
                tg_display_name,
                None,
            )
            .await?
            {
                let (telemt_user, secret) = state
                    .db
                    .get_approved(tg_user_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Пользователь импортирован, но ссылка не может быть построена"))?;
                send_existing_user_link_message(bot, chat_id, state, &telemt_user, &secret).await?;
            } else {
                bot.send_message(
                    chat_id,
                    state.config.bot_messages.no_access_link_or_default(),
                )
                .await?;
            }
        }
    }
    Ok(())
}

pub async fn perform_hard_ban(state: &BotState, tg_user_id: i64) -> Result<String, anyhow::Error> {
    let telemt_user = telemt_username(tg_user_id);
    state.db.set_user_group_membership(tg_user_id, None).await?;
    let delete_result = state.telemt_backend.delete_user(&telemt_user).await?;
    let removed_from_db = state.db.deactivate_user(tg_user_id).await?;
    sync_state_for_delete(state.telemt_backend.mode(), &delete_result)
        .persist(&state.db, tg_user_id)
        .await?;

    if delete_result.removed || removed_from_db {
        Ok(format!("Пользователь {} удалён", telemt_user))
    } else {
        Ok(format!("Пользователь {} не найден", telemt_user))
    }
}
