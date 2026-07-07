use axum::{
    extract::State,
    http::{HeaderMap, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;

use crate::{security::password, services::settings_service, state::AppState, time};

const ADMIN_SESSION_COOKIE: &str = "dujiao_admin_session";

#[derive(Debug, Clone)]
pub struct AdminContext {
    pub id: i64,
    #[allow(dead_code)]
    pub role: String,
}

pub fn new_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn session_cookie(admin_prefix: &str, secure: bool, token: &str) -> String {
    let mut cookie = format!(
        "{}={}; Path={}; HttpOnly; SameSite=Lax; Max-Age={}",
        ADMIN_SESSION_COOKIE,
        token,
        admin_prefix,
        60 * 60 * 12
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn expired_session_cookie(admin_prefix: &str, secure: bool) -> String {
    let mut cookie = format!(
        "{}=; Path={}; HttpOnly; SameSite=Lax; Max-Age=0",
        ADMIN_SESSION_COOKIE, admin_prefix
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let mut kv = part.trim().splitn(2, '=');
        let key = kv.next()?.trim();
        let value = kv.next()?.trim();
        (key == ADMIN_SESSION_COOKIE && !value.is_empty()).then(|| value.to_string())
    })
}

pub async fn create_session(
    state: &AppState,
    username: &str,
    raw_password: &str,
) -> anyhow::Result<Option<String>> {
    let row: Option<(i64, String, i64)> =
        sqlx::query_as("SELECT id, password_hash, is_active FROM admins WHERE username = ?")
            .bind(username)
            .fetch_optional(&state.pool)
            .await?;
    let Some((admin_id, password_hash, is_active)) = row else {
        return Ok(None);
    };
    if is_active != 1 || !password::verify_password(raw_password, &password_hash) {
        return Ok(None);
    }

    let token = new_token();
    let now = time::now_str();
    let expires_at = (time::now() + chrono::Duration::hours(12)).to_rfc3339();
    sqlx::query(
        "INSERT INTO admin_sessions(admin_id, token, expires_at, created_at, last_seen_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(admin_id)
    .bind(&token)
    .bind(expires_at)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(Some(token))
}

pub async fn login_blocked(state: &AppState, username: &str, ip: &str) -> anyhow::Result<bool> {
    let config = settings_service::security_config(state).await;
    let since = (time::now() - chrono::Duration::minutes(config.login_lock_minutes)).to_rfc3339();
    let failures: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_login_attempts
         WHERE username = ? AND ip = ? AND success = 0 AND created_at >= ?",
    )
    .bind(username)
    .bind(ip)
    .bind(since)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);
    Ok(failures >= config.login_max_attempts)
}

pub async fn record_login_attempt(
    state: &AppState,
    username: &str,
    ip: &str,
    success: bool,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO admin_login_attempts(username, ip, success, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(username)
    .bind(ip)
    .bind(if success { 1 } else { 0 })
    .bind(time::now_str())
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn destroy_session(state: &AppState, token: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM admin_sessions WHERE token = ?")
        .bind(token)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn admin_auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let login_path = state.admin_url("/login");
    let prefix = state.admin_prefix().to_string();
    let Some(token) = extract_session_token(&headers) else {
        return Redirect::to(&login_path).into_response();
    };
    let now = time::now_str();
    let row: Result<Option<(i64, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT s.admin_id, COALESCE(a.role, 'owner') AS role
             FROM admin_sessions s
             JOIN admins a ON a.id = s.admin_id
             WHERE s.token = ? AND s.expires_at > ? AND a.is_active = 1",
    )
    .bind(&token)
    .bind(&now)
    .fetch_optional(&state.pool)
    .await;
    match row {
        Ok(Some((admin_id, role))) => {
            if !role_allows(
                &role,
                &prefix,
                request.uri().path(),
                request.method().as_str(),
            ) {
                return StatusCode::FORBIDDEN.into_response();
            }
            let method = request.method().as_str().to_string();
            let path = request.uri().path().to_string();
            let _ = sqlx::query("UPDATE admin_sessions SET last_seen_at = ? WHERE token = ?")
                .bind(&now)
                .bind(&token)
                .execute(&state.pool)
                .await;
            let should_audit = method != "GET";
            request.extensions_mut().insert(AdminContext {
                id: admin_id,
                role: role.clone(),
            });
            let response = next.run(request).await;
            if should_audit {
                let _ = sqlx::query(
                    "INSERT INTO admin_audit_logs(admin_id, method, path, action, created_at)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(admin_id)
                .bind(method)
                .bind(&path)
                .bind(path_action(&path, &prefix))
                .bind(time::now_str())
                .execute(&state.pool)
                .await;
            }
            response
        }
        Ok(None) => Redirect::to(&login_path).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn path_action(path: &str, admin_prefix: &str) -> String {
    let stripped = path
        .strip_prefix(admin_prefix)
        .unwrap_or(path)
        .trim_start_matches('/');
    stripped.replace('/', ".").trim_matches('.').to_string()
}

fn role_allows(role: &str, admin_prefix: &str, path: &str, method: &str) -> bool {
    let admins_path = format!("{}/admins", admin_prefix);
    let settings_path = format!("{}/settings", admin_prefix);
    match role {
        "owner" => true,
        "operator" => {
            if path.starts_with(&admins_path) || path.starts_with(&settings_path) {
                return false;
            }
            true
        }
        "viewer" => method == "GET",
        _ => false,
    }
}
