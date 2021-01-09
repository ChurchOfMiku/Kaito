use anyhow::Result;
use sqlx::{
    migrate::Migrator,
    sqlite::{Sqlite, SqliteConnectOptions, SqliteSynchronous},
    Pool,
};
use std::{path::Path, sync::Arc};

pub struct BotDb {
    pool: Pool<Sqlite>,
}

impl BotDb {
    pub async fn new(root_path: &Path, data_path: &Path) -> Result<Arc<BotDb>> {
        let pool = Pool::connect_with(
            SqliteConnectOptions::new()
                .filename(data_path.join("kaito.db"))
                .synchronous(SqliteSynchronous::Normal)
                .create_if_missing(true),
        )
        .await?;

        let m = Migrator::new(root_path.join("migrations")).await?;
        m.run(&pool).await?;

        Ok(Arc::new(BotDb { pool }))
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}
