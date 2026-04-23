use crate::db::Db;

const ALLOWED_TABLES: &[&str] = &["registration_requests", "invite_tokens"];
const ALLOWED_COLUMNS: &[&str] = &[
    "tg_display_name",
    "backend_mode",
    "last_sync_error",
    "last_seen_revision",
    "last_synced_at",
    "invite_token_id",
    "max_usage",
    "is_active",
    "revoked_at",
];

impl Db {
    pub(crate) async fn migrate(&self) -> Result<(), anyhow::Error> {
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
                backend_mode TEXT,
                last_sync_error TEXT,
                last_seen_revision TEXT,
                last_synced_at INTEGER,
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

        self.ensure_column_exists("registration_requests", "backend_mode", "TEXT")
            .await?;
        self.ensure_column_exists("registration_requests", "last_sync_error", "TEXT")
            .await?;
        self.ensure_column_exists("registration_requests", "last_seen_revision", "TEXT")
            .await?;
        self.ensure_column_exists("registration_requests", "last_synced_at", "INTEGER")
            .await?;
        self.ensure_column_exists("registration_requests", "invite_token_id", "INTEGER")
            .await?;

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

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bot_wizard_states (
                tg_user_id INTEGER PRIMARY KEY,
                state_key TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_bot_wizard_states_updated_at ON bot_wizard_states(updated_at);
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Миграция bot_wizard_states: {}", e))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL,
                expires_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS user_group_members (
                tg_user_id INTEGER PRIMARY KEY,
                group_id INTEGER NOT NULL,
                joined_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_user_group_members_group ON user_group_members(group_id);
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Миграция user_groups: {}", e))?;

        Ok(())
    }

    async fn ensure_column_exists(
        &self,
        table: &str,
        column: &str,
        sql_type: &str,
    ) -> Result<(), anyhow::Error> {
        // table и column — внутренние константы, заданные в коде миграций;
        // для metadata-запроса format! приемлем.
        let count = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = '{}'",
            table, column
        ))
        .fetch_one(&self.pool)
        .await?;
        if count == 0 {
            if !ALLOWED_TABLES.contains(&table) {
                anyhow::bail!(
                    "ALTER TABLE: таблица '{}' не входит в белый список",
                    table
                );
            }
            if !ALLOWED_COLUMNS.contains(&column) {
                anyhow::bail!(
                    "ALTER TABLE: колонка '{}' не входит в белый список",
                    column
                );
            }
            sqlx::query(&format!(
                "ALTER TABLE {} ADD COLUMN {} {}",
                table, column, sql_type
            ))
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::test_support::TestDb;

    #[tokio::test]
    async fn migrations_create_expected_columns() -> Result<(), anyhow::Error> {
        let fixture = TestDb::new().await?;

        let request_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('registration_requests') ORDER BY name",
        )
        .fetch_all(&fixture.db.pool)
        .await?;
        let invite_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('invite_tokens') ORDER BY name",
        )
        .fetch_all(&fixture.db.pool)
        .await?;

        assert!(request_columns.iter().any(|name| name == "backend_mode"));
        assert!(request_columns.iter().any(|name| name == "last_sync_error"));
        assert!(request_columns.iter().any(|name| name == "last_seen_revision"));
        assert!(request_columns.iter().any(|name| name == "last_synced_at"));
        assert!(request_columns.iter().any(|name| name == "invite_token_id"));
        assert!(invite_columns.iter().any(|name| name == "max_usage"));
        assert!(invite_columns.iter().any(|name| name == "is_active"));
        assert!(invite_columns.iter().any(|name| name == "revoked_at"));
        Ok(())
    }
}
