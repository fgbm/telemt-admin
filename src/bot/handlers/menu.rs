use super::commands::{
    admin_show_pending_cmd, admin_show_service_cmd, admin_show_stats_cmd, admin_show_users_cmd,
    cmd_help, try_process_waiting_invite,
};
use super::format::usage_guide_text;
use super::shared::{send_user_link, HandlerResult};
use super::state::{sender_user_id, BotState};
use teloxide::prelude::*;

pub async fn handle_menu_buttons(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };
    let is_admin = state.config.is_admin(user_id);

    if try_process_waiting_invite(&bot, &msg, &state, user_id).await? {
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
            admin_show_pending_cmd(&bot, msg.chat.id, &state).await?;
        }
        crate::bot::keyboards::BTN_ADMIN_USERS if is_admin => {
            admin_show_users_cmd(&bot, msg.chat.id, &state).await?;
        }
        crate::bot::keyboards::BTN_ADMIN_SERVICE if is_admin => {
            admin_show_service_cmd(&bot, msg.chat.id, &state).await?;
        }
        crate::bot::keyboards::BTN_ADMIN_STATS if is_admin => {
            admin_show_stats_cmd(&bot, msg.chat.id, &state).await?;
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
            let reply_text = if is_admin {
                "Не понял команду. Используйте кнопки админ-меню ниже."
            } else {
                "Не понял запрос. Используйте кнопки меню ниже."
            };
            let reply_markup = if is_admin {
                crate::bot::keyboards::admin_menu()
            } else {
                crate::bot::keyboards::user_menu()
            };
            bot.send_message(msg.chat.id, reply_text)
                .reply_markup(reply_markup)
                .await?;
        }
    }
    Ok(())
}
