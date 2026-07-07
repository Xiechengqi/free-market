use std::{
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub site: SiteConfig,
    pub admin: AdminConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
    #[serde(default = "default_true")]
    pub run_worker: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub name: String,
    pub logo_text: String,
    pub notice: String,
    pub footer: String,
    pub base_url: String,
    pub theme: String,
    pub order_expire_minutes: i64,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_open_anti_red: bool,
    #[serde(default)]
    pub is_open_google_translate: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub img_logo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    pub route_prefix: String,
    pub bootstrap_username: String,
    pub bootstrap_password: String,
    #[serde(default)]
    pub app_secret: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        // Priority:
        //   1. $FREEMARKET_CONFIG (explicit override)
        //   2. ./config.toml in CWD (common when running from the project dir)
        //   3. $HOME/.freemarket/config.toml (matches the default data dir)
        let explicit = std::env::var("FREEMARKET_CONFIG").ok();
        let candidates: Vec<PathBuf> = explicit
            .as_deref()
            .map(|p| vec![PathBuf::from(p)])
            .unwrap_or_else(|| {
                let mut paths = vec![PathBuf::from("config.toml")];
                if let Some(home) = home_dir() {
                    paths.push(home.join(".freemarket").join("config.toml"));
                }
                paths
            });
        for path in candidates {
            if let Ok(raw) = fs::read_to_string(&path) {
                return Ok(toml::from_str(&raw)?);
            }
        }
        Ok(Self::default())
    }

    /// Root data directory: the parent of `database.path`.
    /// All derived defaults (uploads, backups, app.secret) sit here.
    pub fn data_dir(&self) -> PathBuf {
        self.database
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Uploads root. Override by overriding `database.path` to a parent of choice.
    pub fn uploads_dir(&self) -> PathBuf {
        self.data_dir().join("uploads")
    }
}

/// `$HOME/.freemarket` on unix, falling back to CWD-relative `./.freemarket` if HOME isn't set.
pub fn default_data_dir() -> PathBuf {
    home_dir()
        .map(|home| home.join(".freemarket"))
        .unwrap_or_else(|| PathBuf::from(".freemarket"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".parse().expect("valid ip"),
                port: 8080,
                run_worker: true,
            },
            database: DatabaseConfig {
                path: default_data_dir().join("freemarket.db"),
            },
            site: SiteConfig {
                name: "freeMarket".to_string(),
                logo_text: "freeMarket".to_string(),
                notice: "欢迎使用 freeMarket。".to_string(),
                footer: String::new(),
                base_url: "http://0.0.0.0:8080".to_string(),
                theme: "luna".to_string(),
                order_expire_minutes: 5,
                keywords: String::new(),
                description: String::new(),
                is_open_anti_red: false,
                is_open_google_translate: false,
                language: default_language(),
                img_logo: String::new(),
            },
            admin: AdminConfig {
                route_prefix: "/admin".to_string(),
                bootstrap_username: "admin".to_string(),
                bootstrap_password: "admin123456".to_string(),
                app_secret: String::new(),
            },
        }
    }
}

impl AppConfig {
    pub fn effective_app_secret(&self) -> String {
        let cfg = self.admin.app_secret.trim().to_string();
        if !cfg.is_empty() {
            return cfg;
        }
        if let Ok(env) = std::env::var("FREEMARKET_APP_SECRET") {
            let trimmed = env.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
        // Legacy fallback only. New installs use data/app.secret, generated and rotated by
        // SecretManager; this value is used to migrate older encrypted settings once.
        format!(
            "fallback-key/{}/{}",
            self.admin.bootstrap_username, self.site.base_url
        )
    }
}

fn default_true() -> bool {
    true
}

fn default_language() -> String {
    "zh-CN".to_string()
}
