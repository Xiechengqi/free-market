use std::net::SocketAddr;

use axum::{extract::ConnectInfo, http::HeaderMap};

use crate::services::settings_service;

/// Best-effort client IP extraction honoring the configured `trust_proxy_hops`.
///
/// When `trust_proxy_hops > 0`, walks `X-Forwarded-For` from right to left and skips
/// `trust_proxy_hops` trusted proxies before returning the next address.
/// Falls back to `X-Real-IP` if XFF is missing.
/// When `trust_proxy_hops == 0`, ignores forwarded headers and returns the peer
/// socket address (so spoofed XFF on direct connections is rejected).
pub async fn client_ip(
    state: &crate::state::AppState,
    headers: &HeaderMap,
    connect: Option<&ConnectInfo<SocketAddr>>,
) -> String {
    let security = settings_service::security_config(state).await;
    let direct = connect
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_default();
    if security.trust_proxy_hops <= 0 {
        return direct;
    }
    if let Some(xff) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    {
        let chain: Vec<&str> = xff
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if !chain.is_empty() {
            let skip = security.trust_proxy_hops as usize;
            let idx = chain.len().saturating_sub(skip.saturating_add(1));
            return chain.get(idx).copied().unwrap_or(chain[0]).to_string();
        }
    }
    if let Some(real_ip) = headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return real_ip.to_string();
    }
    direct
}
