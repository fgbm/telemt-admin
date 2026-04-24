//! Конфигурация telemt-admin бота.

use serde::Deserialize;
use std::path::PathBuf;

use crate::runtime::RuntimeMode;

/// Лениво инициализированный Handlebars registry для рендеринга шаблонов.
fn handlebars_registry() -> handlebars::Handlebars<'static> {
    handlebars::Handlebars::new()
}

/// Путь к конфигу по умолчанию.
pub const DEFAULT_CONFIG_PATH: &str = "/etc/telemt-admin.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Токен Telegram бота (или через TELOXIDE_TOKEN)
    pub bot_token: Option<String>,
    /// Username бота без ведущего @ (для ссылок `t.me/...` и deep link), если `getMe` недоступен
    #[serde(default)]
    pub bot_username: Option<String>,
    /// Список Telegram user_id администраторов
    pub admin_ids: Vec<i64>,
    /// Путь к конфигу telemt (по умолчанию /etc/telemt.toml)
    #[serde(default = "default_telemt_config_path")]
    pub telemt_config_path: PathBuf,
    /// Путь к SQLite БД (по умолчанию /var/lib/telemt-admin/state.db)
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    /// Имя systemd-сервиса telemt
    #[serde(default = "default_service_name")]
    pub service_name: String,
    /// Размер страницы в списке активных пользователей
    #[serde(default = "default_users_page_size")]
    pub users_page_size: i64,
    /// Политики безопасности invite-токенов
    #[serde(default)]
    pub security: SecurityConfig,
    /// Настройки control API telemt
    #[serde(default)]
    pub telemt_api: TelemtApiConfig,
    /// Настройки уведомлений и фонового мониторинга
    #[serde(default)]
    pub notifications: NotificationsConfig,
    /// Переопределяемые тексты бота (см. дефолты в коде)
    #[serde(default)]
    pub bot_messages: BotMessages,
    /// Режим управления процессом telemt на хосте (systemd / внешний supervisor / без unit)
    #[serde(default)]
    pub runtime: Option<RuntimeSection>,
}

/// Тексты интерфейса; пустые/отсутствующие поля — поведение по умолчанию (как в коде до настройки).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BotMessages {
    /// При `/start` без токена: показать этот текст и не включать автоматически wizard ввода токена.
    #[serde(default)]
    pub start_without_invite: Option<String>,
    /// Сообщение при `/start` без токена и без `start_without_invite`: сразу запрос токена (wizard).
    #[serde(default)]
    pub invite_manual_prompt: Option<String>,
    /// Текст после нажатия «Ввести invite-токен» (callback).
    #[serde(default)]
    pub invite_followup_prompt: Option<String>,
    /// Шаблон сообщения со ссылкой пользователя. Поддерживает `{link}`.
    #[serde(default)]
    pub user_link_template: Option<String>,
    /// Шаблон сообщения после авто-одобрения. Поддерживает `{link}`.
    #[serde(default)]
    pub access_approved_template: Option<String>,
    /// Сообщение после отправки manual-заявки.
    #[serde(default)]
    pub request_submitted: Option<String>,
    /// Сообщение для уже ожидающей manual-заявки.
    #[serde(default)]
    pub request_pending: Option<String>,
    /// Сообщение для отклонённой заявки.
    #[serde(default)]
    pub request_rejected: Option<String>,
    /// Текст-подсказка перед рассылкой. Поддерживает `{audience}`.
    #[serde(default)]
    pub broadcast_prompt: Option<String>,
    /// Сообщение при отмене рассылки пустым текстом.
    #[serde(default)]
    pub broadcast_cancelled: Option<String>,
    /// Итог рассылки. Поддерживает `{ok}`, `{failed}`, `{total}`.
    #[serde(default)]
    pub broadcast_summary_template: Option<String>,
    /// Текст при обновлении статуса без доступа (нет заявки / не одобрено).
    #[serde(default)]
    pub no_access_status_text: Option<String>,
    /// Текст при запросе ссылки без доступа.
    #[serde(default)]
    pub no_access_link_text: Option<String>,
    /// Заголовок панели администратора.
    #[serde(default)]
    pub admin_home_text: Option<String>,
    /// Текст user home при статусе Approved. Поддерживает {link}.
    #[serde(default)]
    pub user_home_approved: Option<String>,
    /// Текст user home при статусе Pending.
    #[serde(default)]
    pub user_home_pending: Option<String>,
    /// Текст user home при статусе Rejected.
    #[serde(default)]
    pub user_home_rejected: Option<String>,
    /// Текст user home при статусе Deleted.
    #[serde(default)]
    pub user_home_deleted: Option<String>,
    /// Инструкция по подключению (usage guide).
    #[serde(default)]
    pub usage_guide_text: Option<String>,
    /// Текст команды /help для администратора.
    #[serde(default)]
    pub help_admin_text: Option<String>,
    /// Текст команды /help для обычного пользователя.
    #[serde(default)]
    pub help_user_text: Option<String>,
    /// Сообщение об ошибке при вводе неизвестной команды.
    #[serde(default)]
    pub unknown_command_text: Option<String>,
    /// Сообщение о недостаточных правах для deep link.
    #[serde(default)]
    pub admin_only_deep_link_text: Option<String>,
    /// Сообщение Пользователь не найден или уже неактивен.
    #[serde(default)]
    pub user_not_found_text: Option<String>,
    /// Сообщение Токен не найден или уже недоступен.
    #[serde(default)]
    pub token_not_found_text: Option<String>,
    /// Текст подтверждения действия над сервисом. Поддерживает {action}.
    #[serde(default)]
    pub service_action_confirm_template: Option<String>,
    /// Текст при ошибке токена: NotFound.
    #[serde(default)]
    pub token_error_not_found: Option<String>,
    /// Текст при ошибке токена: Revoked.
    #[serde(default)]
    pub token_error_revoked: Option<String>,
    /// Текст при ошибке токена: Expired.
    #[serde(default)]
    pub token_error_expired: Option<String>,
    /// Текст при ошибке токена: UsageLimitReached.
    #[serde(default)]
    pub token_error_usage_limit: Option<String>,
    /// Шаблон уведомления админам о новой заявке.
    #[serde(default)]
    pub admin_notify_new_request_template: Option<String>,
    /// Шаблон уведомления админам об автоподключении.
    #[serde(default)]
    pub admin_notify_auto_approve_template: Option<String>,
    /// Текст при отсутствии wizard-состояния (непонятный запрос).
    #[serde(default)]
    pub fallback_unknown_request_text: Option<String>,
}

impl BotMessages {
    fn non_empty(value: Option<&str>) -> Option<&str> {
        value.map(str::trim).filter(|s| !s.is_empty())
    }

    fn render_template(
        template: Option<&str>,
        default: &str,
        replacements: &[(&str, String)],
    ) -> String {
        let base = Self::non_empty(template).unwrap_or(default).to_string();
        // Legacy placeholder support: replace {key} before Handlebars.
        let mut pre_rendered = base.clone();
        for (key, value) in replacements {
            pre_rendered = pre_rendered.replace(&format!("{{{key}}}"), value);
        }
        // Handlebars for advanced templates (conditionals, loops).
        // Build JSON context from replacements so both {key} and {{key}} work.
        let mut ctx = serde_json::Map::new();
        for (key, value) in replacements {
            ctx.insert(key.to_string(), serde_json::Value::String(value.clone()));
        }
        match handlebars_registry().render_template(&base, &serde_json::Value::Object(ctx)) {
            Ok(hb_result) => {
                // If Handlebars produced the same result (no advanced syntax used),
                // prefer the legacy-pre_rendered to preserve any literal {{ }} in text.
                if hb_result == base {
                    pre_rendered
                } else {
                    hb_result
                }
            }
            Err(error) => {
                tracing::warn!(template = %base, error = %error, "Handlebars render failed, falling back to legacy replace");
                pre_rendered
            }
        }
    }

    pub fn invite_manual_prompt_or_default(&self) -> &str {
        const DEFAULT: &str = "Введите пригласительный токен следующим сообщением.\n\nЕсли передумали, нажмите «Отмена».";
        Self::non_empty(self.invite_manual_prompt.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn invite_followup_prompt_or_default(&self) -> &str {
        const DEFAULT: &str =
            "Отправьте invite-токен следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.";
        Self::non_empty(self.invite_followup_prompt.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn user_link_text(&self, link: &str) -> String {
        const DEFAULT: &str = "Ваша ссылка на прокси:\n\n{link}";
        Self::render_template(
            self.user_link_template.as_deref(),
            DEFAULT,
            &[("link", link.to_string())],
        )
    }

    pub fn access_approved_text(&self, link: &str) -> String {
        const DEFAULT: &str = "Доступ одобрен! Ваша ссылка для подключения:\n\n{link}";
        Self::render_template(
            self.access_approved_template.as_deref(),
            DEFAULT,
            &[("link", link.to_string())],
        )
    }

    pub fn request_submitted_or_default(&self) -> &str {
        const DEFAULT: &str = "Заявка отправлена. Ожидайте подтверждения.";
        Self::non_empty(self.request_submitted.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn request_pending_or_default(&self) -> &str {
        const DEFAULT: &str =
            "Ваша заявка уже на рассмотрении. Ожидайте подтверждения администратора.";
        Self::non_empty(self.request_pending.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn request_rejected_or_default(&self) -> &str {
        const DEFAULT: &str = "Ваша заявка на регистрацию отклонена администратором.";
        Self::non_empty(self.request_rejected.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn broadcast_prompt_text(&self, audience: &str) -> String {
        const DEFAULT: &str = "Рассылка {audience}.\n\nОтправьте текст одним сообщением.\n\nУчтите лимиты Telegram и то, что бот не может писать пользователям, которые ни разу не нажали /start.";
        Self::render_template(
            self.broadcast_prompt.as_deref(),
            DEFAULT,
            &[("audience", audience.to_string())],
        )
    }

    pub fn broadcast_cancelled_or_default(&self) -> &str {
        const DEFAULT: &str = "Рассылка отменена.";
        Self::non_empty(self.broadcast_cancelled.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn broadcast_summary_text(&self, ok: u64, failed: u64, total: u64) -> String {
        const DEFAULT: &str =
            "Рассылка завершена.\nУспешно: {ok}\nОшибок: {failed}\nВсего получателей в списке: {total}";
        Self::render_template(
            self.broadcast_summary_template.as_deref(),
            DEFAULT,
            &[
                ("ok", ok.to_string()),
                ("failed", failed.to_string()),
                ("total", total.to_string()),
            ],
        )
    }

    pub fn no_access_status_or_default(&self) -> &str {
        const DEFAULT: &str = "Чтобы получить доступ, отправьте /start и введите invite-токен.\n\nЕсли токен уже есть, нажмите кнопку ниже.";
        Self::non_empty(self.no_access_status_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn no_access_link_or_default(&self) -> &str {
        const DEFAULT: &str = "У вас нет доступа к прокси. Отправьте /start для регистрации.";
        Self::non_empty(self.no_access_link_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn admin_home_or_default(&self) -> &str {
        const DEFAULT: &str = "Панель администратора\n\nВыберите раздел ниже.";
        Self::non_empty(self.admin_home_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn user_home_approved_text(&self, link: &str) -> String {
        const DEFAULT: &str = "Доступ уже открыт.\n\nНажмите «Получить ссылку».";
        Self::render_template(self.user_home_approved.as_deref(), DEFAULT, &[("link", link.to_string())])
    }

    pub fn user_home_pending_or_default(&self) -> &str {
        const DEFAULT: &str = "Заявка уже на рассмотрении.\n\nДождитесь решения администратора.";
        Self::non_empty(self.user_home_pending.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn user_home_rejected_or_default(&self) -> &str {
        const DEFAULT: &str = "Заявка отклонена.\n\nЕсли есть новый invite-токен, отправьте /start и введите его заново.";
        Self::non_empty(self.user_home_rejected.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn user_home_deleted_or_default(&self) -> &str {
        const DEFAULT: &str = "Доступ был отозван.\n\nДля новой регистрации отправьте /start и введите invite-токен заново.";
        Self::non_empty(self.user_home_deleted.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn usage_guide_or_default(&self) -> &str {
        const DEFAULT: &str = r#"Как подключиться к прокси:

1) Получите ссылку через команду /link.
2) Нажмите на ссылку — Telegram автоматически предложит добавить прокси.
3) Подтвердите добавление.

Если доступа ещё нет, начните с /start и введите invite-токен.
Если не получается, обратитесь к администратору."#;
        Self::non_empty(self.usage_guide_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn help_admin_or_default(&self) -> &str {
        const DEFAULT: &str = r#"Команды:
/start — главный экран
/user — пользователи
/token — токены
/service — сервис
/link — моя ссылка
/help — справка

Основные действия выполняются внутри разделов через кнопки."#;
        Self::non_empty(self.help_admin_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn help_user_or_default(&self) -> &str {
        const DEFAULT: &str = r#"Команды:
/start — главный экран
/help — справка

Если доступа ещё нет, бот подскажет следующий шаг."#;
        Self::non_empty(self.help_user_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn unknown_command_or_default(&self) -> &str {
        const DEFAULT: &str = "Неизвестная команда. Используйте /help.";
        Self::non_empty(self.unknown_command_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn admin_only_deep_link_or_default(&self) -> &str {
        const DEFAULT: &str = "Этот deep link доступен только администраторам.";
        Self::non_empty(self.admin_only_deep_link_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn user_not_found_or_default(&self) -> &str {
        const DEFAULT: &str = "Пользователь не найден или уже неактивен.";
        Self::non_empty(self.user_not_found_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn token_not_found_or_default(&self) -> &str {
        const DEFAULT: &str = "Токен не найден или уже недоступен.";
        Self::non_empty(self.token_not_found_text.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn service_action_confirm_text(&self, action: &str) -> String {
        const DEFAULT: &str = "Подтвердить действие: {action}?\n\nЭто может временно прервать доступ пользователей.";
        Self::render_template(self.service_action_confirm_template.as_deref(), DEFAULT, &[("action", action.to_string())])
    }

    pub fn token_error_not_found_or_default(&self) -> &str {
        const DEFAULT: &str = "Токен не найден. Проверьте код и попробуйте снова.";
        Self::non_empty(self.token_error_not_found.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn token_error_revoked_or_default(&self) -> &str {
        const DEFAULT: &str = "Этот токен отозван администратором.";
        Self::non_empty(self.token_error_revoked.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn token_error_expired_or_default(&self) -> &str {
        const DEFAULT: &str = "Срок действия invite-токена (ссылки) истёк.";
        Self::non_empty(self.token_error_expired.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn token_error_usage_limit_or_default(&self) -> &str {
        const DEFAULT: &str = "Лимит активаций invite-токена исчерпан.";
        Self::non_empty(self.token_error_usage_limit.as_deref()).unwrap_or(DEFAULT)
    }

    pub fn admin_notify_new_request_text(&self, req: &crate::db::RegistrationRequest) -> String {
        const DEFAULT: &str = "📋 Новая заявка #{id}:\n User ID: {user_id}\n Username: @{username}\n Имя: {display_name}\n Время: {time}{invite_token_id}";
        let invite_line = req.invite_token_id.map(|id| format!("\n🎟 ID ссылки (invite): {}", id)).unwrap_or_default();
        Self::render_template(
            self.admin_notify_new_request_template.as_deref(),
            DEFAULT,
            &[
                ("id", req.id.to_string()),
                ("user_id", req.tg_user_id.to_string()),
                ("username", req.tg_username.as_deref().unwrap_or("—").to_string()),
                ("display_name", req.tg_display_name.as_deref().unwrap_or("—").to_string()),
                ("time", crate::bot::handlers::format::format_timestamp(req.created_at)),
                ("invite_token_id", invite_line),
            ],
        )
    }

    pub fn admin_notify_auto_approve_text(&self,
        tg_user_id: i64,
        tg_username: Option<&str>,
        tg_display_name: Option<&str>,
        token: &crate::db::ConsumedInviteToken,
    ) -> String {
        const DEFAULT: &str = "✅ Автоподключение по invite-токену\n User ID: {user_id}\n Username: @{username}\n Имя: {display_name}\n Invite token: {token}\n Token ID: {token_id}\n Mode: {mode}\n Срок действия ссылки (invite), не пользователя: {expires_at}\n Активаций по токену: {usage_count}/{max_usage}\n Created by: {created_by}";
        let mode_label = match token.mode {
            crate::db::TokenMode::AutoApprove => "auto",
            crate::db::TokenMode::Manual => "manual",
        };
        Self::render_template(
            self.admin_notify_auto_approve_template.as_deref(),
            DEFAULT,
            &[
                ("user_id", tg_user_id.to_string()),
                ("username", tg_username.unwrap_or("—").to_string()),
                ("display_name", tg_display_name.unwrap_or("—").to_string()),
                ("token", token.token.clone()),
                ("token_id", token.id.to_string()),
                ("mode", mode_label.to_string()),
                ("expires_at", crate::bot::handlers::format::format_timestamp(token.expires_at)),
                ("usage_count", token.usage_count.to_string()),
                ("max_usage", token.max_usage.map(|v| v.to_string()).unwrap_or_else(|| "∞".to_string())),
                ("created_by", token.created_by.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string())),
            ],
        )
    }

    pub fn fallback_unknown_request_or_default(&self) -> &str {
        const DEFAULT: &str = "Не понял запрос. Используйте /help или начните нужный сценарий через slash-команду либо кнопку.";
        Self::non_empty(self.fallback_unknown_request_text.as_deref()).unwrap_or(DEFAULT)
    }
}

/// Секция `[runtime]` в `telemt-admin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSection {
    #[serde(default = "default_runtime_mode")]
    pub mode: RuntimeMode,
    /// Имя systemd unit для `mode = "systemd"` (если не задано — используется верхнеуровневый `service_name`)
    #[serde(default)]
    pub service_name: Option<String>,
    /// Подпись для UI при `mode = "external"`
    #[serde(default)]
    pub label: Option<String>,
}

pub(crate) fn default_runtime_mode() -> RuntimeMode {
    RuntimeMode::Systemd
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_token_days")]
    pub default_token_days: i64,
    #[serde(default = "default_max_token_days")]
    pub max_token_days: i64,
    #[serde(default = "default_allow_auto_approve_tokens")]
    pub allow_auto_approve_tokens: bool,
    pub wizard_state_ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelemtApiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_telemt_api_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub auth_header: Option<String>,
    #[serde(default = "default_telemt_api_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_allow_file_fallback")]
    pub allow_file_fallback: bool,
    #[serde(default = "default_prefer_api_links")]
    pub prefer_api_links: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationsConfig {
    #[serde(default = "default_notifications_enabled")]
    pub enabled: bool,
    #[serde(default = "default_health_check_interval_secs")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_notify_on_health_change")]
    pub notify_on_health_change: bool,
    #[serde(default = "default_notify_on_runtime_alerts")]
    pub notify_on_runtime_alerts: bool,
    #[serde(default = "default_notify_on_new_request")]
    pub notify_on_new_request: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            default_token_days: default_token_days(),
            max_token_days: default_max_token_days(),
            allow_auto_approve_tokens: default_allow_auto_approve_tokens(),
            wizard_state_ttl_seconds: None,
        }
    }
}

impl Default for TelemtApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_telemt_api_base_url(),
            auth_header: None,
            timeout_ms: default_telemt_api_timeout_ms(),
            allow_file_fallback: default_allow_file_fallback(),
            prefer_api_links: default_prefer_api_links(),
        }
    }
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: default_notifications_enabled(),
            health_check_interval_secs: default_health_check_interval_secs(),
            notify_on_health_change: default_notify_on_health_change(),
            notify_on_runtime_alerts: default_notify_on_runtime_alerts(),
            notify_on_new_request: default_notify_on_new_request(),
        }
    }
}

fn default_telemt_config_path() -> PathBuf {
    PathBuf::from("/etc/telemt.toml")
}

fn default_db_path() -> PathBuf {
    PathBuf::from("/var/lib/telemt-admin/state.db")
}

fn default_service_name() -> String {
    "telemt.service".to_string()
}

fn default_users_page_size() -> i64 {
    10
}

fn default_token_days() -> i64 {
    14
}

fn default_max_token_days() -> i64 {
    180
}

fn default_allow_auto_approve_tokens() -> bool {
    true
}

fn default_telemt_api_base_url() -> String {
    "http://127.0.0.1:9091".to_string()
}

fn default_telemt_api_timeout_ms() -> u64 {
    5_000
}

fn default_allow_file_fallback() -> bool {
    true
}

fn default_prefer_api_links() -> bool {
    true
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_health_check_interval_secs() -> u64 {
    60
}

fn default_notify_on_health_change() -> bool {
    true
}

fn default_notify_on_runtime_alerts() -> bool {
    true
}

fn default_notify_on_new_request() -> bool {
    true
}

fn normalize_bot_username_input(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches('@');
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        tracing::debug!("Loading config from {}", path.display());
        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Не удалось прочитать конфиг {}: {}", path.display(), e)
        })?;
        let mut config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Ошибка парсинга конфига: {}", e))?;
        let env_override_keys = crate::env_config_overlay::apply(&mut config)?;
        if let Some(ttl) = config.security.wizard_state_ttl_seconds
            && ttl <= 0
        {
            return Err(anyhow::anyhow!(
                "security.wizard_state_ttl_seconds должен быть положительным числом секунд"
            ));
        }
        if config.notifications.health_check_interval_secs == 0 {
            return Err(anyhow::anyhow!(
                "notifications.health_check_interval_secs должен быть положительным"
            ));
        }
        tracing::info!(
            admin_count = config.admin_ids.len(),
            bot_username = ?config.configured_bot_username(),
            telemt_config_path = %config.telemt_config_path.display(),
            db_path = %config.db_path.display(),
            service_name = %config.service_name,
            env_overrides_count = env_override_keys.len(),
            env_override_keys = ?env_override_keys,
            runtime_mode = %config.effective_runtime_mode().as_str(),
            runtime_systemd_unit = %config.effective_systemd_unit(),
            users_page_size = config.users_page_size,
            telemt_api_enabled = config.telemt_api.enabled,
            telemt_api_base_url = %config.telemt_api.base_url,
            telemt_api_timeout_ms = config.telemt_api.timeout_ms,
            telemt_api_allow_file_fallback = config.telemt_api.allow_file_fallback,
            telemt_api_prefer_api_links = config.telemt_api.prefer_api_links,
            notifications_enabled = config.notifications.enabled,
            notifications_health_check_interval_secs = config.notifications.health_check_interval_secs,
            notifications_notify_on_health_change = config.notifications.notify_on_health_change,
            notifications_notify_on_runtime_alerts = config.notifications.notify_on_runtime_alerts,
            notifications_notify_on_new_request = config.notifications.notify_on_new_request,
            security_default_days = config.security.default_token_days,
            security_max_days = config.security.max_token_days,
            allow_auto_approve_tokens = config.security.allow_auto_approve_tokens,
            wizard_state_ttl_seconds = ?config.security.wizard_state_ttl_seconds,
            "Config parsed successfully"
        );
        Ok(config)
    }

    pub fn bot_token(&self) -> Result<String, anyhow::Error> {
        self.bot_token
            .clone()
            .or_else(|| std::env::var("TELOXIDE_TOKEN").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Не задан bot_token (в TOML, TELEMT_ADMIN__BOT_TOKEN или TELOXIDE_TOKEN)"
                )
            })
    }

    pub fn is_admin(&self, user_id: i64) -> bool {
        self.admin_ids.contains(&user_id)
    }

    /// Явный username из TOML/env после нормализации (без `@`, без пробелов).
    pub fn configured_bot_username(&self) -> Option<String> {
        self.bot_username
            .as_ref()
            .and_then(|s| normalize_bot_username_input(s))
    }

    /// Username для ссылок и разбора команд: приоритет у конфигурации, иначе результат `getMe`.
    pub fn resolved_bot_username(&self, from_get_me: Option<String>) -> Option<String> {
        if let Some(u) = self.configured_bot_username() {
            return Some(u);
        }
        from_get_me.and_then(|s| normalize_bot_username_input(&s))
    }

    /// Режим runtime: при отсутствии `[runtime]` считается `systemd` (как раньше).
    pub fn effective_runtime_mode(&self) -> RuntimeMode {
        self.runtime
            .as_ref()
            .map(|r| r.mode)
            .unwrap_or(RuntimeMode::Systemd)
    }

    /// Имя unit для `systemctl` при режиме systemd.
    pub fn effective_systemd_unit(&self) -> String {
        self.runtime
            .as_ref()
            .and_then(|r| r.service_name.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.service_name.clone())
    }

    /// Метка для UI при `external`.
    pub fn effective_external_label(&self) -> Option<String> {
        self.runtime.as_ref().and_then(|r| {
            r.label
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_runtime_mode, BotMessages, Config, NotificationsConfig, RuntimeSection,
        SecurityConfig, TelemtApiConfig,
    };
    use crate::runtime::RuntimeMode;
    use std::path::PathBuf;

    fn sample_config() -> Config {
        Config {
            bot_token: Some("token".to_string()),
            bot_username: Some(" @TelemtAdmin ".to_string()),
            admin_ids: vec![1],
            telemt_config_path: PathBuf::from("/tmp/telemt.toml"),
            db_path: PathBuf::from("/tmp/state.db"),
            service_name: "telemt.service".to_string(),
            users_page_size: 10,
            security: SecurityConfig::default(),
            telemt_api: TelemtApiConfig::default(),
            notifications: NotificationsConfig::default(),
            bot_messages: BotMessages::default(),
            runtime: None,
        }
    }

    #[test]
    fn bot_messages_return_defaults_for_empty_values() {
        let messages = BotMessages {
            start_without_invite: None,
            invite_manual_prompt: Some("   ".to_string()),
            invite_followup_prompt: None,
            ..Default::default()
        };

        assert!(messages.invite_manual_prompt_or_default().contains("Введите"));
        assert!(messages.invite_followup_prompt_or_default().contains("invite-токен"));
    }

    #[test]
    fn configured_bot_username_normalizes_input() {
        let config = sample_config();

        assert_eq!(config.configured_bot_username().as_deref(), Some("TelemtAdmin"));
    }

    #[test]
    fn resolved_bot_username_prefers_config_over_get_me() {
        let config = sample_config();

        assert_eq!(
            config.resolved_bot_username(Some("@AnotherBot".to_string())).as_deref(),
            Some("TelemtAdmin")
        );
    }

    #[test]
    fn runtime_helpers_apply_defaults_and_trim_values() {
        let mut config = sample_config();
        assert_eq!(config.effective_runtime_mode(), default_runtime_mode());
        assert_eq!(config.effective_systemd_unit(), "telemt.service");
        assert_eq!(config.effective_external_label(), None);

        config.runtime = Some(RuntimeSection {
            mode: RuntimeMode::External,
            service_name: Some(" custom.service ".to_string()),
            label: Some(" external supervisor ".to_string()),
        });

        assert_eq!(config.effective_runtime_mode(), RuntimeMode::External);
        assert_eq!(config.effective_systemd_unit(), "custom.service");
        assert_eq!(
            config.effective_external_label().as_deref(),
            Some("external supervisor")
        );
    }

    #[test]
    fn no_access_messages_return_defaults_when_empty() {
        let messages = BotMessages {
            no_access_status_text: None,
            no_access_link_text: Some("   ".to_string()),
            ..Default::default()
        };

        assert!(messages.no_access_status_or_default().contains("invite-токен"));
        assert!(messages.no_access_link_or_default().contains("нет доступа"));
    }

    #[test]
    fn no_access_messages_use_custom_values_when_set() {
        let messages = BotMessages {
            no_access_status_text: Some("Custom status".to_string()),
            no_access_link_text: Some("Custom link".to_string()),
            ..Default::default()
        };

        assert_eq!(messages.no_access_status_or_default(), "Custom status");
        assert_eq!(messages.no_access_link_or_default(), "Custom link");
    }
}