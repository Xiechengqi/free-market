use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::{
    services::admin_service,
    state::AppState,
    time,
    web::admin::api::{ApiError, ApiResponse, ApiResult, auth::LoginTokens},
};

#[derive(Debug, Serialize)]
pub struct SetupStatus {
    pub installed: bool,
}

/// GET /admin/api/setup/status — public.
/// SPA checks this before deciding whether to show /setup or /login.
pub async fn status(State(state): State<AppState>) -> ApiResult<SetupStatus> {
    let installed = admin_service::is_installed(&state).await?;
    Ok(ApiResponse::ok(SetupStatus { installed }))
}

#[derive(Debug, Deserialize)]
pub struct SetupInput {
    #[serde(alias = "userName")]
    pub username: String,
    #[serde(alias = "displayName", default)]
    pub display_name: Option<String>,
    pub password: String,
    #[serde(alias = "passwordConfirm")]
    pub password_confirm: String,
    #[serde(alias = "siteName", default)]
    pub site_name: Option<String>,
    #[serde(alias = "logoText", default)]
    pub logo_text: Option<String>,
}

/// POST /admin/api/setup/install — public, but rejects once `admins` table is non-empty.
/// Creates the first owner + initial site_config in one transaction and immediately
/// returns access/refresh tokens so the SPA can log straight into the dashboard
/// without an extra round trip.
pub async fn install(
    State(state): State<AppState>,
    Json(input): Json<SetupInput>,
) -> ApiResult<LoginTokens> {
    let username = input.username.trim();
    if username.is_empty() {
        return Err(ApiError::bad_request("用户名不能为空"));
    }
    let display_name = input
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(username)
        .to_string();
    let site_name = input
        .site_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&state.config.site.name)
        .to_string();
    let logo_text = input
        .logo_text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&state.config.site.logo_text)
        .to_string();

    let form = admin_service::InstallForm {
        site_name,
        logo_text,
        username: username.to_string(),
        display_name,
        password: input.password.clone(),
        password_confirm: input.password_confirm.clone(),
    };
    let (admin_id, role) = admin_service::install_first_admin(&state, form).await?;

    // Immediately issue tokens so the SPA can navigate straight to /home.
    let access = state
        .jwt
        .sign_access(admin_id, &role)
        .map_err(|err| ApiError::internal(format!("sign access: {err}")))?;
    let jti = crate::security::jwt::new_jti();
    let (refresh, exp) = state
        .jwt
        .sign_refresh(admin_id, &jti)
        .map_err(|err| ApiError::internal(format!("sign refresh: {err}")))?;
    let now = time::now_str();
    let exp_str = chrono::DateTime::<chrono::Utc>::from_timestamp(exp, 0)
        .ok_or_else(|| ApiError::internal("bad exp"))?
        .to_rfc3339();
    sqlx::query(
        "INSERT INTO admin_refresh_tokens(jti, admin_id, issued_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&jti)
    .bind(admin_id)
    .bind(&now)
    .bind(&exp_str)
    .execute(&state.pool)
    .await?;
    Ok(ApiResponse::ok(LoginTokens {
        token: access,
        refresh_token: refresh,
    }))
}
