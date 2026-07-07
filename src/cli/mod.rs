use std::{io::Write, net::IpAddr};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use sqlx::{Row, SqlitePool};

use crate::{build_info, config::AppConfig, db, security::password};

#[derive(Debug, Parser)]
#[command(name = "free-market", version, about = "freeMarket self-service card shop backend")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Check database connectivity only (for systemd probes or `free-market healthcheck`).
    #[arg(long, hide = true)]
    pub healthcheck: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the web server (default when no subcommand is given).
    Serve {
        /// TCP listen address (overrides config).
        #[arg(long, default_value = "0.0.0.0")]
        host: IpAddr,
        /// TCP listen port (overrides config).
        #[arg(long)]
        port: Option<u16>,
    },
    /// Check database connectivity; exits 0 on success.
    Healthcheck,
    /// Print build metadata (commit, build time, etc.).
    Version,
    /// Show database schema revision (read-only; does not start the web server).
    SchemaVersion,
    /// Inspect runtime configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage administrator accounts.
    Admin {
        #[command(subcommand)]
        action: AdminAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print the effective configuration (sensitive fields are redacted).
    Show,
    /// Print the config file path in use, or a hint when none was found.
    Path,
}

#[derive(Debug, Subcommand)]
pub enum AdminAction {
    /// List all administrators.
    List,
    /// Reset an administrator password; prompts twice when --password is omitted.
    ResetPassword {
        username: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// Create an administrator. Prompts for a password when --password is omitted.
    Create {
        username: String,
        #[arg(long, default_value = "owner")]
        role: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        password: Option<String>,
    },
    /// Enable an administrator account.
    Activate { username: String },
    /// Disable an administrator account.
    Deactivate { username: String },
}

#[derive(Debug, Default)]
pub struct ServeOptions {
    pub host: Option<IpAddr>,
    pub port: Option<u16>,
}

/// Route CLI arguments to the matching handler.
/// Returns `Ok(None)` when the process should exit after handling a subcommand.
/// Returns `Ok(Some(options))` when the web server should start.
pub async fn dispatch(cli: Cli) -> anyhow::Result<Option<ServeOptions>> {
    if cli.healthcheck {
        run_healthcheck().await?;
        return Ok(None);
    }
    match cli.command {
        None => Ok(Some(ServeOptions::default())),
        Some(Command::Serve { host, port }) => Ok(Some(ServeOptions {
            host: Some(host),
            port,
        })),
        Some(Command::Healthcheck) => {
            run_healthcheck().await?;
            Ok(None)
        }
        Some(Command::Version) => {
            run_version();
            Ok(None)
        }
        Some(Command::SchemaVersion) => {
            run_schema_version().await?;
            Ok(None)
        }
        Some(Command::Config { action }) => {
            run_config(action)?;
            Ok(None)
        }
        Some(Command::Admin { action }) => {
            run_admin(action).await?;
            Ok(None)
        }
    }
}

async fn run_healthcheck() -> anyhow::Result<()> {
    let config = AppConfig::load().context("load config")?;
    let pool = db::sqlite::connect(&config.database)
        .await
        .context("connect sqlite")?;
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&pool)
        .await
        .context("sqlite ping")?;
    pool.close().await;
    Ok(())
}

fn run_version() {
    print!("{}", build_info::version_text());
}

async fn run_schema_version() -> anyhow::Result<()> {
    let config = AppConfig::load().context("load config")?;
    let pool = db::sqlite::connect(&config.database)
        .await
        .context("connect sqlite")?;
    let status = db::schema::revision_status(&pool)
        .await
        .context("read schema revision")?;
    pool.close().await;

    println!("database path: {}", config.database.path.display());
    println!("database schema revision: {}", status.database_revision);
    println!(
        "application supports up to schema revision: {}",
        status.application_revision
    );
    if status.is_newer_than_app() {
        println!("status: database is newer than this binary — upgrade free-market");
        std::process::exit(2);
    }
    if status.needs_upgrade() {
        println!(
            "status: upgrade required — start `free-market` once to apply pending schema revisions"
        );
        std::process::exit(1);
    }
    println!("status: up to date");
    Ok(())
}

fn run_config(action: ConfigAction) -> anyhow::Result<()> {
    match action {
        ConfigAction::Show => {
            let mut cfg = AppConfig::load().context("load config")?;
            if !cfg.admin.bootstrap_password.is_empty() {
                cfg.admin.bootstrap_password = "***".to_string();
            }
            if !cfg.admin.app_secret.is_empty() {
                cfg.admin.app_secret = "***".to_string();
            }
            let rendered = toml::to_string_pretty(&cfg).context("serialize config")?;
            print!("{rendered}");
        }
        ConfigAction::Path => {
            let explicit = std::env::var("FREEMARKET_CONFIG").ok();
            if let Some(p) = explicit {
                println!("{p}  (FREEMARKET_CONFIG)");
                return Ok(());
            }
            for candidate in default_config_candidates() {
                if candidate.exists() {
                    println!("{}", candidate.display());
                    return Ok(());
                }
            }
            println!("config.toml not found; using compiled-in defaults.");
        }
    }
    Ok(())
}

fn default_config_candidates() -> Vec<std::path::PathBuf> {
    let mut out = vec![std::path::PathBuf::from("config.toml")];
    if let Some(home) = std::env::var_os("HOME").filter(|s| !s.is_empty()) {
        out.push(std::path::PathBuf::from(home).join(".free-market/config.toml"));
    }
    out
}

async fn run_admin(action: AdminAction) -> anyhow::Result<()> {
    let config = AppConfig::load().context("load config")?;
    let pool = db::sqlite::connect(&config.database)
        .await
        .context("connect sqlite")?;
    let result = handle_admin(&pool, action).await;
    pool.close().await;
    result
}

async fn handle_admin(pool: &SqlitePool, action: AdminAction) -> anyhow::Result<()> {
    match action {
        AdminAction::List => {
            let rows = sqlx::query(
                "SELECT id, username, COALESCE(display_name, '') AS display_name, \
                        COALESCE(role, 'owner') AS role, is_active, \
                        COALESCE(created_at, '') AS created_at \
                 FROM admins ORDER BY id ASC",
            )
            .fetch_all(pool)
            .await?;
            if rows.is_empty() {
                println!("(no administrators yet — visit /setup to finish first-time setup)");
                return Ok(());
            }
            println!(
                "{:>4}  {:<20}  {:<20}  {:<8}  {:<6}  {}",
                "ID", "USERNAME", "DISPLAY NAME", "ROLE", "ACTIVE", "CREATED AT"
            );
            for row in rows {
                let id: i64 = row.try_get("id")?;
                let username: String = row.try_get("username")?;
                let display_name: String = row.try_get("display_name")?;
                let role: String = row.try_get("role")?;
                let is_active: i64 = row.try_get("is_active")?;
                let created_at: String = row.try_get("created_at")?;
                println!(
                    "{:>4}  {:<20}  {:<20}  {:<8}  {:<6}  {}",
                    id,
                    username,
                    display_name,
                    role,
                    if is_active == 1 { "yes" } else { "no" },
                    created_at,
                );
            }
        }
        AdminAction::ResetPassword { username, password } => {
            ensure_user_exists(pool, &username).await?;
            let pwd = match password {
                Some(p) => validate_password(p)?,
                None => prompt_password_twice("New password: ", "Confirm password: ")?,
            };
            let hash = password::hash_password(&pwd).context("hash password")?;
            let now = crate::time::now_str();
            let affected = sqlx::query(
                "UPDATE admins SET password_hash = ?, updated_at = ? WHERE username = ?",
            )
            .bind(&hash)
            .bind(&now)
            .bind(&username)
            .execute(pool)
            .await?
            .rows_affected();
            if affected == 0 {
                bail!("user not found: {username}");
            }
            println!("password reset for {username}");
        }
        AdminAction::Create {
            username,
            role,
            display_name,
            password,
        } => {
            if !["owner", "admin"].contains(&role.as_str()) {
                bail!("role must be owner or admin");
            }
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT id FROM admins WHERE username = ?")
                    .bind(&username)
                    .fetch_optional(pool)
                    .await?;
            if exists.is_some() {
                bail!("username already exists: {username}");
            }
            let pwd = match password {
                Some(p) => validate_password(p)?,
                None => prompt_password_twice("Password: ", "Confirm password: ")?,
            };
            let hash = password::hash_password(&pwd).context("hash password")?;
            let now = crate::time::now_str();
            let display = display_name.unwrap_or_else(|| username.clone());
            sqlx::query(
                "INSERT INTO admins(username, password_hash, display_name, role, is_active, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, 1, ?, ?)",
            )
            .bind(&username)
            .bind(&hash)
            .bind(&display)
            .bind(&role)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
            println!("created administrator {username} ({role})");
        }
        AdminAction::Activate { username } => set_active(pool, &username, 1).await?,
        AdminAction::Deactivate { username } => set_active(pool, &username, 0).await?,
    }
    Ok(())
}

async fn ensure_user_exists(pool: &SqlitePool, username: &str) -> anyhow::Result<()> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM admins WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        bail!("user not found: {username}");
    }
    Ok(())
}

async fn set_active(pool: &SqlitePool, username: &str, value: i64) -> anyhow::Result<()> {
    let now = crate::time::now_str();
    let affected =
        sqlx::query("UPDATE admins SET is_active = ?, updated_at = ? WHERE username = ?")
            .bind(value)
            .bind(&now)
            .bind(username)
            .execute(pool)
            .await?
            .rows_affected();
    if affected == 0 {
        bail!("user not found: {username}");
    }
    println!(
        "{} user {username}",
        if value == 1 { "activated" } else { "deactivated" }
    );
    Ok(())
}

fn prompt_password_twice(prompt1: &str, prompt2: &str) -> anyhow::Result<String> {
    let first = rpassword::prompt_password(prompt1).context("read password")?;
    let first = validate_password(first)?;
    let second = rpassword::prompt_password(prompt2).context("read password")?;
    if first != second {
        bail!("passwords do not match");
    }
    Ok(first)
}

fn validate_password(pwd: String) -> anyhow::Result<String> {
    if pwd.len() < 8 {
        bail!("password must be at least 8 characters");
    }
    std::io::stdout().flush().ok();
    Ok(pwd)
}
