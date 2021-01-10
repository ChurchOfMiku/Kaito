use anyhow::{anyhow, Result};
use sqlx::{
    migrate::Migrator,
    sqlite::{Sqlite, SqliteConnectOptions, SqliteSynchronous},
    Executor, Pool,
};
use std::{path::Path, sync::Arc};

use super::{DEFAULT_ROLE, ROLES};
use crate::{config::Config, services::UserId};

pub struct BotDb {
    pool: Pool<Sqlite>,
}

impl BotDb {
    pub async fn new(data_path: &Path, share_path: &Path, config: &Config) -> Result<Arc<BotDb>> {
        let pool = Pool::connect_with(
            SqliteConnectOptions::new()
                .filename(data_path.join("kaito.db"))
                .synchronous(SqliteSynchronous::Normal)
                .create_if_missing(true),
        )
        .await?;

        let m = Migrator::new(share_path.join("migrations")).await?;
        m.run(&pool).await?;

        let db = Arc::new(BotDb { pool });

        if let Some(user_roles) = config.user_roles.as_ref() {
            for (id_str, role) in user_roles {
                let user_id = UserId::from_str(&id_str)?;

                db.set_role_for_user(user_id, role).await?;
            }
        }

        Ok(db)
    }

    pub async fn get_role_for_user(&self, user_id: UserId) -> Result<String> {
        let role: (String,) = sqlx::query_as("SELECT role FROM roles WHERE user_id = ?")
            .bind(user_id.to_short_str())
            .fetch_one(self.pool())
            .await
            .or_else(|err| match err {
                sqlx::Error::RowNotFound => Ok((DEFAULT_ROLE.into(),)),
                _ => Err(err),
            })?;

        if !ROLES.contains(&role.0.as_str()) {
            return Ok(DEFAULT_ROLE.into());
        }

        Ok(role.0)
    }

    pub async fn set_role_for_user(&self, user_id: UserId, role: &str) -> Result<()> {
        if !ROLES.contains(&role) {
            return Err(anyhow!("unknown role \"{}\"", role));
        }

        self.pool()
            .execute(
                sqlx::query("REPLACE INTO roles ( user_id, role ) VALUES ( ?, ? )")
                    .bind(user_id.to_short_str())
                    .bind(role),
            )
            .await?;

        Ok(())
    }

    pub async fn restrict_user(&self, user_id: UserId, restrictor_user_id: UserId) -> Result<()> {
        self.pool()
            .execute(
                sqlx::query(
                    "INSERT INTO restrictions ( user_id, restrictor_user_id ) VALUES ( ?, ? )",
                )
                .bind(user_id.to_short_str())
                .bind(restrictor_user_id.to_short_str()),
            )
            .await?;

        Ok(())
    }

    pub async fn unrestrict_user(&self, user_id: UserId) -> Result<()> {
        self.pool()
            .execute(
                sqlx::query("DELETE FROM restrictions WHERE user_id = ?")
                    .bind(user_id.to_short_str()),
            )
            .await?;

        Ok(())
    }

    pub async fn is_restricted(&self, user_id: UserId) -> Result<bool> {
        let restricted = sqlx::query_as("SELECT user_id FROM restrictions WHERE user_id = ?")
            .bind(user_id.to_short_str())
            .fetch_one(self.pool())
            .await
            .map(|_a: (String,)| true)
            .or_else(|err| match err {
                sqlx::Error::RowNotFound => Ok(false),
                _ => Err(err),
            })?;

        Ok(restricted)
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}
