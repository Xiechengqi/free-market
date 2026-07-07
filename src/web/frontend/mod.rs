use axum::{
    Json,
    body::Bytes,
    extract::{ConnectInfo, Form, Path, Query, State},
    http::StatusCode,
    http::{HeaderMap, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde_json::json;
use std::{collections::HashMap, net::SocketAddr};

use crate::{
    error::AppResult,
    services::{
        captcha_service, catalog_service, order_service, payment_service, settings_service,
    },
    state::AppState,
    view::render::frontend_template,
};

pub async fn home(State(state): State<AppState>) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let data = catalog_service::home_data(&state).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "home.html"),
        &site,
        data,
    )?))
}

pub async fn buy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let buy_url = current_buy_url(&site.base_url, id, &headers);
    let data = catalog_service::product_for_buy(&state, id, user_agent, &buy_url).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "buy.html"),
        &site,
        data,
    )?))
}

pub async fn create_order(
    State(state): State<AppState>,
    connect: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Form(form): Form<order_service::CreateOrderForm>,
) -> AppResult<Response> {
    let ip = crate::security::net::client_ip(&state, &headers, connect.as_ref()).await;
    let order_no = order_service::create_guest_order(&state, form, ip).await?;
    let cookie_value = order_service::order_cookie_value(&headers, &order_no);
    let cookie = format!(
        "dujiaoka_orders={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
        percent_encode_cookie(&cookie_value)
    );
    let mut response = Redirect::to(&format!("/bill/{}", order_no)).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|err| {
            crate::error::AppError::Anyhow(anyhow::anyhow!("invalid cookie: {err}"))
        })?,
    );
    Ok(response)
}

pub async fn bill(
    State(state): State<AppState>,
    Path(order_no): Path<String>,
) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let data = order_service::bill_data(&state, &order_no).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "bill.html"),
        &site,
        data,
    )?))
}

pub async fn detail_order(
    State(state): State<AppState>,
    Path(order_no): Path<String>,
) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let data = order_service::detail_data(&state, &order_no).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "order.html"),
        &site,
        data,
    )?))
}

pub async fn search_page(State(state): State<AppState>) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    Ok(Html(state.views.render(
        &frontend_template(&site, "search.html"),
        &site,
        json!({}),
    )?))
}

pub async fn search_by_sn(
    State(state): State<AppState>,
    Form(form): Form<order_service::SearchBySnForm>,
) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let data = order_service::search_by_order_no(&state, form).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "order.html"),
        &site,
        data,
    )?))
}

pub async fn search_by_email(
    State(state): State<AppState>,
    Form(form): Form<order_service::SearchByEmailForm>,
) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let data = order_service::search_by_email(&state, form).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "orders.html"),
        &site,
        data,
    )?))
}

pub async fn search_by_browser(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let data = order_service::search_by_browser(&state, &headers).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "orders.html"),
        &site,
        data,
    )?))
}

pub async fn check_order_status(
    State(state): State<AppState>,
    Path(order_no): Path<String>,
) -> AppResult<impl IntoResponse> {
    let data = order_service::status_data(&state, &order_no).await?;
    Ok(Json(json!({
        "order_no": data.order_no,
        "status": data.status,
        "msg": data.msg,
        "code": data.code,
    })))
}

pub async fn pay_gateway(
    State(state): State<AppState>,
    Path((_handle, payway, order_no)): Path<(String, i64, String)>,
    headers: HeaderMap,
) -> AppResult<Html<String>> {
    payment_page(state, payway, order_no, headers).await
}

pub async fn provider_pay_gateway(
    State(state): State<AppState>,
    Path((_provider, payway, order_no)): Path<(String, i64, String)>,
    headers: HeaderMap,
) -> AppResult<Html<String>> {
    payment_page(state, payway, order_no, headers).await
}

async fn payment_page(
    state: AppState,
    payway: i64,
    order_no: String,
    headers: HeaderMap,
) -> AppResult<Html<String>> {
    let site = settings_service::runtime_site_config(&state).await;
    let data = payment_service::create_payment(&state, &order_no, payway, &headers).await?;
    Ok(Html(state.views.render(
        &frontend_template(&site, "pay.html"),
        &site,
        data,
    )?))
}

pub async fn payment_return(
    Query(params): Query<HashMap<String, String>>,
    Path(provider): Path<String>,
) -> AppResult<Redirect> {
    let order_no = params
        .get("order_id")
        .or_else(|| params.get("orderid"))
        .or_else(|| params.get("out_trade_no"))
        .or_else(|| params.get("trade_no"))
        .cloned()
        .unwrap_or_default();
    if order_no.trim().is_empty() {
        return Ok(Redirect::to("/order-search"));
    }
    tracing::info!(provider, order_no, "payment return");
    Ok(Redirect::to(&format!("/detail-order-sn/{}", order_no)))
}

pub async fn captcha_svg(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = id.trim_end_matches(".svg");
    if let Some(svg) = captcha_service::svg(&state, id).await? {
        let mut response = svg.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            "image/svg+xml; charset=utf-8".parse().map_err(|err| {
                crate::error::AppError::Anyhow(anyhow::anyhow!("invalid content-type: {err}"))
            })?,
        );
        return Ok(response);
    }
    Ok(StatusCode::NOT_FOUND.into_response())
}

pub async fn noop_success(
    State(state): State<AppState>,
    Path(payment_no): Path<String>,
) -> AppResult<Redirect> {
    let order_no = payment_service::noop_success(&state, &payment_no).await?;
    Ok(Redirect::to(&format!("/detail-order-sn/{}", order_no)))
}

pub async fn epay_callback_get(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<String> {
    payment_service::epay_callback(&state, &params).await
}

pub async fn provider_callback_get(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<String> {
    dispatch_provider_callback(&state, &provider, &params).await
}

pub async fn provider_channel_callback_get(
    State(state): State<AppState>,
    Path((provider, _channel_type)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<String> {
    dispatch_provider_callback(&state, &provider, &params).await
}

pub async fn provider_channel_callback_post(
    State(state): State<AppState>,
    Path((provider, _channel_type)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<String> {
    provider_callback_post(State(state), Path(provider), headers, body).await
}

pub async fn provider_callback_post(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<String> {
    let params = parse_callback_body(&headers, &body)?;
    match provider.to_ascii_lowercase().as_str() {
        "stripe" | "paypal" | "dujiaopay" | "wechat" | "wxpay" => {
            payment_service::provider_webhook(&state, &provider, &headers, &body).await
        }
        _ => dispatch_provider_callback_with_body(&state, &provider, &params, &body).await,
    }
}

async fn dispatch_provider_callback(
    state: &AppState,
    provider: &str,
    params: &HashMap<String, String>,
) -> AppResult<String> {
    dispatch_provider_callback_with_body(state, provider, params, &[]).await
}

async fn dispatch_provider_callback_with_body(
    state: &AppState,
    provider: &str,
    params: &HashMap<String, String>,
    body: &[u8],
) -> AppResult<String> {
    match provider.to_ascii_lowercase().as_str() {
        "epay" | "yipay" => payment_service::epay_callback(state, params).await,
        "tokenpay" => payment_service::tokenpay_callback(state, params).await,
        "epusdt" => payment_service::epusdt_callback(state, params).await,
        "bepusdt" | "okpay" | "alipay" => {
            payment_service::provider_form_callback(state, provider, params, body).await
        }
        _ => Err(crate::error::AppError::BadRequest(
            "支付 provider 未注册".to_string(),
        )),
    }
}

pub async fn epay_callback_post(
    State(state): State<AppState>,
    Form(params): Form<HashMap<String, String>>,
) -> AppResult<String> {
    payment_service::epay_callback(&state, &params).await
}

pub async fn tokenpay_callback_get(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<String> {
    payment_service::tokenpay_callback(&state, &params).await
}

pub async fn tokenpay_callback_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<String> {
    let params = parse_callback_body(&headers, &body)?;
    payment_service::provider_form_callback(&state, "tokenpay", &params, &body).await
}

pub async fn epusdt_callback_get(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<String> {
    payment_service::epusdt_callback(&state, &params).await
}

pub async fn epusdt_callback_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<String> {
    let params = parse_callback_body(&headers, &body)?;
    payment_service::provider_form_callback(&state, "epusdt", &params, &body).await
}

fn parse_callback_body(headers: &HeaderMap, body: &[u8]) -> AppResult<HashMap<String, String>> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if content_type.contains("application/json") {
        let value: serde_json::Value = serde_json::from_slice(body).map_err(|err| {
            crate::error::AppError::BadRequest(format!("JSON 回调格式错误: {err}"))
        })?;
        let object = value
            .as_object()
            .ok_or_else(|| crate::error::AppError::BadRequest("JSON 回调必须是对象".to_string()))?;
        return Ok(object
            .iter()
            .map(|(key, value)| {
                let value = value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string().trim_matches('"').to_string());
                (key.clone(), value)
            })
            .collect());
    }
    let raw = String::from_utf8_lossy(body);
    Ok(raw
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or_default();
            Some((url_decode(key), url_decode(value)))
        })
        .collect())
}

fn percent_encode_cookie(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char)
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

fn current_buy_url(base_url: &str, id: i64, headers: &HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if host.is_empty() {
        return format!("{}/buy/{id}", base_url.trim_end_matches('/'));
    }
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("http");
    format!("{proto}://{host}/buy/{id}")
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'+' => {
                output.push(b' ');
                idx += 1;
            }
            b'%' if idx + 2 < bytes.len() => {
                if let Ok(hex) = u8::from_str_radix(&value[idx + 1..idx + 3], 16) {
                    output.push(hex);
                    idx += 3;
                } else {
                    output.push(bytes[idx]);
                    idx += 1;
                }
            }
            byte => {
                output.push(byte);
                idx += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).to_string()
}
