use axum::{
    body::Body,
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

const REQUEST_ID_HEADER: &str = "x-request-id";

pub async fn request_id_middleware(mut req: Request<Body>, next: Next) -> Response {
    let header_name = HeaderName::from_static(REQUEST_ID_HEADER);
    let incoming = req
        .headers()
        .get(&header_name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let request_id = incoming.unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        req.headers_mut().insert(header_name.clone(), value);
    }
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let span = tracing::info_span!(
        "http",
        request_id = %request_id,
        method = %method,
        path = %path,
    );
    let _enter = span.enter();
    let start = std::time::Instant::now();
    let mut response = next.run(req).await;
    let elapsed_ms = start.elapsed().as_millis();
    if elapsed_ms > 500 {
        tracing::warn!(
            request_id = %request_id,
            method = %method,
            path = %path,
            elapsed_ms = elapsed_ms as u64,
            "slow request"
        );
    }
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(header_name, value);
    }
    response
}
