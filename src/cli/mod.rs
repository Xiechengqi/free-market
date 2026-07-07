use std::io::Write;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use sqlx::{Row, SqlitePool};

use crate::{config::AppConfig, db, security::password};

#[derive(Debug, Parser)]
#[command(name = "dujiao-rust", version, about = "Dujiao Rust 自助发卡后端")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// 仅检查数据库连通性，用于容器 HEALTHCHECK（保留兼容旧 docker-compose）。
    #[arg(long, hide = true)]
    pub healthcheck: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 启动 Web 服务（默认行为）。
    Serve,
    /// 检查数据库连通性，成功返回 0。
    Healthcheck,
    /// 查看运行配置。
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// 管理员账户维护。
    Admin {
        #[command(subcommand)]
        action: AdminAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// 打印当前生效的配置（敏感字段会脱敏）。
    Show,
    /// 打印实际加载的 config 文件路径，未找到则提示。
    Path,
}

#[derive(Debug, Subcommand)]
pub enum AdminAction {
    /// 列出全部管理员。
    List,
    /// 重置管理员密码；未传 --password 则交互式输入并要求二次确认。
    ResetPassword {
        username: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// 创建一个管理员。未传 --password 则交互输入。
    Create {
        username: String,
        #[arg(long, default_value = "owner")]
        role: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        password: Option<String>,
    },
    /// 启用管理员账户。
    Activate { username: String },
    /// 禁用管理员账户。
    Deactivate { username: String },
}

/// 根据 CLI 参数路由到对应处理流程。
/// 返回 `Ok(true)` 表示需要继续启动 Web Server；`Ok(false)` 表示子命令已处理完毕，调用方应直接退出。
pub async fn dispatch(cli: Cli) -> anyhow::Result<bool> {
    if cli.healthcheck {
        run_healthcheck().await?;
        return Ok(false);
    }
    match cli.command {
        None | Some(Command::Serve) => Ok(true),
        Some(Command::Healthcheck) => {
            run_healthcheck().await?;
            Ok(false)
        }
        Some(Command::Config { action }) => {
            run_config(action)?;
            Ok(false)
        }
        Some(Command::Admin { action }) => {
            run_admin(action).await?;
            Ok(false)
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

fn run_config(action: ConfigAction) -> anyhow::Result<()> {
    match action {
        ConfigAction::Show => {
            let mut cfg = AppConfig::load().context("load config")?;
            // 脱敏：不打印 bootstrap_password 与 app_secret。
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
            let explicit = std::env::var("DUJIAO_CONFIG").ok();
            if let Some(p) = explicit {
                println!("{p}  (DUJIAO_CONFIG)");
                return Ok(());
            }
            for candidate in default_config_candidates() {
                if candidate.exists() {
                    println!("{}", candidate.display());
                    return Ok(());
                }
            }
            println!("未找到 config.toml，正在使用编译期默认值。");
        }
    }
    Ok(())
}

fn default_config_candidates() -> Vec<std::path::PathBuf> {
    let mut out = vec![std::path::PathBuf::from("config.toml")];
    if let Some(home) = std::env::var_os("HOME").filter(|s| !s.is_empty()) {
        out.push(std::path::PathBuf::from(home).join(".dujiao/config.toml"));
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
                println!("(暂无管理员，请先访问 /setup 完成首次安装)");
                return Ok(());
            }
            println!(
                "{:>4}  {:<20}  {:<20}  {:<8}  {:<6}  {}",
                "ID", "用户名", "显示名", "角色", "启用", "创建时间"
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
                None => prompt_password_twice("请输入新密码: ", "请再次确认: ")?,
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
                bail!("未找到用户名 {username}");
            }
            println!("已重置 {username} 的密码。");
        }
        AdminAction::Create {
            username,
            role,
            display_name,
            password,
        } => {
            if !["owner", "admin"].contains(&role.as_str()) {
                bail!("role 仅支持 owner 或 admin");
            }
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT id FROM admins WHERE username = ?")
                    .bind(&username)
                    .fetch_optional(pool)
                    .await?;
            if exists.is_some() {
                bail!("用户名 {username} 已存在");
            }
            let pwd = match password {
                Some(p) => validate_password(p)?,
                None => prompt_password_twice("请输入密码: ", "请再次确认: ")?,
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
            println!("已创建管理员 {username} ({role})。");
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
        bail!("未找到用户名 {username}");
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
        bail!("未找到用户名 {username}");
    }
    println!(
        "已{}用户 {username}。",
        if value == 1 { "启用" } else { "禁用" }
    );
    Ok(())
}

fn prompt_password_twice(prompt1: &str, prompt2: &str) -> anyhow::Result<String> {
    let first = rpassword::prompt_password(prompt1).context("read password")?;
    let first = validate_password(first)?;
    let second = rpassword::prompt_password(prompt2).context("read password")?;
    if first != second {
        bail!("两次输入的密码不一致");
    }
    Ok(first)
}

fn validate_password(pwd: String) -> anyhow::Result<String> {
    if pwd.len() < 8 {
        bail!("密码长度需 ≥ 8");
    }
    std::io::stdout().flush().ok();
    Ok(pwd)
}
