use axum::http::{HeaderMap, header};
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    models::{
        self,
        payment::{Payment, PaymentChannel},
    },
    money,
    payment::{
        provider::{CreatePaymentInput, PaymentCallback, PaymentStatus, first_param},
        registry::PaymentRegistry,
    },
    services::{
        api_hook_service, evm_local_service, fulfillment_service, order_service, settings_service,
    },
    state::AppState,
    time,
};

#[derive(Debug, Serialize)]
pub struct PayPageData {
    pub order_no: String,
    pub payment_no: String,
    pub amount_display: String,
    pub pay_url: String,
    pub qr_code: String,
    pub interaction_mode: String,
}

pub async fn create_payment(
    state: &AppState,
    order_no: &str,
    channel_id: i64,
    headers: &HeaderMap,
) -> AppResult<PayPageData> {
    order_service::maybe_cancel_expired(state, order_no).await?;
    let locale = settings_service::runtime_site_config(state).await.language;
    let order = order_service::get_order_by_no(&state.pool, order_no, &locale).await?;
    if order.status != models::ORDER_PENDING_PAYMENT {
        return Err(AppError::BadRequest("订单不是待支付状态".to_string()));
    }

    let channel: PaymentChannel = sqlx::query_as(
        "SELECT id, name, provider_type, channel_type, interaction_mode, config_json, is_active, sort_order
         FROM payment_channels WHERE id = ? AND is_active = 1",
    )
    .bind(channel_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("支付渠道不存在".to_string()))?;

    if order.total_amount_cents == 0 {
        return create_zero_payment(state, &order, &channel).await;
    }

    let site = settings_service::runtime_site_config(state).await;
    let base_url = effective_public_base_url(&site.base_url, headers);

    if evm_local_service::is_evm_local_provider(&channel.provider_type) {
        evm_local_service::expire_old_intents(&state.pool)
            .await
            .map_err(AppError::Anyhow)?;
    }

    if let Some(payment) = existing_pending_payment(state, order.id, channel_id).await? {
        let pay_url = if channel.provider_type == "noop" && unusable_public_url(&payment.pay_url) {
            let rewritten = format!("{}/payment/noop/success/{}", base_url, payment.payment_no);
            sqlx::query("UPDATE payments SET pay_url = ?, updated_at = ? WHERE id = ?")
                .bind(&rewritten)
                .bind(time::now_str())
                .bind(payment.id)
                .execute(&state.pool)
                .await?;
            rewritten
        } else {
            payment.pay_url
        };
        return Ok(PayPageData {
            order_no: order.order_no,
            payment_no: payment.payment_no,
            amount_display: money::format_cents(payment.amount_cents),
            pay_url,
            qr_code: payment.qr_code,
            interaction_mode: payment.interaction_mode,
        });
    }

    let payment_no = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
    let now = time::now_str();
    let config: Value =
        serde_json::from_str(&channel.config_json).unwrap_or_else(|_| serde_json::json!({}));
    if evm_local_service::is_evm_local_provider(&channel.provider_type) {
        return evm_local_service::create_payment(
            state,
            &channel,
            &config,
            &order,
            &payment_no,
            &base_url,
        )
        .await;
    }
    let registry = PaymentRegistry::default_registry();
    let provider = registry
        .lookup(&channel.provider_type, &channel.channel_type)
        .ok_or_else(|| AppError::BadRequest("支付 provider 未注册".to_string()))?;
    provider.validate_config(&config, &channel.channel_type)?;
    let route_provider = if channel.provider_type == "official" {
        channel.channel_type.as_str()
    } else {
        channel.provider_type.as_str()
    };
    let return_url = if channel.provider_type == "noop" {
        format!("{}/payment/noop/success/{}", base_url, payment_no)
    } else {
        format!(
            "{}/pay/{}/return_url?order_id={}",
            base_url, route_provider, order.order_no
        )
    };
    let result = provider
        .create_payment(
            &config,
            CreatePaymentInput {
                payment_no: payment_no.clone(),
                order_no: order.order_no.clone(),
                subject: order.order_no.clone(),
                amount_cents: order.total_amount_cents,
                currency: order.currency.clone(),
                return_url,
                notify_url: format!("{}/pay/{}/notify_url", base_url, route_provider),
                client_ip: "0.0.0.0".to_string(),
                channel_type: channel.channel_type.clone(),
            },
        )
        .await?;

    sqlx::query(
        "INSERT INTO payments(payment_no, order_id, channel_id, provider_type, channel_type, interaction_mode,
         amount_cents, currency, status, provider_ref, gateway_order_no, pay_url, qr_code, provider_payload_json,
         created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&payment_no)
    .bind(order.id)
    .bind(channel.id)
    .bind(&channel.provider_type)
    .bind(&channel.channel_type)
    .bind(&channel.interaction_mode)
    .bind(order.total_amount_cents)
    .bind(&order.currency)
    .bind(models::PAYMENT_PENDING)
    .bind(&result.provider_ref)
    .bind(&payment_no)
    .bind(&result.pay_url)
    .bind(&result.qr_code)
    .bind(result.payload.to_string())
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    Ok(PayPageData {
        order_no: order.order_no,
        payment_no,
        amount_display: money::format_cents(order.total_amount_cents),
        pay_url: result.pay_url,
        qr_code: result.qr_code,
        interaction_mode: channel.interaction_mode,
    })
}

async fn create_zero_payment(
    state: &AppState,
    order: &models::order::Order,
    channel: &PaymentChannel,
) -> AppResult<PayPageData> {
    if let Some(payment) = existing_pending_payment(state, order.id, channel.id).await? {
        apply_success(state, payment.id, 0, &payment.currency).await?;
        return Ok(PayPageData {
            order_no: order.order_no.clone(),
            payment_no: payment.payment_no,
            amount_display: money::format_cents(0),
            pay_url: format!("/detail-order-sn/{}", order.order_no),
            qr_code: String::new(),
            interaction_mode: "redirect".to_string(),
        });
    }
    let payment_no = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
    let now = time::now_str();
    let detail_url = format!("/detail-order-sn/{}", order.order_no);
    let payment_id = sqlx::query(
        "INSERT INTO payments(payment_no, order_id, channel_id, provider_type, channel_type, interaction_mode,
         amount_cents, currency, status, provider_ref, gateway_order_no, pay_url, qr_code, provider_payload_json,
         created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 'redirect', 0, ?, ?, '', ?, ?, '', '{}', ?, ?)",
    )
    .bind(&payment_no)
    .bind(order.id)
    .bind(channel.id)
    .bind(&channel.provider_type)
    .bind(&channel.channel_type)
    .bind(&order.currency)
    .bind(models::PAYMENT_PENDING)
    .bind(&payment_no)
    .bind(&detail_url)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?
    .last_insert_rowid();
    apply_success(state, payment_id, 0, &order.currency).await?;
    Ok(PayPageData {
        order_no: order.order_no.clone(),
        payment_no,
        amount_display: money::format_cents(0),
        pay_url: detail_url,
        qr_code: String::new(),
        interaction_mode: "redirect".to_string(),
    })
}

async fn existing_pending_payment(
    state: &AppState,
    order_id: i64,
    channel_id: i64,
) -> AppResult<Option<Payment>> {
    Ok(sqlx::query_as(
        "SELECT id, payment_no, order_id, channel_id, provider_type, channel_type, interaction_mode,
         amount_cents, currency, status, provider_ref, gateway_order_no, pay_url, qr_code,
         provider_payload_json, paid_at, expired_at, callback_at
         FROM payments WHERE order_id = ? AND channel_id = ? AND status = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(order_id)
    .bind(channel_id)
    .bind(models::PAYMENT_PENDING)
    .fetch_optional(&state.pool)
    .await?)
}

fn effective_public_base_url(configured: &str, headers: &HeaderMap) -> String {
    let configured = configured.trim().trim_end_matches('/');
    if !unusable_public_url(configured) {
        return configured.to_string();
    }
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim();
    if host.is_empty() {
        return configured.to_string();
    }
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| *value == "http" || *value == "https")
        .unwrap_or("http");
    format!("{proto}://{host}")
}

fn unusable_public_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://0.0.0.0")
        || lower.starts_with("https://0.0.0.0")
        || lower.starts_with("http://127.0.0.1")
        || lower.starts_with("https://127.0.0.1")
        || lower.starts_with("http://localhost")
        || lower.starts_with("https://localhost")
}

pub async fn noop_success(state: &AppState, payment_no: &str) -> AppResult<String> {
    let payment: Payment = sqlx::query_as(
        "SELECT id, payment_no, order_id, channel_id, provider_type, channel_type, interaction_mode,
         amount_cents, currency, status, provider_ref, gateway_order_no, pay_url, qr_code,
         provider_payload_json, paid_at, expired_at, callback_at
         FROM payments WHERE payment_no = ?",
    )
    .bind(payment_no)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("支付单不存在".to_string()))?;

    apply_success(state, payment.id, payment.amount_cents, &payment.currency).await?;
    let order_no: String = sqlx::query_scalar("SELECT order_no FROM orders WHERE id = ?")
        .bind(payment.order_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(order_no)
}

pub async fn epay_callback(
    state: &AppState,
    params: &std::collections::HashMap<String, String>,
) -> AppResult<String> {
    provider_form_callback(state, "epay", params, &[]).await
}

pub async fn tokenpay_callback(
    state: &AppState,
    params: &std::collections::HashMap<String, String>,
) -> AppResult<String> {
    provider_form_callback(state, "tokenpay", params, &[]).await
}

pub async fn epusdt_callback(
    state: &AppState,
    params: &std::collections::HashMap<String, String>,
) -> AppResult<String> {
    provider_form_callback(state, "epusdt", params, &[]).await
}

pub async fn provider_form_callback(
    state: &AppState,
    provider_type: &str,
    params: &std::collections::HashMap<String, String>,
    body: &[u8],
) -> AppResult<String> {
    let payment_no = extract_payment_no(params, body)
        .ok_or_else(|| AppError::BadRequest("缺少支付单号".to_string()))?;
    let payment: Payment = sqlx::query_as(
        "SELECT id, payment_no, order_id, channel_id, provider_type, channel_type, interaction_mode,
         amount_cents, currency, status, provider_ref, gateway_order_no, pay_url, qr_code,
         provider_payload_json, paid_at, expired_at, callback_at
         FROM payments WHERE payment_no = ? OR gateway_order_no = ?",
    )
    .bind(&payment_no)
    .bind(&payment_no)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("支付单不存在".to_string()))?;
    if !payment.provider_type.eq_ignore_ascii_case(provider_type)
        && !(payment.provider_type.eq_ignore_ascii_case("official")
            && payment.channel_type.eq_ignore_ascii_case(provider_type))
    {
        return Err(AppError::BadRequest("支付 provider 不匹配".to_string()));
    }
    let (channel, config) = load_channel_config(state, payment.channel_id).await?;
    let registry = PaymentRegistry::default_registry();
    let provider = registry
        .lookup(&channel.provider_type, &channel.channel_type)
        .ok_or_else(|| AppError::BadRequest("支付 provider 未注册".to_string()))?;
    let result = provider.verify_callback(&config, params, body).await?;
    apply_callback_result(state, &payment, result).await?;
    Ok(callback_ack(&channel.provider_type).to_string())
}

pub async fn provider_webhook(
    state: &AppState,
    provider_type: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> AppResult<String> {
    let params = parse_body_for_lookup(body);
    let payment_no = extract_payment_no(&params, body)
        .ok_or_else(|| AppError::BadRequest("缺少支付单号".to_string()))?;
    let payment = find_payment_by_no(state, &payment_no).await?;
    if !payment.provider_type.eq_ignore_ascii_case(provider_type)
        && !(payment.provider_type.eq_ignore_ascii_case("official")
            && matches!(
                provider_type,
                "stripe" | "paypal" | "alipay" | "wechat" | "wxpay"
            ))
    {
        return Err(AppError::BadRequest("支付 provider 不匹配".to_string()));
    }
    let (channel, config) = load_channel_config(state, payment.channel_id).await?;
    let registry = PaymentRegistry::default_registry();
    let provider = registry
        .lookup(&channel.provider_type, &channel.channel_type)
        .ok_or_else(|| AppError::BadRequest("支付 provider 未注册".to_string()))?;
    let result = provider.parse_webhook(&config, headers, body).await?;
    apply_callback_result(state, &payment, result).await?;
    Ok(callback_ack(&channel.provider_type).to_string())
}

async fn apply_callback_result(
    state: &AppState,
    payment: &Payment,
    result: PaymentCallback,
) -> AppResult<()> {
    match result.status {
        PaymentStatus::Success => {
            let amount = if result.amount_cents > 0 {
                result.amount_cents
            } else {
                payment.amount_cents
            };
            let currency = if result.currency.trim().is_empty() {
                payment.currency.as_str()
            } else {
                result.currency.as_str()
            };
            apply_success(state, payment.id, amount, currency).await
        }
        PaymentStatus::Expired => {
            mark_payment_status(state, payment.id, models::PAYMENT_EXPIRED).await
        }
        PaymentStatus::Failed => {
            mark_payment_status(state, payment.id, models::PAYMENT_FAILED).await
        }
        PaymentStatus::Pending => Ok(()),
    }
}

async fn mark_payment_status(state: &AppState, payment_id: i64, status: &str) -> AppResult<()> {
    let now = time::now_str();
    sqlx::query("UPDATE payments SET status = ?, callback_at = ?, updated_at = ? WHERE id = ? AND status = ?")
        .bind(status)
        .bind(&now)
        .bind(&now)
        .bind(payment_id)
        .bind(models::PAYMENT_PENDING)
        .execute(&state.pool)
        .await?;
    Ok(())
}

async fn load_channel_config(
    state: &AppState,
    channel_id: i64,
) -> AppResult<(PaymentChannel, Value)> {
    let channel: PaymentChannel = sqlx::query_as(
        "SELECT id, name, provider_type, channel_type, interaction_mode, config_json, is_active, sort_order
         FROM payment_channels WHERE id = ?",
    )
    .bind(channel_id)
    .fetch_one(&state.pool)
    .await?;
    let config: Value =
        serde_json::from_str(&channel.config_json).unwrap_or_else(|_| serde_json::json!({}));
    Ok((channel, config))
}

async fn find_payment_by_no(state: &AppState, payment_no: &str) -> AppResult<Payment> {
    sqlx::query_as(
        "SELECT id, payment_no, order_id, channel_id, provider_type, channel_type, interaction_mode,
         amount_cents, currency, status, provider_ref, gateway_order_no, pay_url, qr_code,
         provider_payload_json, paid_at, expired_at, callback_at
         FROM payments WHERE payment_no = ? OR gateway_order_no = ? OR provider_ref = ?",
    )
    .bind(payment_no)
    .bind(payment_no)
    .bind(payment_no)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("支付单不存在".to_string()))
}

fn extract_payment_no(
    params: &std::collections::HashMap<String, String>,
    body: &[u8],
) -> Option<String> {
    first_param(
        params,
        &[
            "payment_no",
            "out_order_id",
            "OutOrderId",
            "out_trade_no",
            "trade_no",
            "order_id",
            "orderid",
            "orderId",
            "outTradeNo",
            "unique_id",
            "data[unique_id]",
            "merchant_order_id",
            "data[merchant_order_id]",
            "client_reference_id",
            "invoice_id",
        ],
    )
    .map(str::to_string)
    .or_else(|| {
        if body.is_empty() {
            None
        } else {
            let params = parse_body_for_lookup(body);
            first_param(
                &params,
                &[
                    "payment_no",
                    "out_order_id",
                    "OutOrderId",
                    "out_trade_no",
                    "order_id",
                    "unique_id",
                    "data[unique_id]",
                    "merchant_order_id",
                    "data[merchant_order_id]",
                    "data[object][client_reference_id]",
                    "data[object][metadata][payment_no]",
                    "resource[invoice_id]",
                    "resource[purchase_units][0][invoice_id]",
                ],
            )
            .map(str::to_string)
        }
    })
}

fn parse_body_for_lookup(body: &[u8]) -> std::collections::HashMap<String, String> {
    if body.is_empty() {
        return std::collections::HashMap::new();
    }
    if let Ok(value) = serde_json::from_slice::<Value>(body) {
        return crate::payment::provider::flatten_json(&value);
    }
    String::from_utf8_lossy(body)
        .split('&')
        .filter_map(|item| {
            let mut parts = item.splitn(2, '=');
            Some((
                parts.next()?.to_string(),
                parts.next().unwrap_or_default().to_string(),
            ))
        })
        .collect()
}

fn callback_ack(provider_type: &str) -> &'static str {
    match provider_type {
        "tokenpay" | "epusdt" => "ok",
        "okpay" => "{\"status\":\"success\"}",
        _ => "success",
    }
}

pub async fn apply_success(
    state: &AppState,
    payment_id: i64,
    amount_cents: i64,
    currency: &str,
) -> AppResult<()> {
    let now = time::now_str();
    let mut tx = state.pool.begin().await?;
    let payment = sqlx::query(
        "SELECT id, order_id, amount_cents, currency, status FROM payments WHERE id = ?",
    )
    .bind(payment_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("支付单不存在".to_string()))?;
    let order_id = payment.get::<i64, _>("order_id");
    let stored_amount = payment.get::<i64, _>("amount_cents");
    let stored_currency = payment.get::<String, _>("currency");
    let payment_status = payment.get::<String, _>("status");
    if stored_amount != amount_cents {
        return Err(AppError::BadRequest("支付金额不一致".to_string()));
    }
    if !stored_currency.eq_ignore_ascii_case(currency) {
        return Err(AppError::BadRequest("支付币种不一致".to_string()));
    }
    if payment_status == models::PAYMENT_SUCCESS {
        tx.commit().await?;
        return Ok(());
    }
    let order_status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id = ?")
        .bind(order_id)
        .fetch_one(&mut *tx)
        .await?;
    if order_status != models::ORDER_PENDING_PAYMENT {
        return Err(AppError::BadRequest("订单状态不允许支付".to_string()));
    }
    let fulfillment_type: String =
        sqlx::query_scalar("SELECT fulfillment_type FROM order_items WHERE order_id = ? LIMIT 1")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_else(|| models::FULFILLMENT_AUTO.to_string());
    let next_order_status = if fulfillment_type == models::FULFILLMENT_AUTO {
        models::ORDER_FULFILLING
    } else {
        models::ORDER_PAID
    };
    sqlx::query(
        "UPDATE payments SET status = ?, paid_at = ?, callback_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(models::PAYMENT_SUCCESS)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(payment_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE orders SET status = ?, paid_at = ?, updated_at = ? WHERE id = ?")
        .bind(next_order_status)
        .bind(&now)
        .bind(&now)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE coupon_usages SET status = 'used', updated_at = ? WHERE order_id = ?")
        .bind(&now)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if fulfillment_type == models::FULFILLMENT_AUTO {
        fulfillment_service::auto_fulfill(state, order_id).await?;
    } else {
        fulfillment_service::enqueue_manual_paid_emails(state, order_id).await?;
    }
    let _ = api_hook_service::enqueue_for_order(state, order_id).await;
    Ok(())
}
