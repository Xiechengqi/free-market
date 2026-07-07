use sqlx::SqlitePool;

use crate::{
    security::secrets::{SecretBox, SecretManager, is_encrypted},
    state::AppState,
};

const ROTATE_AFTER_DAYS: i64 = 90;

pub async fn migrate_from_legacy_secret(
    pool: &SqlitePool,
    old_box: &SecretBox,
    manager: &SecretManager,
) -> anyhow::Result<()> {
    let new_box = manager.current_box()?;
    reencrypt_sensitive_settings(pool, old_box, &new_box).await?;
    ensure_last_rotation_marker(pool).await?;
    Ok(())
}

pub async fn rotate_if_due(state: &AppState) -> anyhow::Result<()> {
    let Some(last_rotated) = last_rotation(state).await? else {
        set_last_rotation(&state.pool, crate::time::now_str()).await?;
        return Ok(());
    };
    let Some(last_rotated) = crate::time::parse_rfc3339(&last_rotated) else {
        set_last_rotation(&state.pool, crate::time::now_str()).await?;
        return Ok(());
    };
    if crate::time::now() - last_rotated < chrono::Duration::days(ROTATE_AFTER_DAYS) {
        return Ok(());
    }

    let old_box = state.secret_box.current_box()?;
    let (new_secret, new_box) = SecretManager::random_secret_box();
    reencrypt_sensitive_settings(&state.pool, &old_box, &new_box).await?;
    state.secret_box.install_secret(&new_secret, new_box)?;
    set_last_rotation(&state.pool, crate::time::now_str()).await?;
    tracing::info!("application secret rotated and sensitive settings re-encrypted");
    Ok(())
}

async fn last_rotation(state: &AppState) -> anyhow::Result<Option<String>> {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'secret_last_rotated_at'")
            .fetch_optional(&state.pool)
            .await?;
    Ok(raw.and_then(|raw| serde_json::from_str::<String>(&raw).ok()))
}

async fn ensure_last_rotation_marker(pool: &SqlitePool) -> anyhow::Result<()> {
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = 'secret_last_rotated_at'")
            .fetch_one(pool)
            .await?;
    if exists == 0 {
        set_last_rotation(pool, crate::time::now_str()).await?;
    }
    Ok(())
}

async fn set_last_rotation(pool: &SqlitePool, value: String) -> anyhow::Result<()> {
    let now = crate::time::now_str();
    sqlx::query(
        "INSERT INTO settings(key, value_json, created_at, updated_at)
         VALUES ('secret_last_rotated_at', ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(serde_json::to_string(&value)?)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

async fn reencrypt_sensitive_settings(
    pool: &SqlitePool,
    old_box: &SecretBox,
    new_box: &SecretBox,
) -> anyhow::Result<()> {
    reencrypt_setting(pool, "smtp_config", &["password"], old_box, new_box).await?;
    reencrypt_setting(
        pool,
        "notification_config",
        &[
            "server_chan_key",
            "telegram_bot_token",
            "bark_url",
            "wecom_webhook",
        ],
        old_box,
        new_box,
    )
    .await?;
    Ok(())
}

async fn reencrypt_setting(
    pool: &SqlitePool,
    key: &str,
    fields: &[&str],
    old_box: &SecretBox,
    new_box: &SecretBox,
) -> anyhow::Result<()> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    let Some(raw) = raw else {
        return Ok(());
    };
    let mut value: serde_json::Value = serde_json::from_str(&raw)?;
    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };
    let mut changed = false;
    for field in fields {
        let Some(raw_value) = object.get(*field).and_then(|value| value.as_str()) else {
            continue;
        };
        if raw_value.is_empty() {
            continue;
        }
        let plaintext = old_box.decrypt(raw_value);
        if plaintext.is_empty() && is_encrypted(raw_value) {
            anyhow::bail!("failed to decrypt sensitive setting {key}.{field}");
        }
        object.insert(
            (*field).to_string(),
            serde_json::json!(new_box.encrypt(&plaintext)),
        );
        changed = true;
    }
    if changed {
        let now = crate::time::now_str();
        sqlx::query("UPDATE settings SET value_json = ?, updated_at = ? WHERE key = ?")
            .bind(value.to_string())
            .bind(now)
            .bind(key)
            .execute(pool)
            .await?;
    }
    Ok(())
}
