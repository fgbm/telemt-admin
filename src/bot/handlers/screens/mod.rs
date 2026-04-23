use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, MessageId};

use crate::bot::handlers::shared::HandlerResult;
use crate::db::{AdminActivity, AdminActivityKind};
use std::future::Future;

pub mod connections;
pub mod groups;
pub mod home;
pub mod pending;
pub mod service;
pub mod stats;
pub mod tokens;
pub mod users;

pub use connections::*;
pub use groups::*;
pub use home::*;
pub use pending::*;
pub use service::*;
pub use stats::*;
pub use tokens::*;
pub use users::*;

async fn upsert_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    text: String,
    reply_markup: InlineKeyboardMarkup,
) -> HandlerResult {
    if let Some(message_id) = message_id {
        bot.edit_message_text(chat_id, message_id, text)
            .reply_markup(reply_markup)
            .await?;
    } else {
        bot.send_message(chat_id, text)
            .reply_markup(reply_markup)
            .await?;
    }
    Ok(())
}

struct PagedSelectorConfig {
    chat_id: ChatId,
    message_id: Option<MessageId>,
    total_items: i64,
    page_size: i64,
    requested_page: i64,
    empty_text: String,
    empty_keyboard: InlineKeyboardMarkup,
}

async fn render_paged_selector_screen<T, LoadFn, LoadFut, MapFn, TextFn, KeyboardFn>(
    bot: &Bot,
    config: PagedSelectorConfig,
    load_items: LoadFn,
    map_item: MapFn,
    text_builder: TextFn,
    keyboard_builder: KeyboardFn,
) -> HandlerResult
where
    LoadFn: FnOnce(i64, i64) -> LoadFut,
    LoadFut: Future<Output = Result<Vec<T>, anyhow::Error>>,
    MapFn: Fn(&T) -> (i64, String),
    TextFn: Fn(i64, i64, i64) -> String,
    KeyboardFn: Fn(&[(i64, String)], i64, i64) -> InlineKeyboardMarkup,
{
    if config.total_items <= 0 {
        upsert_screen(
            bot,
            config.chat_id,
            config.message_id,
            config.empty_text,
            config.empty_keyboard,
        )
        .await?;
        return Ok(());
    }

    let (page, total_pages, offset) =
        page_bounds(config.total_items, config.page_size, config.requested_page);
    let rows = load_items(config.page_size, offset).await?;
    let items: Vec<(i64, String)> = rows.iter().map(map_item).collect();
    let text = text_builder(config.total_items, page, total_pages);
    let keyboard = keyboard_builder(&items, page, total_pages);
    upsert_screen(bot, config.chat_id, config.message_id, text, keyboard).await?;
    Ok(())
}

fn page_bounds(total_items: i64, page_size: i64, requested_page: i64) -> (i64, i64, i64) {
    let total_pages = ((total_items + page_size - 1) / page_size).max(1);
    let page = requested_page.clamp(1, total_pages);
    let offset = (page - 1) * page_size;
    (page, total_pages, offset)
}

fn compact_line(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        format!(
            "{}...",
            trimmed
                .chars()
                .take(limit.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn service_status_label(active_state: &str, sub_state: &str) -> String {
    match (active_state, sub_state) {
        ("active", "running") => "работает".to_string(),
        ("active", value) => value.to_string(),
        ("inactive", value) => value.to_string(),
        (value, _) => value.to_string(),
    }
}

fn admin_activity_summary(activity: &AdminActivity) -> String {
    match &activity.kind {
        AdminActivityKind::RequestApproved { request_id } => {
            format!("Заявка #{} одобрена", request_id)
        }
        AdminActivityKind::RequestRejected { request_id } => {
            format!("Заявка #{} отклонена", request_id)
        }
        AdminActivityKind::TokenCreated { token } => format!("Токен {} создан", token),
        AdminActivityKind::TokenRevoked { token } => format!("Токен {} отозван", token),
    }
}
