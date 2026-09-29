//! HTTP API и статический фронтенд Mini App.
//!
//! Каждый запрос к `/api/*` авторизуется подписанным `initData` в заголовке
//! `Authorization: tma <initData>`; права администратора — те же `admin_ids`, что у бота.
//! Доменные операции переиспользуются из `bot::handlers`, чтобы бот и приложение
//! вели себя одинаково.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use teloxide::prelude::*;

use super::AppState;
use super::auth::{WebAppUser, validate_init_data};
use crate::bot::handlers::{
    approve_request_and_build_link, build_bot_start_link, build_user_qr_png_bytes,
    perform_hard_ban, telemt_username,
};
use crate::db::{InviteToken, RegistrationRequest, RequestStatus};
use crate::telemt_backend::TelemtUserActivity;
use std::collections::HashMap;

const INDEX_HTML: &str = include_str!("assets/index.html");
const APP_JS: &str = include_str!("assets/app.js");
const APP_CSS: &str = include_str!("assets/app.css");

/// Размер страницы списков в приложении.
const PAGE_SIZE: i64 = 20;
/// Максимум результатов поиска пользователей.
const SEARCH_LIMIT: i64 = 30;
/// Максимум строк в списке «онлайн сейчас».
const ONLINE_LIMIT: usize = 200;
/// Максимальный размер тела запроса.
const BODY_LIMIT_BYTES: usize = 8 * 1024;

const CSP: &str = "default-src 'self'; script-src 'self' https://telegram.org; \
style-src 'self'; img-src 'self' blob:; connect-src 'self'; base-uri 'none'; object-src 'none'; \
form-action 'none'; frame-ancestors https://web.telegram.org https://*.telegram.org";

pub fn router(app: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/app.css", get(app_css))
        .route("/healthz", get(healthz))
        .route("/api/me", get(me))
        .route("/api/me/qr", get(me_qr))
        .route("/api/admin/summary", get(admin_summary))
        .route("/api/admin/requests", get(admin_requests))
        .route("/api/admin/requests/{id}/approve", post(admin_approve))
        .route("/api/admin/requests/{id}/reject", post(admin_reject))
        .route("/api/admin/users", get(admin_users))
        .route(
            "/api/admin/users/{tg_user_id}",
            get(admin_user).delete(admin_delete_user),
        )
        .route(
            "/api/admin/tokens",
            get(admin_tokens).post(admin_create_token),
        )
        .route("/api/admin/tokens/{id}/revoke", post(admin_revoke_token))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(BODY_LIMIT_BYTES))
        .layer(axum::middleware::map_response(security_headers))
        .with_state(app)
}

/// Заголовки для всех ответов, включая ошибки экстракторов axum: ответы API содержат
/// ссылки с секретами и не должны кешироваться.
async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    headers
        .entry(header::X_CONTENT_TYPE_OPTIONS)
        .or_insert(HeaderValue::from_static("nosniff"));
    headers
        .entry(header::REFERRER_POLICY)
        .or_insert(HeaderValue::from_static("no-referrer"));
    response
}

// ---------- ошибки и авторизация ----------

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        tracing::warn!(
            error = format!("{error:#}"),
            "Mini App: ошибка обработки запроса"
        );
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Внутренняя ошибка, попробуйте позже",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

/// Возвращает `initData` из заголовка `Authorization: tma <initData>`.
fn init_data_from_header(value: &str) -> Option<&str> {
    let (scheme, data) = value.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("tma")
        .then_some(data.trim())
        .filter(|data| !data.is_empty())
}

fn authenticate(app: &AppState, headers: &HeaderMap) -> Result<WebAppUser, ApiError> {
    let header_value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(init_data_from_header)
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "Откройте приложение из Telegram")
        })?;
    let now = chrono::Utc::now().timestamp();
    validate_init_data(
        header_value,
        &app.bot_token,
        now,
        app.state.config.webapp.init_data_max_age_secs,
    )
    .map_err(|error| {
        tracing::info!(error = %error, "Mini App: initData отклонён");
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "Сессия недействительна, откройте приложение заново",
        )
    })
}

fn authenticate_admin(app: &AppState, headers: &HeaderMap) -> Result<WebAppUser, ApiError> {
    let user = authenticate(app, headers)?;
    if app.state.config.is_admin(user.id) {
        Ok(user)
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Доступно только администраторам",
        ))
    }
}

/// Нормализует номер страницы (с 0) и возвращает `(limit, offset)`.
fn page_window(page: Option<i64>) -> (i64, i64) {
    let page = page.unwrap_or(0).clamp(0, 10_000);
    (PAGE_SIZE, page * PAGE_SIZE)
}

// ---------- статика ----------

fn asset(content_type: &'static str, body: &'static str) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    response
}

async fn index() -> Response {
    asset("text/html; charset=utf-8", INDEX_HTML)
}

async fn app_js() -> Response {
    asset("text/javascript; charset=utf-8", APP_JS)
}

async fn app_css() -> Response {
    asset("text/css; charset=utf-8", APP_CSS)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "Не найдено")
}

// ---------- DTO ----------

#[derive(Serialize)]
struct Links {
    native: String,
    web: Option<String>,
}

#[derive(Serialize)]
struct MeResponse {
    id: i64,
    name: Option<String>,
    username: Option<String>,
    is_admin: bool,
    /// `approved` | `pending` | `rejected` | `deleted` | `none`
    access: &'static str,
    links: Option<Links>,
    links_error: Option<String>,
    bot_username: Option<String>,
    web_proxy_host: Option<String>,
}

/// Текущая активность пользователя в telemt (без IP-адресов).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
struct Activity {
    connections: u64,
    active_ips: usize,
    total_octets: u64,
}

impl From<&TelemtUserActivity> for Activity {
    fn from(a: &TelemtUserActivity) -> Self {
        Self {
            connections: a.current_connections,
            active_ips: a.active_unique_ips,
            total_octets: a.total_octets,
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
struct OnlineSummary {
    active_users: usize,
    current_connections: u64,
    active_ips: usize,
}

/// Итоги «сейчас онлайн» по списку активности.
fn online_summary(activity: &[TelemtUserActivity]) -> OnlineSummary {
    let online = activity.iter().filter(|a| a.current_connections > 0);
    OnlineSummary {
        active_users: online.clone().count(),
        current_connections: online.clone().map(|a| a.current_connections).sum(),
        active_ips: online.map(|a| a.active_unique_ips).sum(),
    }
}

/// Пользователи с открытыми соединениями: больше соединений — выше, затем по трафику.
fn online_sorted(activity: &[TelemtUserActivity]) -> Vec<&TelemtUserActivity> {
    let mut online: Vec<_> = activity
        .iter()
        .filter(|a| a.current_connections > 0)
        .collect();
    online.sort_by(|a, b| {
        b.current_connections
            .cmp(&a.current_connections)
            .then(b.total_octets.cmp(&a.total_octets))
            .then(a.username.cmp(&b.username))
    });
    online
}

/// `tg_<id>` → `id`.
fn tg_id_from_telemt_username(username: &str) -> Option<i64> {
    let digits = username.strip_prefix("tg_")?;
    let canonical = !digits.is_empty()
        && !digits.starts_with('0')
        && digits.bytes().all(|b| b.is_ascii_digit());
    canonical.then(|| digits.parse().ok()).flatten()
}

#[derive(Serialize)]
struct UserItem {
    tg_user_id: i64,
    username: Option<String>,
    name: Option<String>,
    telemt_username: Option<String>,
    status: &'static str,
    created_at: i64,
    last_sync_error: Option<String>,
    activity: Option<Activity>,
}

impl From<&RegistrationRequest> for UserItem {
    fn from(request: &RegistrationRequest) -> Self {
        Self {
            tg_user_id: request.tg_user_id,
            username: request.tg_username.clone(),
            name: request.tg_display_name.clone(),
            telemt_username: request.telemt_username.clone(),
            status: status_str(request.status),
            created_at: request.created_at,
            last_sync_error: request.last_sync_error.clone(),
            activity: None,
        }
    }
}

#[derive(Serialize)]
struct RequestItem {
    id: i64,
    tg_user_id: i64,
    username: Option<String>,
    name: Option<String>,
    created_at: i64,
}

#[derive(Serialize)]
struct Page<T> {
    items: Vec<T>,
    total: i64,
    page: i64,
    page_size: i64,
    /// Список обрезан лимитом (поиск, «онлайн»): уточните запрос.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    truncated: bool,
}

#[derive(Serialize)]
struct UserCard {
    user: UserItem,
    links: Option<Links>,
    links_error: Option<String>,
}

#[derive(Serialize)]
struct TokenItem {
    id: i64,
    token: String,
    start_link: Option<String>,
    created_at: i64,
    expires_at: i64,
    auto_approve: bool,
    usage_count: i64,
    max_usage: Option<i64>,
}

#[derive(Serialize)]
struct TelemtSummary {
    uptime_seconds: f64,
    connections_total: u64,
    connections_bad_total: u64,
    handshake_timeouts_total: u64,
    configured_users: usize,
}

#[derive(Serialize)]
struct AdminSummary {
    online: Option<OnlineSummary>,
    users_total: i64,
    pending: i64,
    approved: i64,
    rejected: i64,
    deleted: i64,
    tokens_active: i64,
    backend_mode: String,
    telemt: Option<TelemtSummary>,
    telemt_error: Option<String>,
    web_proxy_host: Option<String>,
    allow_auto_approve_tokens: bool,
    default_token_days: i64,
    max_token_days: i64,
}

#[derive(Serialize)]
struct Done {
    ok: bool,
    message: String,
}

fn status_str(status: RequestStatus) -> &'static str {
    match status {
        RequestStatus::Pending => "pending",
        RequestStatus::Approved => "approved",
        RequestStatus::Rejected => "rejected",
        RequestStatus::Deleted => "deleted",
    }
}

fn token_item(app: &AppState, token: InviteToken) -> TokenItem {
    let start_link = app
        .state
        .bot_username
        .as_deref()
        .map(|bot| build_bot_start_link(bot, &token.token));
    TokenItem {
        id: token.id,
        token: token.token,
        start_link,
        created_at: token.created_at,
        expires_at: token.expires_at,
        auto_approve: token.auto_approve,
        usage_count: token.usage_count,
        max_usage: token.max_usage,
    }
}

/// Список активности: `Ok(None)` — не поддерживается (legacy-режим), `Err` — telemt API недоступен.
async fn activity_list(app: &AppState) -> Result<Option<Vec<TelemtUserActivity>>, ApiError> {
    app.state
        .telemt_backend
        .user_activity()
        .await
        .map_err(|error| {
            tracing::warn!(
                error = format!("{error:#}"),
                "Mini App: активность пользователей недоступна"
            );
            ApiError::new(StatusCode::BAD_GATEWAY, "telemt API недоступен")
        })
}

/// Активность по имени пользователя telemt; `None`, если данных нет (legacy или API недоступен).
async fn activity_map(app: &AppState) -> Option<HashMap<String, TelemtUserActivity>> {
    activity_list(app)
        .await
        .ok()
        .flatten()
        .map(|list| list.into_iter().map(|a| (a.username.clone(), a)).collect())
}

/// Добавляет активность; если пользователя нет в telemt, `activity` остаётся `None`.
fn with_activity(
    mut item: UserItem,
    activity: Option<&HashMap<String, TelemtUserActivity>>,
) -> UserItem {
    if let (Some(map), Some(name)) = (activity, item.telemt_username.as_deref()) {
        item.activity = map.get(name).map(Activity::from);
    }
    item
}

async fn user_links(
    app: &AppState,
    telemt_user: &str,
    secret: Option<&str>,
) -> Result<Links, String> {
    match app
        .state
        .telemt_backend
        .build_user_link(telemt_user, secret)
        .await
    {
        Ok(native) => Ok(Links {
            web: app.state.config.web_proxy_link(&native),
            native,
        }),
        Err(error) => {
            tracing::warn!(telemt_user = %telemt_user, error = %error, "Mini App: не удалось получить ссылку");
            Err("Не удалось получить ссылку, попробуйте позже".to_string())
        }
    }
}

// ---------- пользователь ----------

async fn me(State(app): State<AppState>, headers: HeaderMap) -> ApiResult<MeResponse> {
    let user = authenticate(&app, &headers)?;
    let request = app
        .state
        .db
        .get_request_by_tg_user(user.id)
        .await
        .map_err(ApiError::internal)?;
    let access = request.as_ref().map_or("none", |r| status_str(r.status));

    let (links, links_error) = match app
        .state
        .db
        .get_approved(user.id)
        .await
        .map_err(ApiError::internal)?
    {
        Some((telemt_user, secret)) => {
            let secret = (!secret.is_empty()).then_some(secret.as_str());
            match user_links(&app, &telemt_user, secret).await {
                Ok(links) => (Some(links), None),
                Err(error) => (None, Some(error)),
            }
        }
        None => (None, None),
    };

    Ok(Json(MeResponse {
        id: user.id,
        name: user.display_name(),
        username: user.username.clone(),
        is_admin: app.state.config.is_admin(user.id),
        access,
        links,
        links_error,
        bot_username: app.state.bot_username.clone(),
        web_proxy_host: app.state.config.web_proxy.normalized_host(),
    }))
}

#[derive(Deserialize)]
struct QrQuery {
    kind: Option<String>,
}

async fn me_qr(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<QrQuery>,
) -> Result<Response, ApiError> {
    let user = authenticate(&app, &headers)?;
    let Some((telemt_user, secret)) = app
        .state
        .db
        .get_approved(user.id)
        .await
        .map_err(ApiError::internal)?
    else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Доступ не открыт"));
    };
    let secret = (!secret.is_empty()).then_some(secret.as_str());
    let links = user_links(&app, &telemt_user, secret)
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error))?;
    let payload = match query.kind.as_deref() {
        Some("web") => links
            .web
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "WEB-ссылка недоступна"))?,
        None | Some("native") => links.native,
        Some(_) => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Неизвестный тип ссылки",
            ));
        }
    };
    let png = build_user_qr_png_bytes(&payload).map_err(ApiError::internal)?;
    let mut response = (StatusCode::OK, png).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    Ok(response)
}

// ---------- администратор ----------

async fn admin_summary(State(app): State<AppState>, headers: HeaderMap) -> ApiResult<AdminSummary> {
    authenticate_admin(&app, &headers)?;
    let stats = app
        .state
        .db
        .admin_stats()
        .await
        .map_err(ApiError::internal)?;
    let (telemt, telemt_error) = match app.state.telemt_backend.stats_summary().await {
        Ok(summary) => (
            summary.map(|s| TelemtSummary {
                uptime_seconds: s.uptime_seconds,
                connections_total: s.connections_total,
                connections_bad_total: s.connections_bad_total,
                handshake_timeouts_total: s.handshake_timeouts_total,
                configured_users: s.configured_users,
            }),
            None,
        ),
        Err(error) => {
            tracing::warn!(error = %error, "Mini App: telemt stats недоступны");
            (None, Some("telemt API недоступен".to_string()))
        }
    };
    let online = activity_list(&app)
        .await
        .ok()
        .flatten()
        .map(|list| online_summary(&list));
    let security = &app.state.config.security;
    Ok(Json(AdminSummary {
        online,
        users_total: stats.total,
        pending: stats.pending,
        approved: stats.approved,
        rejected: stats.rejected,
        deleted: stats.deleted,
        tokens_active: stats.tokens_active,
        backend_mode: format!("{:?}", app.state.telemt_backend.mode()),
        telemt,
        telemt_error,
        web_proxy_host: app.state.config.web_proxy.normalized_host(),
        allow_auto_approve_tokens: security.allow_auto_approve_tokens,
        default_token_days: security.default_token_days,
        max_token_days: security.max_token_days,
    }))
}

#[derive(Deserialize)]
struct PageQuery {
    page: Option<i64>,
    q: Option<String>,
    online: Option<bool>,
}

async fn admin_requests(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> ApiResult<Page<RequestItem>> {
    authenticate_admin(&app, &headers)?;
    let (limit, offset) = page_window(query.page);
    let db = &app.state.db;
    let total = db
        .count_pending_requests()
        .await
        .map_err(ApiError::internal)?;
    let items = db
        .list_pending_requests_page(limit, offset)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|r| RequestItem {
            id: r.id,
            tg_user_id: r.tg_user_id,
            username: r.tg_username,
            name: r.tg_display_name,
            created_at: r.created_at,
        })
        .collect();
    Ok(Json(Page {
        items,
        total,
        page: offset / limit,
        page_size: limit,
        truncated: false,
    }))
}

async fn admin_approve(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Done> {
    let admin = authenticate_admin(&app, &headers)?;
    let Some((request, link)) = approve_request_and_build_link(&app.state, id)
        .await
        .map_err(ApiError::internal)?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Заявка не найдена или уже обработана",
        ));
    };
    let text = app.state.config.user_link_message(&link);
    let message = match app.bot.send_message(ChatId(request.tg_user_id), text).await {
        Ok(_) => "Заявка одобрена, ссылка отправлена пользователю",
        Err(error) => {
            tracing::warn!(tg_user_id = request.tg_user_id, error = %error, "Mini App: не удалось отправить ссылку пользователю");
            "Заявка одобрена, но отправить ссылку пользователю не удалось — выдайте её из карточки пользователя"
        }
    };
    tracing::info!(
        admin_id = admin.id,
        request_id = id,
        "Mini App: заявка одобрена"
    );
    Ok(Json(Done {
        ok: true,
        message: message.to_string(),
    }))
}

async fn admin_reject(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Done> {
    let admin = authenticate_admin(&app, &headers)?;
    let Some(request) = app.state.db.reject(id).await.map_err(ApiError::internal)? else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Заявка не найдена или уже обработана",
        ));
    };
    let text = app
        .state
        .config
        .bot_messages
        .request_rejected_or_default()
        .to_string();
    if let Err(error) = app.bot.send_message(ChatId(request.tg_user_id), text).await {
        tracing::warn!(tg_user_id = request.tg_user_id, error = %error, "Mini App: не удалось уведомить пользователя");
    }
    tracing::info!(
        admin_id = admin.id,
        request_id = id,
        "Mini App: заявка отклонена"
    );
    Ok(Json(Done {
        ok: true,
        message: "Заявка отклонена".to_string(),
    }))
}

async fn admin_users(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> ApiResult<Page<UserItem>> {
    authenticate_admin(&app, &headers)?;
    let db = &app.state.db;

    if query.online.unwrap_or(false) {
        let Some(list) = activity_list(&app).await? else {
            return Err(ApiError::new(
                StatusCode::NOT_IMPLEMENTED,
                "Активность доступна только в режиме control API telemt",
            ));
        };
        let online = online_sorted(&list);
        let truncated = online.len() > ONLINE_LIMIT;
        let total = online.len() as i64;
        let mut items = Vec::new();
        for entry in online.into_iter().take(ONLINE_LIMIT) {
            let request = match tg_id_from_telemt_username(&entry.username) {
                Some(id) => db
                    .get_active_user_by_tg_user(id)
                    .await
                    .map_err(ApiError::internal)?,
                None => None,
            };
            let mut item = match request {
                Some(request) => UserItem::from(&request),
                // Есть в telemt, но нет среди активных в БД бота (служебный или рассинхрон).
                None => UserItem {
                    tg_user_id: tg_id_from_telemt_username(&entry.username).unwrap_or(0),
                    username: None,
                    name: None,
                    telemt_username: Some(entry.username.clone()),
                    status: "unknown",
                    created_at: 0,
                    last_sync_error: None,
                    activity: None,
                },
            };
            item.activity = Some(Activity::from(entry));
            items.push(item);
        }
        return Ok(Json(Page {
            total,
            items,
            page: 0,
            page_size: ONLINE_LIMIT as i64,
            truncated,
        }));
    }

    let activity = activity_map(&app).await;
    if let Some(needle) = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let found = db
            .search_active_users_by_partial(needle, SEARCH_LIMIT)
            .await
            .map_err(ApiError::internal)?;
        let items: Vec<UserItem> = found
            .iter()
            .map(|r| with_activity(UserItem::from(r), activity.as_ref()))
            .collect();
        return Ok(Json(Page {
            total: items.len() as i64,
            truncated: items.len() as i64 >= SEARCH_LIMIT,
            items,
            page: 0,
            page_size: SEARCH_LIMIT,
        }));
    }
    let (limit, offset) = page_window(query.page);
    let total = db.count_active_users().await.map_err(ApiError::internal)?;
    let items = db
        .list_active_users_page(limit, offset)
        .await
        .map_err(ApiError::internal)?
        .iter()
        .map(|r| with_activity(UserItem::from(r), activity.as_ref()))
        .collect();
    Ok(Json(Page {
        items,
        total,
        page: offset / limit,
        page_size: limit,
        truncated: false,
    }))
}

async fn admin_user(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(tg_user_id): Path<i64>,
) -> ApiResult<UserCard> {
    authenticate_admin(&app, &headers)?;
    let Some(request) = app
        .state
        .db
        .get_active_user_by_tg_user(tg_user_id)
        .await
        .map_err(ApiError::internal)?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Пользователь не найден",
        ));
    };
    let telemt_user = request
        .telemt_username
        .clone()
        .unwrap_or_else(|| telemt_username(tg_user_id));
    let secret = request.secret.as_deref().filter(|s| !s.is_empty());
    let (links, links_error) = match user_links(&app, &telemt_user, secret).await {
        Ok(links) => (Some(links), None),
        Err(error) => (None, Some(error)),
    };
    let activity = activity_map(&app).await;
    Ok(Json(UserCard {
        user: with_activity(UserItem::from(&request), activity.as_ref()),
        links,
        links_error,
    }))
}

async fn admin_delete_user(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(tg_user_id): Path<i64>,
) -> ApiResult<Done> {
    let admin = authenticate_admin(&app, &headers)?;
    if admin.id == tg_user_id {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Нельзя удалить себя из приложения",
        ));
    }
    if app
        .state
        .db
        .get_active_user_by_tg_user(tg_user_id)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Пользователь не найден",
        ));
    }
    let message = perform_hard_ban(&app.state, tg_user_id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(
        admin_id = admin.id,
        tg_user_id,
        "Mini App: пользователь удалён"
    );
    Ok(Json(Done { ok: true, message }))
}

async fn admin_tokens(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> ApiResult<Page<TokenItem>> {
    authenticate_admin(&app, &headers)?;
    let (limit, offset) = page_window(query.page);
    let db = &app.state.db;
    let total = db
        .count_active_invite_tokens()
        .await
        .map_err(ApiError::internal)?;
    let items = db
        .list_active_invite_tokens_page(limit, offset)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|token| token_item(&app, token))
        .collect();
    Ok(Json(Page {
        items,
        total,
        page: offset / limit,
        page_size: limit,
        truncated: false,
    }))
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    days: Option<i64>,
    #[serde(default)]
    auto_approve: bool,
    max_usage: Option<i64>,
}

/// Проверяет параметры нового токена по политике `[security]`.
fn validate_token_request(
    request: &CreateTokenRequest,
    default_days: i64,
    max_days: i64,
    allow_auto_approve: bool,
) -> Result<(i64, Option<i64>), String> {
    let days = request.days.unwrap_or(default_days);
    if !(1..=max_days).contains(&days) {
        return Err(format!("Срок действия — от 1 до {max_days} дней"));
    }
    if request.auto_approve && !allow_auto_approve {
        return Err("Токены с авто-одобрением запрещены настройками".to_string());
    }
    let max_usage = match request.max_usage {
        Some(n) if !(1..=100_000).contains(&n) => {
            return Err("Лимит активаций — от 1 до 100000".to_string());
        }
        other => other,
    };
    Ok((days, max_usage))
}

async fn admin_create_token(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<TokenItem> {
    // Авторизация до разбора тела: неаутентифицированный клиент не получает ошибок serde.
    let admin = authenticate_admin(&app, &headers)?;
    let request: CreateTokenRequest = serde_json::from_slice(&body)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Некорректные параметры токена"))?;
    let security = &app.state.config.security;
    let (days, max_usage) = validate_token_request(
        &request,
        security.default_token_days,
        security.max_token_days,
        security.allow_auto_approve_tokens,
    )
    .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error))?;
    let token = app
        .state
        .db
        .create_invite_token(days, request.auto_approve, max_usage, Some(admin.id))
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(
        admin_id = admin.id,
        token_id = token.id,
        days,
        auto_approve = request.auto_approve,
        "Mini App: создан invite-токен"
    );
    Ok(Json(token_item(&app, token)))
}

async fn admin_revoke_token(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Done> {
    let admin = authenticate_admin(&app, &headers)?;
    let revoked = app
        .state
        .db
        .revoke_invite_token_by_id(id)
        .await
        .map_err(ApiError::internal)?;
    if !revoked {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Токен не найден или уже неактивен",
        ));
    }
    tracing::info!(
        admin_id = admin.id,
        token_id = id,
        "Mini App: invite-токен отозван"
    );
    Ok(Json(Done {
        ok: true,
        message: "Токен отозван".to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        Activity, CreateTokenRequest, OnlineSummary, init_data_from_header, online_sorted,
        online_summary, page_window, tg_id_from_telemt_username, validate_token_request,
    };
    use crate::telemt_backend::TelemtUserActivity;

    fn act(name: &str, conns: u64, ips: usize, octets: u64) -> TelemtUserActivity {
        TelemtUserActivity {
            username: name.to_string(),
            current_connections: conns,
            active_unique_ips: ips,
            total_octets: octets,
        }
    }

    #[test]
    fn online_summary_counts_only_connected_users() {
        let list = [
            act("tg_1", 3, 1, 10),
            act("tg_2", 0, 0, 999),
            act("tg_3", 2, 2, 5),
        ];

        assert_eq!(
            online_summary(&list),
            OnlineSummary {
                active_users: 2,
                current_connections: 5,
                active_ips: 3
            }
        );
        assert_eq!(
            online_summary(&[]),
            OnlineSummary {
                active_users: 0,
                current_connections: 0,
                active_ips: 0
            }
        );
    }

    #[test]
    fn online_sorted_orders_by_connections_then_traffic() {
        let list = [
            act("tg_1", 1, 1, 50),
            act("tg_2", 0, 0, 999),
            act("tg_3", 4, 1, 1),
            act("tg_4", 1, 1, 70),
        ];
        let names: Vec<&str> = online_sorted(&list)
            .iter()
            .map(|a| a.username.as_str())
            .collect();

        assert_eq!(names, ["tg_3", "tg_4", "tg_1"]);
        assert_eq!(
            Activity::from(&list[2]),
            Activity {
                connections: 4,
                active_ips: 1,
                total_octets: 1
            }
        );
    }

    #[test]
    fn tg_id_from_telemt_username_parses_only_tg_prefix() {
        assert_eq!(tg_id_from_telemt_username("tg_123"), Some(123));
        assert_eq!(tg_id_from_telemt_username("web_pilot"), None);
        assert_eq!(tg_id_from_telemt_username("tg_"), None);
        assert_eq!(tg_id_from_telemt_username("tg_-5"), None);
        assert_eq!(tg_id_from_telemt_username("tg_+5"), None);
        assert_eq!(tg_id_from_telemt_username("tg_007"), None);
    }

    #[test]
    fn init_data_from_header_requires_tma_scheme() {
        assert_eq!(
            init_data_from_header("tma query_id=1&hash=x"),
            Some("query_id=1&hash=x")
        );
        assert_eq!(init_data_from_header("  TMA   a=b "), Some("a=b"));
        assert_eq!(init_data_from_header("Bearer a=b"), None);
        assert_eq!(init_data_from_header("tma "), None);
        assert_eq!(init_data_from_header("tma"), None);
    }

    #[test]
    fn page_window_clamps_page() {
        assert_eq!(page_window(None), (20, 0));
        assert_eq!(page_window(Some(2)), (20, 40));
        assert_eq!(page_window(Some(-5)), (20, 0));
    }

    fn request(
        days: Option<i64>,
        auto_approve: bool,
        max_usage: Option<i64>,
    ) -> CreateTokenRequest {
        CreateTokenRequest {
            days,
            auto_approve,
            max_usage,
        }
    }

    #[test]
    fn validate_token_request_applies_security_policy() {
        assert_eq!(
            validate_token_request(&request(None, false, None), 14, 180, true),
            Ok((14, None))
        );
        assert_eq!(
            validate_token_request(&request(Some(30), true, Some(5)), 14, 180, true),
            Ok((30, Some(5)))
        );
        assert!(validate_token_request(&request(Some(0), false, None), 14, 180, true).is_err());
        assert!(validate_token_request(&request(Some(181), false, None), 14, 180, true).is_err());
        assert!(validate_token_request(&request(None, true, None), 14, 180, false).is_err());
        assert!(validate_token_request(&request(None, false, Some(0)), 14, 180, true).is_err());
    }
}
