use std::collections::HashMap;

use axum::http::{HeaderMap, header};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::{
    error::{AppError, AppResult},
    models::{
        self,
        order::{Fulfillment, Order},
    },
    money,
    services::{captcha_service, pricing_service, settings_service},
    state::AppState,
    time,
};

#[derive(Debug, Deserialize)]
pub struct CreateOrderForm {
    pub gid: i64,
    pub email: String,
    pub payway: i64,
    pub search_pwd: Option<String>,
    pub by_amount: i64,
    pub coupon_code: Option<String>,
    pub captcha_id: Option<String>,
    pub captcha_answer: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchBySnForm {
    pub order_sn: String,
    pub search_pwd: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchByEmailForm {
    pub email: String,
    pub search_pwd: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BillData {
    pub order: Order,
    pub amount_display: String,
    pub original_amount_display: String,
    pub coupon_discount_display: String,
    pub wholesale_discount_display: String,
    pub channel_id: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderDetailData {
    pub order: Order,
    pub amount_display: String,
    pub fulfillment: Option<Fulfillment>,
}

#[derive(Debug, Serialize)]
pub struct OrderListData {
    pub orders: Vec<OrderDetailData>,
}

#[derive(Debug, Serialize)]
pub struct OrderStatusData {
    pub order_no: String,
    pub status: String,
    pub msg: String,
    pub code: i64,
}

pub async fn create_guest_order(
    state: &AppState,
    form: CreateOrderForm,
    ip: String,
) -> AppResult<String> {
    if form.by_amount < 1 {
        return Err(AppError::BadRequest("购买数量不能低于 1".to_string()));
    }
    if !form.email.contains('@') {
        return Err(AppError::BadRequest("邮箱格式不正确".to_string()));
    }
    let captcha_config = settings_service::captcha_config(state).await;
    if captcha_config.is_open_img_code {
        let captcha_id = form.captcha_id.as_deref().unwrap_or_default();
        let captcha_answer = form.captcha_answer.as_deref().unwrap_or_default();
        if !captcha_service::verify(state, captcha_id, captcha_answer).await? {
            return Err(AppError::BadRequest("验证码错误".to_string()));
        }
    }

    let now = time::now_str();
    let order_config = settings_service::order_config(state).await;
    if order_config.purchase_rate_window_minutes > 0
        && (order_config.purchase_rate_max_per_email > 0
            || order_config.purchase_rate_max_per_ip > 0)
    {
        let window_start = (time::now()
            - chrono::Duration::minutes(order_config.purchase_rate_window_minutes))
        .to_rfc3339();
        if order_config.purchase_rate_max_per_email > 0 {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM purchase_rate WHERE email = ? AND created_at >= ?",
            )
            .bind(form.email.trim())
            .bind(&window_start)
            .fetch_one(&state.pool)
            .await?;
            if count >= order_config.purchase_rate_max_per_email {
                return Err(AppError::BadRequest(
                    "同一邮箱下单过于频繁，请稍后再试".to_string(),
                ));
            }
        }
        if order_config.purchase_rate_max_per_ip > 0 && !ip.is_empty() {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM purchase_rate WHERE client_ip = ? AND created_at >= ?",
            )
            .bind(&ip)
            .bind(&window_start)
            .fetch_one(&state.pool)
            .await?;
            if count >= order_config.purchase_rate_max_per_ip {
                return Err(AppError::BadRequest(
                    "当前 IP 下单过于频繁，请稍后再试".to_string(),
                ));
            }
        }
    }
    let expires_at = time::add_minutes(order_config.order_expire_minutes);
    let order_no = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
    let mut tx = state.pool.begin().await?;

    let product = sqlx::query(
        "SELECT id, name, price_cents, wholesale_prices_json, fulfillment_type, buy_limit_num,
                manual_form_schema_json, manual_stock_total, manual_stock_locked
         FROM products WHERE id = ? AND is_active = 1 AND deleted_at IS NULL",
    )
    .bind(form.gid)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("商品不存在".to_string()))?;

    let product_id = product.get::<i64, _>("id");
    let product_name = product.get::<String, _>("name");
    let price_cents = product.get::<i64, _>("price_cents");
    let wholesale_prices_json = product.get::<String, _>("wholesale_prices_json");
    let fulfillment_type = product.get::<String, _>("fulfillment_type");
    let manual_form_schema_json = product.get::<String, _>("manual_form_schema_json");
    let buy_limit_num = product.get::<i64, _>("buy_limit_num");
    if buy_limit_num > 0 && form.by_amount > buy_limit_num {
        return Err(AppError::BadRequest("超过单次限购数量".to_string()));
    }

    let manual_form_json = collect_manual_form(&manual_form_schema_json, &form.extra)?;
    let quote = pricing_service::quote(price_cents, form.by_amount, &wholesale_prices_json);
    let (coupon_id, coupon_discount_cents) = resolve_coupon_discount(
        &mut tx,
        form.coupon_code.as_deref(),
        product_id,
        quote.payable_before_coupon_cents,
    )
    .await?;
    let payable_cents = (quote.payable_before_coupon_cents - coupon_discount_cents).max(0);

    let order_id = sqlx::query(
        "INSERT INTO orders(order_no, status, currency, guest_email, guest_password, client_ip,
         original_amount_cents, coupon_discount_cents, wholesale_discount_cents, total_amount_cents,
         coupon_id, payment_channel_id, legacy_info, expires_at, created_at, updated_at)
         VALUES (?, ?, 'CNY', ?, ?, ?, ?, ?, ?, ?, ?, ?, '', ?, ?, ?)",
    )
    .bind(&order_no)
    .bind(models::ORDER_PENDING_PAYMENT)
    .bind(form.email.trim())
    .bind(form.search_pwd.unwrap_or_default())
    .bind(&ip)
    .bind(quote.original_amount_cents)
    .bind(coupon_discount_cents)
    .bind(quote.wholesale_discount_cents)
    .bind(payable_cents)
    .bind(coupon_id)
    .bind(form.payway)
    .bind(&expires_at)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    if let Some(coupon_id) = coupon_id {
        sqlx::query("UPDATE coupons SET used_count = used_count + 1, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(coupon_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO coupon_usages(coupon_id, order_id, discount_cents, status, created_at, updated_at)
             VALUES (?, ?, ?, 'reserved', ?, ?)",
        )
        .bind(coupon_id)
        .bind(order_id)
        .bind(coupon_discount_cents)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO order_items(order_id, product_id, sku_id, product_name, unit_price_cents,
         quantity, total_price_cents, fulfillment_type, manual_form_json, created_at)
         VALUES (?, ?, 0, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(order_id)
    .bind(product_id)
    .bind(&product_name)
    .bind(quote.unit_price_cents)
    .bind(form.by_amount)
    .bind(quote.payable_before_coupon_cents)
    .bind(&fulfillment_type)
    .bind(manual_form_json.to_string())
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    if fulfillment_type == models::FULFILLMENT_AUTO {
        let allow_loop_card = form.by_amount == 1;
        if !allow_loop_card {
            let normal_available: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM card_secrets
                 WHERE product_id = ? AND sku_id = 0 AND status = 'available'
                   AND is_loop = 0 AND deleted_at IS NULL",
            )
            .bind(product_id)
            .fetch_one(&mut *tx)
            .await?;
            if normal_available < form.by_amount {
                let loop_available: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM card_secrets
                     WHERE product_id = ? AND sku_id = 0 AND status = 'available'
                       AND is_loop = 1 AND deleted_at IS NULL",
                )
                .bind(product_id)
                .fetch_one(&mut *tx)
                .await?;
                if loop_available > 0 {
                    return Err(AppError::BadRequest(
                        "循环卡密一次只能购买 1 件".to_string(),
                    ));
                }
            }
        }
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM card_secrets
             WHERE product_id = ? AND sku_id = 0 AND status = 'available' AND deleted_at IS NULL
               AND (? = 1 OR is_loop = 0)
             ORDER BY is_loop DESC, id ASC LIMIT ?",
        )
        .bind(product_id)
        .bind(if allow_loop_card { 1 } else { 0 })
        .bind(form.by_amount)
        .fetch_all(&mut *tx)
        .await?;
        if ids.len() != form.by_amount as usize {
            return Err(AppError::Conflict("库存不足".to_string()));
        }
        for id in ids {
            let affected = sqlx::query(
                "UPDATE card_secrets SET status = 'reserved', order_id = ?, reserved_at = ?, updated_at = ?
                 WHERE id = ? AND status = 'available'",
            )
            .bind(order_id)
            .bind(&now)
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if affected != 1 {
                return Err(AppError::Conflict("库存预占失败".to_string()));
            }
        }
    } else {
        let manual_total = product.get::<i64, _>("manual_stock_total");
        let manual_locked = product.get::<i64, _>("manual_stock_locked");
        if manual_total >= 0 && manual_total - manual_locked < form.by_amount {
            return Err(AppError::Conflict("人工库存不足".to_string()));
        }
        sqlx::query("UPDATE products SET manual_stock_locked = manual_stock_locked + ?, updated_at = ? WHERE id = ?")
            .bind(form.by_amount)
            .bind(&now)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
    }

    enqueue_job_tx(
        &mut tx,
        "order_timeout_cancel",
        serde_json::json!({ "order_id": order_id }),
        &expires_at,
    )
    .await?;
    tx.commit().await?;
    if order_config.purchase_rate_window_minutes > 0
        && (order_config.purchase_rate_max_per_email > 0
            || order_config.purchase_rate_max_per_ip > 0)
    {
        let _ =
            sqlx::query("INSERT INTO purchase_rate(email, client_ip, created_at) VALUES (?, ?, ?)")
                .bind(form.email.trim())
                .bind(&ip)
                .bind(&now)
                .execute(&state.pool)
                .await;
    }
    Ok(order_no)
}

pub fn order_cookie_value(headers: &HeaderMap, order_no: &str) -> String {
    let mut orders = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(extract_browser_order_cookies)
        .unwrap_or_default();
    if !orders.iter().any(|item| item == order_no) {
        orders.push(order_no.to_string());
    }
    if orders.len() > 20 {
        let drain = orders.len() - 20;
        orders.drain(0..drain);
    }
    serde_json::to_string(&orders).unwrap_or_else(|_| "[]".to_string())
}

pub fn extract_browser_order_cookies(cookie_header: &str) -> Option<Vec<String>> {
    cookie_header.split(';').find_map(|part| {
        let mut kv = part.trim().splitn(2, '=');
        let key = kv.next()?.trim();
        let value = kv.next()?.trim();
        if key != "freemarket_orders" && key != "dujiao_orders" && key != "dujiaoka_orders" {
            return None;
        }
        let decoded = percent_decode(value);
        serde_json::from_str::<Vec<String>>(&decoded).ok()
    })
}

fn collect_manual_form(
    schema_raw: &str,
    extra: &HashMap<String, String>,
) -> AppResult<serde_json::Value> {
    let schema = parse_manual_schema(schema_raw);
    if schema.is_empty() {
        return Ok(serde_json::json!({}));
    }
    let mut result = serde_json::Map::new();
    for field in schema {
        let value = extra
            .get(&field.field)
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if field.required && value.is_empty() {
            return Err(AppError::BadRequest(format!("{}不能为空", field.label)));
        }
        result.insert(
            field.field,
            serde_json::json!({
                "label": field.label,
                "value": value,
            }),
        );
    }
    Ok(serde_json::Value::Object(result))
}

#[derive(Debug, Deserialize)]
struct ManualField {
    field: String,
    label: String,
    #[serde(default)]
    required: bool,
}

fn parse_manual_schema(schema_raw: &str) -> Vec<ManualField> {
    let raw = schema_raw.trim();
    if raw.is_empty() || raw == "[]" {
        return Vec::new();
    }
    serde_json::from_str(raw).unwrap_or_default()
}

async fn resolve_coupon_discount(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    code: Option<&str>,
    product_id: i64,
    total_cents: i64,
) -> AppResult<(Option<i64>, i64)> {
    let Some(code) = code.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok((None, 0));
    };
    let coupon = sqlx::query(
        "SELECT id, value_cents, min_amount_cents, usage_limit, used_count
         FROM coupons WHERE code = ? AND is_active = 1 AND deleted_at IS NULL",
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("优惠码不存在".to_string()))?;
    let coupon_id = coupon.get::<i64, _>("id");
    let scoped_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM coupon_products WHERE coupon_id = ?")
            .bind(coupon_id)
            .fetch_one(&mut **tx)
            .await?;
    if scoped_count > 0 {
        let allowed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM coupon_products WHERE coupon_id = ? AND product_id = ?",
        )
        .bind(coupon_id)
        .bind(product_id)
        .fetch_one(&mut **tx)
        .await?;
        if allowed == 0 {
            return Err(AppError::BadRequest("此商品不可用该优惠码".to_string()));
        }
    }
    let usage_limit = coupon.get::<i64, _>("usage_limit");
    let used_count = coupon.get::<i64, _>("used_count");
    if usage_limit > 0 && used_count >= usage_limit {
        return Err(AppError::BadRequest("优惠码可用次数不足".to_string()));
    }
    let min_amount = coupon.get::<i64, _>("min_amount_cents");
    if min_amount > 0 && total_cents < min_amount {
        return Err(AppError::BadRequest("订单金额未达到优惠码门槛".to_string()));
    }
    let discount = coupon.get::<i64, _>("value_cents").min(total_cents);
    Ok((Some(coupon_id), discount))
}

pub async fn enqueue_job_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    kind: &str,
    payload: serde_json::Value,
    run_at: &str,
) -> AppResult<()> {
    let now = time::now_str();
    sqlx::query(
        "INSERT INTO jobs(kind, payload_json, status, run_at, attempts, max_attempts, created_at, updated_at)
         VALUES (?, ?, 'pending', ?, 0, 5, ?, ?)",
    )
    .bind(kind)
    .bind(payload.to_string())
    .bind(run_at)
    .bind(&now)
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn bill_data(state: &AppState, order_no: &str) -> AppResult<BillData> {
    maybe_cancel_expired(state, order_no).await?;
    let order = get_order_by_no(&state.pool, order_no).await?;
    Ok(BillData {
        amount_display: money::format_cents(order.total_amount_cents),
        original_amount_display: money::format_cents(order.original_amount_cents),
        coupon_discount_display: money::format_cents(order.coupon_discount_cents),
        wholesale_discount_display: money::format_cents(order.wholesale_discount_cents),
        channel_id: order.payment_channel_id.unwrap_or(1),
        order,
    })
}

pub async fn detail_data(state: &AppState, order_no: &str) -> AppResult<OrderDetailData> {
    maybe_cancel_expired(state, order_no).await?;
    let order = get_order_by_no(&state.pool, order_no).await?;
    let fulfillment = sqlx::query_as::<_, Fulfillment>(
        "SELECT id, order_id, type, status, payload, delivered_at FROM fulfillments WHERE order_id = ?",
    )
    .bind(order.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(OrderDetailData {
        amount_display: money::format_cents(order.total_amount_cents),
        order,
        fulfillment,
    })
}

pub async fn detail_data_by_id(state: &AppState, order_id: i64) -> AppResult<OrderDetailData> {
    let order: Order = sqlx::query_as(
        "SELECT id, order_no, status, currency, guest_email, guest_password, client_ip,
         original_amount_cents, coupon_discount_cents, wholesale_discount_cents, total_amount_cents,
         coupon_id, payment_channel_id, legacy_info, expires_at, paid_at, canceled_at, created_at, updated_at
         FROM orders WHERE id = ?",
    )
    .bind(order_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("订单不存在".to_string()))?;
    let fulfillment = sqlx::query_as::<_, Fulfillment>(
        "SELECT id, order_id, type, status, payload, delivered_at FROM fulfillments WHERE order_id = ?",
    )
    .bind(order.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(OrderDetailData {
        amount_display: money::format_cents(order.total_amount_cents),
        order,
        fulfillment,
    })
}

pub async fn search_by_order_no(
    state: &AppState,
    form: SearchBySnForm,
) -> AppResult<OrderDetailData> {
    let _legacy_search_pwd = form.search_pwd.as_deref().unwrap_or_default();
    let order = get_order_by_no(&state.pool, form.order_sn.trim()).await?;
    detail_data(state, &order.order_no).await
}

pub async fn search_by_email(
    state: &AppState,
    form: SearchByEmailForm,
) -> AppResult<OrderListData> {
    if !form.email.contains('@') {
        return Err(AppError::BadRequest("邮箱格式不正确".to_string()));
    }
    let order_config = settings_service::order_config(state).await;
    let search_pwd = form.search_pwd.as_deref().unwrap_or_default().trim();
    if order_config.is_open_search_pwd && search_pwd.is_empty() {
        return Err(AppError::BadRequest("查询密码不能为空".to_string()));
    }
    let rows: Vec<Order> = if order_config.is_open_search_pwd {
        sqlx::query_as(
            "SELECT id, order_no, status, currency, guest_email, guest_password, client_ip,
             original_amount_cents, coupon_discount_cents, wholesale_discount_cents, total_amount_cents,
             coupon_id, payment_channel_id, legacy_info, expires_at, paid_at, canceled_at, created_at, updated_at
             FROM orders WHERE guest_email = ? AND guest_password = ? ORDER BY id DESC LIMIT 5",
        )
        .bind(form.email.trim())
        .bind(search_pwd)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, order_no, status, currency, guest_email, guest_password, client_ip,
             original_amount_cents, coupon_discount_cents, wholesale_discount_cents, total_amount_cents,
             coupon_id, payment_channel_id, legacy_info, expires_at, paid_at, canceled_at, created_at, updated_at
             FROM orders WHERE guest_email = ? ORDER BY id DESC LIMIT 5",
        )
        .bind(form.email.trim())
        .fetch_all(&state.pool)
        .await?
    };
    orders_to_list(state, rows).await
}

pub async fn search_by_browser(state: &AppState, headers: &HeaderMap) -> AppResult<OrderListData> {
    let Some(cookie_header) = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
    else {
        return Ok(OrderListData { orders: Vec::new() });
    };
    let Some(order_nos) = extract_browser_order_cookies(cookie_header) else {
        return Ok(OrderListData { orders: Vec::new() });
    };
    let mut orders = Vec::new();
    for order_no in order_nos.iter().rev().take(5) {
        if let Ok(data) = detail_data(state, order_no).await {
            orders.push(data);
        }
    }
    Ok(OrderListData { orders })
}

async fn orders_to_list(state: &AppState, rows: Vec<Order>) -> AppResult<OrderListData> {
    let mut orders = Vec::new();
    for order in rows {
        if let Ok(data) = detail_data_by_id(state, order.id).await {
            orders.push(data);
        }
    }
    Ok(OrderListData { orders })
}

pub async fn status_data(state: &AppState, order_no: &str) -> AppResult<OrderStatusData> {
    let maybe_order = get_order_by_no(&state.pool, order_no).await;
    let order = match maybe_order {
        Ok(order) => order,
        Err(_) => {
            return Ok(OrderStatusData {
                order_no: order_no.to_string(),
                status: "expired".to_string(),
                msg: "expired".to_string(),
                code: 400001,
            });
        }
    };
    if order.status == models::ORDER_PENDING_PAYMENT {
        maybe_cancel_expired(state, order_no).await?;
    }
    let order = get_order_by_no(&state.pool, order_no).await?;
    if order.status == models::ORDER_PENDING_PAYMENT {
        return Ok(OrderStatusData {
            order_no: order.order_no,
            status: order.status,
            msg: "wait....".to_string(),
            code: 400000,
        });
    }
    if order.status == models::ORDER_CANCELED {
        return Ok(OrderStatusData {
            order_no: order.order_no,
            status: order.status,
            msg: "expired".to_string(),
            code: 400001,
        });
    }
    if order.status == models::ORDER_ABNORMAL || order.status == models::ORDER_FAILED {
        return Ok(OrderStatusData {
            order_no: order.order_no,
            status: order.status,
            msg: "expired".to_string(),
            code: 400001,
        });
    }
    if order.status == models::ORDER_FULFILLING {
        return Ok(OrderStatusData {
            order_no: order.order_no,
            status: order.status,
            msg: "success".to_string(),
            code: 200,
        });
    }
    Ok(OrderStatusData {
        order_no: order.order_no,
        status: order.status,
        msg: "success".to_string(),
        code: 200,
    })
}

pub async fn get_order_by_no(pool: &SqlitePool, order_no: &str) -> AppResult<Order> {
    sqlx::query_as(
        "SELECT id, order_no, status, currency, guest_email, guest_password, client_ip,
         original_amount_cents, coupon_discount_cents, wholesale_discount_cents, total_amount_cents,
         coupon_id, payment_channel_id, legacy_info, expires_at, paid_at, canceled_at, created_at, updated_at
         FROM orders WHERE order_no = ?",
    )
    .bind(order_no)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("订单不存在".to_string()))
}

pub async fn maybe_cancel_expired(state: &AppState, order_no: &str) -> AppResult<()> {
    let order = get_order_by_no(&state.pool, order_no).await?;
    if order.status == models::ORDER_PENDING_PAYMENT {
        if let Some(expires_at) = time::parse_rfc3339(&order.expires_at) {
            if expires_at <= time::now() {
                cancel_expired_order(state, order.id).await?;
            }
        }
    }
    Ok(())
}

pub async fn cancel_expired_order(state: &AppState, order_id: i64) -> AppResult<()> {
    let now = time::now_str();
    let mut tx = state.pool.begin().await?;
    let order: Option<Order> = sqlx::query_as(
        "SELECT id, order_no, status, currency, guest_email, guest_password, client_ip,
         original_amount_cents, coupon_discount_cents, wholesale_discount_cents, total_amount_cents,
         coupon_id, payment_channel_id, legacy_info, expires_at, paid_at, canceled_at, created_at, updated_at
         FROM orders WHERE id = ?",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(order) = order else {
        tx.commit().await?;
        return Ok(());
    };
    if order.status != models::ORDER_PENDING_PAYMENT {
        tx.commit().await?;
        return Ok(());
    }
    sqlx::query("UPDATE orders SET status = ?, canceled_at = ?, updated_at = ? WHERE id = ?")
        .bind(models::ORDER_CANCELED)
        .bind(&now)
        .bind(&now)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE card_secrets SET status = 'available', order_id = NULL, reserved_at = NULL, updated_at = ?
         WHERE order_id = ? AND status = 'reserved'",
    )
    .bind(&now)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE payments SET status = ?, expired_at = ?, updated_at = ? WHERE order_id = ? AND status = ?")
        .bind(models::PAYMENT_EXPIRED)
        .bind(&now)
        .bind(&now)
        .bind(order_id)
        .bind(models::PAYMENT_PENDING)
        .execute(&mut *tx)
        .await?;
    let manual_items = sqlx::query(
        "SELECT product_id, quantity FROM order_items WHERE order_id = ? AND fulfillment_type = ?",
    )
    .bind(order_id)
    .bind(models::FULFILLMENT_MANUAL)
    .fetch_all(&mut *tx)
    .await?;
    for item in manual_items {
        let product_id = item.get::<i64, _>("product_id");
        let quantity = item.get::<i64, _>("quantity");
        sqlx::query(
            "UPDATE products
             SET manual_stock_locked = CASE WHEN manual_stock_locked >= ? THEN manual_stock_locked - ? ELSE 0 END,
                 updated_at = ?
             WHERE id = ?",
        )
        .bind(quantity)
        .bind(quantity)
        .bind(&now)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    }
    if let Some(coupon_id) = order.coupon_id {
        let refunded = sqlx::query(
            "UPDATE orders SET coupon_ret_back = 1, updated_at = ?
             WHERE id = ? AND coupon_ret_back = 0",
        )
        .bind(&now)
        .bind(order_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if refunded == 1 {
            sqlx::query(
                "UPDATE coupons
                 SET used_count = CASE WHEN used_count > 0 THEN used_count - 1 ELSE 0 END,
                     updated_at = ?
                 WHERE id = ?",
            )
            .bind(&now)
            .bind(coupon_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE coupon_usages SET status = 'canceled', updated_at = ? WHERE order_id = ? AND status = 'reserved'")
                .bind(&now)
                .bind(order_id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'%' && idx + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_value(bytes[idx + 1]), hex_value(bytes[idx + 2])) {
                output.push((hi << 4) | lo);
                idx += 3;
                continue;
            }
        }
        output.push(bytes[idx]);
        idx += 1;
    }
    String::from_utf8(output).unwrap_or_default()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
