use teloxide::prelude::*;
use teloxide::types::MessageId;

use crate::bot::handlers::format::format_timestamp;
use crate::bot::handlers::state::BotState;
use super::upsert_screen;
use crate::bot::handlers::shared::HandlerResult;

pub async fn admin_show_groups_menu(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
) -> HandlerResult {
    let groups = state.db.list_user_groups().await?;
    let text = if groups.is_empty() {
        "📁 Группы пользователей\n\nПока нет ни одной группы. Нажмите «Новая группа».".to_string()
    } else {
        "📁 Группы пользователей\n\nВыберите группу или создайте новую.".to_string()
    };
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::groups_menu_keyboard(&groups),
    )
    .await
}

pub async fn admin_show_group_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
    group: &crate::db::UserGroup,
) -> HandlerResult {
    let n = state.db.count_group_members(group.id).await?;
    let created_line = format_timestamp(group.created_at);
    let exp_line = match group.expires_at {
        Some(ts) => format!(
            "\nОбщий срок группы: {}\nUnix timestamp: {}",
            format_timestamp(ts),
            ts
        ),
        None => "\nОбщий срок группы: не задан.".to_string(),
    };
    let text = format!(
        "📁 Группа: {}\nID: {}\nСоздана: {}\nУчастников: {}{}\n\n\
         «Задать/изменить срок» обновит общий срок группы через UI.\n\
         «Снять срок» очистит общий срок группы.\n\
         «Отключить всех» удалит пользователей из telemt и локальной БД, затем удалит группу.\n\
         «Применить срок» выставит всем участникам `expiration` из RFC3339, вычисленного из unix-срока группы.",
        group.name,
        group.id,
        created_line,
        n,
        exp_line
    );
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::group_card_keyboard(group.id),
    )
    .await
}
