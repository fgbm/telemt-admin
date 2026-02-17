//! SQLite-слой для заявок на регистрацию и связей tg_user_id -> telemt_user.

use rand::distr::{Alphanumeric, SampleString};
use sqlx::FromRow;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

/// Результат регистрации.
#[derive(Debug)]
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
    pub status: String,
    pub telemt_username: Option<String>,
    pub secret: Option<String>,
    pub created_at: i64,
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
}

const STATUS_APPROVED: &str = "approved";
const STATUS_PENDING: &str = "pending";
const STATUS_REJECTED: &str = "rejected";
const STATUS_DELETED: &str = "deleted";

#[derive(Debug, Clone)]
pub struct AdminStats {
    pub total: i64,
    pub pending: i64,
    pub approved: i64,
    pub rejected: i64,
    pub deleted: i64,
}

pub struct Db {
    pool: SqlitePool,
}

fn current_unix_timestamp() -> Result<i64, anyhow::Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|err| anyhow::anyhow!("Системное время меньше UNIX_EPOCH: {}", err))
}

impl Db {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Не удалось создать директорию для БД: {}", e))?;
        }

        let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))?
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(opts)
            .await
            .map_err(|e| anyhow::anyhow!("Не удалось подключиться к SQLite: {}", e))?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS registration_requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tg_user_id INTEGER NOT NULL,
                tg_username TEXT,
                tg_display_name TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                telemt_username TEXT,
                secret TEXT,
                created_at INTEGER NOT NULL,
                resolved_at INTEGER,
                UNIQUE(tg_user_id)
            );
            CREATE INDEX IF NOT EXISTS idx_requests_status ON registration_requests(status);
            CREATE INDEX IF NOT EXISTS idx_requests_tg_user ON registration_requests(tg_user_id);
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Миграция БД: {}", e))?;

        let has_display_name_column = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pragma_table_info('registration_requests') WHERE name = 'tg_display_name'",
        )
        .fetch_one(&self.pool)
        .await?;

        if has_display_name_column == 0 {
            sqlx::query("ALTER TABLE registration_requests ADD COLUMN tg_display_name TEXT")
                .execute(&self.pool)
                .await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS invite_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token TEXT UNIQUE NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                auto_approve INTEGER NOT NULL DEFAULT 0,
                created_by INTEGER,
                usage_count INTEGER NOT NULL DEFAULT 0,
                max_usage INTEGER,
                is_active INTEGER NOT NULL DEFAULT 1,
                revoked_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_invite_tokens_token ON invite_tokens(token);
            CREATE INDEX IF NOT EXISTS idx_invite_tokens_active ON invite_tokens(is_active);
            CREATE INDEX IF NOT EXISTS idx_invite_tokens_expires_at ON invite_tokens(expires_at);
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Миграция invite_tokens: {}", e))?;

        self.ensure_column_exists("invite_tokens", "max_usage", "INTEGER")
            .await?;
        self.ensure_column_exists("invite_tokens", "is_active", "INTEGER NOT NULL DEFAULT 1")
            .await?;
        self.ensure_column_exists("invite_tokens", "revoked_at", "INTEGER")
            .await?;

        Ok(())
    }

    async fn ensure_column_exists(
        &self,
        table: &str,
        column: &str,
        sql_type: &str,
    ) -> Result<(), anyhow::Error> {
        let count = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = '{}'",
            table, column
        ))
        .fetch_one(&self.pool)
        .await?;
        if count == 0 {
            sqlx::query(&format!(
                "ALTER TABLE {} ADD COLUMN {} {}",
                table, column, sql_type
            ))
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    fn generate_invite_token() -> String {
        Alphanumeric.sample_string(&mut rand::rng(), 10)
    }

    /// Создаёт или возвращает существующую pending-заявку.
    pub async fn register_or_get(
        &self,
        tg_user_id: i64,
        tg_username: Option<&str>,
        tg_display_name: Option<&str>,
    ) -> Result<RegisterResult, anyhow::Error> {
        let now = current_unix_timestamp()?;

        let existing = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at FROM registration_requests WHERE tg_user_id = ?",
        )
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = existing {
            return match r.status.as_str() {
                "approved" => {
                    if let Some(s) = r.secret {
                        Ok(RegisterResult::Approved(s))
                    } else {
                        Ok(RegisterResult::AlreadyPending)
                    }
                }
                "rejected" => Ok(RegisterResult::Rejected),
                _ => {
                    sqlx::query(
                        "UPDATE registration_requests SET tg_username = ?, tg_display_name = ?, created_at = ? WHERE tg_user_id = ?",
                    )
                        .bind(tg_username)
                        .bind(tg_display_name)
                        .bind(now)
                        .bind(tg_user_id)
                        .execute(&self.pool)
                        .await?;
                    Ok(RegisterResult::AlreadyPending)
                }
            };
        }

        sqlx::query(
            "INSERT INTO registration_requests (tg_user_id, tg_username, tg_display_name, status, created_at) VALUES (?, ?, ?, 'pending', ?)",
        )
        .bind(tg_user_id)
        .bind(tg_username)
        .bind(tg_display_name)
        .bind(now)
        .execute(&self.pool)
        .await?;

        let req = self
            .get_pending_by_tg_user(tg_user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("только что создали заявку"))?;
        Ok(RegisterResult::NewPending(req))
    }

    /// Получает pending-заявку по tg_user_id.
    pub async fn get_pending_by_tg_user(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let r = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at FROM registration_requests WHERE tg_user_id = ? AND status = 'pending'",
        )
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(r)
    }

    /// Получает pending-заявку по id.
    pub async fn get_pending_by_id(
        &self,
        id: i64,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let r = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at FROM registration_requests WHERE id = ? AND status = 'pending'",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(r)
    }

    /// Помечает заявку как approved и сохраняет telemt_username и secret.
    pub async fn approve(
        &self,
        id: i64,
        telemt_username: &str,
        secret: &str,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let now = current_unix_timestamp()?;

        let r = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at FROM registration_requests WHERE id = ? AND status = 'pending'",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let req = match r {
            Some(req) => req,
            None => return Ok(None),
        };

        sqlx::query(
            "UPDATE registration_requests SET status = 'approved', telemt_username = ?, secret = ?, resolved_at = ? WHERE id = ?",
        )
        .bind(telemt_username)
        .bind(secret)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(Some(req))
    }

    /// Помечает заявку как rejected.
    pub async fn reject(&self, id: i64) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let now = current_unix_timestamp()?;

        let r = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at FROM registration_requests WHERE id = ? AND status = 'pending'",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let req = r.clone();
        if r.is_some() {
            sqlx::query(
                "UPDATE registration_requests SET status = 'rejected', resolved_at = ? WHERE id = ?",
            )
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        }
        Ok(req)
    }

    /// Деактивирует пользователя (помечает как удалённого для истории; сама запись остаётся).
    pub async fn deactivate_user(&self, tg_user_id: i64) -> Result<bool, anyhow::Error> {
        let r = sqlx::query(
            "UPDATE registration_requests SET status = ? WHERE tg_user_id = ? AND status = ?",
        )
        .bind(STATUS_DELETED)
        .bind(tg_user_id)
        .bind(STATUS_APPROVED)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() > 0)
    }

    /// Устанавливает пользователя как approved (для /create без предварительной заявки).
    pub async fn set_approved(
        &self,
        tg_user_id: i64,
        tg_username: Option<&str>,
        tg_display_name: Option<&str>,
        telemt_username: &str,
        secret: &str,
    ) -> Result<(), anyhow::Error> {
        let now = current_unix_timestamp()?;

        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM registration_requests WHERE tg_user_id = ?",
        )
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;

        if exists.is_some() {
            sqlx::query(
                "UPDATE registration_requests
                 SET status = 'approved',
                     tg_username = ?,
                     tg_display_name = ?,
                     telemt_username = ?,
                     secret = ?,
                     resolved_at = ?
                 WHERE tg_user_id = ?",
            )
            .bind(tg_username)
            .bind(tg_display_name)
            .bind(telemt_username)
            .bind(secret)
            .bind(now)
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO registration_requests
                 (tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at, resolved_at)
                 VALUES (?, ?, ?, 'approved', ?, ?, ?, ?)",
            )
            .bind(tg_user_id)
            .bind(tg_username)
            .bind(tg_display_name)
            .bind(telemt_username)
            .bind(secret)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Получает approved-пользователя по tg_user_id.
    pub async fn get_approved(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<(String, String)>, anyhow::Error> {
        let r = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at FROM registration_requests WHERE tg_user_id = ? AND status = 'approved'",
        )
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(r.and_then(|x| x.telemt_username.zip(x.secret)))
    }

    pub async fn get_request_by_tg_user(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let r = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at FROM registration_requests WHERE tg_user_id = ?",
        )
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(r)
    }

    pub async fn create_invite_token(
        &self,
        days: i64,
        auto_approve: bool,
        max_usage: Option<i64>,
        created_by: Option<i64>,
    ) -> Result<InviteToken, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let ttl_seconds = days
            .checked_mul(86_400)
            .ok_or_else(|| anyhow::anyhow!("Срок действия токена слишком большой"))?;
        let expires_at = now
            .checked_add(ttl_seconds)
            .ok_or_else(|| anyhow::anyhow!("Некорректное время истечения токена"))?;

        let mut created: Option<InviteToken> = None;
        for _ in 0..8 {
            let token = Self::generate_invite_token();
            let result = sqlx::query(
                "INSERT INTO invite_tokens (token, created_at, expires_at, auto_approve, created_by, max_usage) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&token)
            .bind(now)
            .bind(expires_at)
            .bind(auto_approve)
            .bind(created_by)
            .bind(max_usage)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => {
                    created = sqlx::query_as::<_, InviteToken>(
                        "SELECT id, token, created_at, expires_at, auto_approve, created_by, usage_count, max_usage, is_active FROM invite_tokens WHERE token = ?",
                    )
                    .bind(token)
                    .fetch_optional(&self.pool)
                    .await?;
                    if created.is_some() {
                        break;
                    }
                }
                Err(err) => {
                    let message = err.to_string().to_lowercase();
                    if message.contains("unique") {
                        continue;
                    }
                    return Err(anyhow::anyhow!("Не удалось создать invite-токен: {}", err));
                }
            }
        }

        created.ok_or_else(|| anyhow::anyhow!("Не удалось сгенерировать уникальный токен"))
    }

    pub async fn list_active_invite_tokens(
        &self,
        limit: i64,
    ) -> Result<Vec<InviteToken>, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let rows = sqlx::query_as::<_, InviteToken>(
            "SELECT id, token, created_at, expires_at, auto_approve, created_by, usage_count, max_usage, is_active
             FROM invite_tokens
             WHERE is_active = 1
               AND expires_at > ?
               AND (max_usage IS NULL OR usage_count < max_usage)
             ORDER BY expires_at ASC
             LIMIT ?",
        )
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn revoke_invite_token(&self, token: &str) -> Result<bool, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let result = sqlx::query(
            "UPDATE invite_tokens SET is_active = 0, revoked_at = ? WHERE token = ? AND is_active = 1",
        )
        .bind(now)
        .bind(token)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn consume_invite_token(
        &self,
        token: &str,
    ) -> Result<ConsumedInviteToken, TokenConsumeError> {
        let now = current_unix_timestamp().map_err(|_| TokenConsumeError::NotFound)?;
        let update_result = sqlx::query(
            "UPDATE invite_tokens
             SET usage_count = usage_count + 1
             WHERE token = ?
               AND is_active = 1
               AND expires_at > ?
               AND (max_usage IS NULL OR usage_count < max_usage)",
        )
        .bind(token)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|_| TokenConsumeError::NotFound)?;

        if update_result.rows_affected() == 0 {
            let token_row = sqlx::query_as::<_, InviteToken>(
                "SELECT id, token, created_at, expires_at, auto_approve, created_by, usage_count, max_usage, is_active FROM invite_tokens WHERE token = ?",
            )
            .bind(token)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| TokenConsumeError::NotFound)?;

            let Some(row) = token_row else {
                return Err(TokenConsumeError::NotFound);
            };
            if !row.is_active {
                return Err(TokenConsumeError::Revoked);
            }
            if row.expires_at <= now {
                return Err(TokenConsumeError::Expired);
            }
            if row.max_usage.is_some_and(|max| row.usage_count >= max) {
                return Err(TokenConsumeError::UsageLimitReached);
            }
            return Err(TokenConsumeError::NotFound);
        }

        let row = sqlx::query_as::<_, InviteToken>(
            "SELECT id, token, created_at, expires_at, auto_approve, created_by, usage_count, max_usage, is_active FROM invite_tokens WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| TokenConsumeError::NotFound)?;
        let row = row.ok_or(TokenConsumeError::NotFound)?;
        Ok(ConsumedInviteToken {
            id: row.id,
            token: row.token,
            mode: if row.auto_approve {
                TokenMode::AutoApprove
            } else {
                TokenMode::Manual
            },
            expires_at: row.expires_at,
            created_by: row.created_by,
            usage_count: row.usage_count,
            max_usage: row.max_usage,
        })
    }

    /// Ищет tg_user_id по tg_username (без учёта регистра, без @).
    pub async fn find_tg_user_id_by_username(
        &self,
        username: &str,
    ) -> Result<Option<i64>, anyhow::Error> {
        let normalized = username.trim_start_matches('@');
        if normalized.is_empty() {
            return Ok(None);
        }

        let user_id = sqlx::query_scalar::<_, i64>(
            "SELECT tg_user_id FROM registration_requests
             WHERE lower(tg_username) = lower(?)
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(normalized)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user_id)
    }

    pub async fn list_pending_requests(
        &self,
        limit: i64,
    ) -> Result<Vec<RegistrationRequest>, anyhow::Error> {
        let rows = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at
             FROM registration_requests
             WHERE status = ?
             ORDER BY created_at ASC
             LIMIT ?",
        )
        .bind(STATUS_PENDING)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn count_active_users(&self) -> Result<i64, anyhow::Error> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM registration_requests WHERE status = ?",
        )
        .bind(STATUS_APPROVED)
        .fetch_one(&self.pool)
        .await?;
        Ok(total)
    }

    pub async fn list_active_users_page(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RegistrationRequest>, anyhow::Error> {
        let rows = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at
             FROM registration_requests
             WHERE status = ?
             ORDER BY created_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(STATUS_APPROVED)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_active_user_by_tg_user(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let row = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at
             FROM registration_requests
             WHERE status = ? AND tg_user_id = ?
             LIMIT 1",
        )
        .bind(STATUS_APPROVED)
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn admin_stats(&self) -> Result<AdminStats, anyhow::Error> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM registration_requests")
            .fetch_one(&self.pool)
            .await?;
        let pending = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM registration_requests WHERE status = ?",
        )
        .bind(STATUS_PENDING)
        .fetch_one(&self.pool)
        .await?;
        let approved = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM registration_requests WHERE status = ?",
        )
        .bind(STATUS_APPROVED)
        .fetch_one(&self.pool)
        .await?;
        let rejected = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM registration_requests WHERE status = ?",
        )
        .bind(STATUS_REJECTED)
        .fetch_one(&self.pool)
        .await?;
        let deleted = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM registration_requests WHERE status = ?",
        )
        .bind(STATUS_DELETED)
        .fetch_one(&self.pool)
        .await?;

        Ok(AdminStats {
            total,
            pending,
            approved,
            rejected,
            deleted,
        })
    }
}
