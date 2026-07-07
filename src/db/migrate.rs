use sqlx::SqlitePool;

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_core", include_str!("../../migrations/0001_core.sql")),
    (
        "0002_catalog",
        include_str!("../../migrations/0002_catalog.sql"),
    ),
    (
        "0003_order_payment_fulfillment",
        include_str!("../../migrations/0003_order_payment_fulfillment.sql"),
    ),
    (
        "0004_jobs_notifications",
        include_str!("../../migrations/0004_jobs_notifications.sql"),
    ),
    (
        "0005_admin_sessions",
        include_str!("../../migrations/0005_admin_sessions.sql"),
    ),
    (
        "0006_admin_rbac_notifications",
        include_str!("../../migrations/0006_admin_rbac_notifications.sql"),
    ),
    (
        "0007_plan39_compat_ops",
        include_str!("../../migrations/0007_plan39_compat_ops.sql"),
    ),
    (
        "0008_state_machine_v2",
        include_str!("../../migrations/0008_state_machine_v2.sql"),
    ),
    (
        "0009_orders_soft_delete",
        include_str!("../../migrations/0009_orders_soft_delete.sql"),
    ),
    (
        "0010_purchase_rate",
        include_str!("../../migrations/0010_purchase_rate.sql"),
    ),
    (
        "0011_admin_refresh_tokens",
        include_str!("../../migrations/0011_admin_refresh_tokens.sql"),
    ),
    (
        "0012_evm_local_payments",
        include_str!("../../migrations/0012_evm_local_payments.sql"),
    ),
    (
        "0013_evm_local_alchemy",
        include_str!("../../migrations/0013_evm_local_alchemy.sql"),
    ),
    (
        "0014_evm_local_network_env",
        include_str!("../../migrations/0014_evm_local_network_env.sql"),
    ),
];

pub async fn run(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )
    .execute(pool)
    .await?;

    for (version, sql) in MIGRATIONS {
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT version FROM schema_migrations WHERE version = ?")
                .bind(version)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            continue;
        }
        let mut tx = pool.begin().await?;
        for statement in sql.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
        }
        sqlx::query("INSERT INTO schema_migrations(version, applied_at) VALUES (?, ?)")
            .bind(version)
            .bind(crate::time::now_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        tracing::info!(migration = *version, "migration applied");
    }
    Ok(())
}
