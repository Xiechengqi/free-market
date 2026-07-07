use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;

use crate::{services::settings_service, state::AppState, time};

#[derive(Debug, Default, Deserialize)]
struct NotificationConfig {
    #[serde(default)]
    server_chan_key: String,
    #[serde(default)]
    telegram_bot_token: String,
    #[serde(default)]
    telegram_chat_id: String,
    #[serde(default)]
    bark_url: String,
    #[serde(default)]
    wecom_webhook: String,
    #[serde(default)]
    is_open_server_chan: bool,
    #[serde(default)]
    is_open_telegram: bool,
    #[serde(default)]
    is_open_bark: bool,
    #[serde(default)]
    is_open_bark_push_url: bool,
    #[serde(default)]
    is_open_wecom: bool,
}

struct OrderSnapshot {
    order_no: String,
    guest_email: String,
    total_amount: String,
    product_name: String,
    buy_amount: i64,
    payment_name: String,
    created_at: String,
    in_stock: i64,
}

pub async fn enqueue_order_notification(
    state: &AppState,
    order_id: i64,
    event: &str,
) -> anyhow::Result<()> {
    let now = time::now_str();
    let payload = serde_json::json!({
        "order_id": order_id,
        "event": event,
    });
    sqlx::query(
        "INSERT INTO jobs(kind, payload_json, status, run_at, attempts, max_attempts, created_at, updated_at)
         VALUES ('admin_notification', ?, 'pending', ?, 0, 5, ?, ?)",
    )
    .bind(payload.to_string())
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn send_admin_notification(state: &AppState, payload: &Value) -> anyhow::Result<()> {
    let order_id = payload
        .get("order_id")
        .and_then(|value| value.as_i64())
        .unwrap_or_default();
    let event = payload
        .get("event")
        .and_then(|value| value.as_str())
        .unwrap_or("order_event");
    let snapshot = load_snapshot(state, order_id).await?;
    let site = settings_service::runtime_site_config(state).await;
    let webname = if site.logo_text.trim().is_empty() {
        site.name.clone()
    } else {
        site.logo_text.clone()
    };
    let title = format!("【{webname}】新订单({}) {}", snapshot.total_amount, event);
    let base_url = site.base_url.trim_end_matches('/').to_string();
    let detail_url = format!("{}/detail-order-sn/{}", base_url, snapshot.order_no);
    let config = load_config(state).await?;
    let mut sent_any = false;
    let mut errors = Vec::new();

    if config.is_open_server_chan && !config.server_chan_key.trim().is_empty() {
        sent_any = true;
        let desp = format!(
            "- 订单号：`{}`\n- 商品：{} x {}\n- 金额：{} 元\n- 邮箱：{}\n- 支付：{}\n- 时间：{}\n- 库存：{}\n\n[查看详情]({})",
            snapshot.order_no,
            snapshot.product_name,
            snapshot.buy_amount,
            snapshot.total_amount,
            snapshot.guest_email,
            snapshot.payment_name,
            snapshot.created_at,
            snapshot.in_stock,
            detail_url
        );
        if let Err(err) = send_server_chan(&config.server_chan_key, &title, &desp).await {
            errors.push(format!("server_chan:{err}"));
        }
    }
    if config.is_open_telegram
        && !config.telegram_bot_token.trim().is_empty()
        && !config.telegram_chat_id.trim().is_empty()
    {
        sent_any = true;
        let text = format!(
            "*【{}】新订单({} 元)*\n订单号：`{}`\n商品：`{}` x {}\n金额：{} 元\n邮箱：`{}`\n支付：`{}`\n时间：{}\n[查看详情]({})",
            md_escape(&webname),
            md_escape(&snapshot.total_amount),
            md_escape(&snapshot.order_no),
            md_escape(&snapshot.product_name),
            snapshot.buy_amount,
            md_escape(&snapshot.total_amount),
            md_escape(&snapshot.guest_email),
            md_escape(&snapshot.payment_name),
            md_escape(&snapshot.created_at),
            detail_url
        );
        if let Err(err) =
            send_telegram(&config.telegram_bot_token, &config.telegram_chat_id, &text).await
        {
            errors.push(format!("telegram:{err}"));
        }
    }
    if config.is_open_bark && !config.bark_url.trim().is_empty() {
        sent_any = true;
        let body = format!(
            "订单号: {}\n商品: {} x {}\n金额: {} 元\n邮箱: {}\n支付: {}\n库存: {}\n时间: {}",
            snapshot.order_no,
            snapshot.product_name,
            snapshot.buy_amount,
            snapshot.total_amount,
            snapshot.guest_email,
            snapshot.payment_name,
            snapshot.in_stock,
            snapshot.created_at
        );
        let url = if config.is_open_bark_push_url {
            Some(detail_url.clone())
        } else {
            None
        };
        if let Err(err) = send_bark(&config.bark_url, &title, &body, &webname, url.as_deref()).await
        {
            errors.push(format!("bark:{err}"));
        }
    }
    if config.is_open_wecom && !config.wecom_webhook.trim().is_empty() {
        sent_any = true;
        let text = format!(
            "{title}\n订单号: {}\n商品: {} x {}\n金额: {} 元\n邮箱: {}\n支付: {}\n时间: {}",
            snapshot.order_no,
            snapshot.product_name,
            snapshot.buy_amount,
            snapshot.total_amount,
            snapshot.guest_email,
            snapshot.payment_name,
            snapshot.created_at
        );
        if let Err(err) = send_wecom(&config.wecom_webhook, &text).await {
            errors.push(format!("wecom:{err}"));
        }
    }

    if !sent_any {
        record(
            state,
            "admin_notification",
            "",
            payload,
            "skipped",
            "notification_config_empty",
        )
        .await?;
        return Ok(());
    }
    if !errors.is_empty() {
        let error = errors.join("; ");
        record(
            state,
            "admin_notification",
            "admin",
            payload,
            "failed",
            &error,
        )
        .await?;
        anyhow::bail!(error);
    }
    record(state, "admin_notification", "admin", payload, "sent", "").await?;
    Ok(())
}

async fn load_snapshot(state: &AppState, order_id: i64) -> anyhow::Result<OrderSnapshot> {
    let row = sqlx::query(
        "SELECT o.order_no, o.guest_email, o.total_amount_cents, o.created_at,
                COALESCE(pc.name, '') AS payment_name,
                COALESCE(oi.product_name, '') AS product_name,
                COALESCE(oi.quantity, 1) AS quantity,
                COALESCE(oi.product_id, 0) AS product_id
         FROM orders o
         LEFT JOIN order_items oi ON oi.order_id = o.id
         LEFT JOIN payment_channels pc ON pc.id = o.payment_channel_id
         WHERE o.id = ?
         LIMIT 1",
    )
    .bind(order_id)
    .fetch_one(&state.pool)
    .await?;
    let product_id: i64 = row.try_get("product_id").unwrap_or(0);
    let in_stock: i64 = if product_id > 0 {
        sqlx::query_scalar(
            "SELECT CASE WHEN p.fulfillment_type = 'auto'
                THEN COALESCE((SELECT COUNT(*) FROM card_secrets c WHERE c.product_id = p.id AND c.status = 'available' AND c.deleted_at IS NULL), 0)
                ELSE MAX(p.manual_stock_total - p.manual_stock_locked, 0)
             END
             FROM products p WHERE p.id = ?",
        )
        .bind(product_id)
        .fetch_optional(&state.pool)
        .await?
        .unwrap_or(0)
    } else {
        0
    };
    Ok(OrderSnapshot {
        order_no: row.get("order_no"),
        guest_email: row.get("guest_email"),
        total_amount: crate::money::format_cents(row.get::<i64, _>("total_amount_cents")),
        product_name: row.get("product_name"),
        buy_amount: row.get("quantity"),
        payment_name: row.get("payment_name"),
        created_at: row.get("created_at"),
        in_stock,
    })
}

fn md_escape(value: &str) -> String {
    // Telegram legacy Markdown escape: backslash special chars
    value
        .replace('_', "\\_")
        .replace('*', "\\*")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('`', "\\`")
}

async fn load_config(state: &AppState) -> anyhow::Result<NotificationConfig> {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'notification_config'")
            .fetch_optional(&state.pool)
            .await?;
    let mut config: NotificationConfig = raw
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    config.server_chan_key = state.secret_box.decrypt(&config.server_chan_key);
    config.telegram_bot_token = state.secret_box.decrypt(&config.telegram_bot_token);
    config.bark_url = state.secret_box.decrypt(&config.bark_url);
    config.wecom_webhook = state.secret_box.decrypt(&config.wecom_webhook);
    Ok(config)
}

async fn send_server_chan(key: &str, title: &str, body: &str) -> anyhow::Result<()> {
    let url = format!("https://sctapi.ftqq.com/{}.send", key.trim());
    reqwest::Client::new()
        .post(url)
        .form(&[("title", title), ("desp", body)])
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn send_telegram(token: &str, chat_id: &str, text: &str) -> anyhow::Result<()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token.trim());
    reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
            "disable_web_page_preview": true
        }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn send_bark(
    base_url: &str,
    title: &str,
    body: &str,
    group: &str,
    deep_link: Option<&str>,
) -> anyhow::Result<()> {
    let mut payload = serde_json::json!({
        "title": title,
        "body": body,
        "group": group,
        "level": "timeSensitive",
    });
    if let Some(url) = deep_link {
        payload["url"] = serde_json::Value::String(url.to_string());
    }
    reqwest::Client::new()
        .post(base_url.trim())
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn send_wecom(webhook: &str, text: &str) -> anyhow::Result<()> {
    reqwest::Client::new()
        .post(webhook.trim())
        .json(&serde_json::json!({
            "msgtype": "text",
            "text": { "content": text }
        }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn record(
    state: &AppState,
    kind: &str,
    target: &str,
    payload: &Value,
    status: &str,
    error: &str,
) -> anyhow::Result<()> {
    let now = time::now_str();
    sqlx::query(
        "INSERT INTO notification_logs(kind, target, payload_json, status, error, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(kind)
    .bind(target)
    .bind(payload.to_string())
    .bind(status)
    .bind(error)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}
