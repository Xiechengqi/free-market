use axum::{
    Router,
    extract::State,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use std::sync::OnceLock;
use std::time::Instant;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::{
    security::{csrf, session},
    state::AppState,
    view::{admin_spa, assets},
    web::{
        admin::{self, api as admin_api},
        frontend, observability,
    },
};

static START_INSTANT: OnceLock<Instant> = OnceLock::new();

async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    let started = START_INSTANT.get_or_init(Instant::now);
    let db_ok = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    axum::Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "db": if db_ok { "ok" } else { "down" },
        "uptime_secs": started.elapsed().as_secs(),
        "worker": state.worker_id,
    }))
}

/// Legacy `/install` URL — kept as a 302 redirect to the SPA root, which now
/// owns the entire setup → login → dashboard flow.
async fn redirect_install(State(state): State<AppState>) -> impl IntoResponse {
    axum::response::Redirect::to(&format!("{}/", state.admin_prefix()))
}

pub fn router(state: AppState) -> Router {
    let admin_prefix = normalize_admin_prefix(&state.config.admin.route_prefix);
    let _protected_admin = Router::new()
        .route("/", get(admin::dashboard))
        .route("/orders", get(admin::orders))
        .route("/orders/export", get(admin::export_orders))
        .route("/orders/:id", get(admin::order))
        .route("/orders/:id/fulfill", post(admin::fulfill))
        .route("/orders/:id/cancel", post(admin::cancel_order))
        .route("/orders/:id/resend-email", post(admin::resend_order_email))
        .route(
            "/orders/:id/mark-abnormal",
            post(admin::mark_order_abnormal),
        )
        .route("/orders/:id/delete", post(admin::delete_order))
        .route(
            "/orders/:id/start-processing",
            post(admin::start_order_processing),
        )
        .route(
            "/categories",
            get(admin::categories).post(admin::create_category),
        )
        .route("/categories/:id", post(admin::update_category))
        .route("/categories/:id/delete", post(admin::delete_category))
        .route(
            "/products",
            get(admin::products).post(admin::create_product),
        )
        .route("/products/:id", post(admin::update_product))
        .route("/products/:id/delete", post(admin::delete_product))
        .route("/cards", get(admin::global_cards))
        .route("/cards/export", get(admin::export_global_cards))
        .route("/cards/:id/delete", post(admin::delete_global_card))
        .route("/products/:id/cards", get(admin::cards))
        .route("/products/:id/cards/export", get(admin::export_cards))
        .route("/products/:id/cards/import", post(admin::import_cards))
        .route(
            "/products/:product_id/cards/:card_id/delete",
            post(admin::delete_card),
        )
        .route(
            "/payment-channels",
            get(admin::payment_channels).post(admin::create_payment_channel),
        )
        .route("/payment-channels/:id", post(admin::update_payment_channel))
        .route(
            "/payment-channels/:id/delete",
            post(admin::delete_payment_channel),
        )
        .route("/coupons", get(admin::coupons).post(admin::create_coupon))
        .route("/coupons/:id", post(admin::update_coupon))
        .route("/coupons/:id/delete", post(admin::delete_coupon))
        .route(
            "/email-templates",
            get(admin::email_templates).post(admin::create_email_template),
        )
        .route("/email-templates/:id", post(admin::update_email_template))
        .route(
            "/email-templates/:id/delete",
            post(admin::delete_email_template),
        )
        .route(
            "/email-templates/restore-defaults",
            post(admin::restore_default_email_templates),
        )
        .route(
            "/email-test",
            get(admin::email_test).post(admin::send_email_test),
        )
        .route("/admins", get(admin::admins).post(admin::create_admin))
        .route("/admins/:id", post(admin::update_admin))
        .route("/uploads", get(admin::uploads).post(admin::upload_file))
        .route("/uploads/cleanup", post(admin::cleanup_uploads))
        .route("/jobs", get(admin::jobs))
        .route("/jobs/:id/retry", post(admin::retry_job))
        .route("/jobs/cleanup", post(admin::cleanup_runtime))
        .route("/notification-logs", get(admin::notification_logs))
        .route("/trash", get(admin::trash))
        .route("/trash/:table/:id/restore", post(admin::restore_trash))
        .route("/audit-logs", get(admin::audit_logs))
        .route("/backup", get(admin::backup))
        .route("/backup/create", post(admin::create_backup))
        .route("/backup/files/:filename", get(admin::download_backup_file))
        .route("/backup/settings", post(admin::save_backup_settings))
        .route("/settings", get(admin::settings).post(admin::save_settings))
        .route("/logout", post(admin::logout))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            session::admin_auth_middleware,
        ));

    let frontend_forms = Router::new()
        .route("/create-order", post(frontend::create_order))
        .route("/search-order-by-sn", post(frontend::search_by_sn))
        .route("/search-order-by-email", post(frontend::search_by_email))
        .route(
            "/search-order-by-browser",
            post(frontend::search_by_browser),
        );

    Router::new()
        .route("/healthz", get(healthz))
        .route("/assets/*path", get(assets::asset))
        .route("/uploads/*path", get(admin::uploaded_file))
        .route("/", get(frontend::home))
        .route("/buy/:id", get(frontend::buy))
        .route("/bill/:order_no", get(frontend::bill))
        .route("/detail-order-sn/:order_no", get(frontend::detail_order))
        .route("/order-search", get(frontend::search_page))
        .route(
            "/check-order-status/:order_no",
            get(frontend::check_order_status),
        )
        .route(
            "/pay-gateway/:handle/:payway/:order_no",
            get(frontend::pay_gateway),
        )
        .route(
            "/pay/:provider/:payway/:order_no",
            get(frontend::provider_pay_gateway),
        )
        .route("/pay/:provider/return_url", get(frontend::payment_return))
        .route("/captcha/:id", get(frontend::captcha_svg))
        .route(
            "/payment/noop/success/:payment_no",
            get(frontend::noop_success),
        )
        .route(
            "/payment/callback/epay/:channel_type",
            get(frontend::epay_callback_get).post(frontend::epay_callback_post),
        )
        .route(
            "/payment/callback/yipay/:channel_type",
            get(frontend::epay_callback_get).post(frontend::epay_callback_post),
        )
        .route(
            "/pay/yipay/notify_url",
            get(frontend::epay_callback_get).post(frontend::epay_callback_post),
        )
        .route(
            "/pay/:provider/notify_url",
            get(frontend::provider_callback_get).post(frontend::provider_callback_post),
        )
        .route(
            "/payment/callback/tokenpay/:channel_type",
            get(frontend::tokenpay_callback_get).post(frontend::tokenpay_callback_post),
        )
        .route(
            "/payment/callback/epusdt/:channel_type",
            get(frontend::epusdt_callback_get).post(frontend::epusdt_callback_post),
        )
        .route(
            "/payment/callback/:provider/:channel_type",
            get(frontend::provider_channel_callback_get)
                .post(frontend::provider_channel_callback_post),
        )
        .route(
            "/pay/tokenpay/notify_url",
            get(frontend::tokenpay_callback_get).post(frontend::tokenpay_callback_post),
        )
        .route(
            "/pay/epusdt/notify_url",
            get(frontend::epusdt_callback_get).post(frontend::epusdt_callback_post),
        )
        .route("/install", get(redirect_install))
        .nest(
            &format!("{}/api", admin_prefix),
            build_admin_api(state.clone()),
        )
        .route(&admin_prefix, get(admin_spa::admin_spa_index))
        .route(
            &format!("{}/", admin_prefix),
            get(admin_spa::admin_spa_index),
        )
        .route(
            &format!("{}/*path", admin_prefix),
            get(admin_spa::admin_spa_handler),
        )
        .merge(frontend_forms)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            csrf::csrf_middleware,
        ))
        .layer(RequestBodyLimitLayer::new(8 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(observability::request_id_middleware))
        .with_state(state)
}

fn normalize_admin_prefix(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/admin".to_string();
    }
    let mut prefix = trimmed.to_string();
    if !prefix.starts_with('/') {
        prefix = format!("/{}", prefix);
    }
    while prefix.len() > 1 && prefix.ends_with('/') {
        prefix.pop();
    }
    prefix
}

/// `/admin/api/*` JSON sub-tree. `auth/login` and `auth/refreshToken` are
/// unauthenticated; everything else passes through `bearer_auth`.
fn build_admin_api(state: AppState) -> Router<AppState> {
    use admin_api::resources as r;

    let public = Router::new()
        .route("/auth/login", post(admin_api::auth::login))
        .route("/auth/refreshToken", post(admin_api::auth::refresh))
        .route("/setup/status", get(admin_api::setup::status))
        .route("/setup/install", post(admin_api::setup::install))
        .route("/site-info", get(admin_api::site_info::site_info))
        .route("/docs", get(admin_api::docs::docs));

    let private = Router::new()
        .route("/auth/getUserInfo", get(admin_api::auth::get_user_info))
        .route("/auth/logout", post(admin_api::auth::logout))
        // dashboard
        .route("/dashboard", get(r::get_dashboard))
        // orders
        .route("/orders", get(r::list_orders))
        .route("/orders/:id", get(r::get_order))
        .route("/orders/:id/fulfill", post(r::fulfill_order))
        .route("/orders/:id/cancel", post(r::cancel_order))
        .route("/orders/:id/resend-email", post(r::resend_order_email))
        .route("/orders/:id/mark-abnormal", post(r::mark_order_abnormal))
        .route("/orders/:id/delete", post(r::delete_order))
        .route(
            "/orders/:id/start-processing",
            post(r::start_order_processing),
        )
        .route(
            "/orders/:id/evm-intents/:intent_id/confirm",
            post(r::confirm_evm_intent),
        )
        // categories
        .route(
            "/categories",
            get(r::list_categories).post(r::create_category),
        )
        .route(
            "/categories/:id",
            post(r::update_category).delete(r::delete_category),
        )
        // products
        .route("/products", get(r::list_products).post(r::create_product))
        .route(
            "/products/:id",
            post(r::update_product).delete(r::delete_product),
        )
        // coupons
        .route("/coupons", get(r::list_coupons).post(r::create_coupon))
        .route(
            "/coupons/:id",
            post(r::update_coupon).delete(r::delete_coupon),
        )
        // payment channels (owner-only mutate)
        .route(
            "/payment-channels",
            get(r::list_payment_channels).post(r::create_payment_channel),
        )
        .route(
            "/payment-channels/validate",
            post(r::validate_payment_channel),
        )
        .route("/payment-channels/evm-presets", get(r::evm_payment_presets))
        .route(
            "/payment-channels/:id",
            post(r::update_payment_channel).delete(r::delete_payment_channel),
        )
        // settings (owner-only)
        .route("/settings", get(r::get_settings).post(r::save_settings))
        // email templates
        .route(
            "/email-templates",
            get(r::list_email_templates).post(r::create_email_template),
        )
        .route(
            "/email-templates/:id",
            post(r::update_email_template).delete(r::delete_email_template),
        )
        .route(
            "/email-templates/restore-defaults",
            post(r::restore_default_email_templates),
        )
        // admins (owner-only)
        .route("/admins", get(r::list_admins).post(r::create_admin))
        .route("/admins/:id", post(r::update_admin))
        // jobs / notifications / audit / trash
        .route("/jobs", get(r::list_jobs))
        .route("/jobs/:id/retry", post(r::retry_job))
        .route("/jobs/cleanup", post(r::cleanup_runtime))
        .route("/notification-logs", get(r::list_notification_logs))
        .route("/audit-logs", get(r::list_audit_logs))
        .route("/trash", get(r::list_trash))
        .route("/trash/:table/:id/restore", post(r::restore_trash))
        // cards / carmis
        .route("/products/:id/cards", get(r::list_product_cards))
        .route("/products/:id/cards/import", post(r::import_product_cards))
        .route("/products/:id/cards/export", get(r::export_product_cards))
        .route(
            "/products/:id/cards/:card_id",
            axum::routing::delete(r::delete_product_card),
        )
        .route("/cards", get(r::list_global_cards))
        .route("/cards/export", get(r::export_global_cards_csv))
        .route("/cards/:id", axum::routing::delete(r::delete_global_card))
        // uploads
        .route("/uploads", get(r::list_uploads).post(r::upload_file))
        .route("/uploads/cleanup", post(r::cleanup_uploads))
        // email test
        .route(
            "/email-test",
            get(r::get_email_test).post(r::send_email_test),
        )
        // backup
        .route("/backup", get(r::get_backup_page))
        .route("/backup/create", post(r::create_backup))
        .route("/backup/files/:filename", get(r::download_backup))
        .route("/backup/settings", post(r::save_backup_settings))
        // orders export
        .route("/orders/export", get(r::export_orders))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            admin_api::middleware::bearer_auth,
        ));

    Router::new().merge(public).merge(private)
}
