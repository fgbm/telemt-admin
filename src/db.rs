//! SQLite-слой для заявок на регистрацию и связей tg_user_id -> telemt_user.

mod admin;
mod invite_tokens;
mod migrations;
mod registration;
mod user_groups;
mod wizard_state;

use sqlx::FromRow;
use sqlx::error::ErrorKind;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::fmt;
use std::path::Path;
use thiserror::Error;

/// Результат регистрации.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum RegisterResult {
    /// Уже одобрен — secret
    Approved(String),
    /// Новая заявка создана
    NewPending(RegistrationRequest),
    /// Заявка уже на рассмотрении
    AlreadyPending,
    /// Ранее отклонено
    Rejected,
}

#[derive(Debug, Clone, FromRow)]
pub struct RegistrationRequest {
    pub id: i64,
    pub tg_user_id: i64,
    pub tg_username: Option<String>,
    pub tg_display_name: Option<String>,
    pub status: RequestStatus,
    pub telemt_username: Option<String>,
    pub secret: Option<String>,
    pub created_at: i64,
    pub backend_mode: Option<String>,
    pub last_sync_error: Option<String>,
    pub last_seen_revision: Option<String>,
    pub last_synced_at: Option<i64>,
    /// Ссылка на invite_tokens.id, с которой пользователь оставил заявку (если известно).
    pub invite_token_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum RequestStatus {
    Pending,
    Approved,
    Rejected,
    Deleted,
}

impl fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Pending => STATUS_PENDING,
            Self::Approved => STATUS_APPROVED,
            Self::Rejected => STATUS_REJECTED,
            Self::Deleted => STATUS_DELETED,
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct InviteToken {
    pub id: i64,
    pub token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub auto_approve: bool,
    pub created_by: Option<i64>,
    pub usage_count: i64,
    pub max_usage: Option<i64>,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub enum TokenMode {
    Manual,
    AutoApprove,
}

#[derive(Debug, Clone)]
pub struct ConsumedInviteToken {
    pub id: i64,
    pub token: String,
    pub mode: TokenMode,
    pub expires_at: i64,
    pub created_by: Option<i64>,
    pub usage_count: i64,
    pub max_usage: Option<i64>,
}

#[derive(Debug, Error)]
pub enum TokenConsumeError {
    #[error("Токен не найден")]
    NotFound,
    #[error("Токен отозван")]
    Revoked,
    #[error("Срок действия токена истёк")]
    Expired,
    #[error("Лимит использований токена исчерпан")]
    UsageLimitReached,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub(crate) const STATUS_APPROVED: &str = "approved";
pub(crate) const STATUS_PENDING: &str = "pending";
pub(crate) const STATUS_REJECTED: &str = "rejected";
pub(crate) const STATUS_DELETED: &str = "deleted";
pub(crate) const SELECT_REQUEST: &str = "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at, backend_mode, last_sync_error, last_seen_revision, last_synced_at, invite_token_id FROM registration_requests";
pub(crate) const SELECT_INVITE_TOKEN: &str = "SELECT id, token, created_at, expires_at, auto_approve, created_by, usage_count, max_usage, is_active FROM invite_tokens";
pub(crate) const ACTIVE_INVITE_TOKEN_PREDICATE: &str =
    "is_active = 1 AND expires_at > ? AND (max_usage IS NULL OR usage_count < max_usage)";

#[derive(Debug, Clone)]
pub struct AdminStats {
    pub total: i64,
    pub pending: i64,
    pub approved: i64,
    pub rejected: i64,
    pub deleted: i64,
    pub tokens_total: i64,
    pub tokens_active: i64,
    pub tokens_manual_active: i64,
    pub tokens_auto_active: i64,
    pub tokens_revoked: i64,
    pub tokens_expired: i64,
    pub tokens_exhausted: i64,
}

#[derive(Debug, Clone)]
pub struct SyncErrorStat {
    pub code: String,
    pub affected_users: i64,
}

#[derive(Debug, Clone)]
pub struct SyncHealthSummary {
    pub degraded_users: i64,
    pub approved_via_control_api: i64,
    pub approved_via_legacy: i64,
    pub top_sync_errors: Vec<SyncErrorStat>,
}

#[derive(Debug, Clone)]
pub struct AdminActivity {
    pub timestamp: i64,
    pub kind: AdminActivityKind,
}

#[derive(Debug, Clone)]
pub enum AdminActivityKind {
    RequestApproved { request_id: i64 },
    RequestRejected { request_id: i64 },
    TokenCreated { token: String },
    TokenRevoked { token: String },
}

pub use user_groups::UserGroup;

pub struct Db {
    pub(crate) pool: SqlitePool,
    pub(crate) wizard_state_ttl_seconds: Option<i64>,
}

pub(crate) fn current_unix_timestamp() -> Result<i64, anyhow::Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|err| anyhow::anyhow!("Системное время меньше UNIX_EPOCH: {}", err))
}

pub(crate) fn is_unique_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db_err) => db_err.kind() == ErrorKind::UniqueViolation,
        _ => false,
    }
}

impl Db {
    pub async fn open(
        path: impl AsRef<Path>,
        wizard_state_ttl_seconds: Option<i64>,
    ) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Не удалось создать директорию для БД: {}", e))?;
        }

        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(opts)
            .await
            .map_err(|e| anyhow::anyhow!("Не удалось подключиться к SQLite: {}", e))?;

        let db = Self {
            pool,
            wizard_state_ttl_seconds,
        };
        db.migrate().await?;
        db.cleanup_expired_wizard_states().await?;
        Ok(db)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::Db;
    use std::path::PathBuf;
    use tempfile::TempDir;

    pub(crate) struct TestDb {
        pub(crate) db: Db,
        _temp_dir: TempDir,
    }

    impl TestDb {
        pub(crate) async fn new() -> Result<Self, anyhow::Error> {
            Self::with_wizard_state_ttl(None).await
        }

        pub(crate) async fn with_wizard_state_ttl(
            wizard_state_ttl_seconds: Option<i64>,
        ) -> Result<Self, anyhow::Error> {
            let temp_dir = tempfile::tempdir()?;
            let db_path: PathBuf = temp_dir.path().join("state.db");
            let db = Db::open(&db_path, wizard_state_ttl_seconds).await?;
            Ok(Self {
                db,
                _temp_dir: temp_dir,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::TestDb;

    #[tokio::test]
    async fn open_creates_expected_tables() -> Result<(), anyhow::Error> {
        let fixture = TestDb::new().await?;

        let tables = sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        )
        .fetch_all(&fixture.db.pool)
        .await?;

        assert!(tables.iter().any(|name| name == "registration_requests"));
        assert!(tables.iter().any(|name| name == "invite_tokens"));
        assert!(tables.iter().any(|name| name == "bot_wizard_states"));
        assert!(tables.iter().any(|name| name == "user_groups"));
        assert!(tables.iter().any(|name| name == "user_group_members"));
        Ok(())
    }

    #[tokio::test]
    async fn open_cleans_up_expired_wizard_states() -> Result<(), anyhow::Error> {
        let fixture = TestDb::with_wizard_state_ttl(Some(1)).await?;
        sqlx::query(
            "INSERT INTO bot_wizard_states (tg_user_id, state_key, updated_at) VALUES (?, ?, ?)",
        )
        .bind(10_i64)
        .bind("invite")
        .bind(1_i64)
        .execute(&fixture.db.pool)
        .await?;

        fixture.db.cleanup_expired_wizard_states().await?;

        let remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bot_wizard_states")
            .fetch_one(&fixture.db.pool)
            .await?;
        assert_eq!(remaining, 0);
        Ok(())
    }
}
