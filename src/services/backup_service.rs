use std::path::{Path, PathBuf};

use chrono::{Datelike, Timelike};
use flate2::{Compression, write::GzEncoder};
use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_weekday")]
    pub weekday: u32,
    #[serde(default = "default_hour")]
    pub hour: u32,
    #[serde(default = "default_keep_files")]
    pub keep_files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupFile {
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct BackupPageData {
    pub enabled: bool,
    pub weekday: u32,
    pub hour: u32,
    pub keep_files: usize,
    pub files: Vec<BackupFile>,
}

pub async fn page_data(state: &AppState) -> AppResult<BackupPageData> {
    let config = load_config(state).await;
    Ok(BackupPageData {
        enabled: config.enabled,
        weekday: config.weekday,
        hour: config.hour,
        keep_files: config.keep_files,
        files: list_backup_files(state).await?,
    })
}

pub async fn save_config(state: &AppState, config: BackupConfig) -> AppResult<()> {
    let normalized = BackupConfig {
        enabled: config.enabled,
        weekday: config.weekday.clamp(1, 7),
        hour: config.hour.min(23),
        keep_files: config.keep_files.clamp(1, 30),
    };
    let now = crate::time::now_str();
    sqlx::query(
        "INSERT INTO settings(key, value_json, created_at, updated_at)
         VALUES ('backup_config', ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(serde_json::to_string(&normalized).unwrap_or_else(|_| "{}".to_string()))
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn load_config(state: &AppState) -> BackupConfig {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'backup_config'")
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
    raw.and_then(|raw| serde_json::from_str::<BackupConfig>(&raw).ok())
        .unwrap_or_default()
        .normalized()
}

pub async fn create_manual_backup(state: &AppState) -> AppResult<PathBuf> {
    let config = load_config(state).await;
    let snapshot = create_snapshot(state).await?;
    let bytes = tokio::fs::read(&snapshot)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let _ = tokio::fs::remove_file(&snapshot).await;
    let compressed = gzip_bytes(&bytes)?;
    let dir = backup_dir(state);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let path = dir.join(backup_filename("manual"));
    tokio::fs::write(&path, compressed)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    prune_backup_files(state, config.keep_files).await?;
    Ok(path)
}

pub async fn read_stored_backup(state: &AppState, filename: &str) -> AppResult<(String, Vec<u8>)> {
    if !valid_backup_filename(filename) {
        return Err(AppError::BadRequest("备份文件名不合法".to_string()));
    }
    let path = backup_dir(state).join(filename);
    let bytes = tokio::fs::read(&path).await.map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound("备份文件不存在".to_string())
        } else {
            AppError::Anyhow(err.into())
        }
    })?;
    Ok((filename.to_string(), bytes))
}

pub async fn run_scheduled_if_due(state: &AppState) -> anyhow::Result<()> {
    let config = load_config(state).await;
    if !config.enabled {
        return Ok(());
    }
    let now = crate::time::now();
    let weekday = now.weekday().number_from_monday();
    if weekday != config.weekday || now.hour() < config.hour {
        return Ok(());
    }
    let today = now.format("%Y-%m-%d").to_string();
    let last_run: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'backup_last_run_date'")
            .fetch_optional(&state.pool)
            .await?;
    if last_run
        .as_deref()
        .and_then(|raw| serde_json::from_str::<String>(raw).ok())
        .as_deref()
        == Some(today.as_str())
    {
        return Ok(());
    }

    create_scheduled_backup(state, &config).await?;
    let now_str = crate::time::now_str();
    sqlx::query(
        "INSERT INTO settings(key, value_json, created_at, updated_at)
         VALUES ('backup_last_run_date', ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(serde_json::to_string(&today)?)
    .bind(&now_str)
    .bind(&now_str)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn create_scheduled_backup(
    state: &AppState,
    config: &BackupConfig,
) -> AppResult<PathBuf> {
    let snapshot = create_snapshot(state).await?;
    let bytes = tokio::fs::read(&snapshot)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let _ = tokio::fs::remove_file(&snapshot).await;
    let compressed = gzip_bytes(&bytes)?;
    let dir = backup_dir(state);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let path = dir.join(backup_filename("scheduled"));
    tokio::fs::write(&path, compressed)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    prune_backup_files(state, config.keep_files).await?;
    Ok(path)
}

pub async fn list_backup_files(state: &AppState) -> AppResult<Vec<BackupFile>> {
    let dir = backup_dir(state);
    let mut files = Vec::new();
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(err) => return Err(AppError::Anyhow(err.into())),
    };
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("gz") {
            continue;
        }
        let metadata = entry
            .metadata()
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        let filename = entry.file_name().to_string_lossy().to_string();
        files.push(BackupFile {
            filename,
            size_bytes: metadata.len(),
            created_at: metadata
                .modified()
                .ok()
                .map(chrono::DateTime::<chrono::Utc>::from)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
        });
    }
    files.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(files)
}

async fn create_snapshot(state: &AppState) -> AppResult<PathBuf> {
    let parent = state
        .config
        .database
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let temp_dir = parent.join("backup-tmp");
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let temp_path = temp_dir.join(format!(
        "dujiao-backup-{}.sqlite",
        uuid::Uuid::new_v4().simple()
    ));
    let temp_sql = sqlite_string_literal(&temp_path.to_string_lossy());
    sqlx::query(&format!("VACUUM INTO {temp_sql}"))
        .execute(&state.pool)
        .await?;
    Ok(temp_path)
}

async fn prune_backup_files(state: &AppState, keep: usize) -> AppResult<()> {
    let files = list_backup_files(state).await?;
    for file in files.into_iter().skip(keep) {
        let _ = tokio::fs::remove_file(backup_dir(state).join(file.filename)).await;
    }
    Ok(())
}

fn gzip_bytes(bytes: &[u8]) -> AppResult<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::with_capacity(bytes.len() / 2), Compression::default());
    encoder
        .write_all(bytes)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    encoder.finish().map_err(|err| AppError::Anyhow(err.into()))
}

fn backup_dir(state: &AppState) -> PathBuf {
    state
        .config
        .database
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("backups")
}

fn backup_filename(kind: &str) -> String {
    format!(
        "dujiao-backup-{}-{}.sqlite.gz",
        kind,
        crate::time::now_str().replace(':', "-")
    )
}

fn sqlite_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn valid_backup_filename(filename: &str) -> bool {
    filename.starts_with("dujiao-backup-")
        && filename.ends_with(".sqlite.gz")
        && !filename.contains('/')
        && !filename.contains('\\')
        && !filename.contains("..")
}

fn default_enabled() -> bool {
    true
}

fn default_weekday() -> u32 {
    1
}

fn default_hour() -> u32 {
    8
}

fn default_keep_files() -> usize {
    7
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            weekday: default_weekday(),
            hour: default_hour(),
            keep_files: default_keep_files(),
        }
    }
}

impl BackupConfig {
    fn normalized(self) -> Self {
        Self {
            enabled: self.enabled,
            weekday: self.weekday.clamp(1, 7),
            hour: self.hour.min(23),
            keep_files: self.keep_files.clamp(1, 30),
        }
    }
}
