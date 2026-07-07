//! SQLite schema revisions (table-structure versioning).
//!
//! This is **not** dujiaoka / MySQL data import. For that, a separate import path
//! would live outside this module.

use std::path::Path;

use sqlx::{SqliteConnection, SqlitePool};

/// Bump when adding a new schema revision (must match `REVISIONS.len()`).
pub const SCHEMA_VERSION: i32 = 14;

const REVISIONS: &[(&str, &str)] = &[
    ("0001_core", include_str!("../../schema/0001_core.sql")),
    (
        "0002_catalog",
        include_str!("../../schema/0002_catalog.sql"),
    ),
    (
        "0003_order_payment_fulfillment",
        include_str!("../../schema/0003_order_payment_fulfillment.sql"),
    ),
    (
        "0004_jobs_notifications",
        include_str!("../../schema/0004_jobs_notifications.sql"),
    ),
    (
        "0005_admin_sessions",
        include_str!("../../schema/0005_admin_sessions.sql"),
    ),
    (
        "0006_admin_rbac_notifications",
        include_str!("../../schema/0006_admin_rbac_notifications.sql"),
    ),
    (
        "0007_plan39_compat_ops",
        include_str!("../../schema/0007_plan39_compat_ops.sql"),
    ),
    (
        "0008_state_machine_v2",
        include_str!("../../schema/0008_state_machine_v2.sql"),
    ),
    (
        "0009_orders_soft_delete",
        include_str!("../../schema/0009_orders_soft_delete.sql"),
    ),
    (
        "0010_purchase_rate",
        include_str!("../../schema/0010_purchase_rate.sql"),
    ),
    (
        "0011_admin_refresh_tokens",
        include_str!("../../schema/0011_admin_refresh_tokens.sql"),
    ),
    (
        "0012_evm_local_payments",
        include_str!("../../schema/0012_evm_local_payments.sql"),
    ),
    (
        "0013_evm_local_alchemy",
        include_str!("../../schema/0013_evm_local_alchemy.sql"),
    ),
    (
        "0014_evm_local_network_env",
        include_str!("../../schema/0014_evm_local_network_env.sql"),
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaRevisionStatus {
    pub database_revision: i32,
    pub application_revision: i32,
}

impl SchemaRevisionStatus {
    pub fn is_current(&self) -> bool {
        self.database_revision >= self.application_revision
    }

    pub fn is_newer_than_app(&self) -> bool {
        self.database_revision > self.application_revision
    }

    pub fn needs_upgrade(&self) -> bool {
        self.database_revision < self.application_revision
    }
}

pub async fn revision_status(pool: &SqlitePool) -> anyhow::Result<SchemaRevisionStatus> {
    let mut conn = pool.acquire().await?;
    ensure_legacy_migrations_table(&mut conn).await?;
    let mut database_revision = get_user_version(&mut conn).await?;
    if database_revision == 0 {
        let legacy = legacy_applied_revision_count(&mut conn).await?;
        if legacy > 0 {
            database_revision = legacy;
        }
    }
    Ok(SchemaRevisionStatus {
        database_revision,
        application_revision: SCHEMA_VERSION,
    })
}

pub async fn apply(pool: &SqlitePool, db_path: &Path) -> anyhow::Result<()> {
    assert_revision_manifest();

    let mut conn = pool.acquire().await?;
    ensure_legacy_migrations_table(&mut conn).await?;

    let mut version = get_user_version(&mut conn).await?;
    if version == 0 {
        let legacy = legacy_applied_revision_count(&mut conn).await?;
        if legacy > 0 {
            set_user_version(&mut conn, legacy).await?;
            version = legacy;
            tracing::info!(
                legacy_revisions = legacy,
                "synced PRAGMA user_version from schema_migrations"
            );
        }
    }

    if version > SCHEMA_VERSION {
        anyhow::bail!(
            "database schema version {version} is newer than this application supports ({SCHEMA_VERSION}); upgrade free-market"
        );
    }

    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    if version > 0 {
        drop(conn);
        backup_database_file(pool, db_path).await?;
        conn = pool.acquire().await?;
    }

    sqlx::query("SAVEPOINT schema_revision")
        .execute(&mut *conn)
        .await?;

    let apply_result: anyhow::Result<()> = async {
        let mut current = version;
        while current < SCHEMA_VERSION {
            let index = current as usize;
            let (name, sql) = REVISIONS[index];
            apply_revision(&mut conn, name, sql).await?;
            current += 1;
            set_user_version(&mut conn, current).await?;
            tracing::info!(
                revision = name,
                schema_version = current,
                "schema revision applied"
            );
        }
        Ok(())
    }
    .await;

    match apply_result {
        Ok(()) => {
            sqlx::query("RELEASE schema_revision")
                .execute(&mut *conn)
                .await?;
            Ok(())
        }
        Err(err) => {
            let _ = sqlx::query("ROLLBACK TO schema_revision")
                .execute(&mut *conn)
                .await;
            let _ = sqlx::query("RELEASE schema_revision")
                .execute(&mut *conn)
                .await;
            Err(err)
        }
    }
}

fn assert_revision_manifest() {
    assert_eq!(
        REVISIONS.len() as i32,
        SCHEMA_VERSION,
        "SCHEMA_VERSION must equal REVISIONS.len()"
    );
}

async fn ensure_legacy_migrations_table(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )
    .execute(conn)
    .await?;
    Ok(())
}

async fn get_user_version(conn: &mut SqliteConnection) -> anyhow::Result<i32> {
    let version: i32 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *conn)
        .await?;
    Ok(version)
}

async fn set_user_version(conn: &mut SqliteConnection, version: i32) -> anyhow::Result<()> {
    if version < 0 {
        anyhow::bail!("schema user_version cannot be negative");
    }
    let sql = format!("PRAGMA user_version = {version}");
    sqlx::query(&sql).execute(&mut *conn).await?;
    Ok(())
}

async fn legacy_applied_revision_count(conn: &mut SqliteConnection) -> anyhow::Result<i32> {
    let mut count = 0_i32;
    for (name, _) in REVISIONS {
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT version FROM schema_migrations WHERE version = ?")
                .bind(name)
                .fetch_optional(&mut *conn)
                .await?;
        if exists.is_none() {
            break;
        }
        count += 1;
    }
    Ok(count)
}

async fn apply_revision(
    conn: &mut SqliteConnection,
    name: &str,
    sql: &str,
) -> anyhow::Result<()> {
    let already: Option<(String,)> =
        sqlx::query_as("SELECT version FROM schema_migrations WHERE version = ?")
            .bind(name)
            .fetch_optional(&mut *conn)
            .await?;
    if already.is_some() {
        return Ok(());
    }

    for statement in sql.split(';') {
        let statement = statement.trim();
        if !statement.is_empty() {
            sqlx::query(statement).execute(&mut *conn).await?;
        }
    }
    sqlx::query("INSERT INTO schema_migrations(version, applied_at) VALUES (?, ?)")
        .bind(name)
        .bind(crate::time::now_str())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn backup_database_file(pool: &SqlitePool, db_path: &Path) -> anyhow::Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    sqlx::query("PRAGMA wal_checkpoint(FULL)")
        .execute(pool)
        .await?;

    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let file_name = db_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("free-market.db");
    let backup_name = format!("{file_name}.pre-schema-{stamp}.bak");
    let backup_path = db_path.with_file_name(backup_name);
    std::fs::copy(db_path, &backup_path)?;
    tracing::info!(path = %backup_path.display(), "pre-schema-upgrade database backup created");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn memory_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory pool")
    }

    #[test]
    fn revision_manifest_matches_schema_version() {
        assert_revision_manifest();
    }

    #[tokio::test]
    async fn fresh_database_reaches_schema_version() {
        let pool = memory_pool().await;
        apply(&pool, std::path::Path::new(":memory:"))
            .await
            .expect("apply schema");
        let mut conn = pool.acquire().await.expect("conn");
        let version = get_user_version(&mut conn).await.expect("read version");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn legacy_schema_migrations_table_bootstraps_user_version() {
        let pool = memory_pool().await;
        let mut conn = pool.acquire().await.expect("conn");
        ensure_legacy_migrations_table(&mut conn)
            .await
            .expect("legacy table");
        for (name, sql) in &REVISIONS[..3] {
            apply_revision(&mut conn, name, sql)
                .await
                .expect("apply legacy revision");
        }
        let version = get_user_version(&mut conn).await.expect("read version");
        assert_eq!(version, 0);
        drop(conn);

        apply(&pool, std::path::Path::new(":memory:"))
            .await
            .expect("apply remaining revisions");

        let mut conn = pool.acquire().await.expect("conn");
        let version = get_user_version(&mut conn).await.expect("read version");
        assert_eq!(version, SCHEMA_VERSION);

        let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&mut *conn)
            .await
            .expect("count revisions");
        assert_eq!(applied, SCHEMA_VERSION as i64);
    }

    #[tokio::test]
    async fn newer_database_version_is_rejected() {
        let pool = memory_pool().await;
        let mut conn = pool.acquire().await.expect("conn");
        set_user_version(&mut conn, SCHEMA_VERSION + 1)
            .await
            .expect("set version");
        drop(conn);
        let err = apply(&pool, std::path::Path::new(":memory:"))
            .await
            .expect_err("should reject newer db");
        assert!(err.to_string().contains("newer than this application"));
    }
}
