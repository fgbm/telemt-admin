use crate::bot::handlers::BotState;
use crate::telemt_backend::TelemtMonitorSnapshot;
use teloxide::prelude::{Bot, ChatId, Requester};
use tokio::time::{Duration, MissedTickBehavior};

#[derive(Debug, Default, Clone)]
struct MonitorState {
    api_available: Option<bool>,
    health_status: Option<String>,
    accepting_new_connections: Option<bool>,
    me_runtime_ready: Option<bool>,
    upstream_unhealthy_total: Option<u64>,
    kdf_state: Option<String>,
    timeskew_state: Option<String>,
    last_notification_text: Option<String>,
    last_notification_at: Option<tokio::time::Instant>,
}

#[derive(Debug, Clone)]
struct MonitorNotification {
    text: String,
}

pub fn spawn_monitor(bot: Bot, state: BotState) {
    if !state.config.notifications.enabled || !state.config.telemt_api.enabled {
        tracing::info!("Фоновый монитор telemt отключён конфигом");
        return;
    }

    tokio::spawn(async move {
        let interval_secs = state.config.notifications.health_check_interval_secs.max(1);
        tracing::info!(
            interval_secs = interval_secs,
            "Запускаю фоновый монитор telemt"
        );
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut monitor_state = MonitorState::default();

        loop {
            ticker.tick().await;
            if let Err(error) = poll_once(&bot, &state, &mut monitor_state).await {
                tracing::warn!(error = %error, "Ошибка фонового мониторинга telemt");
            }
        }
    });
}

async fn poll_once(
    bot: &Bot,
    state: &BotState,
    monitor_state: &mut MonitorState,
) -> Result<(), anyhow::Error> {
    match state.telemt_backend.monitor_snapshot().await {
        Ok(Some(snapshot)) => {
            tracing::debug!(
                health_status = %snapshot.health_status,
                accepting_new_connections = ?snapshot.accepting_new_connections,
                me_runtime_ready = ?snapshot.me_runtime_ready,
                upstream_unhealthy_total = ?snapshot.upstream_unhealthy_total,
                me_selftest_kdf_state = ?snapshot.me_selftest_kdf_state,
                me_selftest_timeskew_state = ?snapshot.me_selftest_timeskew_state,
                "Снимок monitor telemt обновлён"
            );
            handle_snapshot(bot, state, monitor_state, snapshot).await?;
        }
        Ok(None) => {
            tracing::debug!("monitor_snapshot недоступен для текущего backend");
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                api_available = false,
                previous_api_available = ?monitor_state.api_available,
                "Не удалось получить monitor snapshot telemt"
            );
            let notifications = plan_error_notifications(
                monitor_state,
                state.config.notifications.notify_on_health_change,
                &error.to_string(),
            );
            monitor_state.api_available = Some(false);
            deliver_notifications(bot, state, monitor_state, notifications).await;
        }
    }
    Ok(())
}

async fn handle_snapshot(
    bot: &Bot,
    state: &BotState,
    monitor_state: &mut MonitorState,
    snapshot: TelemtMonitorSnapshot,
) -> Result<(), anyhow::Error> {
    let notifications = plan_snapshot_notifications(
        monitor_state,
        &snapshot,
        state.config.notifications.notify_on_health_change,
        state.config.notifications.notify_on_runtime_alerts,
    );
    apply_snapshot_state(monitor_state, snapshot);
    deliver_notifications(bot, state, monitor_state, notifications).await;
    Ok(())
}

fn plan_error_notifications(
    monitor_state: &MonitorState,
    notify_on_health_change: bool,
    error: &str,
) -> Vec<MonitorNotification> {
    if notify_on_health_change && monitor_state.api_available != Some(false) {
        return vec![MonitorNotification {
            text: format!("🚨 telemt control API недоступен\n\nПричина: {}", error),
        }];
    }
    Vec::new()
}

fn plan_snapshot_notifications(
    monitor_state: &MonitorState,
    snapshot: &TelemtMonitorSnapshot,
    notify_on_health_change: bool,
    notify_on_runtime_alerts: bool,
) -> Vec<MonitorNotification> {
    let mut notifications = Vec::new();

    if monitor_state.api_available == Some(false) && notify_on_health_change {
        notifications.push(MonitorNotification {
            text: "✅ telemt control API снова доступен".to_string(),
        });
    }

    if notify_on_health_change {
        if monitor_state.health_status.as_deref() != Some(snapshot.health_status.as_str())
            && let Some(previous) = monitor_state.health_status.as_deref()
        {
            notifications.push(MonitorNotification {
                text: format!(
                    "ℹ️ telemt health изменился\n\nБыло: {}\nСтало: {}",
                    previous, snapshot.health_status
                ),
            });
        }
        if monitor_state.accepting_new_connections == Some(true)
            && snapshot.accepting_new_connections == Some(false)
        {
            notifications.push(MonitorNotification {
                text: "🚨 telemt перестал принимать новые соединения".to_string(),
            });
        } else if monitor_state.accepting_new_connections == Some(false)
            && snapshot.accepting_new_connections == Some(true)
        {
            notifications.push(MonitorNotification {
                text: "✅ telemt снова принимает новые соединения".to_string(),
            });
        }
        if monitor_state.me_runtime_ready == Some(true) && snapshot.me_runtime_ready == Some(false) {
            notifications.push(MonitorNotification {
                text: "🚨 ME runtime больше не готов".to_string(),
            });
        } else if monitor_state.me_runtime_ready == Some(false)
            && snapshot.me_runtime_ready == Some(true)
        {
            notifications.push(MonitorNotification {
                text: "✅ ME runtime снова готов".to_string(),
            });
        }
    }

    if notify_on_runtime_alerts {
        let prev_upstream_bad = monitor_state.upstream_unhealthy_total.unwrap_or(0);
        let current_upstream_bad = snapshot.upstream_unhealthy_total.unwrap_or(0);
        if prev_upstream_bad == 0 && current_upstream_bad > 0 {
            notifications.push(MonitorNotification {
                text: format!(
                    "🚨 В telemt появились unhealthy upstream\n\nКоличество: {}",
                    current_upstream_bad
                ),
            });
        } else if prev_upstream_bad > 0 && current_upstream_bad == 0 {
            notifications.push(MonitorNotification {
                text: "✅ Все upstream снова healthy".to_string(),
            });
        }

        notifications.extend(plan_state_recovery(
            monitor_state.kdf_state.as_deref(),
            snapshot.me_selftest_kdf_state.as_deref(),
            "ME self-test KDF",
        ));
        notifications.extend(plan_state_recovery(
            monitor_state.timeskew_state.as_deref(),
            snapshot.me_selftest_timeskew_state.as_deref(),
            "ME self-test time skew",
        ));
    }

    notifications
}

fn plan_state_recovery(
    previous: Option<&str>,
    current: Option<&str>,
    title: &str,
) -> Vec<MonitorNotification> {
    let prev_bad = previous.is_some_and(|value| value != "ok");
    let curr_bad = current.is_some_and(|value| value != "ok");
    if !prev_bad && curr_bad {
        return vec![MonitorNotification {
            text: format!(
                "🚨 {} сообщает о проблеме\n\nТекущее состояние: {}",
                title,
                current.unwrap_or("unknown")
            ),
        }];
    }
    if prev_bad && !curr_bad {
        return vec![MonitorNotification {
            text: format!("✅ {} снова в состоянии ok", title),
        }];
    }
    Vec::new()
}

fn apply_snapshot_state(monitor_state: &mut MonitorState, snapshot: TelemtMonitorSnapshot) {
    monitor_state.api_available = Some(true);
    monitor_state.health_status = Some(snapshot.health_status);
    monitor_state.accepting_new_connections = snapshot.accepting_new_connections;
    monitor_state.me_runtime_ready = snapshot.me_runtime_ready;
    monitor_state.upstream_unhealthy_total = snapshot.upstream_unhealthy_total;
    monitor_state.kdf_state = snapshot.me_selftest_kdf_state;
    monitor_state.timeskew_state = snapshot.me_selftest_timeskew_state;
}

async fn deliver_notifications(
    bot: &Bot,
    state: &BotState,
    monitor_state: &mut MonitorState,
    notifications: Vec<MonitorNotification>,
) {
    for notification in notifications {
        if let Some(ref last_text) = monitor_state.last_notification_text
            && last_text == &notification.text
            && let Some(last_at) = monitor_state.last_notification_at
            && last_at.elapsed() < std::time::Duration::from_secs(300)
        {
            continue;
        }
        notify_admins(bot, state, notification.text.clone()).await;
        monitor_state.last_notification_text = Some(notification.text);
        monitor_state.last_notification_at = Some(tokio::time::Instant::now());
    }
}

async fn notify_admins(bot: &Bot, state: &BotState, text: String) {
    for admin_id in &state.config.admin_ids {
        if let Err(error) = bot.send_message(ChatId(*admin_id), text.clone()).await {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось отправить уведомление мониторинга"
            );
        }
    }
}
