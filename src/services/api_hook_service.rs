use serde_json::Value;
use sqlx::Row;

use crate::{state::AppState, time};

pub async fn enqueue_for_order(state: &AppState, order_id: i64) -> anyhow::Result<()> {
    let rows = sqlx::query(
        "SELECT DISTINCT p.id, p.api_hook
         FROM order_items oi
         JOIN products p ON p.id = oi.product_id
         WHERE oi.order_id = ? AND TRIM(p.api_hook) <> ''",
    )
    .bind(order_id)
    .fetch_all(&state.pool)
    .await?;
    let now = time::now_str();
    for row in rows {
        let product_id = row.get::<i64, _>("id");
        let url = row.get::<String, _>("api_hook");
        let payload = serde_json::json!({
            "order_id": order_id,
            "product_id": product_id,
            "url": url,
        });
        sqlx::query(
            "INSERT INTO jobs(kind, payload_json, status, run_at, attempts, max_attempts, created_at, updated_at)
             VALUES ('api_hook', ?, 'pending', ?, 0, 5, ?, ?)",
        )
        .bind(payload.to_string())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&state.pool)
        .await?;
    }
    Ok(())
}

pub async fn send_api_hook(state: &AppState, payload: &Value) -> anyhow::Result<()> {
    let order_id = payload
        .get("order_id")
        .and_then(|value| value.as_i64())
        .unwrap_or_default();
    let product_id = payload
        .get("product_id")
        .and_then(|value| value.as_i64())
        .unwrap_or_default();
    let url = payload
        .get("url")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if url.is_empty() {
        return Ok(());
    }
    let body = build_payload(state, order_id, product_id).await?;
    let now = time::now_str();
    let log_id = sqlx::query(
        "INSERT INTO api_hook_logs(order_id, product_id, url, status, created_at, updated_at)
         VALUES (?, ?, ?, 'pending', ?, ?)",
    )
    .bind(order_id)
    .bind(product_id)
    .bind(&url)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?
    .last_insert_rowid();
    let result = reqwest::Client::new().post(&url).json(&body).send().await;
    match result {
        Ok(response) => {
            let status = response.status().as_u16() as i64;
            let text = response.text().await.unwrap_or_default();
            let ok = (200..300).contains(&status);
            sqlx::query(
                "UPDATE api_hook_logs SET status = ?, http_status = ?, response_body = ?, updated_at = ? WHERE id = ?",
            )
            .bind(if ok { "sent" } else { "failed" })
            .bind(status)
            .bind(text.chars().take(2000).collect::<String>())
            .bind(time::now_str())
            .bind(log_id)
            .execute(&state.pool)
            .await?;
            if ok {
                Ok(())
            } else {
                anyhow::bail!("api_hook_http_status_{status}")
            }
        }
        Err(err) => {
            sqlx::query(
                "UPDATE api_hook_logs SET status = 'failed', error = ?, updated_at = ? WHERE id = ?",
            )
            .bind(err.to_string())
            .bind(time::now_str())
            .bind(log_id)
            .execute(&state.pool)
            .await?;
            Err(err.into())
        }
    }
}

async fn build_payload(
    state: &AppState,
    order_id: i64,
    product_id: i64,
) -> anyhow::Result<serde_json::Value> {
    let order = sqlx::query(
        "SELECT order_no, status, guest_email, total_amount_cents, paid_at, created_at FROM orders WHERE id = ?",
    )
    .bind(order_id)
    .fetch_one(&state.pool)
    .await?;
    let item = sqlx::query(
        "SELECT product_name, quantity, total_price_cents, fulfillment_type, manual_form_json
         FROM order_items WHERE order_id = ? AND product_id = ? LIMIT 1",
    )
    .bind(order_id)
    .bind(product_id)
    .fetch_one(&state.pool)
    .await?;
    let fulfillment: Option<String> =
        sqlx::query_scalar("SELECT payload FROM fulfillments WHERE order_id = ?")
            .bind(order_id)
            .fetch_optional(&state.pool)
            .await?;
    Ok(serde_json::json!({
        "order_id": order_id,
        "order_no": order.get::<String, _>("order_no"),
        "status": order.get::<String, _>("status"),
        "email": order.get::<String, _>("guest_email"),
        "amount_cents": order.get::<i64, _>("total_amount_cents"),
        "paid_at": order.get::<Option<String>, _>("paid_at"),
        "created_at": order.get::<String, _>("created_at"),
        "product_id": product_id,
        "product_name": item.get::<String, _>("product_name"),
        "quantity": item.get::<i64, _>("quantity"),
        "item_total_cents": item.get::<i64, _>("total_price_cents"),
        "fulfillment_type": item.get::<String, _>("fulfillment_type"),
        "manual_form": item.get::<String, _>("manual_form_json"),
        "fulfillment": fulfillment.unwrap_or_default(),
    }))
}
