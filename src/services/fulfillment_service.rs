use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    models, notification,
    services::settings_service,
    state::AppState,
    time,
};

pub async fn auto_fulfill(state: &AppState, order_id: i64) -> AppResult<()> {
    let now = time::now_str();
    let mut tx = state.pool.begin().await?;
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fulfillments WHERE order_id = ?")
        .bind(order_id)
        .fetch_one(&mut *tx)
        .await?;
    if exists > 0 {
        tx.commit().await?;
        return Ok(());
    }

    let payloads: Vec<String> = sqlx::query_scalar(
        "SELECT secret FROM card_secrets WHERE order_id = ? AND status = 'reserved' ORDER BY id ASC",
    )
    .bind(order_id)
    .fetch_all(&mut *tx)
    .await?;
    if payloads.is_empty() {
        sqlx::query("UPDATE orders SET status = ?, updated_at = ? WHERE id = ?")
            .bind(models::ORDER_ABNORMAL)
            .bind(&now)
            .bind(order_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        enqueue_order_email(state, order_id, "failed_order", None).await?;
        let _ = notification::enqueue_order_notification(state, order_id, "auto_failed").await;
        return Err(AppError::Conflict("没有找到预占卡密".to_string()));
    }

    let payload = payloads.join("\n");
    sqlx::query(
        "UPDATE card_secrets SET status = 'used', used_at = ?, updated_at = ?
         WHERE order_id = ? AND status = 'reserved' AND is_loop = 0",
    )
    .bind(&now)
    .bind(&now)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE card_secrets
         SET status = 'available', order_id = NULL, reserved_at = NULL, used_at = ?, updated_at = ?
         WHERE order_id = ? AND status = 'reserved' AND is_loop = 1",
    )
    .bind(&now)
    .bind(&now)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO fulfillments(order_id, type, status, payload, logistics_json, delivered_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, '{}', ?, ?, ?)",
    )
    .bind(order_id)
    .bind(models::FULFILLMENT_AUTO)
    .bind(models::FULFILLMENT_DELIVERED)
    .bind(payload)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE orders SET status = ?, updated_at = ? WHERE id = ?")
        .bind(models::ORDER_COMPLETED)
        .bind(&now)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    let items = sqlx::query(
        "SELECT product_id, quantity, fulfillment_type FROM order_items WHERE order_id = ?",
    )
    .bind(order_id)
    .fetch_all(&mut *tx)
    .await?;
    for item in items {
        let product_id = item.get::<i64, _>("product_id");
        let quantity = item.get::<i64, _>("quantity");
        let item_fulfillment_type = item.get::<String, _>("fulfillment_type");
        sqlx::query(
            "UPDATE products SET sales_volume = sales_volume + ?, updated_at = ? WHERE id = ?",
        )
        .bind(quantity)
        .bind(&now)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
        if item_fulfillment_type == models::FULFILLMENT_MANUAL {
            sqlx::query(
                "UPDATE products
                 SET manual_stock_locked = CASE WHEN manual_stock_locked >= ? THEN manual_stock_locked - ? ELSE 0 END,
                     manual_stock_sold = manual_stock_sold + ?,
                     updated_at = ?
                 WHERE id = ?",
            )
            .bind(quantity)
            .bind(quantity)
            .bind(quantity)
            .bind(&now)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    enqueue_order_email(state, order_id, "card_send_user_email", None).await?;
    let _ = notification::enqueue_order_notification(state, order_id, "auto_fulfilled").await;
    Ok(())
}

pub async fn manual_fulfill(
    state: &AppState,
    order_id: i64,
    payload: String,
    admin_id: Option<i64>,
) -> AppResult<()> {
    let now = time::now_str();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO fulfillments(order_id, type, status, payload, logistics_json, delivered_by, delivered_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, '{}', ?, ?, ?, ?)
         ON CONFLICT(order_id) DO UPDATE SET payload = excluded.payload, delivered_at = excluded.delivered_at, updated_at = excluded.updated_at",
    )
    .bind(order_id)
    .bind(models::FULFILLMENT_MANUAL)
    .bind(models::FULFILLMENT_DELIVERED)
    .bind(payload)
    .bind(admin_id)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE orders SET status = ?, updated_at = ? WHERE id = ?")
        .bind(models::ORDER_COMPLETED)
        .bind(&now)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    let items = sqlx::query(
        "SELECT product_id, quantity FROM order_items WHERE order_id = ? AND fulfillment_type = ?",
    )
    .bind(order_id)
    .bind(models::FULFILLMENT_MANUAL)
    .fetch_all(&mut *tx)
    .await?;
    for item in items {
        let product_id = item.get::<i64, _>("product_id");
        let quantity = item.get::<i64, _>("quantity");
        sqlx::query(
            "UPDATE products
             SET sales_volume = sales_volume + ?,
                 manual_stock_locked = CASE WHEN manual_stock_locked >= ? THEN manual_stock_locked - ? ELSE 0 END,
                 manual_stock_sold = manual_stock_sold + ?,
                 updated_at = ?
             WHERE id = ?",
        )
        .bind(quantity)
        .bind(quantity)
        .bind(quantity)
        .bind(quantity)
        .bind(&now)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    enqueue_order_email(state, order_id, "completed_order", None).await?;
    let _ = notification::enqueue_order_notification(state, order_id, "manual_fulfilled").await;
    Ok(())
}

pub async fn start_processing(state: &AppState, order_id: i64) -> AppResult<()> {
    let now = time::now_str();
    let affected =
        sqlx::query("UPDATE orders SET status = ?, updated_at = ? WHERE id = ? AND status = ?")
            .bind(models::ORDER_FULFILLING)
            .bind(&now)
            .bind(order_id)
            .bind(models::ORDER_PAID)
            .execute(&state.pool)
            .await?
            .rows_affected();
    if affected == 0 {
        return Err(AppError::Conflict("订单状态不允许进入处理中".to_string()));
    }
    Ok(())
}

pub async fn enqueue_manual_paid_emails(state: &AppState, order_id: i64) -> AppResult<()> {
    enqueue_order_email(state, order_id, "pending_order", None).await?;
    let manage_email = settings_service::manage_email(state)
        .await
        .unwrap_or_default();
    if !manage_email.trim().is_empty() {
        enqueue_order_email(
            state,
            order_id,
            "manual_send_manage_mail",
            Some(&manage_email),
        )
        .await?;
    }
    let _ = notification::enqueue_order_notification(state, order_id, "manual_paid").await;
    Ok(())
}

pub async fn resend_status_email(state: &AppState, order_id: i64) -> AppResult<()> {
    let row = sqlx::query(
        "SELECT o.status, oi.fulfillment_type
         FROM orders o
         LEFT JOIN order_items oi ON oi.order_id = o.id
         WHERE o.id = ? LIMIT 1",
    )
    .bind(order_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(row) = row else {
        return Err(AppError::NotFound("订单不存在".to_string()));
    };
    let status: String = row.get("status");
    let fulfillment_type: Option<String> = row.try_get("fulfillment_type").ok();
    let template = match status.as_str() {
        s if s == models::ORDER_COMPLETED => match fulfillment_type.as_deref() {
            Some(t) if t == models::FULFILLMENT_MANUAL => "completed_order",
            _ => "card_send_user_email",
        },
        s if s == models::ORDER_PAID => "pending_order",
        s if s == models::ORDER_ABNORMAL || s == models::ORDER_FAILED => "failed_order",
        _ => "card_send_user_email",
    };
    enqueue_order_email(state, order_id, template, None).await?;
    Ok(())
}

pub async fn mark_abnormal(state: &AppState, order_id: i64) -> AppResult<()> {
    let now = time::now_str();
    sqlx::query("UPDATE orders SET status = ?, updated_at = ? WHERE id = ?")
        .bind(models::ORDER_ABNORMAL)
        .bind(&now)
        .bind(order_id)
        .execute(&state.pool)
        .await?;
    enqueue_order_email(state, order_id, "failed_order", None).await?;
    let _ = notification::enqueue_order_notification(state, order_id, "marked_abnormal").await;
    Ok(())
}

async fn enqueue_order_email(
    state: &AppState,
    order_id: i64,
    token: &str,
    to: Option<&str>,
) -> AppResult<()> {
    let now = time::now_str();
    let order =
        sqlx::query("SELECT order_no, guest_email, total_amount_cents FROM orders WHERE id = ?")
            .bind(order_id)
            .fetch_one(&state.pool)
            .await?;
    let mut payload = serde_json::json!({
        "order_id": order_id,
        "order_no": order.get::<String, _>("order_no"),
        "email": order.get::<String, _>("guest_email"),
        "amount_cents": order.get::<i64, _>("total_amount_cents"),
        "template": token,
    });
    if let Some(to) = to {
        payload["to"] = serde_json::Value::String(to.to_string());
    }
    sqlx::query(
        "INSERT INTO jobs(kind, payload_json, status, run_at, attempts, max_attempts, created_at, updated_at)
         VALUES ('order_status_email', ?, 'pending', ?, 0, 5, ?, ?)",
    )
    .bind(payload.to_string())
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}
