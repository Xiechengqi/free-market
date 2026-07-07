use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;

use crate::state::AppState;

pub const CSRF_COOKIE: &str = "dujiao_csrf";

/// Max body size we'll buffer just to extract `_csrf` from a urlencoded form.
/// Larger bodies are rejected back to the user with 413 by the global
/// RequestBodyLimit; anything within that limit is safe to read here.
const CSRF_BODY_PEEK_LIMIT: usize = 64 * 1024;

#[allow(dead_code)]
pub fn new_csrf_token() -> String {
    let mut bytes = [0_u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn csrf_cookie(token: &str, secure: bool) -> String {
    let mut cookie = format!(
        "{}={}; Path=/; SameSite=Lax; Max-Age={}",
        CSRF_COOKIE,
        token,
        60 * 60 * 12
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn extract_csrf_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let mut kv = part.trim().splitn(2, '=');
        let key = kv.next()?.trim();
        let value = kv.next()?.trim();
        (key == CSRF_COOKIE && !value.is_empty()).then(|| value.to_string())
    })
}

pub async fn csrf_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // SPA API (`/admin/api/*`) is authenticated with `Authorization: Bearer …`,
    // which is itself unforgeable from a third-party origin. Skipping CSRF here
    // lets the SPA do plain JSON requests without juggling cookie tokens.
    let path = request.uri().path();
    let admin_prefix = state.admin_prefix();
    let api_prefix = format!("{}/api/", admin_prefix);
    if path.starts_with(&api_prefix) {
        return next.run(request).await;
    }

    if !requires_csrf(request.method()) {
        let secure = should_use_secure_cookie(&state, request.headers()).await;
        let mut response = next.run(request).await;
        // For navigations/asset GETs, seed the CSRF cookie matching the in-process token
        // so the first form load on a fresh browser already has cookie+form aligned.
        if !has_csrf_cookie_response(&response) {
            if let Ok(value) = csrf_cookie(&state.csrf_token, secure).parse() {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
        }
        return response;
    }

    // CSRF token can come from any of:
    //   1. `X-CSRF-Token` request header
    //   2. `_csrf` URL query param
    //   3. `_csrf` field in an `application/x-www-form-urlencoded` body
    // (3) requires consuming the body, which we then reattach for the downstream
    // handler. We deliberately skip body parsing for non-urlencoded bodies
    // (multipart uploads, JSON) — those must put the token in URL/header.
    let cookie_token = extract_csrf_cookie(request.headers());

    if let Some(form_token) =
        header_token(&request).or_else(|| query_token(request.uri().query().unwrap_or_default()))
    {
        return finish_check(cookie_token, Some(form_token), request, next).await;
    }

    let is_urlencoded = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|ct| ct.starts_with("application/x-www-form-urlencoded"))
        .unwrap_or(false);
    if !is_urlencoded {
        return finish_check(cookie_token, None, request, next).await;
    }

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, CSRF_BODY_PEEK_LIMIT).await {
        Ok(b) => b,
        Err(_) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "CSRF: body too large").into_response();
        }
    };
    let form_token = body_token(&bytes);
    let request = Request::from_parts(parts, Body::from(bytes));
    finish_check(cookie_token, form_token, request, next).await
}

async fn finish_check(
    cookie_token: Option<String>,
    form_token: Option<String>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let valid = match (cookie_token, form_token) {
        (Some(c), Some(f)) => constant_time_eq(c.as_bytes(), f.as_bytes()),
        _ => false,
    };
    if !valid {
        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
    }
    next.run(request).await
}

/// Decide whether `Secure` should be set on cookies for this request.
///
/// The app itself never terminates TLS — Cloudflare / a reverse proxy / a tunnel
/// does that and forwards `X-Forwarded-Proto: https` to us. So the rule is:
/// - `X-Forwarded-Proto: https` present → set Secure (browser will only resend
///   the cookie over HTTPS, which is what we want behind Cloudflare)
/// - Anything else (direct HTTP from a LAN IP / localhost / a plain proxy
///   without XFP) → no Secure, otherwise the browser would refuse to send the
///   cookie back over plain HTTP and CSRF / sessions would silently break.
///
/// `security_config.cookie_secure` acts as an additional kill switch: even
/// when XFP says https, an operator can force Secure off (rarely useful).
pub async fn should_use_secure_cookie(state: &AppState, headers: &HeaderMap) -> bool {
    let security = crate::services::settings_service::security_config(state).await;
    if !security.cookie_secure {
        return false;
    }
    headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .map(|proto| proto.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

fn requires_csrf(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn header_token(request: &Request<Body>) -> Option<String> {
    request
        .headers()
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn query_token(query: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        let value = parts.next().unwrap_or_default();
        (key == "_csrf").then(|| url_decode(value))
    })
}

fn body_token(body: &[u8]) -> Option<String> {
    let body_str = std::str::from_utf8(body).ok()?;
    body_str.split('&').find_map(|pair| {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        let value = parts.next().unwrap_or_default();
        (key == "_csrf").then(|| url_decode(value))
    })
}

fn url_decode(value: &str) -> String {
    let value = value.replace('+', " ");
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn has_csrf_cookie_response(response: &Response) -> bool {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|v| {
            v.to_str()
                .map(|s| s.starts_with(&format!("{}=", CSRF_COOKIE)))
                .unwrap_or(false)
        })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
