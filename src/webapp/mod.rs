//! Telegram Mini App: встроенный HTTP-сервер (API + статический фронтенд).
//!
//! Сервер слушает `webapp.listen` (по умолчанию loopback) и публикуется наружу только
//! через HTTPS reverse proxy. Авторизация — подписанный `initData` Telegram, см. [`auth`].

mod api;
pub mod auth;

use std::net::SocketAddr;
use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{MenuButton, WebAppInfo};

use crate::bot::handlers::BotState;
use crate::config::WebAppConfig;

/// Общее состояние HTTP-обработчиков.
#[derive(Clone)]
pub struct AppState {
    pub bot: Bot,
    pub state: BotState,
    pub bot_token: Arc<String>,
}

/// Запускает Mini App (если включено) и приводит кнопку меню бота в соответствие:
/// кнопка ведёт в приложение, только если HTTP-сервер действительно слушает порт.
/// Ошибка запуска приложения не останавливает бота.
pub async fn start(bot: &Bot, state: BotState, bot_token: String) {
    let config = state.config.webapp.clone();
    let running = config.enabled && serve(bot.clone(), state, bot_token, &config).await;
    sync_menu_button(bot, &config, running).await;
}

async fn serve(bot: Bot, state: BotState, bot_token: String, config: &WebAppConfig) -> bool {
    let addr: SocketAddr = match config.listen.parse() {
        Ok(addr) => addr,
        Err(error) => {
            tracing::error!(listen = %config.listen, error = %error, "Mini App: некорректный webapp.listen");
            return false;
        }
    };
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(%addr, error = %error, "Mini App: не удалось открыть порт, приложение не запущено");
            return false;
        }
    };
    let router = api::router(AppState {
        bot,
        state,
        bot_token: Arc::new(bot_token),
    });
    tracing::info!(%addr, "Mini App: HTTP-сервер запущен");
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            tracing::error!(error = %error, "Mini App: HTTP-сервер остановлен с ошибкой");
        }
    });
    true
}

/// Ставит кнопку меню «Mini App», если приложение работает и есть HTTPS URL,
/// иначе возвращает стандартную кнопку (чтобы не вести на неработающий адрес).
async fn sync_menu_button(bot: &Bot, config: &WebAppConfig, running: bool) {
    let web_app_url = running
        .then(|| config.https_public_url())
        .flatten()
        .and_then(|url| match reqwest::Url::parse(&url) {
            Ok(url) => Some(url),
            Err(error) => {
                tracing::warn!(error = %error, "Mini App: некорректный webapp.public_url");
                None
            }
        });
    let (button, what) = match web_app_url {
        Some(url) => (
            MenuButton::WebApp {
                text: config.menu_button_text.clone(),
                web_app: WebAppInfo { url },
            },
            "кнопка меню Mini App установлена",
        ),
        None if config.enabled => (
            MenuButton::Default,
            "кнопка меню сброшена: Mini App не запущен",
        ),
        None => (MenuButton::Default, "кнопка меню: стандартная"),
    };
    match bot.set_chat_menu_button().menu_button(button).await {
        Ok(_) => tracing::info!("Mini App: {what}"),
        Err(error) => tracing::warn!(error = %error, "Mini App: не удалось обновить кнопку меню"),
    }
}
