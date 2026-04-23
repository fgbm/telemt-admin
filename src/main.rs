//! telemt-admin — Telegram-бот для администрирования MTProxy telemt.

mod bot;
mod cli;
mod config;
mod env_config_overlay;
mod db;
mod link;
mod monitor;
mod runtime;
mod service;
mod telemt_backend;
mod telemt_cfg;
mod update;

use clap::Parser;
use std::sync::Arc;
use teloxide::dispatching::Dispatcher;
use teloxide::prelude::*;
use teloxide::types::BotCommandScope;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::CheckUpdate) => {
            update::run_check_update().await?;
            return Ok(());
        }
        Some(Commands::SelfUpdate) => {
            update::run_self_update().await?;
            return Ok(());
        }
        None => {}
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let config_path = cli.config_path();
    tracing::info!(
        "Starting telemt-admin with config {}",
        config_path.display()
    );

    let config = Arc::new(config::Config::load(&config_path)?);
    let token = config.bot_token()?;
    tracing::info!(
        admin_count = config.admin_ids.len(),
        db_path = %config.db_path.display(),
        telemt_config_path = %config.telemt_config_path.display(),
        service_name = %config.service_name,
        runtime_mode = %config.effective_runtime_mode().as_str(),
        users_page_size = config.users_page_size,
        "Configuration loaded"
    );

    let db =
        Arc::new(db::Db::open(&config.db_path, config.security.wizard_state_ttl_seconds).await?);
    let telemt_cfg = Arc::new(telemt_cfg::TelemtConfig::new(&config.telemt_config_path));
    let telemt_runtime = runtime::TelemtRuntime::new(
        config.effective_runtime_mode(),
        config.effective_systemd_unit(),
        config.effective_external_label(),
    );
    let telemt_backend = telemt_backend::TelemtBackend::new(
        &config.telemt_api,
        telemt_cfg.clone(),
        telemt_runtime.clone(),
    )?;

    let bot = Bot::new(token);
    let from_get_me = match bot.get_me().await {
        Ok(me) => me.user.username.clone(),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Не удалось получить username бота через getMe"
            );
            None
        }
    };
    let bot_username = config.resolved_bot_username(from_get_me);
    if let Some(ref u) = bot_username {
        if config.configured_bot_username().is_some() {
            tracing::info!(
                bot_username = %u,
                "Используется bot_username из конфигурации (приоритет над getMe)"
            );
        } else {
            tracing::info!(bot_username = %u, "Используется username бота из getMe");
        }
    } else {
        tracing::warn!(
            "username бота неизвестен: задайте bot_username в TOML или TELEMT_ADMIN__BOT_USERNAME, \
             иначе ссылки на токены и deep link недоступны"
        );
    }

    if let Err(error) = bot
        .set_my_commands(bot::handlers::public_telegram_commands())
        .scope(BotCommandScope::Default)
        .await
    {
        tracing::warn!(error = %error, "Не удалось обновить список slash-команд бота");
    }
    for admin_id in &config.admin_ids {
        if let Err(error) = bot
            .set_my_commands(bot::handlers::admin_telegram_commands())
            .scope(BotCommandScope::Chat {
                chat_id: ChatId(*admin_id).into(),
            })
            .await
        {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось обновить список admin slash-команд бота"
            );
        }
    }

    let state = bot::handlers::BotState {
        config,
        db,
        telemt_backend,
        telemt_runtime,
        bot_username,
    };
    monitor::spawn_monitor(bot.clone(), state.clone());
    tracing::info!("Dispatcher initialized, bot is ready");

    let state_for_cleanup = state.clone();
    let mut dispatcher = Dispatcher::builder(bot, bot::handlers::schema())
        .dependencies(dptree::deps![state])
        .build();

    let shutdown_token = dispatcher.shutdown_token().clone();
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("Received Ctrl-C, shutting down...");
                let _ = shutdown_token.shutdown();
            }
            Err(err) => {
                tracing::error!("Failed to listen for ctrl-c: {}", err);
            }
        }
    });

    dispatcher.dispatch().await;

    state_for_cleanup.db.pool.close().await;
    tracing::info!("Database pool closed");

    Ok(())
}
