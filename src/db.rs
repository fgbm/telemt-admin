//! SQLite-слой для заявок на регистрацию и связей tg_user_id -> telemt_user.

use sqlx::FromRow;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::path::Path;
use std::str::FromStr;

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

        Ok(())
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
                "UPDATE registration_requests SET status = 'approved', telemt_username = ?, secret = ?, resolved_at = ? WHERE tg_user_id = ?",
            )
            .bind(telemt_username)
            .bind(secret)
            .bind(now)
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO registration_requests (tg_user_id, status, telemt_username, secret, created_at, resolved_at) VALUES (?, 'approved', ?, ?, ?, ?)",
            )
            .bind(tg_user_id)
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

    pub async fn list_active_users(
        &self,
        limit: i64,
    ) -> Result<Vec<RegistrationRequest>, anyhow::Error> {
        let rows = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at
             FROM registration_requests
             WHERE status = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(STATUS_APPROVED)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
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
