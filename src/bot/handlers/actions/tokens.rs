use crate::bot::handlers::format::{format_date, format_mode};
use crate::bot::handlers::screens::show_token_card;
use crate::bot::handlers::shared::build_bot_start_link;
use crate::bot::handlers::state::BotState;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Bot, ChatId, Requester};
use teloxide::types::ParseMode;

pub async fn open_token_from_lookup_input(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    arg: &str,
    page: i64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let token_value = arg.trim().trim_matches('`').trim();
    if token_value.is_empty() {
        bot.send_message(chat_id, "Отправьте код токена одним сообщением.")
            .await?;
        return Ok(false);
    }

    let Some(token) = state
        .db
        .get_active_invite_token_by_token(token_value)
        .await?
    else {
        bot.send_message(
            chat_id,
            "Токен не найден, неактивен или уже недоступен. Можно отправить другой код.",
        )
        .await?;
        return Ok(false);
    };

    show_token_card(bot, chat_id, None, &token, page).await?;
    Ok(true)
}

pub async fn handle_token_create_from_text(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    auto_approve: bool,
    text: &str,
    created_by: Option<i64>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let security = &state.config.security;
    let format_hint = format!(
        "Параметры invite-токена (срок действия ссылки и лимит активаций).\n\
         Это не срок доступа пользователя в telemt — его задают отдельно в карточке пользователя.\n\n\
         Формат: одно число — срок действия invite-токена в днях; или два числа: дни и максимум активаций.\n\
         По умолчанию: {} дней, лимит активаций без ограничения.\n\
         Примеры: 7 или 7 3.\n\
         Лимит активаций можно задать и флагом: --max-uses 3",
        security.default_token_days
    );
    if auto_approve && !security.allow_auto_approve_tokens {
        bot.send_message(
            chat_id,
            "Автоподтверждение токенов запрещено в конфигурации.",
        )
        .await?;
        return Ok(false);
    }

    let mut days: Option<i64> = None;
    let mut max_uses: Option<i64> = None;
    let args: Vec<&str> = text.split_whitespace().collect();
    let mut index = 0;
    let mut positional_numbers: Vec<i64> = Vec::new();

    if args.is_empty() {
        days = Some(security.default_token_days);
    }

    while index < args.len() {
        match args[index] {
            "--max-uses" => {
                let Some(value) = args.get(index + 1) else {
                    bot.send_message(chat_id, &format_hint).await?;
                    return Ok(false);
                };
                let parsed = match value.parse::<i64>() {
                    Ok(parsed) if parsed >= 1 => parsed,
                    _ => {
                        bot.send_message(
                            chat_id,
                            "Лимит активаций токена должен быть не меньше 1.",
                        )
                        .await?;
                        return Ok(false);
                    }
                };
                if max_uses.is_some() {
                    bot.send_message(
                        chat_id,
                        "Лимит активаций токена можно указать только один раз.",
                    )
                    .await?;
                    return Ok(false);
                }
                max_uses = Some(parsed);
                index += 2;
            }
            value => {
                if let Ok(parsed_number) = value.parse::<i64>() {
                    positional_numbers.push(parsed_number);
                    if positional_numbers.len() > 2 {
                        bot.send_message(
                            chat_id,
                            "Укажите не больше двух чисел: срок действия invite-токена (дни) и лимит активаций.",
                        )
                        .await?;
                        return Ok(false);
                    }
                    index += 1;
                    continue;
                }
                bot.send_message(chat_id, &format_hint).await?;
                return Ok(false);
            }
        }
    }

    if let Some(parsed_days) = positional_numbers.first().copied() {
        if days.is_some() {
            bot.send_message(
                chat_id,
                "Срок действия invite-токена (дни) можно указать только один раз.",
            )
            .await?;
            return Ok(false);
        }
        days = Some(parsed_days);
    }
    if let Some(parsed_max_uses) = positional_numbers.get(1).copied() {
        if max_uses.is_some() {
            bot.send_message(
                chat_id,
                "Лимит активаций токена можно указать только один раз.",
            )
            .await?;
            return Ok(false);
        }
        max_uses = Some(parsed_max_uses);
    }

    let days = days.unwrap_or(security.default_token_days);
    if days < 1 {
        bot.send_message(
            chat_id,
            "Срок действия invite-токена (дней) должен быть не меньше 1.",
        )
        .await?;
        return Ok(false);
    }
    if let Some(max_uses) = max_uses
        && max_uses < 1
    {
        bot.send_message(chat_id, "Лимит активаций токена должен быть не меньше 1.")
            .await?;
        return Ok(false);
    }
    if days > security.max_token_days {
        bot.send_message(
            chat_id,
            format!(
                "Срок действия invite-токена не может превышать {} дней.",
                security.max_token_days
            ),
        )
        .await?;
        return Ok(false);
    }

    let token = state
        .db
        .create_invite_token(days, auto_approve, max_uses, created_by)
        .await?;

    let link_line = state
        .bot_username
        .as_deref()
        .map(|bot_username| {
            let invite_link = build_bot_start_link(bot_username, &token.token);
            format!("Ссылка: {}\n", invite_link)
        })
        .unwrap_or_else(|| {
            "Ссылка: недоступна (username бота неизвестен). Укажите `bot_username` в конфиге \
             или `TELEMT_ADMIN__BOT_USERNAME`, если до api.telegram.org нет доступа для getMe.\n"
                .to_string()
        });

    let response = format!(
        "✅ Invite-токен создан:\n\
         Код: <code>{}</code>\n\
         {}\
         Режим: {}\n\
         Срок действия ссылки (invite): до {}\n\
         Лимит активаций по ссылке: {}\n\
         (Срок доступа пользователя в telemt задаётся отдельно.)\n",
        token.token,
        link_line,
        format_mode(token.auto_approve),
        format_date(token.expires_at),
        token
            .max_usage
            .map(|value| value.to_string())
            .unwrap_or_else(|| "без лимита".to_string()),
    );
    bot.send_message(chat_id, response)
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(true)
}
