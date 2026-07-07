use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    config::AppConfig,
    db,
    security::{jwt::Jwt, secrets::SecretManager},
    view::render::ViewRenderer,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub pool: SqlitePool,
    pub views: Arc<ViewRenderer>,
    pub worker_id: String,
    pub csrf_token: String,
    pub secret_box: Arc<SecretManager>,
    pub jwt: Arc<Jwt>,
}

impl AppState {
    pub async fn build(config: AppConfig) -> anyhow::Result<Self> {
        let pool = db::sqlite::connect(&config.database).await?;
        db::schema::apply(&pool, &config.database.path).await?;
        let csrf_token = crate::security::session::new_token();
        let admin_prefix = {
            let raw = config.admin.route_prefix.trim();
            let candidate = if raw.is_empty() { "/admin" } else { raw };
            let mut s = candidate.to_string();
            if !s.starts_with('/') {
                s = format!("/{}", s);
            }
            while s.len() > 1 && s.ends_with('/') {
                s.pop();
            }
            s
        };
        let secret_path = config
            .database
            .path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("app.secret");
        let (secret_manager, legacy_box, secret_created) =
            SecretManager::load_or_create(secret_path, &config.effective_app_secret())?;
        let secret_box = Arc::new(secret_manager);
        if secret_created {
            crate::services::secret_rotation_service::migrate_from_legacy_secret(
                &pool,
                &legacy_box,
                &secret_box,
            )
            .await?;
        }
        Ok(Self {
            views: Arc::new(ViewRenderer::with_admin_prefix(
                csrf_token.clone(),
                admin_prefix,
            )?),
            jwt: Arc::new(Jwt::from_app_secret(&config.effective_app_secret())),
            config: Arc::new(config),
            pool,
            worker_id: format!("worker-{}", uuid::Uuid::new_v4()),
            csrf_token,
            secret_box,
        })
    }

    pub fn admin_prefix(&self) -> &str {
        let raw = self.config.admin.route_prefix.trim();
        if raw.is_empty() { "/admin" } else { raw }
    }

    pub fn admin_url(&self, suffix: &str) -> String {
        let prefix = self.admin_prefix().trim_end_matches('/');
        if suffix.is_empty() || suffix == "/" {
            return prefix.to_string();
        }
        if suffix.starts_with('/') {
            format!("{}{}", prefix, suffix)
        } else {
            format!("{}/{}", prefix, suffix)
        }
    }
}
