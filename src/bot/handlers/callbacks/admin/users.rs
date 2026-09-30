use super::super::common::{ack_callback, admin_callback_target, start_wizard_from_callback};
use super::AdminActionResult;
use crate::bot::handlers::actions::{
    has_active_users, perform_hard_ban, send_user_start_link, show_user_card, user_limit_input_help,
};
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{
    admin_show_users_page, send_user_qr_to_admin, show_user_ban_confirm,
};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use crate::bot::handlers::state::{BotState, clear_wizard_state};
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Bot, CallbackQuery, Requester};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowUsersPage { page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_users_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::PromptUserLookup { page } => {
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptUserLookup { page },
                "Жду ID, @username или часть имени",
                "Отправьте Telegram ID, @username или часть имени/ника следующим сообщением.\n\nСписок можно оставить открытым.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::OpenUserCard { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Открыта карточка"), false).await?;
            show_user_card(bot, chat_id, Some(message_id), &user, page, state).await?;
            Ok(true)
        }
        CallbackAction::PromptUserLimit {
            tg_user_id,
            page,
            field,
        } => {
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptUserLimit {
                    tg_user_id,
                    page,
                    field,
                },
                "Жду новое значение лимита",
                format!(
                    "Пользователь: {}\nИзмените параметр и отправьте новое значение следующим сообщением.\n\n{}",
                    crate::bot::handlers::format::user_display_name(&user),
                    user_limit_input_help(field)
                ),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::SendUserStartLink { tg_user_id } => {
            let Some((_, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Отправляю deep link"), false).await?;
            send_user_start_link(bot, chat_id, state, tg_user_id).await?;
            Ok(true)
        }
        CallbackAction::ViewUserQr { tg_user_id } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Отправляю ссылку и QR"), false).await?;
            send_user_qr_to_admin(bot, q, &user, state).await?;
            Ok(true)
        }
        CallbackAction::ConfirmUserBan { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_user_ban_confirm(bot, chat_id, message_id, tg_user_id, page).await?;
            Ok(true)
        }
        CallbackAction::ExecuteUserBan { tg_user_id, page } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = perform_hard_ban(state, tg_user_id).await?;
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                bot.send_message(chat_id, status_text).await?;
                admin_show_users_page(bot, chat_id, state, page, Some(message_id)).await?;
            }
            Ok(true)
        }
        CallbackAction::PromptDeleteUser => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            if !has_active_users(state).await? {
                clear_wizard_state(state, admin_id).await?;
                ack_callback(bot, q.id.clone(), Some("Активных пользователей нет"), true).await?;
                return Ok(true);
            }
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptDeleteUser,
                "Жду Telegram ID",
                "Отправьте Telegram ID пользователя следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::ExecuteDeleteUser { tg_user_id } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = perform_hard_ban(state, tg_user_id).await?;
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                bot.edit_message_text(chat_id, message_id, status_text)
                    .reply_markup(crate::bot::keyboards::admin_home_keyboard())
                    .await?;
            }
            Ok(true)
        }
        CallbackAction::UserGroupPicker { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            let groups = state.db.list_user_groups().await?;
            let current = state
                .db
                .get_group_for_tg_user(tg_user_id)
                .await?
                .map(|g| g.name)
                .unwrap_or_else(|| "нет".to_string());
            let title = format!(
                "📁 Группа для {}\n\nТекущая: {}",
                crate::bot::handlers::format::user_display_name(&user),
                current
            );
            ack_callback(bot, q.id.clone(), None, false).await?;
            bot.edit_message_text(chat_id, message_id, title)
                .reply_markup(crate::bot::keyboards::user_group_picker_keyboard(
                    tg_user_id, page, &groups,
                ))
                .await?;
            Ok(true)
        }
        CallbackAction::AssignUserToGroup {
            tg_user_id,
            group_id,
            page,
        } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            let gid = if group_id == 0 {
                None
            } else {
                if state.db.get_user_group_by_id(group_id).await?.is_none() {
                    ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                    return Ok(true);
                }
                Some(group_id)
            };
            state.db.set_user_group_membership(tg_user_id, gid).await?;
            ack_callback(bot, q.id.clone(), Some("Сохранено"), false).await?;
            show_user_card(bot, chat_id, Some(message_id), &user, page, state).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
