use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;

use crate::{money, services::settings_service, state::AppState, time};

#[derive(Debug, Clone, Deserialize)]
struct SmtpConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    host: String,
    #[serde(default = "default_smtp_port")]
    port: u16,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    from_email: String,
    #[serde(default = "default_from_name")]
    from_name: String,
    #[serde(default = "default_encryption")]
    encryption: String,
}

#[derive(Debug)]
struct EmailContext {
    order_id: i64,
    order_no: String,
    email: String,
    amount: String,
    fulfillment: String,
    product_name: String,
    buy_amount: i64,
    order_info: String,
    webname: String,
    weburl: String,
    created_at: String,
}

pub async fn record_order_email_job(state: &AppState, payload: &Value) -> anyhow::Result<()> {
    let mut ctx = load_email_context(state, payload).await?;
    if let Some(to) = payload
        .get("to")
        .and_then(|value| value.as_str())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        ctx.email = to.to_string();
    }
    let config = load_smtp_config(state).await?;
    if !config.enabled {
        record_notification(
            state,
            &ctx.email,
            payload,
            "skipped",
            "smtp_config_not_enabled",
        )
        .await?;
        return Ok(());
    }
    if ctx.email.trim().is_empty() {
        record_notification(
            state,
            &ctx.email,
            payload,
            "skipped",
            "recipient_email_empty",
        )
        .await?;
        return Ok(());
    }
    validate_smtp_config(&config)?;

    let template_token = payload
        .get("template")
        .and_then(|value| value.as_str())
        .unwrap_or("card_send_user_email");
    let (subject_template, content_template) = load_template(state, template_token).await?;
    let subject = render_template(&subject_template, &ctx);
    let body = render_template(&content_template, &ctx);
    let message = build_message(&config, &ctx.email, subject, body)?;

    if let Err(err) = send_message(&config, message).await {
        record_notification(state, &ctx.email, payload, "failed", &err.to_string()).await?;
        return Err(err);
    }
    record_notification(state, &ctx.email, payload, "sent", "").await?;
    Ok(())
}

pub async fn send_test_email(
    state: &AppState,
    to: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    let config = load_smtp_config(state).await?;
    validate_smtp_config(&config)?;
    let message = build_message(&config, to, subject.to_string(), body.to_string())?;
    let payload = serde_json::json!({
        "to": to,
        "subject": subject,
        "test": true,
    });
    if let Err(err) = send_message(&config, message).await {
        record_notification(state, to, &payload, "failed", &err.to_string()).await?;
        return Err(err);
    }
    record_notification(state, to, &payload, "sent", "").await?;
    Ok(())
}

async fn load_email_context(state: &AppState, payload: &Value) -> anyhow::Result<EmailContext> {
    let order_id = payload
        .get("order_id")
        .and_then(|value| value.as_i64())
        .unwrap_or_default();
    let order = sqlx::query(
        "SELECT order_no, guest_email, total_amount_cents, legacy_info, created_at FROM orders WHERE id = ?",
    )
    .bind(order_id)
    .fetch_one(&state.pool)
    .await?;
    let fulfillment: Option<String> =
        sqlx::query_scalar("SELECT payload FROM fulfillments WHERE order_id = ?")
            .bind(order_id)
            .fetch_optional(&state.pool)
            .await?;
    let item_row =
        sqlx::query("SELECT product_name, quantity FROM order_items WHERE order_id = ? LIMIT 1")
            .bind(order_id)
            .fetch_optional(&state.pool)
            .await?;
    let (product_name, buy_amount) = match item_row {
        Some(row) => (
            row.get::<String, _>("product_name"),
            row.get::<i64, _>("quantity"),
        ),
        None => (String::new(), 1),
    };
    let guest_email = order.get::<String, _>("guest_email");
    let site = settings_service::runtime_site_config(state).await;
    let order_info_raw: Option<String> =
        sqlx::query_scalar("SELECT manual_form_json FROM order_items WHERE order_id = ? LIMIT 1")
            .bind(order_id)
            .fetch_optional(&state.pool)
            .await?;
    let order_info = match order_info_raw {
        Some(raw) if raw.trim() != "{}" && !raw.trim().is_empty() => raw,
        _ => order.get::<String, _>("legacy_info"),
    };
    Ok(EmailContext {
        order_id,
        order_no: order.get("order_no"),
        email: payload
            .get("email")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(guest_email.as_str())
            .to_string(),
        amount: money::format_cents(order.get("total_amount_cents")),
        fulfillment: fulfillment.unwrap_or_default(),
        product_name,
        buy_amount,
        order_info,
        webname: site.logo_text.clone(),
        weburl: site.base_url.clone(),
        created_at: order.get("created_at"),
    })
}

async fn load_smtp_config(state: &AppState) -> anyhow::Result<SmtpConfig> {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'smtp_config'")
            .fetch_optional(&state.pool)
            .await?;
    let Some(raw) = raw else {
        return Ok(SmtpConfig::default());
    };
    let mut config: SmtpConfig = serde_json::from_str(&raw).unwrap_or_default();
    config.password = state.secret_box.decrypt(&config.password);
    Ok(config)
}

async fn load_template(state: &AppState, token: &str) -> anyhow::Result<(String, String)> {
    let row = sqlx::query("SELECT subject, content FROM email_templates WHERE token = ?")
        .bind(token)
        .fetch_optional(&state.pool)
        .await?;
    if let Some(row) = row {
        return Ok((row.get("subject"), row.get("content")));
    }
    Ok((
        "您的订单 {{ order_no }} 已完成".to_string(),
        "订单号：{{ order_no }}\n订单金额：{{ amount }}\n\n发货内容：\n{{ fulfillment }}"
            .to_string(),
    ))
}

fn validate_smtp_config(config: &SmtpConfig) -> anyhow::Result<()> {
    if config.host.trim().is_empty() || config.from_email.trim().is_empty() || config.port == 0 {
        anyhow::bail!("smtp_config_incomplete");
    }
    if !matches!(config.encryption.as_str(), "starttls" | "tls" | "none") {
        anyhow::bail!("smtp_encryption_invalid");
    }
    Ok(())
}

fn build_message(
    config: &SmtpConfig,
    to: &str,
    subject: String,
    body: String,
) -> anyhow::Result<Message> {
    let from = Mailbox::new(
        Some(config.from_name.clone()),
        config.from_email.trim().parse()?,
    );
    let to = Mailbox::new(None, to.trim().parse()?);
    Ok(Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .body(body)?)
}

async fn send_message(config: &SmtpConfig, message: Message) -> anyhow::Result<()> {
    let mut builder = match config.encryption.as_str() {
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)?,
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)?,
    }
    .port(config.port);
    if !config.username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(
            config.username.clone(),
            config.password.clone(),
        ));
    }
    builder.build().send(message).await?;
    Ok(())
}

fn render_template(template: &str, ctx: &EmailContext) -> String {
    let pairs = [
        ("order_id", ctx.order_id.to_string()),
        ("order_no", ctx.order_no.clone()),
        ("order_sn", ctx.order_no.clone()),
        ("email", ctx.email.clone()),
        ("amount", ctx.amount.clone()),
        ("ord_price", ctx.amount.clone()),
        ("fulfillment", ctx.fulfillment.clone()),
        ("ord_info", ctx.fulfillment.clone()),
        ("product_name", ctx.product_name.clone()),
        ("ord_title", ctx.product_name.clone()),
        ("buy_amount", ctx.buy_amount.to_string()),
        ("order_info", ctx.order_info.clone()),
        ("webname", ctx.webname.clone()),
        ("weburl", ctx.weburl.clone()),
        ("created_at", ctx.created_at.clone()),
    ];
    let mut rendered = template.to_string();
    for (key, value) in pairs {
        rendered = rendered.replace(&format!("{{{{ {} }}}}", key), &value);
        rendered = rendered.replace(&format!("{{{{{}}}}}", key), &value);
        rendered = rendered.replace(&format!("{{{}}}", key), &value);
    }
    rendered
}

async fn record_notification(
    state: &AppState,
    target: &str,
    payload: &Value,
    status: &str,
    error: &str,
) -> anyhow::Result<()> {
    let now = time::now_str();
    sqlx::query(
        "INSERT INTO notification_logs(kind, target, payload_json, status, error, created_at, updated_at)
         VALUES ('order_status_email', ?, ?, ?, ?, ?, ?)",
    )
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

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: default_smtp_port(),
            username: String::new(),
            password: String::new(),
            from_email: String::new(),
            from_name: default_from_name(),
            encryption: default_encryption(),
        }
    }
}

fn default_smtp_port() -> u16 {
    587
}

fn default_from_name() -> String {
    "Dujiao Rust".to_string()
}

fn default_encryption() -> String {
    "starttls".to_string()
}
