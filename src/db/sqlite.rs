use std::fs;

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

use crate::config::DatabaseConfig;

pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = config.path.parent() {
        fs::create_dir_all(parent)?;
    }
    let url = format!("sqlite://{}?mode=rwc", config.path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous=NORMAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout=5000;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await?;
    Ok(pool)
}
