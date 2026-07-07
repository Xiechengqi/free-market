use std::sync::Arc;

use sqlx::SqlitePool;

use free_market::{
    config::{AdminConfig, AppConfig, DatabaseConfig, ServerConfig, SiteConfig},
    db,
    security::{jwt::Jwt, secrets::SecretManager, session},
    services::order_service,
    state::AppState,
    view::render::ViewRenderer,
};

pub struct TestEnv {
    pub state: AppState,
    pub _tempdir: tempfile::TempDir,
}

pub async fn boot() -> TestEnv {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let db_path = tempdir.path().join("freemarket-test.db");
    let config = AppConfig {
        server: ServerConfig {
            host: "127.0.0.1".parse().unwrap(),
            port: 0,
            run_worker: false,
        },
        database: DatabaseConfig { path: db_path },
        site: SiteConfig {
            name: "Test".to_string(),
            logo_text: "Test".to_string(),
            notice: String::new(),
            footer: String::new(),
            base_url: "http://127.0.0.1:0".to_string(),
            theme: "luna".to_string(),
            order_expire_minutes: 1,
            keywords: String::new(),
            description: String::new(),
            is_open_anti_red: false,
            is_open_google_translate: false,
            language: "zh-CN".to_string(),
            img_logo: String::new(),
        },
        admin: AdminConfig {
            route_prefix: "/admin".to_string(),
            bootstrap_username: "admin".to_string(),
            bootstrap_password: "test-not-default-do-not-use".to_string(),
            app_secret: "unit-test-secret".to_string(),
        },
    };
    let pool = db::sqlite::connect(&config.database).await.unwrap();
    db::schema::apply(&pool, &config.database.path).await.unwrap();
    let csrf_token = session::new_token();
    let (secret_manager, _, _) = SecretManager::load_or_create(
        tempdir.path().join("app.secret"),
        &config.effective_app_secret(),
    )
    .unwrap();
    let secret_box = Arc::new(secret_manager);
    let jwt = Arc::new(Jwt::from_app_secret(&config.effective_app_secret()));
    let state = AppState {
        config: Arc::new(config),
        pool,
        views: Arc::new(
            ViewRenderer::with_admin_prefix(csrf_token.clone(), "/admin".to_string()).unwrap(),
        ),
        worker_id: "test-worker".to_string(),
        csrf_token,
        secret_box,
        jwt,
    };
    seed(&state).await;
    TestEnv {
        state,
        _tempdir: tempdir,
    }
}

async fn seed(state: &AppState) {
    let now = "2026-06-16T00:00:00+00:00";
    sqlx::query(
        "INSERT INTO categories(id, name, is_active, sort_order, created_at, updated_at)
         VALUES (1, 'cat', 1, 100, ?, ?)",
    )
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO products(id, category_id, slug, name, short_description, description_html,
         retail_price_cents, price_cents, fulfillment_type, buy_limit_num, is_active, sort_order, created_at, updated_at)
         VALUES (1, 1, 'p1', 'p1', '', '', 0, 1000, 'auto', 0, 1, 100, ?, ?)",
    )
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .unwrap();
    for i in 1..=5 {
        sqlx::query(
            "INSERT INTO card_secrets(product_id, sku_id, secret, status, created_at, updated_at)
             VALUES (1, 0, ?, 'available', ?, ?)",
        )
        .bind(format!("CARD-{i}"))
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO payment_channels(id, name, provider_type, channel_type, interaction_mode, config_json, is_active, sort_order, created_at, updated_at)
         VALUES (1, 'noop', 'noop', 'test', 'redirect', '{}', 1, 100, ?, ?)",
    )
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO coupons(id, code, type, value_cents, min_amount_cents, usage_limit, used_count, is_active, deleted_at, created_at, updated_at)
         VALUES (1, 'TENOFF', 'fixed', 10, 0, 2, 0, 1, NULL, ?, ?)",
    )
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO coupon_products(coupon_id, product_id) VALUES (1, 1)")
        .execute(&state.pool)
        .await
        .unwrap();
}

#[allow(dead_code)]
pub async fn make_order(state: &AppState, email: &str, ip: &str) -> String {
    let form = order_service::CreateOrderForm {
        gid: 1,
        email: email.to_string(),
        by_amount: 1,
        payway: 1,
        search_pwd: None,
        coupon_code: None,
        captcha_id: None,
        captcha_answer: None,
        extra: std::collections::HashMap::new(),
    };
    order_service::create_guest_order(state, form, ip.to_string())
        .await
        .unwrap()
}

#[allow(dead_code)]
pub async fn count_card_status(pool: &SqlitePool, status: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM card_secrets WHERE status = ?")
        .bind(status)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[allow(dead_code)]
pub async fn create_payment_for(state: &AppState, order_no: &str) -> i64 {
    let headers = axum::http::HeaderMap::new();
    let payment =
        free_market::services::payment_service::create_payment(state, order_no, 1, &headers)
            .await
            .expect("create payment");
    let payment_id: i64 = sqlx::query_scalar("SELECT id FROM payments WHERE payment_no = ?")
        .bind(payment.payment_no)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    payment_id
}
