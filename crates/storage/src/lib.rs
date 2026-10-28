//! SQLite metadata connections and embedded, versioned migrations.

use std::{path::Path, time::Duration};

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database connection failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

/// Open/create a file database and apply pending migrations. Parent directories
/// must already exist. Foreign keys are enabled on every pooled connection.
pub async fn open_database(path: impl AsRef<Path>) -> Result<SqlitePool, StorageError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    connect(options, 5).await
}

/// A single-connection in-memory database, suitable for isolated tests.
pub async fn in_memory_database() -> Result<SqlitePool, StorageError> {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true);
    connect(options, 1).await
}

async fn connect(
    options: SqliteConnectOptions,
    max_connections: u32,
) -> Result<SqlitePool, StorageError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?;
    if let Err(error) = MIGRATOR.run(&pool).await {
        pool.close().await;
        return Err(error.into());
    }
    Ok(pool)
}
