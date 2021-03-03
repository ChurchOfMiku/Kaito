use anyhow::{anyhow, Result};
use sqlx::{
    migrate::Migrator,
    sqlite::{Sqlite, SqliteConnectOptions, SqliteSynchronous},
    Executor, Pool,
};
use std::{path::Path, sync::Arc};

use super::{DEFAULT_ROLE, ROLES};
use crate::{
    config::Config,
    services::{ChannelId, ServerId, ServiceUserId},
};

pub type UserId = i64;

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
                let user_id = ServiceUserId::from_str(&id_str)?;
                let user = db.get_user_from_service_user_id(user_id).await?;
                db.set_role_for_user(user.uid, role).await?;
            }
        }

        Ok(db)
    }

    pub async fn get_user_from_service_user_id(
        &self,
        service_user_id: ServiceUserId,
    ) -> Result<User> {
        let res: Result<(UserId, Option<String>, Option<Vec<u8>>), sqlx::Error> =
            match service_user_id {
                ServiceUserId::Discord(discord_id) => {
                    sqlx::query_as("SELECT uid, role, discord_id FROM users WHERE discord_id = ?")
                        .bind(discord_id.to_le_bytes().to_vec())
                }
            }
            .fetch_one(self.pool())
            .await;

        let (uid, role, discord_id) = match res {
            Err(sqlx::Error::RowNotFound) => {
                let (res, discord_id) = match service_user_id {
                    ServiceUserId::Discord(discord_id) => (
                        self.pool()
                            .execute(
                                sqlx::query("INSERT INTO users ( discord_id ) VALUES ( ? )")
                                    .bind(discord_id.to_le_bytes().to_vec()),
                            )
                            .await?,
                        Some(discord_id.to_le_bytes().to_vec()),
                    ),
                };

                (res.last_insert_rowid(), None, discord_id)
            }
            Err(err) => return Err(err.into()),
            Ok(res) => res,
        };

        let role = role
            .filter(|role| ROLES.contains(&role.as_str()))
            .unwrap_or_else(|| DEFAULT_ROLE.into());
        let discord_id = discord_id.map(|data| {
            let mut bytes = [0u8; 8];
            bytes.clone_from_slice(&data[0..8]);
            u64::from_le_bytes(bytes)
        });

        Ok(User {
            uid,
            role,
            discord_id,
        })
    }

    pub async fn set_role_for_user(&self, user_id: UserId, role: &str) -> Result<()> {
        if !ROLES.contains(&role) {
            return Err(anyhow!("unknown role \"{}\"", role));
        }

        self.pool()
            .execute(
                sqlx::query("UPDATE users SET role = ? WHERE uid = ?")
                    .bind(role)
                    .bind(user_id),
            )
            .await?;

        Ok(())
    }

    pub async fn restrict_user(&self, user_id: UserId, restrictor_user_id: UserId) -> Result<()> {
        self.pool()
            .execute(
                sqlx::query("INSERT INTO restrictions ( uid, restrictor_user_id ) VALUES ( ?, ? )")
                    .bind(user_id)
                    .bind(restrictor_user_id),
            )
            .await?;

        Ok(())
    }

    pub async fn unrestrict_user(&self, user_id: UserId) -> Result<()> {
        self.pool()
            .execute(sqlx::query("DELETE FROM restrictions WHERE uid = ?").bind(user_id))
            .await?;

        Ok(())
    }

    pub async fn is_restricted(&self, user_id: UserId) -> Result<bool> {
        let restricted = sqlx::query_as("SELECT uid FROM restrictions WHERE uid = ?")
            .bind(user_id)
            .fetch_one(self.pool())
            .await
            .map(|_a: (i64,)| true)
            .or_else(|err| match err {
                sqlx::Error::RowNotFound => Ok(false),
                _ => Err(err),
            })?;

        Ok(restricted)
    }

    pub async fn get_channel_setting(
        &self,
        channel_id: ChannelId,
        key: &str,
    ) -> Result<Option<String>> {
        sqlx::query_as("SELECT value FROM settings_channel WHERE channel_id = ? AND key = ?")
            .bind(channel_id.to_short_str())
            .bind(key)
            .fetch_one(self.pool())
            .await
            .map(|val: (String,)| Some(val.0))
            .or_else(|err| match err {
                sqlx::Error::RowNotFound => Ok(None),
                _ => Err(err.into()),
            })
    }

    pub async fn save_channel_setting(
        &self,
        channel_id: ChannelId,
        key: &str,
        value: &str,
    ) -> Result<()> {
        self.pool()
            .execute(
                sqlx::query(
                    "REPLACE INTO settings_channel ( channel_id, key, value ) VALUES ( ?, ?, ? )",
                )
                .bind(channel_id.to_short_str())
                .bind(key)
                .bind(value),
            )
            .await?;

        Ok(())
    }

    pub async fn get_server_setting(
        &self,
        server_id: ServerId,
        key: &str,
    ) -> Result<Option<String>> {
        sqlx::query_as("SELECT value FROM settings_server WHERE server_id = ? AND key = ?")
            .bind(server_id.to_short_str())
            .bind(key)
            .fetch_one(self.pool())
            .await
            .map(|val: (String,)| Some(val.0))
            .or_else(|err| match err {
                sqlx::Error::RowNotFound => Ok(None),
                _ => Err(err.into()),
            })
    }

    pub async fn save_server_setting(
        &self,
        server_id: ServerId,
        key: &str,
        value: &str,
    ) -> Result<()> {
        self.pool()
            .execute(
                sqlx::query(
                    "REPLACE INTO settings_server ( server_id, key, value ) VALUES ( ?, ?, ? )",
                )
                .bind(server_id.to_short_str())
                .bind(key)
                .bind(value),
            )
            .await?;

        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

#[derive(Clone)]
pub struct User {
    pub uid: UserId,
    pub role: String,
    pub discord_id: Option<u64>,
}
