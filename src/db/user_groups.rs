use crate::db::{Db, current_unix_timestamp};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserGroup {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    /// Unix-время окончания действия (для массового PATCH), `None` — без общего срока.
    pub expires_at: Option<i64>,
}

impl Db {
    pub async fn create_user_group(
        &self,
        name: &str,
        expires_at: Option<i64>,
    ) -> Result<UserGroup, anyhow::Error> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow::anyhow!("Имя группы не может быть пустым"));
        }
        let now = current_unix_timestamp()?;
        sqlx::query("INSERT INTO user_groups (name, created_at, expires_at) VALUES (?, ?, ?)")
            .bind(name)
            .bind(now)
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        let row = sqlx::query_as::<_, UserGroup>(
            "SELECT id, name, created_at, expires_at FROM user_groups WHERE name = ? LIMIT 1",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_user_groups(&self) -> Result<Vec<UserGroup>, anyhow::Error> {
        let rows = sqlx::query_as::<_, UserGroup>(
            "SELECT id, name, created_at, expires_at FROM user_groups ORDER BY name ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_user_group_by_id(
        &self,
        group_id: i64,
    ) -> Result<Option<UserGroup>, anyhow::Error> {
        let row = sqlx::query_as::<_, UserGroup>(
            "SELECT id, name, created_at, expires_at FROM user_groups WHERE id = ? LIMIT 1",
        )
        .bind(group_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn count_group_members(&self, group_id: i64) -> Result<i64, anyhow::Error> {
        let n = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_group_members WHERE group_id = ?",
        )
        .bind(group_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(n)
    }

    pub async fn list_group_member_tg_ids(&self, group_id: i64) -> Result<Vec<i64>, anyhow::Error> {
        let rows = sqlx::query_scalar::<_, i64>(
            "SELECT tg_user_id FROM user_group_members WHERE group_id = ? ORDER BY tg_user_id ASC",
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_group_for_tg_user(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<UserGroup>, anyhow::Error> {
        let row = sqlx::query_as::<_, UserGroup>(
            "SELECT g.id, g.name, g.created_at, g.expires_at
             FROM user_groups g
             INNER JOIN user_group_members m ON m.group_id = g.id
             WHERE m.tg_user_id = ?
             LIMIT 1",
        )
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn set_user_group_membership(
        &self,
        tg_user_id: i64,
        group_id: Option<i64>,
    ) -> Result<(), anyhow::Error> {
        sqlx::query("DELETE FROM user_group_members WHERE tg_user_id = ?")
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;
        if let Some(gid) = group_id {
            let now = current_unix_timestamp()?;
            sqlx::query(
                "INSERT INTO user_group_members (tg_user_id, group_id, joined_at) VALUES (?, ?, ?)",
            )
            .bind(tg_user_id)
            .bind(gid)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn delete_user_group(&self, group_id: i64) -> Result<bool, anyhow::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM user_group_members WHERE group_id = ?")
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
        let r = sqlx::query("DELETE FROM user_groups WHERE id = ?")
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn set_user_group_expiry(
        &self,
        group_id: i64,
        expires_at: Option<i64>,
    ) -> Result<bool, anyhow::Error> {
        let result = sqlx::query("UPDATE user_groups SET expires_at = ? WHERE id = ?")
            .bind(expires_at)
            .bind(group_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::test_support::TestDb;

    #[tokio::test]
    async fn create_list_and_membership_roundtrip() -> Result<(), anyhow::Error> {
        let fixture = TestDb::new().await?;
        let g = fixture.db.create_user_group("team-a", None).await?;
        assert_eq!(g.name, "team-a");

        fixture
            .db
            .set_user_group_membership(1001, Some(g.id))
            .await?;
        assert_eq!(fixture.db.count_group_members(g.id).await?, 1);
        let group = fixture.db.get_group_for_tg_user(1001).await?.unwrap();
        assert_eq!(group.id, g.id);

        fixture.db.set_user_group_membership(1001, None).await?;
        assert!(fixture.db.get_group_for_tg_user(1001).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_group_removes_members() -> Result<(), anyhow::Error> {
        let fixture = TestDb::new().await?;
        let g = fixture.db.create_user_group("tmp", None).await?;
        fixture
            .db
            .set_user_group_membership(2002, Some(g.id))
            .await?;
        assert!(fixture.db.delete_user_group(g.id).await?);
        assert!(fixture.db.get_user_group_by_id(g.id).await?.is_none());
        assert!(fixture.db.get_group_for_tg_user(2002).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn list_group_member_tg_ids_returns_sorted() -> Result<(), anyhow::Error> {
        let fixture = TestDb::new().await?;
        let g = fixture.db.create_user_group("team", None).await?;
        fixture.db.set_user_group_membership(3, Some(g.id)).await?;
        fixture.db.set_user_group_membership(1, Some(g.id)).await?;
        assert_eq!(fixture.db.list_group_member_tg_ids(g.id).await?, vec![1, 3]);
        Ok(())
    }
}
