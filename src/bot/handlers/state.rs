use crate::config::Config;
use crate::db::Db;
use crate::service::ServiceController;
use crate::telemt_cfg::TelemtConfig;
use std::collections::HashSet;
use std::sync::Arc;
use teloxide::types::Message;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct BotState {
    pub config: Arc<Config>,
    pub db: Arc<Db>,
    pub telemt_cfg: Arc<TelemtConfig>,
    pub service: ServiceController,
    pub bot_username: Option<String>,
    pub awaiting_invite_users: Arc<Mutex<HashSet<i64>>>,
}

pub fn telemt_username(tg_user_id: i64) -> String {
    format!("tg_{}", tg_user_id)
}

pub fn sender_user_id(msg: &Message) -> Option<i64> {
    msg.from.as_ref().map(|user| user.id.0 as i64)
}

pub fn sender_display_name(msg: &Message) -> Option<String> {
    msg.from.as_ref().map(|user| {
        let mut full_name = user.first_name.clone();
        if let Some(last_name) = user.last_name.as_deref()
            && !last_name.trim().is_empty()
        {
            full_name.push(' ');
            full_name.push_str(last_name);
        }
        full_name
    })
}

pub fn is_admin_message(msg: &Message, state: &BotState) -> bool {
    sender_user_id(msg).is_some_and(|user_id| state.config.is_admin(user_id))
}
