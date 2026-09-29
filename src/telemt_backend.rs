#[path = "telemt_backend/api_client.rs"]
mod api_client;
#[path = "telemt_backend/api_dto.rs"]
mod api_dto;
#[path = "telemt_backend/control_api.rs"]
mod control_api;
#[path = "telemt_backend/legacy.rs"]
mod legacy;
#[path = "telemt_backend/mappers.rs"]
mod mappers;
#[path = "telemt_backend/types.rs"]
mod types;

use std::sync::Arc;

use anyhow::anyhow;

use crate::config::TelemtApiConfig;
use crate::runtime::TelemtRuntime;
use crate::telemt_cfg::TelemtConfig;

use control_api::ApiTelemtBackend;
use legacy::LegacyTelemtBackend;

#[allow(unused_imports)]
pub use types::{
    DeleteUserResult, ProvisionedUser, TelemtApiError, TelemtBackendMode, TelemtConnectionTopUser,
    TelemtConnectionsSummary, TelemtMonitorSnapshot, TelemtRuntimeEvent,
    TelemtRuntimeSnapshot, TelemtStatsSummary, TelemtUserActivity, TelemtUserInfo,
    TelemtUserPatch,
};

#[derive(Clone)]
pub struct TelemtBackend {
    inner: Arc<TelemtBackendInner>,
}

enum TelemtBackendInner {
    Legacy(LegacyTelemtBackend),
    Api(ApiTelemtBackend),
}

impl TelemtBackend {
    pub fn new(
        api_cfg: &TelemtApiConfig,
        telemt_cfg: Arc<TelemtConfig>,
        telemt_runtime: TelemtRuntime,
    ) -> Result<Self, anyhow::Error> {
        let inner = if api_cfg.enabled {
            TelemtBackendInner::Api(ApiTelemtBackend::new(
                api_cfg,
                telemt_cfg,
                telemt_runtime,
            )?)
        } else {
            TelemtBackendInner::Legacy(LegacyTelemtBackend::new(telemt_cfg, telemt_runtime))
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub fn mode(&self) -> TelemtBackendMode {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => TelemtBackendMode::LegacyFile,
            TelemtBackendInner::Api(_) => TelemtBackendMode::ControlApi,
        }
    }

    pub async fn provision_user(
        &self,
        username: &str,
        desired_secret: &str,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => {
                legacy.provision_user(username, desired_secret).await
            }
            TelemtBackendInner::Api(api) => api.provision_user(username, desired_secret).await,
        }
    }

    pub async fn delete_user(&self, username: &str) -> Result<DeleteUserResult, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => legacy.delete_user(username).await,
            TelemtBackendInner::Api(api) => api.delete_user(username).await,
        }
    }

    pub async fn build_user_link(
        &self,
        username: &str,
        cached_secret: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => legacy.build_user_link(cached_secret).await,
            TelemtBackendInner::Api(api) => api.build_user_link(username, cached_secret).await,
        }
    }

    pub async fn get_user_info(
        &self,
        username: &str,
        cached_secret: Option<&str>,
    ) -> Result<Option<TelemtUserInfo>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => legacy.get_user_info(cached_secret).await,
            TelemtBackendInner::Api(api) => api.get_user_info(username, cached_secret).await,
        }
    }

    pub async fn patch_user(
        &self,
        username: &str,
        patch: &TelemtUserPatch,
    ) -> Result<TelemtUserInfo, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Err(anyhow!(
                "Изменение лимитов пользователя доступно только через telemt control API"
            )),
            TelemtBackendInner::Api(api) => api.patch_user(username, patch).await,
        }
    }

    pub async fn stats_summary(&self) -> Result<Option<TelemtStatsSummary>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => api.stats_summary().await.map(Some),
        }
    }

    pub async fn connections_summary(
        &self,
        limit: usize,
    ) -> Result<Option<TelemtConnectionsSummary>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => api.connections_summary(limit).await,
        }
    }

    /// Активность всех пользователей (только счётчики); `None` в legacy-режиме.
    pub async fn user_activity(&self) -> Result<Option<Vec<TelemtUserActivity>>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => api.user_activity().await.map(Some),
        }
    }

    pub async fn monitor_snapshot(&self) -> Result<Option<TelemtMonitorSnapshot>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => api.monitor_snapshot().await.map(Some),
        }
    }

    pub async fn runtime_snapshot(
        &self,
        recent_events_limit: usize,
    ) -> Result<Option<TelemtRuntimeSnapshot>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => match api.runtime_snapshot(recent_events_limit).await {
                Ok(snapshot) => Ok(Some(snapshot)),
                Err(error) => {
                    tracing::warn!(error = %error, "Не удалось получить runtime snapshot telemt API");
                    Ok(None)
                }
            },
        }
    }
}
