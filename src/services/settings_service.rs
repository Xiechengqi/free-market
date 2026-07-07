use serde::{Deserialize, Serialize};

use crate::{config::SiteConfig, state::AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfig {
    #[serde(default = "default_order_expire_minutes")]
    pub order_expire_minutes: i64,
    #[serde(default)]
    pub is_open_search_pwd: bool,
    #[serde(default)]
    pub purchase_rate_window_minutes: i64,
    #[serde(default)]
    pub purchase_rate_max_per_email: i64,
    #[serde(default)]
    pub purchase_rate_max_per_ip: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    #[serde(default = "default_theme")]
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaConfig {
    #[serde(default)]
    pub is_open_img_code: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_login_max_attempts")]
    pub login_max_attempts: i64,
    #[serde(default = "default_login_lock_minutes")]
    pub login_lock_minutes: i64,
    #[serde(default = "default_cookie_secure")]
    pub cookie_secure: bool,
    #[serde(default)]
    pub trust_proxy_hops: i64,
}

pub async fn runtime_site_config(state: &AppState) -> SiteConfig {
    let mut site = state.config.site.clone();
    if let Some(value) = setting_json(state, "site_config").await {
        site.name = str_value(&value, "name", &site.name);
        site.logo_text = str_value(&value, "logo_text", &site.logo_text);
        site.notice = str_value(&value, "notice", &site.notice);
        site.footer = str_value(&value, "footer", &site.footer);
        site.keywords = str_value(&value, "keywords", &site.keywords);
        site.description = str_value(&value, "description", &site.description);
        let base = str_value(&value, "base_url", &site.base_url);
        if !base.trim().is_empty() {
            site.base_url = base;
        }
        site.is_open_anti_red = value
            .get("is_open_anti_red")
            .and_then(|v| v.as_bool())
            .unwrap_or(site.is_open_anti_red);
        site.is_open_google_translate = value
            .get("is_open_google_translate")
            .and_then(|v| v.as_bool())
            .unwrap_or(site.is_open_google_translate);
        site.language = str_value(&value, "language", &site.language);
        site.img_logo = str_value(&value, "img_logo", &site.img_logo);
    }
    let order = order_config(state).await;
    let theme = theme_config(state).await;
    site.order_expire_minutes = order.order_expire_minutes;
    site.theme = theme.template;
    site
}

pub async fn order_config(state: &AppState) -> OrderConfig {
    setting_json(state, "order_config")
        .await
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_else(|| OrderConfig {
            order_expire_minutes: state.config.site.order_expire_minutes,
            is_open_search_pwd: false,
            purchase_rate_window_minutes: 0,
            purchase_rate_max_per_email: 0,
            purchase_rate_max_per_ip: 0,
        })
}

pub async fn theme_config(state: &AppState) -> ThemeConfig {
    setting_json(state, "theme_config")
        .await
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_else(|| ThemeConfig {
            template: state.config.site.theme.clone(),
        })
}

pub async fn captcha_config(state: &AppState) -> CaptchaConfig {
    setting_json(state, "captcha_config")
        .await
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

pub async fn security_config(state: &AppState) -> SecurityConfig {
    setting_json(state, "security_config")
        .await
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

pub async fn manage_email(state: &AppState) -> Option<String> {
    let value = setting_json(state, "site_config").await?;
    value
        .get("manage_email")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn setting_json(state: &AppState, key: &str) -> Option<serde_json::Value> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(&state.pool)
        .await
        .ok()?;
    raw.and_then(|raw| serde_json::from_str(&raw).ok())
}

fn str_value(value: &serde_json::Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or(default)
        .to_string()
}

impl Default for CaptchaConfig {
    fn default() -> Self {
        Self {
            is_open_img_code: false,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            login_max_attempts: default_login_max_attempts(),
            login_lock_minutes: default_login_lock_minutes(),
            cookie_secure: default_cookie_secure(),
            trust_proxy_hops: 0,
        }
    }
}

fn default_order_expire_minutes() -> i64 {
    5
}

fn default_theme() -> String {
    "luna".to_string()
}

fn default_login_max_attempts() -> i64 {
    5
}

fn default_login_lock_minutes() -> i64 {
    10
}

fn default_cookie_secure() -> bool {
    true
}
