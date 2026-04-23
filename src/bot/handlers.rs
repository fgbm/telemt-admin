//! Обработчики команд пользователя и админа.

#[path = "handlers/actions/mod.rs"]
mod actions;
#[path = "handlers/callback_data.rs"]
pub mod callback_data;
#[path = "handlers/callbacks/mod.rs"]
mod callbacks;
#[path = "handlers/commands/mod.rs"]
mod commands;
#[path = "handlers/format.rs"]
mod format;
#[path = "handlers/menu.rs"]
mod menu;
#[path = "handlers/screens/mod.rs"]
mod screens;
#[path = "handlers/shared.rs"]
mod shared;
#[path = "handlers/state.rs"]
mod state;

pub use state::BotState;

use teloxide::dispatching::DpHandlerDescription;
use teloxide::dptree;
use teloxide::prelude::*;

pub fn public_telegram_commands() -> Vec<teloxide::types::BotCommand> {
    commands::public_telegram_commands()
}

pub fn admin_telegram_commands() -> Vec<teloxide::types::BotCommand> {
    commands::admin_telegram_commands()
}

pub fn schema() -> dptree::Handler<
    'static,
    Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>,
    DpHandlerDescription,
> {
    let message_handler = Update::filter_message()
        .branch(commands::handler())
        .endpoint(menu::handle_menu_buttons);

    dptree::entry()
        .branch(message_handler)
        .branch(callbacks::handler())
}
