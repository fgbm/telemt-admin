use super::super::common::{ack_callback, admin_callback_target};
use super::AdminActionResult;
use crate::bot::handlers::actions::groups::{
    apply_group_expiry_to_members, deactivate_all_members,
};
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{admin_show_group_card, admin_show_groups_menu};
use crate::bot::handlers::state::{BotState, WizardState, set_wizard_state};
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Bot, CallbackQuery, Requester};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowGroupsMenu => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_groups_menu(bot, chat_id, Some(message_id), state).await?;
            Ok(true)
        }
        CallbackAction::OpenGroupCard { group_id } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(group) = state.db.get_user_group_by_id(group_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_group_card(bot, chat_id, Some(message_id), state, &group).await?;
            Ok(true)
        }
        CallbackAction::PromptCreateGroup => {
            let Some((admin_id, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            set_wizard_state(state, admin_id, WizardState::AdminGroupAwaitingName).await?;
            ack_callback(bot, q.id.clone(), Some("Жду имя группы"), false).await?;
            bot.send_message(
                chat_id,
                "Введите имя новой группы одним сообщением.\n\nОтмена: отправьте пустое сообщение или вернитесь в админку.",
            )
            .await?;
            Ok(true)
        }
        CallbackAction::PromptGroupExpiry { group_id } => {
            let Some((admin_id, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(group) = state.db.get_user_group_by_id(group_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                return Ok(true);
            };
            set_wizard_state(
                state,
                admin_id,
                WizardState::AdminGroupExpiryAwaitingValue { group_id },
            )
            .await?;
            ack_callback(bot, q.id.clone(), Some("Жду срок группы"), false).await?;
            let current = group
                .expires_at
                .map(|value| value.to_string())
                .unwrap_or_else(|| "не задан".to_string());
            bot.send_message(
                chat_id,
                format!(
                    "Введите общий срок для группы «{}».\n\nПоддерживается RFC3339, `YYYY-MM-DD`, `+30d`.\nЧтобы снять срок, отправьте `none`.\n\nТекущее значение: {}",
                    group.name, current
                ),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::ClearGroupExpiry { group_id } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let updated = state.db.set_user_group_expiry(group_id, None).await?;
            if !updated {
                ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                return Ok(true);
            }
            let Some(group) = state.db.get_user_group_by_id(group_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Срок группы снят"), false).await?;
            admin_show_group_card(bot, chat_id, Some(message_id), state, &group).await?;
            Ok(true)
        }
        CallbackAction::GroupDeactivateAll { group_id } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(group) = state.db.get_user_group_by_id(group_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            let (ok, err) = deactivate_all_members(state, &group).await?;
            let groups = state.db.list_user_groups().await?;
            let text = format!(
                "Отключено пользователей: {}, ошибок: {}. Группа «{}» удалена.",
                ok, err, group.name
            );
            bot.edit_message_text(chat_id, message_id, text)
                .reply_markup(crate::bot::keyboards::groups_menu_keyboard(&groups))
                .await?;
            Ok(true)
        }
        CallbackAction::GroupApplyExpiry { group_id } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            if !state.config.telemt_api.enabled {
                ack_callback(bot, q.id.clone(), Some("Нужен telemt control API"), true).await?;
                return Ok(true);
            }
            let Some(group) = state.db.get_user_group_by_id(group_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            let (ok, err) = match apply_group_expiry_to_members(state, &group).await {
                Ok(v) => v,
                Err(error) => {
                    bot.edit_message_text(
                        chat_id,
                        message_id,
                        format!("Не удалось применить срок: {}", error),
                    )
                    .reply_markup(crate::bot::keyboards::group_card_keyboard(group.id))
                    .await?;
                    return Ok(true);
                }
            };
            let text = format!(
                "Срок действия применён через PATCH.\nУспешно: {}\nОшибок: {}",
                ok, err
            );
            bot.edit_message_text(chat_id, message_id, text)
                .reply_markup(crate::bot::keyboards::group_card_keyboard(group.id))
                .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
