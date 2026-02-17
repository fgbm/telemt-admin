use crate::db::{InviteToken, RegistrationRequest};
use chrono::{DateTime, Local, Utc};

pub fn format_date(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&Local).format("%d.%m.%Y").to_string())
        .unwrap_or_else(|| "—".to_string())
}

pub fn format_mode(auto_approve: bool) -> &'static str {
    if auto_approve {
        "АВТОПОДТВЕРЖДЕНИЕ 🚀"
    } else {
        "Ручной ✅"
    }
}

pub fn format_timestamp(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|| format!("Некорректный timestamp: {}", ts))
}

pub fn user_display_name(user: &RegistrationRequest) -> String {
    user.tg_display_name
        .clone()
        .or_else(|| {
            user.tg_username
                .as_ref()
                .map(|username| format!("@{}", username))
        })
        .or_else(|| user.telemt_username.clone())
        .unwrap_or_else(|| format!("tg_{}", user.tg_user_id))
}

pub fn render_invite_token_line(token: &InviteToken) -> String {
    let mode = if token.auto_approve { "AUTO" } else { "MANUAL" };
    let usage = token
        .max_usage
        .map(|max| format!("{}/{}", token.usage_count, max))
        .unwrap_or_else(|| format!("{}/∞", token.usage_count));
    let created_by = token
        .created_by
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_string());
    format!(
        "• {} | {} | до {} | usage {} | creator {} | создан {}",
        token.token,
        mode,
        format_date(token.expires_at),
        usage,
        created_by,
        format_date(token.created_at)
    )
}

pub fn render_user_card_text(user: &RegistrationRequest, page: i64) -> String {
    format!(
        "👤 Карточка пользователя\n\n\
         Страница списка: {}\n\
         TG ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Статус: {}\n\
         Telemt username: {}\n\
         Создано: {}",
        page,
        user.tg_user_id,
        user.tg_username.as_deref().unwrap_or("—"),
        user.tg_display_name.as_deref().unwrap_or("—"),
        user.status,
        user.telemt_username.as_deref().unwrap_or("—"),
        format_timestamp(user.created_at),
    )
}

pub fn usage_guide_text() -> &'static str {
    r#"Как подключиться к прокси:

1) Нажмите «🔗 Моя ссылка» — бот отправит вам ссылку.
2) Нажмите на ссылку — Telegram автоматически предложит добавить прокси.
3) Подтвердите добавление.

Если не получается, обратитесь к администратору."#
}
