use sqlx::Row;

use crate::{models, security::password, state::AppState, time};

pub async fn bootstrap(state: &AppState) -> anyhow::Result<()> {
    let now = time::now_str();
    let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admins")
        .fetch_one(&state.pool)
        .await?;
    let force_install = std::env::var("DUJIAO_ENABLE_INSTALL")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
    let default_password = state.config.admin.bootstrap_password.trim() == "admin123456";
    let allow_auto_seed = std::env::var("DUJIAO_ALLOW_DEFAULT_ADMIN")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
    if admin_count == 0 {
        if force_install || (default_password && !allow_auto_seed) {
            tracing::warn!(
                "no admin exists and bootstrap_password is the well-known default; \
                 visit /install to create the first administrator. \
                 Set DUJIAO_ALLOW_DEFAULT_ADMIN=1 to skip this guard."
            );
        } else {
            let hash = password::hash_password(&state.config.admin.bootstrap_password)?;
            sqlx::query(
                "INSERT INTO admins(username, password_hash, display_name, role, created_at, updated_at)
                 VALUES (?, ?, ?, 'owner', ?, ?)",
            )
            .bind(&state.config.admin.bootstrap_username)
            .bind(hash)
            .bind("Administrator")
            .bind(&now)
            .bind(&now)
            .execute(&state.pool)
            .await?;
        }
    }

    let category_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories")
        .fetch_one(&state.pool)
        .await?;
    if category_count == 0 {
        let mut tx = state.pool.begin().await?;
        sqlx::query(
            "INSERT INTO categories(name, is_active, sort_order, created_at, updated_at)
             VALUES ('AWS', 1, 100, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        let category_id = sqlx::query("SELECT id FROM categories WHERE name='AWS'")
            .fetch_one(&mut *tx)
            .await?
            .get::<i64, _>("id");
        sqlx::query(
            "INSERT INTO products(category_id, slug, name, short_description, description_html,
             retail_price_cents, price_cents, fulfillment_type, buy_limit_num, is_active, sort_order, created_at, updated_at)
             VALUES (?, 'aws-demo', 'AWS 25刀优惠码 21号 2026.12.31到期 可使用2个', '自动发卡演示商品',
             '这是 Dujiao Rust 初始化的演示商品。', 9500, 9500, 'auto', 0, 1, 100, ?, ?)",
        )
        .bind(category_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        let product_id = sqlx::query("SELECT id FROM products WHERE slug='aws-demo'")
            .fetch_one(&mut *tx)
            .await?
            .get::<i64, _>("id");
        sqlx::query(
            "INSERT INTO product_skus(product_id, sku_code, price_cents, is_active, sort_order, created_at, updated_at)
             VALUES (?, 'DEFAULT', 9500, 1, 100, ?, ?)",
        )
        .bind(product_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        for idx in 1..=5 {
            sqlx::query(
                "INSERT INTO card_secrets(product_id, sku_id, secret, status, created_at, updated_at)
                 VALUES (?, 0, ?, ?, ?, ?)",
            )
            .bind(product_id)
            .bind(format!("DEMO-CARD-SECRET-{}", idx))
            .bind(models::CARD_AVAILABLE)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }

    let channel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payment_channels")
        .fetch_one(&state.pool)
        .await?;
    if channel_count == 0 {
        sqlx::query(
            "INSERT INTO payment_channels(name, provider_type, channel_type, interaction_mode, config_json, is_active, sort_order, created_at, updated_at)
             VALUES ('模拟支付', 'noop', 'test', 'redirect', '{}', 1, 100, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&state.pool)
        .await?;
    }

    let smtp_config_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = 'smtp_config'")
            .fetch_one(&state.pool)
            .await?;
    if smtp_config_count == 0 {
        sqlx::query(
            "INSERT INTO settings(key, value_json, created_at, updated_at)
             VALUES ('smtp_config', ?, ?, ?)",
        )
        .bind(
            serde_json::json!({
                "enabled": false,
                "host": "",
                "port": 587,
                "username": "",
                "password": "",
                "from_email": "",
                "from_name": "Dujiao Rust",
                "encryption": "starttls"
            })
            .to_string(),
        )
        .bind(&now)
        .bind(&now)
        .execute(&state.pool)
        .await?;
    }

    let email_template_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_templates")
        .fetch_one(&state.pool)
        .await?;
    if email_template_count == 0 {
        for (token, subject, content) in default_email_templates() {
            sqlx::query(
                "INSERT INTO email_templates(token, subject, content, is_system, created_at, updated_at)
                 VALUES (?, ?, ?, 1, ?, ?)",
            )
            .bind(token)
            .bind(subject)
            .bind(content)
            .bind(&now)
            .bind(&now)
            .execute(&state.pool)
            .await?;
        }
    } else {
        for (token, subject, content) in default_email_templates() {
            let exists: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM email_templates WHERE token = ?")
                    .bind(token)
                    .fetch_one(&state.pool)
                    .await?;
            if exists == 0 {
                sqlx::query(
                    "INSERT INTO email_templates(token, subject, content, is_system, created_at, updated_at)
                     VALUES (?, ?, ?, 1, ?, ?)",
                )
                .bind(token)
                .bind(subject)
                .bind(content)
                .bind(&now)
                .bind(&now)
                .execute(&state.pool)
                .await?;
            }
        }
    }
    sqlx::query(
        "UPDATE email_templates SET is_system = 1
         WHERE token IN ('card_send_user_email', 'manual_send_user_email',
                         'manual_send_manage_mail', 'pending_order',
                         'completed_order', 'failed_order')",
    )
    .execute(&state.pool)
    .await?;

    Ok(())
}

pub fn default_email_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        (
            "card_send_user_email",
            "【{webname}】您的订单 {order_no} 已完成",
            "订单号：{order_no}\n下单时间：{created_at}\n商品：{product_name} x {buy_amount}\n订单金额：{amount}\n\n卡密内容：\n{fulfillment}\n\n来自 {webname} {weburl}",
        ),
        (
            "manual_send_user_email",
            "【{webname}】您的订单 {order_no} 已处理",
            "订单号：{order_no}\n下单时间：{created_at}\n商品：{product_name} x {buy_amount}\n订单金额：{amount}\n\n发货内容：\n{fulfillment}\n\n来自 {webname} {weburl}",
        ),
        (
            "manual_send_manage_mail",
            "【{webname}】新订单等待处理！",
            "尊敬的管理员：\n\n客户购买的商品【{product_name}】已支付成功，请及时处理。\n\n订单号：{order_no}\n数量：{buy_amount}\n金额：{amount}\n时间：{created_at}\n\n用户提交信息：\n{order_info}\n\n来自 {webname} {weburl}",
        ),
        (
            "pending_order",
            "【{webname}】已收到您的订单，请等候处理",
            "尊敬的客户：\n\n订单号：{order_no}\n时间：{created_at}\n商品：{product_name} x {buy_amount}\n金额：{amount}\n\n系统已向工作人员发送订单通知，代充类商品需要工作人员手动处理，请耐心等待通知。\n\n来自 {webname} {weburl}",
        ),
        (
            "completed_order",
            "【{webname}】您的订单已经处理完成！",
            "尊敬的客户：\n\n订单号：{order_no}\n时间：{created_at}\n商品：{product_name} x {buy_amount}\n金额：{amount}\n\n您的订单已经处理完毕，请及时前往网站核对处理结果。\n\n发货内容：\n{fulfillment}\n\n来自 {webname} {weburl}",
        ),
        (
            "failed_order",
            "【{webname}】订单处理失败！",
            "尊敬的客户：\n\n非常遗憾，订单处理失败 (╥﹏╥)。\n\n订单号：{order_no}\n时间：{created_at}\n商品：{product_name} x {buy_amount}\n金额：{amount}\n\n请及时联系网站工作人员核查原因。\n\n来自 {webname} {weburl}",
        ),
    ]
}
