use std::time::Duration;

use anyhow::Context;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, IF_MATCH};
use reqwest::{Client, Method, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::config::TelemtApiConfig;

use super::api_dto::{ErrorEnvelope, HealthData, SuccessEnvelope};
use super::types::TelemtApiError;

pub(crate) struct TelemtApiClient {
    client: Client,
    base_url: String,
    auth_header: Option<HeaderValue>,
    revision: Mutex<Option<String>>,
}

impl TelemtApiClient {
    pub(crate) fn new(api_cfg: &TelemtApiConfig) -> Result<Self, anyhow::Error> {
        let timeout = Duration::from_millis(api_cfg.timeout_ms.max(1));
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .context("Не удалось создать HTTP-клиент telemt API")?;
        let auth_header = api_cfg
            .auth_header
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(HeaderValue::from_str)
            .transpose()
            .context("telemt_api.auth_header содержит невалидные символы")?;

        Ok(Self {
            client,
            base_url: api_cfg.base_url.trim_end_matches('/').to_string(),
            auth_header,
            revision: Mutex::new(None),
        })
    }

    pub(crate) async fn get_success<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<SuccessEnvelope<T>, anyhow::Error> {
        let request = self
            .client
            .request(Method::GET, self.endpoint(path))
            .headers(self.base_headers());
        let response = request
            .send()
            .await
            .with_context(|| format!("Не удалось выполнить GET {}", path))?;
        self.parse_success_response(response).await
    }

    pub(crate) async fn mutate_with_retry<TBody: Serialize + ?Sized, TData: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&TBody>,
    ) -> Result<SuccessEnvelope<TData>, anyhow::Error> {
        let mut retried = false;
        loop {
            if self.revision.lock().await.is_none() {
                let revision = self.refresh_revision().await?;
                *self.revision.lock().await = Some(revision);
            }
            let revision = self.revision.lock().await.clone();
            let mut request = self
                .client
                .request(method.clone(), self.endpoint(path))
                .headers(self.base_headers());
            if let Some(revision) = revision.as_deref() {
                request = request.header(IF_MATCH, revision);
            }
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request
                .send()
                .await
                .with_context(|| format!("Не удалось выполнить {} {}", method, path))?;
            match self.parse_success_response(response).await {
                Ok(success) => return Ok(success),
                Err(error) => {
                    let needs_retry = error
                        .downcast_ref::<TelemtApiError>()
                        .map(|api_error| {
                            api_error.status == StatusCode::CONFLICT
                                && api_error.code == Some("revision_conflict".to_string())
                        })
                        .unwrap_or(false);
                    if needs_retry && !retried {
                        retried = true;
                        let revision = self.refresh_revision().await?;
                        *self.revision.lock().await = Some(revision);
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    pub(crate) async fn cached_revision(&self) -> Option<String> {
        self.revision.lock().await.clone()
    }

    async fn refresh_revision(&self) -> Result<String, anyhow::Error> {
        let health = self.get_success::<HealthData>("/v1/health").await?;
        Ok(health.revision)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn base_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(value) = self.auth_header.clone() {
            headers.insert(AUTHORIZATION, value);
        }
        headers
    }

    async fn parse_success_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<SuccessEnvelope<T>, anyhow::Error> {
        let status = response.status();
        let body = response
            .bytes()
            .await
            .context("Не удалось прочитать ответ telemt API")?;
        if status.is_success() {
            let envelope: SuccessEnvelope<T> =
                serde_json::from_slice(&body).with_context(|| {
                    format!(
                        "Не удалось декодировать успешный ответ: {}",
                        String::from_utf8_lossy(&body)
                    )
                })?;
            *self.revision.lock().await = Some(envelope.revision.clone());
            return Ok(envelope);
        }

        let parsed_error = serde_json::from_slice::<ErrorEnvelope>(&body).ok();
        let message = parsed_error
            .as_ref()
            .map(|error| error.error.message.clone())
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        let code = parsed_error.as_ref().map(|error| error.error.code.clone());
        Err(TelemtApiError {
            status,
            code,
            message,
        }
        .into())
    }
}
