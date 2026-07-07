use axum::{Extension, Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::{
    security::{
        jwt::{Jwt, new_jti},
        password,
    },
    state::AppState,
    time,
    web::admin::api::{ApiError, ApiResponse, ApiResult, middleware::AuthContext},
};

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    #[serde(alias = "userName")]
    pub user_name: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginTokens {
    pub token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
}

/// POST /admin/api/auth/login
/// body: { userName, password }
/// returns: { token, refreshToken } — `token` is the short-lived access JWT,
/// `refreshToken` is a 7-day JWT whose jti is tracked in admin_refresh_tokens.
pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginInput>,
) -> ApiResult<LoginTokens> {
    let username = input.user_name.trim();
    if username.is_empty() {
        return Err(ApiError::bad_request("用户名不能为空"));
    }
    let row: Option<(i64, String, i64, String)> = sqlx::query_as(
        "SELECT id, password_hash, is_active, COALESCE(role, 'owner') FROM admins WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await?;
    let Some((admin_id, password_hash, is_active, role)) = row else {
        return Err(ApiError::unauthorized("用户名或密码错误"));
    };
    if is_active != 1 {
        return Err(ApiError::forbidden("账号已禁用"));
    }
    if !password::verify_password(&input.password, &password_hash) {
        return Err(ApiError::unauthorized("用户名或密码错误"));
    }
    let tokens = issue_tokens(&state, admin_id, &role).await?;
    Ok(ApiResponse::ok(tokens))
}

async fn issue_tokens(
    state: &AppState,
    admin_id: i64,
    role: &str,
) -> Result<LoginTokens, ApiError> {
    let access = state
        .jwt
        .sign_access(admin_id, role)
        .map_err(|err| ApiError::internal(format!("sign access: {err}")))?;
    let jti = new_jti();
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
    Ok(LoginTokens {
        token: access,
        refresh_token: refresh,
    })
}

#[derive(Debug, Deserialize)]
pub struct RefreshInput {
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
}

/// POST /admin/api/auth/refreshToken
/// Rotates the refresh token: the old jti is revoked, a new pair is issued.
pub async fn refresh(
    State(state): State<AppState>,
    Json(input): Json<RefreshInput>,
) -> ApiResult<LoginTokens> {
    let data = state
        .jwt
        .verify_refresh(&input.refresh_token)
        .map_err(|err| ApiError::token_expired(format!("refresh token invalid: {err}")))?;
    if data.claims.typ != "refresh" {
        return Err(ApiError::unauthorized("wrong token type"));
    }

    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT revoked_at FROM admin_refresh_tokens WHERE jti = ? AND admin_id = ? AND expires_at > ?",
    )
    .bind(&data.claims.jti)
    .bind(data.claims.sub)
    .bind(time::now_str())
    .fetch_optional(&state.pool)
    .await?;
    let Some((revoked_at,)) = row else {
        return Err(ApiError::token_expired("refresh token not found"));
    };
    if revoked_at.is_some() {
        return Err(ApiError::token_expired("refresh token revoked"));
    }

    let admin_row: Option<(i64, String, i64)> =
        sqlx::query_as("SELECT id, COALESCE(role, 'owner'), is_active FROM admins WHERE id = ?")
            .bind(data.claims.sub)
            .fetch_optional(&state.pool)
            .await?;
    let Some((_, role, is_active)) = admin_row else {
        return Err(ApiError::unauthorized("admin not found"));
    };
    if is_active != 1 {
        return Err(ApiError::forbidden("账号已禁用"));
    }

    // Revoke the old refresh token (single-use rotation).
    sqlx::query("UPDATE admin_refresh_tokens SET revoked_at = ? WHERE jti = ?")
        .bind(time::now_str())
        .bind(&data.claims.jti)
        .execute(&state.pool)
        .await?;

    let tokens = issue_tokens(&state, data.claims.sub, &role).await?;
    Ok(ApiResponse::ok(tokens))
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "userName")]
    pub user_name: String,
    pub roles: Vec<String>,
    pub buttons: Vec<String>,
}

/// GET /admin/api/auth/getUserInfo
/// soybean expects userId/userName/roles[]; buttons[] is for per-button RBAC (unused).
pub async fn get_user_info(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<UserInfo> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT username, display_name FROM admins WHERE id = ?")
            .bind(ctx.admin_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((username, display_name)) = row else {
        return Err(ApiError::unauthorized("admin missing"));
    };
    Ok(ApiResponse::ok(UserInfo {
        user_id: ctx.admin_id.to_string(),
        user_name: display_name.unwrap_or(username),
        roles: vec![ctx.role.clone()],
        buttons: vec![],
    }))
}

/// POST /admin/api/auth/logout
/// Best-effort: revoke all this admin's refresh tokens so subsequent refreshes
/// fail. Access tokens stay valid until exp (≤30 min).
pub async fn logout(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<serde_json::Value> {
    sqlx::query(
        "UPDATE admin_refresh_tokens SET revoked_at = ? WHERE admin_id = ? AND revoked_at IS NULL",
    )
    .bind(time::now_str())
    .bind(ctx.admin_id)
    .execute(&state.pool)
    .await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// Silence the unused-import warning when Jwt isn't referenced after the file shrinks.
#[allow(dead_code)]
fn _retain_imports(_: &Jwt) {}
