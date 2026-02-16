//! Клавиатуры бота: inline и постоянные reply-кнопки.

use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup};

pub const BTN_USER_LINK: &str = "🔗 Моя ссылка";
pub const BTN_USER_GUIDE: &str = "❓ Инструкция";
pub const BTN_USER_SUPPORT: &str = "🆘 Поддержка";

pub const BTN_ADMIN_PENDING: &str = "📥 Новые заявки";
pub const BTN_ADMIN_USERS: &str = "👥 Список пользователей";
pub const BTN_ADMIN_SERVICE: &str = "⚙️ Статус сервиса";
pub const BTN_ADMIN_STATS: &str = "📊 Статистика";

pub fn user_menu() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new(BTN_USER_LINK),
            KeyboardButton::new(BTN_USER_GUIDE),
        ],
        vec![KeyboardButton::new(BTN_USER_SUPPORT)],
    ])
    .resize_keyboard()
    .persistent()
}

pub fn admin_menu() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new(BTN_ADMIN_PENDING),
            KeyboardButton::new(BTN_ADMIN_USERS),
        ],
        vec![
            KeyboardButton::new(BTN_ADMIN_SERVICE),
            KeyboardButton::new(BTN_ADMIN_STATS),
        ],
    ])
    .resize_keyboard()
    .persistent()
}

pub fn approve_reject_buttons(request_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![
        InlineKeyboardButton::callback("✅ Одобрить", format!("approve:{}", request_id)),
        InlineKeyboardButton::callback("❌ Отклонить", format!("reject:{}", request_id)),
    ])
}

pub fn delete_user_button(tg_user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![InlineKeyboardButton::callback(
        "🗑 Удалить пользователя",
        format!("delete_user:{}", tg_user_id),
    )])
}

pub fn service_control_buttons() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default()
        .append_row(vec![
            InlineKeyboardButton::callback("🔄 Обновить", "service:status"),
            InlineKeyboardButton::callback("♻️ Рестарт", "service:restart"),
        ])
        .append_row(vec![InlineKeyboardButton::callback(
            "📖 Перечитать конфиг",
            "service:reload",
        )])
}
