use axum::{
    body::Body,
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::{security::jwt::AccessClaims, state::AppState, web::admin::api::response::ApiError};

/// Context attached to a request after `bearer_auth` middleware verifies the JWT.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthContext {
    pub admin_id: i64,
    pub role: String,
}

impl From<AccessClaims> for AuthContext {
    fn from(claims: AccessClaims) -> Self {
        Self {
            admin_id: claims.sub,
            role: claims.role,
        }
    }
}

/// Extracts and validates `Authorization: Bearer <jwt>`. On success, the
/// decoded `AuthContext` is stuffed into request extensions so handlers can
/// pull it via `Extension<AuthContext>`.
pub async fn bearer_auth(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let token = match extract_bearer(&request) {
        Some(t) => t,
        None => {
            return ApiError::unauthorized("missing access token").into_response();
        }
    };
    let data = match state.jwt.verify_access(&token) {
        Ok(d) => d,
        Err(err) => {
            use jsonwebtoken::errors::ErrorKind;
            let is_expired = matches!(err.kind(), ErrorKind::ExpiredSignature);
            if is_expired {
                return ApiError::token_expired("access token expired").into_response();
            }
            return ApiError::unauthorized(format!("invalid token: {err}")).into_response();
        }
    };
    if data.claims.typ != "access" {
        return ApiError::unauthorized("wrong token type").into_response();
    }
    // role check: ensure the admin is still active
    let row: Option<(i64, String)> = match sqlx::query_as(
        "SELECT id, COALESCE(role, 'owner') FROM admins WHERE id = ? AND is_active = 1",
    )
    .bind(data.claims.sub)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(v) => v,
        Err(_) => return ApiError::internal("db error").into_response(),
    };
    let Some((admin_id, role)) = row else {
        return ApiError::unauthorized("admin disabled").into_response();
    };
    request
        .extensions_mut()
        .insert(AuthContext { admin_id, role });
    next.run(request).await
}

fn extract_bearer(request: &Request<Body>) -> Option<String> {
    let raw = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("Bearer ") {
        if !rest.is_empty() {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Operator/viewer guard. Owners pass everything; operators are blocked from
/// `/auth/*` admin mgmt and global settings; viewers only see GET.
pub fn role_allows(role: &str, path: &str, method: &str) -> bool {
    match role {
        "owner" => true,
        "operator" => {
            if path.starts_with("/admin/api/admins") || path.starts_with("/admin/api/settings") {
                return false;
            }
            true
        }
        "viewer" => method == "GET",
        _ => false,
    }
}

#[allow(dead_code)]
pub fn forbidden() -> Response {
    ApiError::forbidden("forbidden").into_response()
}
