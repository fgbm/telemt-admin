use std::sync::Arc;

use anyhow::anyhow;

use crate::link::build_proxy_link;
use crate::runtime::TelemtRuntime;
use crate::telemt_cfg::TelemtConfig;

use super::types::{DeleteUserResult, ProvisionedUser, TelemtBackendMode, TelemtUserInfo};

#[derive(Clone)]
pub(crate) struct LegacyTelemtBackend {
    telemt_cfg: Arc<TelemtConfig>,
    telemt_runtime: TelemtRuntime,
}

impl LegacyTelemtBackend {
    pub(crate) fn new(telemt_cfg: Arc<TelemtConfig>, telemt_runtime: TelemtRuntime) -> Self {
        Self {
            telemt_cfg,
            telemt_runtime,
        }
    }

    pub(crate) async fn provision_user(
        &self,
        username: &str,
        desired_secret: &str,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        self.telemt_cfg
            .clone()
            .upsert_user_offloaded(username.to_string(), desired_secret.to_string())
            .await?;
        let reload = self.telemt_runtime.notify_config_reloaded().await;
        if !reload.success {
            tracing::warn!(stderr = %reload.stderr, "telemt config reload/restart had issues");
        }
        let link = self.build_user_link(Some(desired_secret)).await?;
        Ok(ProvisionedUser {
            secret: desired_secret.to_string(),
            link: Some(link),
            mode: TelemtBackendMode::LegacyFile,
            revision: None,
        })
    }

    pub(crate) async fn delete_user(
        &self,
        username: &str,
    ) -> Result<DeleteUserResult, anyhow::Error> {
        let removed = self
            .telemt_cfg
            .clone()
            .remove_user_offloaded(username.to_string())
            .await?;
        if removed {
            let reload = self.telemt_runtime.notify_config_reloaded().await;
            if !reload.success {
                tracing::warn!(stderr = %reload.stderr, "telemt config reload/restart had issues");
            }
        }
        Ok(DeleteUserResult {
            removed,
            mode: TelemtBackendMode::LegacyFile,
            revision: None,
        })
    }

    pub(crate) async fn build_user_link(
        &self,
        cached_secret: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        let secret =
            cached_secret.ok_or_else(|| anyhow!("Не найден локальный секрет пользователя"))?;
        let params = self.telemt_cfg.clone().read_link_params_offloaded().await?;
        build_proxy_link(&params, secret).map_err(anyhow::Error::from)
    }

    pub(crate) async fn get_user_info(
        &self,
        cached_secret: Option<&str>,
    ) -> Result<Option<TelemtUserInfo>, anyhow::Error> {
        let link = self.build_user_link(cached_secret).await?;
        Ok(Some(TelemtUserInfo {
            source: TelemtBackendMode::LegacyFile,
            user_ad_tag: None,
            max_tcp_conns: None,
            expiration_rfc3339: None,
            data_quota_bytes: None,
            max_unique_ips: None,
            current_connections: None,
            active_unique_ips: None,
            active_unique_ips_list: Vec::new(),
            recent_unique_ips: None,
            recent_unique_ips_list: Vec::new(),
            total_octets: None,
            links: vec![link],
        }))
    }
}
