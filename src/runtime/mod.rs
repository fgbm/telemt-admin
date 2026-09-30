//! Универсальный слой управления процессом `telemt`: systemd, внешний supervisor или без host-управления.

pub mod types;

pub use types::{RuntimeCapabilities, ServiceEvents, ServiceResult, ServiceSummary};

use crate::service::ServiceController;
use serde::Deserialize;

/// Режим интеграции с ОС/supervisor (см. `[runtime]` в конфиге).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeMode {
    Systemd,
    External,
    None,
}

impl RuntimeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Systemd => "systemd",
            Self::External => "external",
            Self::None => "none",
        }
    }
}

/// Единая точка для `systemctl`/`journalctl`, API-only UI и legacy reload.
#[derive(Debug, Clone)]
pub struct TelemtRuntime {
    inner: TelemtRuntimeInner,
}

#[derive(Debug, Clone)]
enum TelemtRuntimeInner {
    Systemd(ServiceController),
    External { label: String },
    None,
}

impl TelemtRuntime {
    /// Создание из эффективных параметров после загрузки конфига.
    pub fn new(mode: RuntimeMode, systemd_unit: String, external_label: Option<String>) -> Self {
        let inner = match mode {
            RuntimeMode::Systemd => {
                TelemtRuntimeInner::Systemd(ServiceController::new(systemd_unit))
            }
            RuntimeMode::External => TelemtRuntimeInner::External {
                label: external_label
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "внешний supervisor".to_string()),
            },
            RuntimeMode::None => TelemtRuntimeInner::None,
        };
        Self { inner }
    }

    pub fn capabilities(&self) -> RuntimeCapabilities {
        match &self.inner {
            TelemtRuntimeInner::Systemd(_) => RuntimeCapabilities {
                shows_systemd_unit: true,
                shows_journal_tail: true,
                can_start: true,
                can_stop: true,
                can_restart: true,
                can_reload_config: true,
            },
            TelemtRuntimeInner::External { .. } | TelemtRuntimeInner::None => RuntimeCapabilities {
                shows_systemd_unit: false,
                shows_journal_tail: false,
                can_start: false,
                can_stop: false,
                can_restart: false,
                can_reload_config: false,
            },
        }
    }

    /// Имя unit или метка для экрана статуса.
    pub fn display_label(&self) -> String {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.service_name().to_string(),
            TelemtRuntimeInner::External { label } => label.clone(),
            TelemtRuntimeInner::None => "telemt (без host-управления)".to_string(),
        }
    }

    pub async fn notify_config_reloaded(&self) -> ServiceResult {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.notify_config_reloaded().await,
            TelemtRuntimeInner::External { .. } | TelemtRuntimeInner::None => ServiceResult {
                success: false,
                stderr: "В режиме runtime external/none нельзя перечитать конфиг через systemd; \
                          перезапустите telemt вручную (контейнер/supervisor)."
                    .to_string(),
            },
        }
    }

    pub async fn start(&self) -> ServiceResult {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.start().await,
            TelemtRuntimeInner::External { .. } | TelemtRuntimeInner::None => {
                unsupported_action("start")
            }
        }
    }

    pub async fn stop(&self) -> ServiceResult {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.stop().await,
            TelemtRuntimeInner::External { .. } | TelemtRuntimeInner::None => {
                unsupported_action("stop")
            }
        }
    }

    pub async fn restart(&self) -> ServiceResult {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.restart().await,
            TelemtRuntimeInner::External { .. } | TelemtRuntimeInner::None => {
                unsupported_action("restart")
            }
        }
    }

    pub async fn reload(&self) -> ServiceResult {
        self.notify_config_reloaded().await
    }

    pub async fn status(&self) -> ServiceResult {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.status().await,
            TelemtRuntimeInner::External { .. } | TelemtRuntimeInner::None => {
                unsupported_action("status")
            }
        }
    }

    pub async fn summary(&self) -> ServiceSummary {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.summary().await,
            TelemtRuntimeInner::External { label } => ServiceSummary {
                success: true,
                active_state: "external".to_string(),
                sub_state: label.clone(),
                unit_file_state: "—".to_string(),
                main_pid: None,
                exec_main_status: None,
                error: None,
            },
            TelemtRuntimeInner::None => ServiceSummary {
                success: true,
                active_state: "n/a".to_string(),
                sub_state: "none".to_string(),
                unit_file_state: "—".to_string(),
                main_pid: None,
                exec_main_status: None,
                error: None,
            },
        }
    }

    pub async fn recent_events(&self, limit: usize) -> ServiceEvents {
        match &self.inner {
            TelemtRuntimeInner::Systemd(s) => s.recent_events(limit).await,
            TelemtRuntimeInner::External { .. } => ServiceEvents {
                success: true,
                lines: vec![
                    "Журнал systemd недоступен: процесс telemt управляется внешне.".to_string(),
                ],
                error: None,
            },
            TelemtRuntimeInner::None => ServiceEvents {
                success: true,
                lines: vec!["Режим runtime=none: журнал unit не используется.".to_string()],
                error: None,
            },
        }
    }
}

fn unsupported_action(action: &str) -> ServiceResult {
    ServiceResult {
        success: false,
        stderr: format!(
            "Действие «{action}» недоступно в текущем режиме runtime (используйте control API и внешний supervisor)."
        ),
    }
}
