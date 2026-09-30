//! Массовые операции по группам пользователей.

use crate::bot::handlers::actions::access::perform_hard_ban;
use crate::bot::handlers::state::BotState;
use crate::db::UserGroup;
use crate::telemt_backend::TelemtUserPatch;
use chrono::{TimeZone, Utc};

pub async fn deactivate_all_members(
    state: &BotState,
    group: &UserGroup,
) -> Result<(usize, usize), anyhow::Error> {
    let ids = state.db.list_group_member_tg_ids(group.id).await?;
    let mut ok = 0usize;
    let mut err = 0usize;
    for tg_user_id in ids {
        match perform_hard_ban(state, tg_user_id).await {
            Ok(_) => ok += 1,
            Err(e) => {
                err += 1;
                tracing::warn!(
                    group_id = group.id,
                    tg_user_id,
                    error = %e,
                    "Не удалось отключить пользователя группы"
                );
            }
        }
    }
    let _ = state.db.delete_user_group(group.id).await?;
    Ok((ok, err))
}

pub async fn apply_group_expiry_to_members(
    state: &BotState,
    group: &UserGroup,
) -> Result<(usize, usize), anyhow::Error> {
    let Some(exp_unix) = group.expires_at else {
        return Err(anyhow::anyhow!(
            "У группы не задан общий срок действия (expires_at). Задайте срок в карточке группы."
        ));
    };
    let dt = Utc
        .timestamp_opt(exp_unix, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Некорректное время expires_at у группы"))?;
    let rfc = dt.to_rfc3339();

    let ids = state.db.list_group_member_tg_ids(group.id).await?;
    let mut ok = 0usize;
    let mut err = 0usize;

    for tg_user_id in ids {
        let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
            err += 1;
            continue;
        };
        let Some(telemt_username) = user.telemt_username.as_deref() else {
            err += 1;
            continue;
        };
        let patch = TelemtUserPatch {
            expiration_rfc3339: Some(rfc.clone()),
            ..Default::default()
        };
        match state
            .telemt_backend
            .patch_user(telemt_username, &patch)
            .await
        {
            Ok(_) => ok += 1,
            Err(e) => {
                err += 1;
                tracing::warn!(
                    telemt_username,
                    error = %e,
                    "PATCH expiration для участника группы"
                );
            }
        }
    }

    Ok((ok, err))
}
