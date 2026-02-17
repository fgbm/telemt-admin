//! Обработчики команд пользователя и админа.

#[path = "handlers/callbacks/mod.rs"]
mod callbacks;
#[path = "handlers/commands/mod.rs"]
mod commands;
#[path = "handlers/format.rs"]
mod format;
#[path = "handlers/menu.rs"]
mod menu;
#[path = "handlers/shared.rs"]
mod shared;
#[path = "handlers/state.rs"]
mod state;

pub use state::BotState;

use teloxide::dispatching::DpHandlerDescription;
use teloxide::dptree;
use teloxide::prelude::*;

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
