use std::sync::Arc;

use anyhow::anyhow;
use reqwest::{Method, StatusCode};

use crate::config::TelemtApiConfig;
use crate::link::build_proxy_link;
use crate::runtime::TelemtRuntime;
use crate::telemt_cfg::TelemtConfig;

use super::api_client::TelemtApiClient;
use super::api_dto::{
    ApiUserInfo, CreateUserRequest, CreateUserResponse, HealthData, MeSelftestTop,
    RuntimeConnectionsSummaryTop, RuntimeEventsData, RuntimeGatesData, RuntimeInitializationData,
    SecurityPostureData, StatsSummaryData, SystemInfoData, UpstreamQualityTop, UserInfo,
};
use super::legacy::LegacyTelemtBackend;
use super::mappers::{map_api_user_info, map_connection_top_user, pick_best_link};
use super::types::{
    DeleteUserResult, ProvisionedUser, TelemtApiError, TelemtBackendMode, TelemtConnectionsSummary,
    TelemtMonitorSnapshot, TelemtRuntimeEvent, TelemtRuntimeSnapshot, TelemtStatsSummary,
    TelemtUserInfo, TelemtUserPatch,
};

pub(crate) struct ApiTelemtBackend {
    client: TelemtApiClient,
    allow_file_fallback: bool,
    prefer_api_links: bool,
    telemt_cfg: Arc<TelemtConfig>,
    legacy_fallback: Option<LegacyTelemtBackend>,
}

impl ApiTelemtBackend {
    pub(crate) fn new(
        api_cfg: &TelemtApiConfig,
        telemt_cfg: Arc<TelemtConfig>,
        telemt_runtime: TelemtRuntime,
    ) -> Result<Self, anyhow::Error> {
        let legacy_fallback = api_cfg
            .allow_file_fallback
            .then(|| LegacyTelemtBackend::new(telemt_cfg.clone(), telemt_runtime));

        Ok(Self {
            client: TelemtApiClient::new(api_cfg)?,
            allow_file_fallback: api_cfg.allow_file_fallback,
            prefer_api_links: api_cfg.prefer_api_links,
            telemt_cfg,
            legacy_fallback,
        })
    }

    pub(crate) async fn provision_user(
        &self,
        username: &str,
        desired_secret: &str,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        let body = CreateUserRequest {
            username,
            secret: Some(desired_secret),
        };
        let response = self
            .client
            .mutate_with_retry::<CreateUserRequest<'_>, CreateUserResponse>(
                Method::POST,
                "/v1/users",
                Some(&body),
            )
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                if let Some(legacy) = &self.legacy_fallback {
                    tracing::warn!(
                        username = username,
                        error = %error,
                        "telemt API provision failed; falling back to legacy backend"
                    );
                    return legacy.provision_user(username, desired_secret).await;
                }
                return Err(error);
            }
        };
        let CreateUserResponse { user, secret } = response.data;
        let link = if self.prefer_api_links {
            match pick_best_link(&user.links) {
                Some(link) => Some(link),
                None => self.try_build_fallback_link(Some(&secret)).await.ok(),
            }
        } else {
            match self.try_build_fallback_link(Some(&secret)).await {
                Ok(link) => Some(link),
                Err(_) => pick_best_link(&user.links),
            }
        };
        Ok(ProvisionedUser {
            secret,
            link,
            mode: TelemtBackendMode::ControlApi,
            revision: Some(response.revision),
        })
    }

    pub(crate) async fn delete_user(
        &self,
        username: &str,
    ) -> Result<DeleteUserResult, anyhow::Error> {
        let path = format!("/v1/users/{}", username);
        match self
            .client
            .mutate_with_retry::<(), serde_json::Value>(Method::DELETE, &path, None)
            .await
        {
            Ok(response) => Ok(DeleteUserResult {
                removed: true,
                mode: TelemtBackendMode::ControlApi,
                revision: Some(response.revision),
            }),
            Err(error) => {
                if let Some(api_error) = error.downcast_ref::<TelemtApiError>()
                    && api_error.status == StatusCode::NOT_FOUND
                {
                    return Ok(DeleteUserResult {
                        removed: false,
                        mode: TelemtBackendMode::ControlApi,
                        revision: self.client.cached_revision().await,
                    });
                }
                if let Some(legacy) = &self.legacy_fallback {
                    tracing::warn!(
                        username = username,
                        error = %error,
                        "telemt API delete failed; falling back to legacy backend"
                    );
                    return legacy.delete_user(username).await;
                }
                Err(error)
            }
        }
    }

    pub(crate) async fn build_user_link(
        &self,
        username: &str,
        cached_secret: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        if self.prefer_api_links {
            match self.fetch_user_link(username).await {
                Ok(Some(link)) => return Ok(link),
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        username = username,
                        error = %error,
                        "telemt API link lookup failed; trying fallback"
                    );
                }
            }
            return self.try_build_fallback_link(cached_secret).await;
        }

        match self.try_build_fallback_link(cached_secret).await {
            Ok(link) => return Ok(link),
            Err(error) => {
                tracing::warn!(
                    username = username,
                    error = %error,
                    "Local link build failed; trying telemt API lookup"
                );
            }
        }
        self.fetch_user_link(username)
            .await?
            .ok_or_else(|| anyhow!("Не удалось получить ссылку пользователя из telemt API"))
    }

    pub(crate) async fn get_user_info(
        &self,
        username: &str,
        cached_secret: Option<&str>,
    ) -> Result<Option<TelemtUserInfo>, anyhow::Error> {
        let path = format!("/v1/users/{username}");
        match self.client.get_success::<ApiUserInfo>(&path).await {
            Ok(response) => Ok(Some(map_api_user_info(
                TelemtBackendMode::ControlApi,
                response.data,
            ))),
            Err(error) => {
                if let Some(api_error) = error.downcast_ref::<TelemtApiError>()
                    && api_error.status == StatusCode::NOT_FOUND
                {
                    return Ok(None);
                }
                if let Some(legacy) = &self.legacy_fallback {
                    tracing::warn!(
                        username = username,
                        error = %error,
                        "telemt API user lookup failed; falling back to legacy link data"
                    );
                    return legacy.get_user_info(cached_secret).await;
                }
                Err(error)
            }
        }
    }

    pub(crate) async fn patch_user(
        &self,
        username: &str,
        patch: &TelemtUserPatch,
    ) -> Result<TelemtUserInfo, anyhow::Error> {
        if patch.is_empty() {
            return Err(anyhow!("Не задано ни одно изменение лимитов"));
        }
        let path = format!("/v1/users/{username}");
        let response = self
            .client
            .mutate_with_retry::<TelemtUserPatch, ApiUserInfo>(Method::PATCH, &path, Some(patch))
            .await?;
        Ok(map_api_user_info(
            TelemtBackendMode::ControlApi,
            response.data,
        ))
    }

    pub(crate) async fn stats_summary(&self) -> Result<TelemtStatsSummary, anyhow::Error> {
        let response = self
            .client
            .get_success::<StatsSummaryData>("/v1/stats/summary")
            .await?;
        Ok(TelemtStatsSummary {
            uptime_seconds: response.data.uptime_seconds,
            connections_total: response.data.connections_total,
            connections_bad_total: response.data.connections_bad_total,
            handshake_timeouts_total: response.data.handshake_timeouts_total,
            configured_users: response.data.configured_users,
        })
    }

    pub(crate) async fn connections_summary(
        &self,
        limit: usize,
    ) -> Result<Option<TelemtConnectionsSummary>, anyhow::Error> {
        let path = format!(
            "/v1/runtime/connections/summary?limit={}",
            limit.clamp(1, 50)
        );
        let response = self
            .client
            .get_success::<RuntimeConnectionsSummaryTop>(&path)
            .await?;
        let Some(data) = response.data.data else {
            return Ok(None);
        };
        Ok(Some(TelemtConnectionsSummary {
            current_connections: data.totals.current_connections,
            current_connections_me: data.totals.current_connections_me,
            current_connections_direct: data.totals.current_connections_direct,
            active_users: data.totals.active_users,
            top_by_connections: data
                .top
                .by_connections
                .into_iter()
                .map(map_connection_top_user)
                .collect(),
            top_by_throughput: data
                .top
                .by_throughput
                .into_iter()
                .map(map_connection_top_user)
                .collect(),
        }))
    }

    pub(crate) async fn monitor_snapshot(&self) -> Result<TelemtMonitorSnapshot, anyhow::Error> {
        let health = self.client.get_success::<HealthData>("/v1/health").await?;
        let (gates, me_selftest, upstream) = self.fetch_runtime_aux().await;

        Ok(TelemtMonitorSnapshot {
            health_status: health.data.status,
            accepting_new_connections: gates.as_ref().map(|g| g.accepting_new_connections),
            me_runtime_ready: gates.as_ref().map(|g| g.me_runtime_ready),
            upstream_unhealthy_total: upstream
                .as_ref()
                .and_then(|u| u.summary.as_ref().map(|s| s.unhealthy_total)),
            me_selftest_kdf_state: me_selftest
                .as_ref()
                .and_then(|m| m.data.as_ref().map(|p| p.kdf.state.clone())),
            me_selftest_timeskew_state: me_selftest
                .as_ref()
                .and_then(|m| m.data.as_ref().map(|p| p.timeskew.state.clone())),
        })
    }

    pub(crate) async fn runtime_snapshot(
        &self,
        recent_events_limit: usize,
    ) -> Result<TelemtRuntimeSnapshot, anyhow::Error> {
        let health = self.client.get_success::<HealthData>("/v1/health").await?;
        let system_info = self
            .client
            .get_success::<SystemInfoData>("/v1/system/info")
            .await?;
        let runtime_init = self
            .client
            .get_success::<RuntimeInitializationData>("/v1/runtime/initialization")
            .await?;
        let security = self
            .client
            .get_success::<SecurityPostureData>("/v1/security/posture")
            .await?;
        let HealthData {
            status: health_status,
            read_only: api_read_only,
        } = health.data;
        let SystemInfoData {
            version: build_version,
        } = system_info.data;
        let RuntimeInitializationData {
            status: startup_status,
            current_stage: startup_stage,
            progress_pct: startup_progress_pct,
            transport_mode,
        } = runtime_init.data;
        let SecurityPostureData {
            api_whitelist_enabled,
            api_whitelist_entries,
            api_auth_header_enabled,
        } = security.data;
        let events_path = format!(
            "/v1/runtime/events/recent?limit={}",
            recent_events_limit.max(1)
        );
        let events = self
            .client
            .get_success::<RuntimeEventsData>(&events_path)
            .await
            .ok();
        let (gates, me_selftest, upstream) = self.fetch_runtime_aux().await;

        let (
            accepting_new_connections,
            me_runtime_ready,
            use_middle_proxy,
            route_mode,
        ) = match &gates {
            Some(g) => (
                Some(g.accepting_new_connections),
                Some(g.me_runtime_ready),
                Some(g.use_middle_proxy),
                Some(g.route_mode.clone()),
            ),
            None => (None, None, None, None),
        };

        let (me_selftest_enabled, me_selftest_kdf_state, me_selftest_timeskew_state) =
            match &me_selftest {
                Some(m) => {
                    let kdf = m.data.as_ref().map(|p| p.kdf.state.clone());
                    let timeskew = m.data.as_ref().map(|p| p.timeskew.state.clone());
                    (Some(m.enabled), kdf, timeskew)
                }
                None => (None, None, None),
            };

        let (upstream_configured_total, upstream_healthy_total, upstream_unhealthy_total) =
            match &upstream {
                Some(u) if u.enabled => match &u.summary {
                    Some(s) => (
                        Some(s.configured_total),
                        Some(s.healthy_total),
                        Some(s.unhealthy_total),
                    ),
                    None => (None, None, None),
                },
                _ => (None, None, None),
            };

        let event_rows = events
            .map(|payload| {
                payload
                    .data
                    .data
                    .map(|payload| {
                        payload
                            .events
                            .into_iter()
                            .map(|event| TelemtRuntimeEvent {
                                ts_epoch_secs: event.ts_epoch_secs,
                                event_type: event.event_type,
                                context: event.context,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let revision = self.client.cached_revision().await;

        Ok(TelemtRuntimeSnapshot {
            source: TelemtBackendMode::ControlApi,
            health_status,
            api_read_only,
            build_version: Some(build_version),
            transport_mode: Some(transport_mode),
            startup_status: Some(startup_status),
            startup_stage: Some(startup_stage),
            startup_progress_pct: Some(startup_progress_pct),
            api_whitelist_enabled: Some(api_whitelist_enabled),
            api_whitelist_entries: Some(api_whitelist_entries),
            api_auth_header_enabled: Some(api_auth_header_enabled),
            accepting_new_connections,
            me_runtime_ready,
            use_middle_proxy,
            route_mode,
            me_selftest_kdf_state,
            me_selftest_timeskew_state,
            me_selftest_enabled,
            upstream_configured_total,
            upstream_healthy_total,
            upstream_unhealthy_total,
            events: event_rows,
            last_revision: revision,
        })
    }

    async fn fetch_user_link(&self, username: &str) -> Result<Option<String>, anyhow::Error> {
        let path = format!("/v1/users/{}", username);
        let response = self.client.get_success::<UserInfo>(&path).await?;
        Ok(pick_best_link(&response.data.links))
    }

    async fn try_build_fallback_link(
        &self,
        cached_secret: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        if !self.allow_file_fallback {
            return Err(anyhow!("Fallback на локальный telemt.toml отключён"));
        }
        let secret =
            cached_secret.ok_or_else(|| anyhow!("Не найден локальный секрет пользователя"))?;
        let params = self.telemt_cfg.clone().read_link_params_offloaded().await?;
        build_proxy_link(&params, secret).map_err(anyhow::Error::from)
    }

    async fn fetch_runtime_aux(
        &self,
    ) -> (
        Option<RuntimeGatesData>,
        Option<MeSelftestTop>,
        Option<UpstreamQualityTop>,
    ) {
        let gates = match self
            .client
            .get_success::<RuntimeGatesData>("/v1/runtime/gates")
            .await
        {
            Ok(envelope) => Some(envelope.data),
            Err(error) => {
                tracing::debug!(error = %error, "telemt API /v1/runtime/gates недоступен");
                None
            }
        };
        let me_selftest = match self
            .client
            .get_success::<MeSelftestTop>("/v1/runtime/me-selftest")
            .await
        {
            Ok(envelope) => Some(envelope.data),
            Err(error) => {
                tracing::debug!(error = %error, "telemt API /v1/runtime/me-selftest недоступен");
                None
            }
        };
        let upstream = match self
            .client
            .get_success::<UpstreamQualityTop>("/v1/runtime/upstream_quality")
            .await
        {
            Ok(envelope) => Some(envelope.data),
            Err(error) => {
                tracing::debug!(error = %error, "telemt API /v1/runtime/upstream_quality недоступен");
                None
            }
        };

        (gates, me_selftest, upstream)
    }
}
