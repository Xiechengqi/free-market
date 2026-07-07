use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    mail,
    models::order::{Fulfillment, Order},
    money,
    security::password,
    services::{fulfillment_service, order_service, settings_service},
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct DashboardData {
    pub order_count: i64,
    pub product_count: i64,
    pub available_cards: i64,
    pub completed_order_count: i64,
    pub pending_order_count: i64,
    pub canceled_order_count: i64,
    pub total_sales_display: String,
    pub today_order_count: i64,
    pub today_sales_display: String,
    pub success_rate: String,
    pub low_stock_count: i64,
    pub readiness_blockers: i64,
    pub readiness_warnings: i64,
    pub readiness_checks: Vec<ReadinessCheck>,
}

#[derive(Debug, Serialize)]
pub struct ReadinessCheck {
    pub level: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Deserialize)]
pub struct InstallForm {
    pub site_name: String,
    pub logo_text: String,
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub password_confirm: String,
}

#[derive(Debug, Serialize)]
pub struct AdminOrderRow {
    pub id: i64,
    pub order_no: String,
    pub status: String,
    pub amount_display: String,
    pub guest_email: String,
    pub product_name: String,
    pub payment_channel_name: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AdminOrdersData {
    pub orders: Vec<AdminOrderRow>,
    pub filter: AdminOrdersFilter,
    pub products: Vec<ProductOption>,
    pub payment_channels: Vec<PaymentChannelOption>,
    pub pagination: Pagination,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct AdminOrdersFilter {
    pub order_no: Option<String>,
    pub email: Option<String>,
    pub status: Option<String>,
    pub product_id: Option<i64>,
    pub payment_channel_id: Option<i64>,
    pub provider_ref: Option<String>,
    pub coupon_id: Option<i64>,
    pub fulfillment_type: Option<String>,
    pub payment_status: Option<String>,
    pub ip: Option<String>,
    pub amount_min_cents: Option<i64>,
    pub amount_max_cents: Option<i64>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AdminOrderData {
    pub order: Order,
    pub amount_display: String,
    pub fulfillment: Option<Fulfillment>,
    pub items: Vec<AdminOrderItemRow>,
    pub payments: Vec<AdminPaymentRow>,
    pub evm_intents: Vec<AdminEvmIntentRow>,
    pub notifications: Vec<NotificationLogRow>,
}

#[derive(Debug, Serialize)]
pub struct AdminOrderItemRow {
    pub product_name: String,
    pub quantity: i64,
    pub unit_price_display: String,
    pub total_price_display: String,
    pub fulfillment_type: String,
    pub manual_form_json: String,
}

#[derive(Debug, Serialize)]
pub struct AdminPaymentRow {
    pub payment_no: String,
    pub provider_type: String,
    pub channel_type: String,
    pub status: String,
    pub amount_display: String,
    pub provider_ref: String,
    pub gateway_order_no: String,
    pub paid_at: String,
    pub callback_at: String,
}

#[derive(Debug, Serialize)]
pub struct AdminEvmIntentRow {
    pub id: i64,
    pub payment_no: String,
    pub network_env: String,
    pub chain_id: i64,
    pub chain_slug: String,
    pub token_symbol: String,
    pub token_contract: String,
    pub receive_address: String,
    pub amount_text: String,
    pub status: String,
    pub scan_from_block: i64,
    pub last_scanned_block: i64,
    pub matched_tx_hash: String,
    pub matched_log_index: String,
    pub matched_from_address: String,
    pub matched_at: String,
    pub last_checked_at: String,
    pub last_error: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationLogRow {
    pub kind: String,
    pub target: String,
    pub status: String,
    pub error: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub is_active: i64,
    pub sort_order: i64,
}

#[derive(Debug, Serialize)]
pub struct CategoriesData {
    pub categories: Vec<CategoryRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct CategoryForm {
    pub name: String,
    pub sort_order: Option<i64>,
    pub is_active: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProductRow {
    pub id: i64,
    pub category_id: i64,
    pub category_name: String,
    pub name: String,
    pub short_description: String,
    pub keywords: String,
    pub image_path: String,
    pub retail_price_cents: i64,
    pub retail_price_display: String,
    pub price_cents: i64,
    pub price_display: String,
    pub fulfillment_type: String,
    pub description_html: String,
    pub wholesale_prices_json: String,
    pub manual_form_schema_json: String,
    pub buy_prompt: String,
    pub api_hook: String,
    pub payment_channel_ids_json: String,
    pub manual_stock_total: i64,
    pub buy_limit_num: i64,
    pub sort_order: i64,
    pub stock: i64,
    pub is_active: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductsData {
    pub products: Vec<ProductRow>,
    pub categories: Vec<CategoryRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct ProductForm {
    pub category_id: i64,
    pub name: String,
    pub short_description: Option<String>,
    pub keywords: Option<String>,
    pub image_path: Option<String>,
    pub retail_price_cents: Option<i64>,
    pub price_cents: i64,
    pub fulfillment_type: String,
    pub description_html: Option<String>,
    pub wholesale_prices_json: Option<String>,
    pub manual_form_schema_json: Option<String>,
    pub buy_prompt: Option<String>,
    pub api_hook: Option<String>,
    pub payment_channel_ids_json: Option<String>,
    pub manual_stock_total: Option<i64>,
    pub buy_limit_num: Option<i64>,
    pub sort_order: Option<i64>,
    pub is_active: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CardSecretRow {
    pub id: i64,
    pub secret: String,
    pub status: String,
    pub is_loop: i64,
    pub order_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CardsData {
    pub product_id: i64,
    pub product_name: String,
    pub cards: Vec<CardSecretRow>,
    pub filter: CardsFilter,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct GlobalCardsData {
    pub cards: Vec<GlobalCardSecretRow>,
    pub filter: GlobalCardsFilter,
    pub products: Vec<ProductOption>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct GlobalCardSecretRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub secret: String,
    pub status: String,
    pub is_loop: i64,
    pub order_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct GlobalCardsFilter {
    pub product_id: Option<i64>,
    pub status: Option<String>,
    pub is_loop: Option<String>,
    pub keyword: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct CardsFilter {
    pub status: Option<String>,
    pub is_loop: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ImportCardsForm {
    pub secrets: String,
    pub is_loop: Option<String>,
    pub remove_duplication: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaymentChannelRow {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub channel_type: String,
    pub interaction_mode: String,
    pub pay_check: String,
    pub client_scope: String,
    pub handleroute: String,
    pub config_json: String,
    pub is_active: i64,
}

#[derive(Debug, Serialize)]
pub struct PaymentChannelsData {
    pub channels: Vec<PaymentChannelRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct PaymentChannelOption {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct PaymentChannelForm {
    pub name: String,
    pub provider_type: String,
    pub channel_type: String,
    pub interaction_mode: String,
    pub pay_check: Option<String>,
    pub client_scope: Option<String>,
    pub handleroute: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_key: Option<String>,
    pub merchant_pem: Option<String>,
    pub config_json: Option<String>,
    pub is_active: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SettingsData {
    pub name: String,
    pub logo_text: String,
    pub keywords: String,
    pub description: String,
    pub manage_email: String,
    pub notice: String,
    pub footer: String,
    pub base_url: String,
    pub is_open_anti_red: bool,
    pub is_open_google_translate: bool,
    pub template: String,
    pub order_expire_minutes: i64,
    pub is_open_search_pwd: bool,
    pub purchase_rate_window_minutes: i64,
    pub purchase_rate_max_per_email: i64,
    pub purchase_rate_max_per_ip: i64,
    pub is_open_img_code: bool,
    pub login_max_attempts: i64,
    pub login_lock_minutes: i64,
    pub cookie_secure: bool,
    pub trust_proxy_hops: i64,
    pub smtp_enabled: bool,
    pub smtp_host: String,
    pub smtp_port: i64,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from_email: String,
    pub smtp_from_name: String,
    pub smtp_encryption: String,
    pub notify_server_chan_key: String,
    pub notify_telegram_bot_token: String,
    pub notify_telegram_chat_id: String,
    pub notify_bark_url: String,
    pub notify_wecom_webhook: String,
    pub is_open_server_chan: bool,
    pub is_open_telegram: bool,
    pub is_open_bark: bool,
    pub is_open_bark_push_url: bool,
    pub is_open_wecom: bool,
    pub language: String,
    pub img_logo: String,
}

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
    pub name: String,
    pub logo_text: String,
    pub keywords: Option<String>,
    pub description: Option<String>,
    pub manage_email: Option<String>,
    pub notice: String,
    pub footer: String,
    pub base_url: Option<String>,
    pub is_open_anti_red: Option<String>,
    pub is_open_google_translate: Option<String>,
    pub template: Option<String>,
    pub order_expire_minutes: Option<i64>,
    pub is_open_search_pwd: Option<String>,
    pub purchase_rate_window_minutes: Option<i64>,
    pub purchase_rate_max_per_email: Option<i64>,
    pub purchase_rate_max_per_ip: Option<i64>,
    pub is_open_img_code: Option<String>,
    pub login_max_attempts: Option<i64>,
    pub login_lock_minutes: Option<i64>,
    pub cookie_secure: Option<String>,
    pub trust_proxy_hops: Option<i64>,
    pub smtp_enabled: Option<String>,
    pub smtp_host: String,
    pub smtp_port: Option<i64>,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from_email: String,
    pub smtp_from_name: String,
    pub smtp_encryption: String,
    pub notify_server_chan_key: Option<String>,
    pub notify_telegram_bot_token: Option<String>,
    pub notify_telegram_chat_id: Option<String>,
    pub notify_bark_url: Option<String>,
    pub notify_wecom_webhook: Option<String>,
    pub is_open_server_chan: Option<String>,
    pub is_open_telegram: Option<String>,
    pub is_open_bark: Option<String>,
    pub is_open_bark_push_url: Option<String>,
    pub is_open_wecom: Option<String>,
    pub language: Option<String>,
    pub img_logo: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CouponRow {
    pub id: i64,
    pub code: String,
    pub r#type: String,
    pub value_cents: i64,
    pub value_display: String,
    pub min_amount_cents: i64,
    pub min_amount_display: String,
    pub usage_limit: i64,
    pub used_count: i64,
    pub is_active: i64,
    pub product_scope: String,
    pub product_id: Option<i64>,
    pub product_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct CouponsData {
    pub coupons: Vec<CouponRow>,
    pub products: Vec<ProductOption>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct ProductOption {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CouponForm {
    pub code: String,
    pub value_cents: i64,
    pub min_amount_cents: Option<i64>,
    pub usage_limit: Option<i64>,
    pub product_id: Option<i64>,
    #[serde(default)]
    pub product_ids: Vec<i64>,
    pub is_active: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmailTemplateRow {
    pub id: i64,
    pub token: String,
    pub subject: String,
    pub content: String,
    pub is_system: i64,
}

#[derive(Debug, Serialize)]
pub struct EmailTemplatesData {
    pub templates: Vec<EmailTemplateRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct EmailTemplateForm {
    pub token: String,
    pub subject: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct AdminRow {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub is_active: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminsData {
    pub admins: Vec<AdminRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct AdminForm {
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub role: Option<String>,
    pub is_active: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MediaRow {
    pub id: i64,
    pub path: String,
    pub mime: String,
    pub size_bytes: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct UploadsData {
    pub media: Vec<MediaRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct EmailTestForm {
    pub to: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct EmailTestData {
    pub smtp_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct JobRow {
    pub id: i64,
    pub kind: String,
    pub status: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub run_at: String,
    pub last_error: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct JobsData {
    pub jobs: Vec<JobRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct NotificationLogsData {
    pub logs: Vec<NotificationLogRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct TrashRow {
    pub table_name: String,
    pub id: i64,
    pub title: String,
    pub deleted_at: String,
}

#[derive(Debug, Serialize)]
pub struct TrashData {
    pub rows: Vec<TrashRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct AuditLogRow {
    pub id: i64,
    pub admin_id: Option<i64>,
    pub method: String,
    pub path: String,
    pub action: String,
    pub ip: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AuditLogsData {
    pub logs: Vec<AuditLogRow>,
    pub pagination: Pagination,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct PageParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pagination {
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
    pub total_pages: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_prev: bool,
    pub has_next: bool,
    pub prev_page: i64,
    pub next_page: i64,
    pub from: i64,
    pub to: i64,
    pub prev_query: String,
    pub next_query: String,
}

pub async fn dashboard(state: &AppState) -> AppResult<DashboardData> {
    let completed_order_count = scalar_i64(
        state,
        "SELECT COUNT(*) FROM orders WHERE status = 'completed'",
    )
    .await?;
    let order_count = scalar_i64(state, "SELECT COUNT(*) FROM orders").await?;
    let total_sales: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_amount_cents), 0) FROM orders WHERE status = 'completed'",
    )
    .fetch_one(&state.pool)
    .await?;
    let today_prefix = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let today_order_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE created_at LIKE ?")
            .bind(format!("{}%", today_prefix))
            .fetch_one(&state.pool)
            .await?;
    let today_sales: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_amount_cents), 0) FROM orders WHERE status = 'completed' AND created_at LIKE ?",
    )
    .bind(format!("{}%", today_prefix))
    .fetch_one(&state.pool)
    .await?;
    let success_rate = if order_count == 0 {
        "0.00%".to_string()
    } else {
        format!(
            "{:.2}%",
            completed_order_count as f64 * 100.0 / order_count as f64
        )
    };
    let readiness_checks = production_readiness_checks(state).await?;
    let readiness_blockers = readiness_checks
        .iter()
        .filter(|check| check.level == "blocker")
        .count() as i64;
    let readiness_warnings = readiness_checks
        .iter()
        .filter(|check| check.level == "warning")
        .count() as i64;

    Ok(DashboardData {
        order_count,
        product_count: scalar_i64(
            state,
            "SELECT COUNT(*) FROM products WHERE deleted_at IS NULL",
        )
        .await?,
        available_cards: scalar_i64(
            state,
            "SELECT COUNT(*) FROM card_secrets WHERE status = 'available' AND deleted_at IS NULL",
        )
        .await?,
        completed_order_count,
        pending_order_count: scalar_i64(
            state,
            "SELECT COUNT(*) FROM orders WHERE status = 'pending_payment'",
        )
        .await?,
        canceled_order_count: scalar_i64(
            state,
            "SELECT COUNT(*) FROM orders WHERE status = 'canceled'",
        )
        .await?,
        total_sales_display: money::format_cents(total_sales),
        today_order_count,
        today_sales_display: money::format_cents(today_sales),
        success_rate,
        low_stock_count: scalar_i64(
            state,
            "SELECT COUNT(*) FROM products p WHERE p.deleted_at IS NULL AND (
                SELECT COUNT(*) FROM card_secrets cs
                WHERE cs.product_id = p.id AND cs.status = 'available' AND cs.deleted_at IS NULL
            ) <= 2",
        )
        .await?,
        readiness_blockers,
        readiness_warnings,
        readiness_checks,
    })
}

async fn production_readiness_checks(state: &AppState) -> AppResult<Vec<ReadinessCheck>> {
    let mut checks = Vec::new();

    let secret_path = state.secret_box.key_path();
    if secret_path.exists() {
        push_check(
            &mut checks,
            "info",
            "应用密钥自动管理已启用",
            &format!(
                "敏感字段使用本地自动密钥加密落库；密钥文件为 {}，会随系统定期换新并重加密敏感配置。",
                secret_path.display()
            ),
        );
    } else {
        push_check(
            &mut checks,
            "blocker",
            "应用密钥文件缺失",
            "未找到本地自动密钥文件，敏感字段加密不可用；请检查数据目录写入权限并重启应用。",
        );
    }

    let site = settings_service::runtime_site_config(state).await;
    if !production_base_url_ok(&site.base_url) {
        push_check(
            &mut checks,
            "blocker",
            "站点外部 URL 不适合生产",
            "site.base_url 必须改为真实 HTTPS 域名，不能使用 0.0.0.0、127.0.0.1 或 localhost。",
        );
    } else {
        push_check(
            &mut checks,
            "info",
            "站点外部 URL 已配置",
            "支付回调、同步返回和邮件链接会使用该域名。",
        );
    }

    let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admins")
        .fetch_one(&state.pool)
        .await?;
    if admin_count == 0 {
        push_check(
            &mut checks,
            "blocker",
            "尚未创建管理员",
            "请通过 /install 创建首个管理员后再上线。",
        );
    } else {
        push_check(&mut checks, "info", "管理员已创建", "后台已具备登录入口。");
    }

    if let Some(hash) =
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM admins WHERE username = 'admin'")
            .fetch_optional(&state.pool)
            .await?
    {
        if password::verify_password("admin123456", &hash) {
            push_check(
                &mut checks,
                "blocker",
                "存在默认管理员密码",
                "admin/admin123456 不能用于生产，请立即修改或删除该管理员。",
            );
        }
    }

    let security = settings_service::security_config(state).await;
    if site.base_url.starts_with("https://") && !security.cookie_secure {
        push_check(
            &mut checks,
            "warning",
            "HTTPS 站点关闭了 Cookie Secure",
            "生产 HTTPS 环境建议开启 Cookie Secure，降低会话 Cookie 泄露风险。",
        );
    }
    if site.base_url.starts_with("https://") && security.trust_proxy_hops == 0 {
        push_check(
            &mut checks,
            "warning",
            "反代层数可能未配置",
            "如果前面有 Cloudflare、Nginx 或隧道，请按实际层数设置 trust_proxy_hops，确保限流和审计 IP 正确。",
        );
    }

    let real_channels: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payment_channels
         WHERE is_active = 1 AND deleted_at IS NULL AND provider_type != 'noop'",
    )
    .fetch_one(&state.pool)
    .await?;
    if real_channels == 0 {
        push_check(
            &mut checks,
            "blocker",
            "没有启用真实支付通道",
            "生产售卖至少需要启用并验证一个真实支付通道；noop 只能用于测试。",
        );
    } else {
        push_check(
            &mut checks,
            "warning",
            "真实支付通道需完成外部验收",
            "启用前请按 docs/payment-provider-acceptance.md 完成沙箱或小额实付验证。",
        );
    }

    let smtp_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'smtp_config'")
            .fetch_optional(&state.pool)
            .await?;
    let notification_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'notification_config'")
            .fetch_optional(&state.pool)
            .await?;
    let smtp = parse_smtp_settings(smtp_json.as_deref());
    let notification = parse_notification_settings(notification_json.as_deref());
    if !smtp.enabled && !notification.any_enabled() {
        push_check(
            &mut checks,
            "warning",
            "邮件和通知均未启用",
            "生产环境建议至少启用 SMTP 或一种管理员通知渠道，便于发现支付、发货和任务异常。",
        );
    }

    Ok(checks)
}

fn push_check(checks: &mut Vec<ReadinessCheck>, level: &str, title: &str, detail: &str) {
    checks.push(ReadinessCheck {
        level: level.to_string(),
        title: title.to_string(),
        detail: detail.to_string(),
    });
}

fn production_base_url_ok(base_url: &str) -> bool {
    let value = base_url.trim().to_ascii_lowercase();
    value.starts_with("https://")
        && !value.contains("0.0.0.0")
        && !value.contains("127.0.0.1")
        && !value.contains("localhost")
}

pub async fn is_installed(state: &AppState) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admins")
        .fetch_one(&state.pool)
        .await?;
    Ok(count > 0)
}

impl Pagination {
    fn new(page: Option<i64>, per_page: Option<i64>, total: i64) -> Self {
        let per_page = per_page.unwrap_or(20).clamp(10, 100);
        let total_pages = ((total + per_page - 1) / per_page).max(1);
        let page = page.unwrap_or(1).clamp(1, total_pages);
        let offset = (page - 1) * per_page;
        let to = (offset + per_page).min(total);
        Self {
            page,
            per_page,
            total,
            total_pages,
            offset,
            limit: per_page,
            has_prev: page > 1,
            has_next: page < total_pages,
            prev_page: (page - 1).max(1),
            next_page: (page + 1).min(total_pages),
            from: if total == 0 { 0 } else { offset + 1 },
            to,
            prev_query: page_query((page - 1).max(1), per_page),
            next_query: page_query((page + 1).min(total_pages), per_page),
        }
    }

    fn from_params(params: &PageParams, total: i64) -> Self {
        Self::new(params.page, params.per_page, total)
    }

    fn with_queries(mut self, prev_query: String, next_query: String) -> Self {
        self.prev_query = prev_query;
        self.next_query = next_query;
        self
    }
}

fn page_query(page: i64, per_page: i64) -> String {
    format!("page={page}&per_page={per_page}")
}

fn append_query_value(parts: &mut Vec<String>, key: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    parts.push(format!("{}={}", key, form_urlencode(value.trim())));
}

fn append_query_i64(parts: &mut Vec<String>, key: &str, value: Option<i64>) {
    if let Some(value) = value {
        if value != 0 {
            parts.push(format!("{key}={value}"));
        }
    }
}

fn form_urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn orders_query(filter: &AdminOrdersFilter, page: i64, per_page: i64) -> String {
    let mut parts = vec![page_query(page, per_page)];
    append_query_value(
        &mut parts,
        "order_no",
        filter.order_no.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "email",
        filter.email.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "provider_ref",
        filter.provider_ref.as_deref().unwrap_or_default(),
    );
    append_query_value(&mut parts, "ip", filter.ip.as_deref().unwrap_or_default());
    append_query_value(
        &mut parts,
        "status",
        filter.status.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "fulfillment_type",
        filter.fulfillment_type.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "payment_status",
        filter.payment_status.as_deref().unwrap_or_default(),
    );
    append_query_i64(&mut parts, "product_id", filter.product_id);
    append_query_i64(&mut parts, "payment_channel_id", filter.payment_channel_id);
    append_query_i64(&mut parts, "coupon_id", filter.coupon_id);
    append_query_i64(&mut parts, "amount_min_cents", filter.amount_min_cents);
    append_query_i64(&mut parts, "amount_max_cents", filter.amount_max_cents);
    append_query_value(
        &mut parts,
        "date_from",
        filter.date_from.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "date_to",
        filter.date_to.as_deref().unwrap_or_default(),
    );
    parts.join("&")
}

fn cards_query(filter: &CardsFilter, page: i64, per_page: i64) -> String {
    let mut parts = vec![page_query(page, per_page)];
    append_query_value(
        &mut parts,
        "status",
        filter.status.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "is_loop",
        filter.is_loop.as_deref().unwrap_or_default(),
    );
    parts.join("&")
}

fn global_cards_query(filter: &GlobalCardsFilter, page: i64, per_page: i64) -> String {
    let mut parts = vec![page_query(page, per_page)];
    append_query_i64(&mut parts, "product_id", filter.product_id);
    append_query_value(
        &mut parts,
        "status",
        filter.status.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "is_loop",
        filter.is_loop.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "keyword",
        filter.keyword.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "date_from",
        filter.date_from.as_deref().unwrap_or_default(),
    );
    append_query_value(
        &mut parts,
        "date_to",
        filter.date_to.as_deref().unwrap_or_default(),
    );
    parts.join("&")
}

pub async fn install(state: &AppState, form: InstallForm) -> AppResult<()> {
    install_first_admin(state, form).await.map(|_| ())
}

/// Creates the first owner admin in a single transaction, also persisting the
/// site name/logo so the install screen can capture both at once.
/// Returns `(admin_id, role)` so callers (JSON setup endpoint) can immediately
/// sign JWTs without a second login round-trip.
pub async fn install_first_admin(state: &AppState, form: InstallForm) -> AppResult<(i64, String)> {
    if is_installed(state).await? {
        return Err(AppError::Conflict("系统已初始化".to_string()));
    }
    if form.site_name.trim().is_empty()
        || form.username.trim().is_empty()
        || form.display_name.trim().is_empty()
    {
        return Err(AppError::BadRequest("初始化字段不能为空".to_string()));
    }
    if form.password.len() < 8 || form.password != form.password_confirm {
        return Err(AppError::BadRequest(
            "密码至少 8 位且两次输入必须一致".to_string(),
        ));
    }
    let now = crate::time::now_str();
    let mut tx = state.pool.begin().await?;
    // Re-check inside the transaction to defeat concurrent setup races.
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admins")
        .fetch_one(&mut *tx)
        .await?;
    if existing > 0 {
        return Err(AppError::Conflict("系统已初始化".to_string()));
    }
    let hash = password::hash_password(&form.password)?;
    let admin_id = sqlx::query(
        "INSERT INTO admins(username, password_hash, display_name, role, is_active, created_at, updated_at)
         VALUES (?, ?, ?, 'owner', 1, ?, ?)",
    )
    .bind(form.username.trim())
    .bind(hash)
    .bind(form.display_name.trim())
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    let site_value = serde_json::json!({
        "name": form.site_name.trim(),
        "logo_text": form.logo_text.trim(),
        "notice": state.config.site.notice,
        "footer": state.config.site.footer,
        "base_url": state.config.site.base_url.clone(),
    });
    sqlx::query(
        "INSERT INTO settings(key, value_json, created_at, updated_at)
         VALUES ('site_config', ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(site_value.to_string())
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((admin_id, "owner".to_string()))
}

async fn scalar_i64(state: &AppState, sql: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(sql).fetch_one(&state.pool).await?)
}

fn like_value(value: Option<&str>) -> String {
    let value = value.unwrap_or_default().trim();
    if value.is_empty() {
        String::new()
    } else {
        format!("%{}%", value)
    }
}

pub async fn orders(state: &AppState, filter: AdminOrdersFilter) -> AppResult<AdminOrdersData> {
    let order_no = like_value(filter.order_no.as_deref());
    let email = like_value(filter.email.as_deref());
    let status = filter.status.clone().unwrap_or_default();
    let product_id = filter.product_id.unwrap_or(0);
    let payment_channel_id = filter.payment_channel_id.unwrap_or(0);
    let provider_ref = like_value(filter.provider_ref.as_deref());
    let coupon_id = filter.coupon_id.unwrap_or(0);
    let fulfillment_type = filter.fulfillment_type.clone().unwrap_or_default();
    let payment_status = filter.payment_status.clone().unwrap_or_default();
    let ip = like_value(filter.ip.as_deref());
    let amount_min_cents = filter.amount_min_cents.unwrap_or(0);
    let amount_max_cents = filter.amount_max_cents.unwrap_or(0);
    let date_from = filter.date_from.clone().unwrap_or_default();
    let date_to = filter.date_to.clone().unwrap_or_default();
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT o.id)
         FROM orders o
         LEFT JOIN order_items oi ON oi.order_id = o.id
         LEFT JOIN payments pay ON pay.order_id = o.id
         WHERE o.deleted_at IS NULL
           AND (? = '' OR o.order_no LIKE ?)
           AND (? = '' OR o.guest_email LIKE ?)
           AND (? = '' OR o.status = ?)
           AND (? = 0 OR oi.product_id = ?)
           AND (? = 0 OR o.payment_channel_id = ?)
           AND (? = '' OR pay.provider_ref LIKE ? OR pay.gateway_order_no LIKE ? OR pay.payment_no LIKE ?)
           AND (? = 0 OR o.coupon_id = ?)
           AND (? = '' OR oi.fulfillment_type = ?)
           AND (? = '' OR pay.status = ?)
           AND (? = '' OR o.client_ip LIKE ?)
           AND (? = 0 OR o.total_amount_cents >= ?)
           AND (? = 0 OR o.total_amount_cents <= ?)
           AND (? = '' OR o.created_at >= ?)
           AND (? = '' OR o.created_at <= ?)",
    )
    .bind(filter.order_no.clone().unwrap_or_default())
    .bind(order_no.clone())
    .bind(filter.email.clone().unwrap_or_default())
    .bind(email.clone())
    .bind(status.clone())
    .bind(status.clone())
    .bind(product_id)
    .bind(product_id)
    .bind(payment_channel_id)
    .bind(payment_channel_id)
    .bind(filter.provider_ref.clone().unwrap_or_default())
    .bind(provider_ref.clone())
    .bind(provider_ref.clone())
    .bind(provider_ref.clone())
    .bind(coupon_id)
    .bind(coupon_id)
    .bind(fulfillment_type.clone())
    .bind(fulfillment_type.clone())
    .bind(payment_status.clone())
    .bind(payment_status.clone())
    .bind(filter.ip.clone().unwrap_or_default())
    .bind(ip.clone())
    .bind(amount_min_cents)
    .bind(amount_min_cents)
    .bind(amount_max_cents)
    .bind(amount_max_cents)
    .bind(date_from.clone())
    .bind(date_from.clone())
    .bind(date_to.clone())
    .bind(date_to.clone())
    .fetch_one(&state.pool)
    .await?;
    let pagination = Pagination::new(filter.page, filter.per_page, total).with_queries(
        orders_query(
            &filter,
            Pagination::new(filter.page, filter.per_page, total).prev_page,
            Pagination::new(filter.page, filter.per_page, total).per_page,
        ),
        orders_query(
            &filter,
            Pagination::new(filter.page, filter.per_page, total).next_page,
            Pagination::new(filter.page, filter.per_page, total).per_page,
        ),
    );
    let rows = sqlx::query(
        "SELECT o.id, o.order_no, o.status, o.total_amount_cents, o.guest_email, o.created_at,
                COALESCE(GROUP_CONCAT(DISTINCT oi.product_name), '') AS product_name,
                COALESCE(pc.name, '') AS payment_channel_name
         FROM orders o
         LEFT JOIN order_items oi ON oi.order_id = o.id
         LEFT JOIN payment_channels pc ON pc.id = o.payment_channel_id
         LEFT JOIN payments pay ON pay.order_id = o.id
         WHERE o.deleted_at IS NULL
           AND (? = '' OR o.order_no LIKE ?)
           AND (? = '' OR o.guest_email LIKE ?)
           AND (? = '' OR o.status = ?)
           AND (? = 0 OR oi.product_id = ?)
           AND (? = 0 OR o.payment_channel_id = ?)
           AND (? = '' OR pay.provider_ref LIKE ? OR pay.gateway_order_no LIKE ? OR pay.payment_no LIKE ?)
           AND (? = 0 OR o.coupon_id = ?)
           AND (? = '' OR oi.fulfillment_type = ?)
           AND (? = '' OR pay.status = ?)
           AND (? = '' OR o.client_ip LIKE ?)
           AND (? = 0 OR o.total_amount_cents >= ?)
           AND (? = 0 OR o.total_amount_cents <= ?)
           AND (? = '' OR o.created_at >= ?)
           AND (? = '' OR o.created_at <= ?)
         GROUP BY o.id
         ORDER BY o.id DESC LIMIT ? OFFSET ?",
    )
    .bind(filter.order_no.clone().unwrap_or_default())
    .bind(order_no)
    .bind(filter.email.clone().unwrap_or_default())
    .bind(email)
    .bind(status.clone())
    .bind(status)
    .bind(product_id)
    .bind(product_id)
    .bind(payment_channel_id)
    .bind(payment_channel_id)
    .bind(filter.provider_ref.clone().unwrap_or_default())
    .bind(provider_ref.clone())
    .bind(provider_ref.clone())
    .bind(provider_ref)
    .bind(coupon_id)
    .bind(coupon_id)
    .bind(fulfillment_type.clone())
    .bind(fulfillment_type)
    .bind(payment_status.clone())
    .bind(payment_status)
    .bind(filter.ip.clone().unwrap_or_default())
    .bind(ip)
    .bind(amount_min_cents)
    .bind(amount_min_cents)
    .bind(amount_max_cents)
    .bind(amount_max_cents)
    .bind(date_from.clone())
    .bind(date_from)
    .bind(date_to.clone())
    .bind(date_to)
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    Ok(AdminOrdersData {
        orders: rows
            .into_iter()
            .map(|row| AdminOrderRow {
                id: row.get("id"),
                order_no: row.get("order_no"),
                status: row.get("status"),
                amount_display: money::format_cents(row.get("total_amount_cents")),
                guest_email: row.get("guest_email"),
                product_name: row.get("product_name"),
                payment_channel_name: row.get("payment_channel_name"),
                created_at: row.get("created_at"),
            })
            .collect(),
        filter,
        products: product_options(state).await?,
        payment_channels: payment_channel_options(state).await?,
        pagination,
    })
}

pub async fn order(state: &AppState, id: i64) -> AppResult<AdminOrderData> {
    let order: Order = sqlx::query_as(
        "SELECT id, order_no, status, currency, guest_email, guest_password, client_ip,
         original_amount_cents, coupon_discount_cents, wholesale_discount_cents, total_amount_cents,
         coupon_id, payment_channel_id, legacy_info, expires_at, paid_at, canceled_at, created_at, updated_at
         FROM orders WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("订单不存在".to_string()))?;
    let fulfillment = sqlx::query_as::<_, Fulfillment>(
        "SELECT id, order_id, type, status, payload, delivered_at FROM fulfillments WHERE order_id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let item_rows = sqlx::query(
        "SELECT product_name, quantity, unit_price_cents, total_price_cents, fulfillment_type, manual_form_json
         FROM order_items WHERE order_id = ? ORDER BY id ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let payment_rows = sqlx::query(
        "SELECT payment_no, provider_type, channel_type, status, amount_cents,
                provider_ref, gateway_order_no, paid_at, callback_at
         FROM payments WHERE order_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let evm_intent_rows = sqlx::query(
        "SELECT i.id, p.payment_no, i.network_env, i.chain_id, i.chain_slug, i.token_symbol, i.token_contract,
                i.receive_address, i.amount_text, i.status, i.scan_from_block, i.last_scanned_block,
                i.matched_tx_hash, i.matched_log_index, i.matched_from_address, i.matched_at,
                i.last_checked_at, i.last_error, i.expires_at
         FROM evm_payment_intents i
         JOIN payments p ON p.id = i.payment_id
         WHERE p.order_id = ?
         ORDER BY i.id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let notification_rows = sqlx::query(
        "SELECT kind, target, status, error, created_at
         FROM notification_logs
         WHERE payload_json LIKE ?
         ORDER BY id DESC LIMIT 20",
    )
    .bind(format!("%\"order_id\":{}%", id))
    .fetch_all(&state.pool)
    .await?;
    Ok(AdminOrderData {
        amount_display: money::format_cents(order.total_amount_cents),
        order,
        fulfillment,
        items: item_rows
            .into_iter()
            .map(|row| AdminOrderItemRow {
                product_name: row.get("product_name"),
                quantity: row.get("quantity"),
                unit_price_display: money::format_cents(row.get("unit_price_cents")),
                total_price_display: money::format_cents(row.get("total_price_cents")),
                fulfillment_type: row.get("fulfillment_type"),
                manual_form_json: row.get("manual_form_json"),
            })
            .collect(),
        payments: payment_rows
            .into_iter()
            .map(|row| AdminPaymentRow {
                payment_no: row.get("payment_no"),
                provider_type: row.get("provider_type"),
                channel_type: row.get("channel_type"),
                status: row.get("status"),
                amount_display: money::format_cents(row.get("amount_cents")),
                provider_ref: row.get("provider_ref"),
                gateway_order_no: row.get("gateway_order_no"),
                paid_at: row.get::<Option<String>, _>("paid_at").unwrap_or_default(),
                callback_at: row
                    .get::<Option<String>, _>("callback_at")
                    .unwrap_or_default(),
            })
            .collect(),
        evm_intents: evm_intent_rows
            .into_iter()
            .map(|row| AdminEvmIntentRow {
                id: row.get("id"),
                payment_no: row.get("payment_no"),
                network_env: row.get("network_env"),
                chain_id: row.get("chain_id"),
                chain_slug: row.get("chain_slug"),
                token_symbol: row.get("token_symbol"),
                token_contract: row.get("token_contract"),
                receive_address: row.get("receive_address"),
                amount_text: row.get("amount_text"),
                status: row.get("status"),
                scan_from_block: row.get("scan_from_block"),
                last_scanned_block: row.get("last_scanned_block"),
                matched_tx_hash: row.get("matched_tx_hash"),
                matched_log_index: row.get("matched_log_index"),
                matched_from_address: row.get("matched_from_address"),
                matched_at: row
                    .get::<Option<String>, _>("matched_at")
                    .unwrap_or_default(),
                last_checked_at: row
                    .get::<Option<String>, _>("last_checked_at")
                    .unwrap_or_default(),
                last_error: row.get("last_error"),
                expires_at: row.get("expires_at"),
            })
            .collect(),
        notifications: notification_rows
            .into_iter()
            .map(|row| NotificationLogRow {
                kind: row.get("kind"),
                target: row.get("target"),
                status: row.get("status"),
                error: row.get("error"),
                created_at: row.get("created_at"),
            })
            .collect(),
    })
}

pub async fn confirm_evm_intent(
    state: &AppState,
    order_id: i64,
    intent_id: i64,
    tx_hash: String,
) -> AppResult<()> {
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT i.id
         FROM evm_payment_intents i
         JOIN payments p ON p.id = i.payment_id
         WHERE i.id = ? AND p.order_id = ?
         LIMIT 1",
    )
    .bind(intent_id)
    .bind(order_id)
    .fetch_optional(&state.pool)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(
            "EVM payment intent 不属于该订单".to_string(),
        ));
    }
    crate::services::evm_local_service::manual_confirm_intent(state, intent_id, &tx_hash).await
}

pub async fn fulfill(state: &AppState, id: i64, payload: String, admin_id: i64) -> AppResult<()> {
    fulfillment_service::manual_fulfill(state, id, payload, Some(admin_id)).await
}

pub async fn cancel_order(state: &AppState, id: i64) -> AppResult<()> {
    order_service::cancel_expired_order(state, id).await
}

pub async fn resend_order_email(state: &AppState, id: i64) -> AppResult<()> {
    fulfillment_service::resend_status_email(state, id).await
}

pub async fn mark_order_abnormal(state: &AppState, id: i64) -> AppResult<()> {
    fulfillment_service::mark_abnormal(state, id).await
}

pub async fn start_order_processing(state: &AppState, id: i64) -> AppResult<()> {
    fulfillment_service::start_processing(state, id).await
}

pub async fn soft_delete_order(state: &AppState, id: i64) -> AppResult<()> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM orders WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let Some(status) = status else {
        return Err(AppError::NotFound("订单不存在".to_string()));
    };
    if !matches!(
        status.as_str(),
        "canceled" | "abnormal" | "failed" | "pending_payment"
    ) {
        return Err(AppError::BadRequest(
            "仅可软删除取消/异常/失败/待支付订单，已支付订单需先标记异常".to_string(),
        ));
    }
    let now = crate::time::now_str();
    sqlx::query("UPDATE orders SET deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn restore_order(state: &AppState, id: i64) -> AppResult<()> {
    let now = crate::time::now_str();
    sqlx::query("UPDATE orders SET deleted_at = NULL, updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn export_orders(state: &AppState, filter: AdminOrdersFilter) -> AppResult<String> {
    let order_no = like_value(filter.order_no.as_deref());
    let email = like_value(filter.email.as_deref());
    let status = filter.status.clone().unwrap_or_default();
    let product_id = filter.product_id.unwrap_or(0);
    let payment_channel_id = filter.payment_channel_id.unwrap_or(0);
    let provider_ref = like_value(filter.provider_ref.as_deref());
    let coupon_id = filter.coupon_id.unwrap_or(0);
    let fulfillment_type = filter.fulfillment_type.clone().unwrap_or_default();
    let payment_status = filter.payment_status.clone().unwrap_or_default();
    let ip = like_value(filter.ip.as_deref());
    let amount_min_cents = filter.amount_min_cents.unwrap_or(0);
    let amount_max_cents = filter.amount_max_cents.unwrap_or(0);
    let date_from = filter.date_from.clone().unwrap_or_default();
    let date_to = filter.date_to.clone().unwrap_or_default();

    let rows = sqlx::query(
        "SELECT o.id, o.order_no, o.status, o.guest_email, o.client_ip, o.created_at, o.paid_at,
                o.original_amount_cents, o.coupon_discount_cents, o.wholesale_discount_cents,
                o.total_amount_cents,
                COALESCE(GROUP_CONCAT(DISTINCT oi.product_name), '') AS product_name,
                COALESCE(GROUP_CONCAT(DISTINCT oi.fulfillment_type), '') AS fulfillment_types,
                COALESCE(SUM(oi.quantity), 0) AS quantity,
                COALESCE(pc.name, '') AS payment_channel_name,
                COALESCE(MAX(pay.provider_ref), '') AS provider_ref,
                COALESCE(MAX(pay.gateway_order_no), '') AS gateway_order_no
         FROM orders o
         LEFT JOIN order_items oi ON oi.order_id = o.id
         LEFT JOIN payment_channels pc ON pc.id = o.payment_channel_id
         LEFT JOIN payments pay ON pay.order_id = o.id
         WHERE o.deleted_at IS NULL
           AND (? = '' OR o.order_no LIKE ?)
           AND (? = '' OR o.guest_email LIKE ?)
           AND (? = '' OR o.status = ?)
           AND (? = 0 OR oi.product_id = ?)
           AND (? = 0 OR o.payment_channel_id = ?)
           AND (? = '' OR pay.provider_ref LIKE ? OR pay.gateway_order_no LIKE ? OR pay.payment_no LIKE ?)
           AND (? = 0 OR o.coupon_id = ?)
           AND (? = '' OR oi.fulfillment_type = ?)
           AND (? = '' OR pay.status = ?)
           AND (? = '' OR o.client_ip LIKE ?)
           AND (? = 0 OR o.total_amount_cents >= ?)
           AND (? = 0 OR o.total_amount_cents <= ?)
           AND (? = '' OR o.created_at >= ?)
           AND (? = '' OR o.created_at <= ?)
         GROUP BY o.id
         ORDER BY o.id DESC LIMIT 10000",
    )
    .bind(filter.order_no.clone().unwrap_or_default())
    .bind(order_no)
    .bind(filter.email.clone().unwrap_or_default())
    .bind(email)
    .bind(status.clone())
    .bind(status)
    .bind(product_id)
    .bind(product_id)
    .bind(payment_channel_id)
    .bind(payment_channel_id)
    .bind(filter.provider_ref.clone().unwrap_or_default())
    .bind(provider_ref.clone())
    .bind(provider_ref.clone())
    .bind(provider_ref)
    .bind(coupon_id)
    .bind(coupon_id)
    .bind(fulfillment_type.clone())
    .bind(fulfillment_type)
    .bind(payment_status.clone())
    .bind(payment_status)
    .bind(filter.ip.clone().unwrap_or_default())
    .bind(ip)
    .bind(amount_min_cents)
    .bind(amount_min_cents)
    .bind(amount_max_cents)
    .bind(amount_max_cents)
    .bind(date_from.clone())
    .bind(date_from)
    .bind(date_to.clone())
    .bind(date_to)
    .fetch_all(&state.pool)
    .await?;

    let mut csv = String::from(
        "id,order_no,status,email,client_ip,product,quantity,payment,provider_ref,gateway_order_no,original_cents,coupon_cents,wholesale_cents,total_cents,total_display,created_at,paid_at,fulfillment_types\n",
    );
    for row in rows {
        let total_cents: i64 = row.get("total_amount_cents");
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            row.get::<i64, _>("id"),
            csv_escape(row.get::<String, _>("order_no")),
            csv_escape(row.get::<String, _>("status")),
            csv_escape(row.get::<String, _>("guest_email")),
            csv_escape(row.get::<String, _>("client_ip")),
            csv_escape(row.get::<String, _>("product_name")),
            row.get::<i64, _>("quantity"),
            csv_escape(row.get::<String, _>("payment_channel_name")),
            csv_escape(row.get::<String, _>("provider_ref")),
            csv_escape(row.get::<String, _>("gateway_order_no")),
            row.get::<i64, _>("original_amount_cents"),
            row.get::<i64, _>("coupon_discount_cents"),
            row.get::<i64, _>("wholesale_discount_cents"),
            total_cents,
            crate::money::format_cents(total_cents),
            csv_escape(row.get::<String, _>("created_at")),
            csv_escape(
                row.try_get::<Option<String>, _>("paid_at")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            ),
            csv_escape(row.get::<String, _>("fulfillment_types")),
        ));
    }
    Ok(csv)
}

fn csv_escape(value: String) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

pub async fn categories(state: &AppState, page: PageParams) -> AppResult<CategoriesData> {
    let total = scalar_i64(
        state,
        "SELECT COUNT(*) FROM categories WHERE deleted_at IS NULL",
    )
    .await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT id, name, is_active, sort_order FROM categories WHERE deleted_at IS NULL ORDER BY sort_order DESC, id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    let categories = rows
        .into_iter()
        .map(|row| CategoryRow {
            id: row.get("id"),
            name: row.get("name"),
            is_active: row.get("is_active"),
            sort_order: row.get("sort_order"),
        })
        .collect();
    Ok(CategoriesData {
        categories,
        pagination,
    })
}

pub async fn create_category(state: &AppState, form: CategoryForm) -> AppResult<()> {
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("分类名称不能为空".to_string()));
    }
    let now = crate::time::now_str();
    sqlx::query(
        "INSERT INTO categories(name, is_active, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(name)
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(form.sort_order.unwrap_or(0))
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn update_category(state: &AppState, id: i64, form: CategoryForm) -> AppResult<()> {
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("分类名称不能为空".to_string()));
    }
    let affected = sqlx::query(
        "UPDATE categories SET name = ?, is_active = ?, sort_order = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(name)
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(form.sort_order.unwrap_or(0))
    .bind(crate::time::now_str())
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("分类不存在".to_string()));
    }
    Ok(())
}

pub async fn delete_category(state: &AppState, id: i64) -> AppResult<()> {
    let product_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM products WHERE category_id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    if product_count > 0 {
        return Err(AppError::Conflict("分类下仍有商品，不能删除".to_string()));
    }
    soft_delete_by_id(state, "categories", id).await
}

pub async fn products(state: &AppState, page: PageParams) -> AppResult<ProductsData> {
    let categories = category_options(state).await?;
    let total = scalar_i64(
        state,
        "SELECT COUNT(*) FROM products WHERE deleted_at IS NULL",
    )
    .await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT p.id, p.category_id, c.name AS category_name, p.name, p.short_description,
                p.keywords, p.image_path, p.retail_price_cents, p.price_cents,
                p.description_html, p.wholesale_prices_json, p.manual_form_schema_json, p.buy_prompt,
                p.api_hook, p.payment_channel_ids_json, p.manual_stock_total, p.buy_limit_num, p.sort_order, p.fulfillment_type, p.is_active,
                CASE WHEN p.fulfillment_type = 'auto' THEN (
                    SELECT COUNT(*) FROM card_secrets cs WHERE cs.product_id = p.id AND cs.status = 'available' AND cs.deleted_at IS NULL
                ) ELSE MAX(p.manual_stock_total - p.manual_stock_locked, 0) END AS stock
         FROM products p
         JOIN categories c ON c.id = p.category_id
         WHERE p.deleted_at IS NULL
         ORDER BY p.sort_order DESC, p.id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    let products = rows
        .into_iter()
        .map(|row| ProductRow {
            id: row.get("id"),
            category_id: row.get("category_id"),
            category_name: row.get("category_name"),
            name: row.get("name"),
            short_description: row.get("short_description"),
            keywords: row.get("keywords"),
            image_path: row.get("image_path"),
            retail_price_cents: row.get("retail_price_cents"),
            retail_price_display: money::format_cents(row.get("retail_price_cents")),
            price_cents: row.get("price_cents"),
            price_display: money::format_cents(row.get("price_cents")),
            fulfillment_type: row.get("fulfillment_type"),
            description_html: row.get("description_html"),
            wholesale_prices_json: row.get("wholesale_prices_json"),
            manual_form_schema_json: row.get("manual_form_schema_json"),
            buy_prompt: row.get("buy_prompt"),
            api_hook: row.get("api_hook"),
            payment_channel_ids_json: row.get("payment_channel_ids_json"),
            manual_stock_total: row.get("manual_stock_total"),
            buy_limit_num: row.get("buy_limit_num"),
            sort_order: row.get("sort_order"),
            stock: row.get("stock"),
            is_active: row.get("is_active"),
        })
        .collect();
    Ok(ProductsData {
        products,
        categories,
        pagination,
    })
}

pub async fn create_product(state: &AppState, form: ProductForm) -> AppResult<()> {
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("商品名称不能为空".to_string()));
    }
    if form.price_cents < 0 {
        return Err(AppError::BadRequest("商品售价不能为负数".to_string()));
    }
    let fulfillment_type = form.fulfillment_type.trim().to_string();
    if !matches!(fulfillment_type.as_str(), "auto" | "manual") {
        return Err(AppError::BadRequest("发货类型不支持".to_string()));
    }
    let now = crate::time::now_str();
    let wholesale_prices_json = normalize_json_array(form.wholesale_prices_json.as_deref())?;
    let manual_form_schema_json = normalize_json_array(form.manual_form_schema_json.as_deref())?;
    let payment_channel_ids_json = normalize_json_array(form.payment_channel_ids_json.as_deref())?;
    let slug = format!("product-{}", uuid::Uuid::new_v4().simple());
    let id = sqlx::query(
        "INSERT INTO products(category_id, slug, name, short_description, keywords, image_path,
         description_html, retail_price_cents, price_cents,
         wholesale_prices_json, fulfillment_type, manual_form_schema_json, buy_prompt,
         api_hook, payment_channel_ids_json, manual_stock_total, buy_limit_num, is_active, sort_order, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(form.category_id)
    .bind(slug)
    .bind(name)
    .bind(form.short_description.unwrap_or_default())
    .bind(form.keywords.unwrap_or_default())
    .bind(form.image_path.unwrap_or_default())
    .bind(form.description_html.unwrap_or_default())
    .bind(form.retail_price_cents.unwrap_or(0).max(0))
    .bind(form.price_cents)
    .bind(wholesale_prices_json)
    .bind(fulfillment_type)
    .bind(manual_form_schema_json)
    .bind(form.buy_prompt.unwrap_or_default())
    .bind(form.api_hook.unwrap_or_default())
    .bind(&payment_channel_ids_json)
    .bind(form.manual_stock_total.unwrap_or(0).max(0))
    .bind(form.buy_limit_num.unwrap_or(0).max(0))
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(form.sort_order.unwrap_or(0))
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?
    .last_insert_rowid();
    save_product_payment_channels(state, id, &payment_channel_ids_json).await?;
    sqlx::query(
        "INSERT INTO product_skus(product_id, sku_code, price_cents, is_active, sort_order, created_at, updated_at)
         VALUES (?, 'DEFAULT', ?, 1, 0, ?, ?)",
    )
    .bind(id)
    .bind(form.price_cents)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn update_product(state: &AppState, id: i64, form: ProductForm) -> AppResult<()> {
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("商品名称不能为空".to_string()));
    }
    if form.price_cents < 0 {
        return Err(AppError::BadRequest("商品售价不能为负数".to_string()));
    }
    let fulfillment_type = form.fulfillment_type.trim().to_string();
    if !matches!(fulfillment_type.as_str(), "auto" | "manual") {
        return Err(AppError::BadRequest("发货类型不支持".to_string()));
    }
    let now = crate::time::now_str();
    let wholesale_prices_json = normalize_json_array(form.wholesale_prices_json.as_deref())?;
    let manual_form_schema_json = normalize_json_array(form.manual_form_schema_json.as_deref())?;
    let payment_channel_ids_json = normalize_json_array(form.payment_channel_ids_json.as_deref())?;
    let affected = sqlx::query(
        "UPDATE products
         SET category_id = ?, name = ?, short_description = ?, keywords = ?, image_path = ?,
             description_html = ?, retail_price_cents = ?, price_cents = ?, wholesale_prices_json = ?,
             fulfillment_type = ?, manual_form_schema_json = ?, buy_prompt = ?, api_hook = ?, payment_channel_ids_json = ?,
             manual_stock_total = ?, buy_limit_num = ?, sort_order = ?, is_active = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(form.category_id)
    .bind(name)
    .bind(form.short_description.unwrap_or_default())
    .bind(form.keywords.unwrap_or_default())
    .bind(form.image_path.unwrap_or_default())
    .bind(form.description_html.unwrap_or_default())
    .bind(form.retail_price_cents.unwrap_or(0).max(0))
    .bind(form.price_cents)
    .bind(wholesale_prices_json)
    .bind(fulfillment_type)
    .bind(manual_form_schema_json)
    .bind(form.buy_prompt.unwrap_or_default())
    .bind(form.api_hook.unwrap_or_default())
    .bind(&payment_channel_ids_json)
    .bind(form.manual_stock_total.unwrap_or(0).max(0))
    .bind(form.buy_limit_num.unwrap_or(0).max(0))
    .bind(form.sort_order.unwrap_or(0))
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(&now)
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("商品不存在".to_string()));
    }
    save_product_payment_channels(state, id, &payment_channel_ids_json).await?;
    sqlx::query("UPDATE product_skus SET price_cents = ?, updated_at = ? WHERE product_id = ? AND sku_code = 'DEFAULT'")
        .bind(form.price_cents)
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn delete_product(state: &AppState, id: i64) -> AppResult<()> {
    let active_order_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM order_items oi
         JOIN orders o ON o.id = oi.order_id
         WHERE oi.product_id = ? AND o.status IN ('pending_payment', 'paid')",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    if active_order_count > 0 {
        return Err(AppError::Conflict(
            "商品仍有关联未完成订单，不能删除".to_string(),
        ));
    }
    let now = crate::time::now_str();
    sqlx::query("UPDATE products SET deleted_at = ?, updated_at = ?, is_active = 0 WHERE id = ? AND deleted_at IS NULL")
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn cards(state: &AppState, product_id: i64, filter: CardsFilter) -> AppResult<CardsData> {
    let product_name: String = sqlx::query_scalar("SELECT name FROM products WHERE id = ?")
        .bind(product_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("商品不存在".to_string()))?;
    let status = filter.status.clone().unwrap_or_default();
    let is_loop = filter.is_loop.clone().unwrap_or_default();
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM card_secrets
         WHERE product_id = ? AND deleted_at IS NULL
           AND (? = '' OR status = ?)
           AND (? = '' OR is_loop = ?)",
    )
    .bind(product_id)
    .bind(status.clone())
    .bind(status.clone())
    .bind(is_loop.clone())
    .bind(is_loop.clone())
    .fetch_one(&state.pool)
    .await?;
    let base_pagination = Pagination::new(filter.page, filter.per_page, total);
    let pagination = base_pagination.clone().with_queries(
        cards_query(&filter, base_pagination.prev_page, base_pagination.per_page),
        cards_query(&filter, base_pagination.next_page, base_pagination.per_page),
    );
    let rows = sqlx::query(
        "SELECT id, secret, status, is_loop, order_id
         FROM card_secrets
         WHERE product_id = ? AND deleted_at IS NULL
           AND (? = '' OR status = ?)
           AND (? = '' OR is_loop = ?)
         ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(product_id)
    .bind(status.clone())
    .bind(status)
    .bind(is_loop.clone())
    .bind(is_loop)
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    let cards = rows
        .into_iter()
        .map(|row| CardSecretRow {
            id: row.get("id"),
            secret: row.get("secret"),
            status: row.get("status"),
            is_loop: row.get("is_loop"),
            order_id: row.get("order_id"),
        })
        .collect();
    Ok(CardsData {
        product_id,
        product_name,
        cards,
        filter,
        pagination,
    })
}

pub async fn export_cards(
    state: &AppState,
    product_id: i64,
    filter: CardsFilter,
) -> AppResult<String> {
    let status = filter.status.unwrap_or_default();
    let is_loop = filter.is_loop.unwrap_or_default();
    let rows = sqlx::query(
        "SELECT secret
         FROM card_secrets
         WHERE product_id = ? AND deleted_at IS NULL
           AND (? = '' OR status = ?)
           AND (? = '' OR is_loop = ?)
         ORDER BY id ASC",
    )
    .bind(product_id)
    .bind(status.clone())
    .bind(status)
    .bind(is_loop.clone())
    .bind(is_loop)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("secret"))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub async fn import_cards(
    state: &AppState,
    product_id: i64,
    form: ImportCardsForm,
) -> AppResult<()> {
    let secrets: Vec<&str> = form
        .secrets
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if secrets.is_empty() {
        return Err(AppError::BadRequest("卡密内容不能为空".to_string()));
    }
    let now = crate::time::now_str();
    let is_loop = if form.is_loop.is_some() { 1 } else { 0 };
    let remove_duplication = form.remove_duplication.is_some();
    let mut seen = HashSet::new();
    let mut tx = state.pool.begin().await?;
    for line in secrets {
        if remove_duplication && !seen.insert(line.to_string()) {
            continue;
        }
        if remove_duplication {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM card_secrets WHERE product_id = ? AND secret = ? AND deleted_at IS NULL",
            )
            .bind(product_id)
            .bind(line)
            .fetch_one(&mut *tx)
            .await?;
            if exists > 0 {
                continue;
            }
        }
        sqlx::query(
            "INSERT INTO card_secrets(product_id, sku_id, secret, status, is_loop, created_at, updated_at)
             VALUES (?, 0, ?, 'available', ?, ?, ?)",
        )
        .bind(product_id)
        .bind(line)
        .bind(is_loop)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_card(state: &AppState, product_id: i64, card_id: i64) -> AppResult<()> {
    let affected = sqlx::query(
        "UPDATE card_secrets SET deleted_at = ?, updated_at = ?
         WHERE id = ? AND product_id = ? AND status = 'available' AND deleted_at IS NULL",
    )
    .bind(crate::time::now_str())
    .bind(crate::time::now_str())
    .bind(card_id)
    .bind(product_id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::Conflict("只能删除未售出的可用卡密".to_string()));
    }
    Ok(())
}

pub async fn global_cards(
    state: &AppState,
    filter: GlobalCardsFilter,
) -> AppResult<GlobalCardsData> {
    let product_id = filter.product_id.unwrap_or(0);
    let status = filter.status.clone().unwrap_or_default();
    let is_loop = filter.is_loop.clone().unwrap_or_default();
    let keyword = like_value(filter.keyword.as_deref());
    let date_from = filter.date_from.clone().unwrap_or_default();
    let date_to = filter.date_to.clone().unwrap_or_default();
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM card_secrets cs
         JOIN products p ON p.id = cs.product_id
         WHERE cs.deleted_at IS NULL
           AND (? = 0 OR cs.product_id = ?)
           AND (? = '' OR cs.status = ?)
           AND (? = '' OR cs.is_loop = ?)
           AND (? = '' OR cs.secret LIKE ?)
           AND (? = '' OR cs.created_at >= ?)
           AND (? = '' OR cs.created_at <= ?)",
    )
    .bind(product_id)
    .bind(product_id)
    .bind(status.clone())
    .bind(status.clone())
    .bind(is_loop.clone())
    .bind(is_loop.clone())
    .bind(filter.keyword.clone().unwrap_or_default())
    .bind(keyword.clone())
    .bind(date_from.clone())
    .bind(date_from.clone())
    .bind(date_to.clone())
    .bind(date_to.clone())
    .fetch_one(&state.pool)
    .await?;
    let base_pagination = Pagination::new(filter.page, filter.per_page, total);
    let pagination = base_pagination.clone().with_queries(
        global_cards_query(&filter, base_pagination.prev_page, base_pagination.per_page),
        global_cards_query(&filter, base_pagination.next_page, base_pagination.per_page),
    );
    let rows = sqlx::query(
        "SELECT cs.id, cs.product_id, p.name AS product_name, cs.secret, cs.status, cs.is_loop, cs.order_id, cs.created_at
         FROM card_secrets cs
         JOIN products p ON p.id = cs.product_id
         WHERE cs.deleted_at IS NULL
           AND (? = 0 OR cs.product_id = ?)
           AND (? = '' OR cs.status = ?)
           AND (? = '' OR cs.is_loop = ?)
           AND (? = '' OR cs.secret LIKE ?)
           AND (? = '' OR cs.created_at >= ?)
           AND (? = '' OR cs.created_at <= ?)
         ORDER BY cs.id DESC LIMIT ? OFFSET ?",
    )
    .bind(product_id)
    .bind(product_id)
    .bind(status.clone())
    .bind(status)
    .bind(is_loop.clone())
    .bind(is_loop)
    .bind(filter.keyword.clone().unwrap_or_default())
    .bind(keyword)
    .bind(date_from.clone())
    .bind(date_from)
    .bind(date_to.clone())
    .bind(date_to)
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    Ok(GlobalCardsData {
        cards: rows
            .into_iter()
            .map(|row| GlobalCardSecretRow {
                id: row.get("id"),
                product_id: row.get("product_id"),
                product_name: row.get("product_name"),
                secret: row.get("secret"),
                status: row.get("status"),
                is_loop: row.get("is_loop"),
                order_id: row.get("order_id"),
                created_at: row.get("created_at"),
            })
            .collect(),
        filter,
        products: product_options(state).await?,
        pagination,
    })
}

pub async fn export_global_cards(state: &AppState, filter: GlobalCardsFilter) -> AppResult<String> {
    let product_id = filter.product_id.unwrap_or(0);
    let status = filter.status.clone().unwrap_or_default();
    let is_loop = filter.is_loop.clone().unwrap_or_default();
    let keyword = like_value(filter.keyword.as_deref());
    let date_from = filter.date_from.clone().unwrap_or_default();
    let date_to = filter.date_to.clone().unwrap_or_default();
    let rows = sqlx::query(
        "SELECT cs.secret
         FROM card_secrets cs
         JOIN products p ON p.id = cs.product_id
         WHERE cs.deleted_at IS NULL
           AND (? = 0 OR cs.product_id = ?)
           AND (? = '' OR cs.status = ?)
           AND (? = '' OR cs.is_loop = ?)
           AND (? = '' OR cs.secret LIKE ?)
           AND (? = '' OR cs.created_at >= ?)
           AND (? = '' OR cs.created_at <= ?)
         ORDER BY cs.id ASC",
    )
    .bind(product_id)
    .bind(product_id)
    .bind(status.clone())
    .bind(status)
    .bind(is_loop.clone())
    .bind(is_loop)
    .bind(filter.keyword.clone().unwrap_or_default())
    .bind(keyword)
    .bind(date_from.clone())
    .bind(date_from)
    .bind(date_to.clone())
    .bind(date_to)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("secret"))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub async fn delete_global_card(state: &AppState, card_id: i64) -> AppResult<()> {
    let affected = sqlx::query(
        "UPDATE card_secrets SET deleted_at = ?, updated_at = ?
         WHERE id = ? AND status = 'available' AND deleted_at IS NULL",
    )
    .bind(crate::time::now_str())
    .bind(crate::time::now_str())
    .bind(card_id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::Conflict("只能删除未售出的可用卡密".to_string()));
    }
    Ok(())
}

pub async fn payment_channels(
    state: &AppState,
    page: PageParams,
) -> AppResult<PaymentChannelsData> {
    let total = scalar_i64(
        state,
        "SELECT COUNT(*) FROM payment_channels WHERE deleted_at IS NULL",
    )
    .await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT id, name, provider_type, channel_type, interaction_mode, pay_check, client_scope, handleroute, config_json, is_active
         FROM payment_channels WHERE deleted_at IS NULL ORDER BY sort_order DESC, id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    let channels = rows
        .into_iter()
        .map(|row| PaymentChannelRow {
            id: row.get("id"),
            name: row.get("name"),
            provider_type: row.get("provider_type"),
            channel_type: row.get("channel_type"),
            interaction_mode: row.get("interaction_mode"),
            pay_check: row.get("pay_check"),
            client_scope: row.get("client_scope"),
            handleroute: row.get("handleroute"),
            config_json: row.get("config_json"),
            is_active: row.get("is_active"),
        })
        .collect();
    Ok(PaymentChannelsData {
        channels,
        pagination,
    })
}

pub async fn create_payment_channel(state: &AppState, form: PaymentChannelForm) -> AppResult<()> {
    let name = form.name.trim();
    let provider_type = form.provider_type.trim();
    let channel_type = form.channel_type.trim();
    let interaction_mode = form.interaction_mode.trim();
    let client_scope = normalize_client_scope(form.client_scope.as_deref())?;
    if name.is_empty() || provider_type.is_empty() || channel_type.is_empty() {
        return Err(AppError::BadRequest("支付通道字段不能为空".to_string()));
    }
    if !matches!(interaction_mode, "redirect" | "qrcode") {
        return Err(AppError::BadRequest("支付交互方式不支持".to_string()));
    }
    let config_json = merge_payment_config(&form)?;
    validate_evm_local_config_if_needed(provider_type, &config_json)?;
    let now = crate::time::now_str();
    sqlx::query(
        "INSERT INTO payment_channels(name, provider_type, channel_type, interaction_mode, pay_check, client_scope, handleroute, config_json, is_active, sort_order, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(name)
    .bind(provider_type)
    .bind(channel_type)
    .bind(interaction_mode)
    .bind(form.pay_check.as_deref().unwrap_or(channel_type).trim())
    .bind(client_scope)
    .bind(form.handleroute.as_deref().unwrap_or_default().trim())
    .bind(config_json)
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn update_payment_channel(
    state: &AppState,
    id: i64,
    form: PaymentChannelForm,
) -> AppResult<()> {
    let name = form.name.trim();
    let provider_type = form.provider_type.trim();
    let channel_type = form.channel_type.trim();
    let interaction_mode = form.interaction_mode.trim();
    let client_scope = normalize_client_scope(form.client_scope.as_deref())?;
    if name.is_empty() || provider_type.is_empty() || channel_type.is_empty() {
        return Err(AppError::BadRequest("支付通道字段不能为空".to_string()));
    }
    if !matches!(interaction_mode, "redirect" | "qrcode") {
        return Err(AppError::BadRequest("支付交互方式不支持".to_string()));
    }
    let config_json = merge_payment_config(&form)?;
    validate_evm_local_config_if_needed(provider_type, &config_json)?;
    let affected = sqlx::query(
        "UPDATE payment_channels
         SET name = ?, provider_type = ?, channel_type = ?, interaction_mode = ?, pay_check = ?, client_scope = ?, handleroute = ?, config_json = ?, is_active = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(name)
    .bind(provider_type)
    .bind(channel_type)
    .bind(interaction_mode)
    .bind(form.pay_check.as_deref().unwrap_or(channel_type).trim())
    .bind(client_scope)
    .bind(form.handleroute.as_deref().unwrap_or_default().trim())
    .bind(config_json)
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(crate::time::now_str())
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("支付通道不存在".to_string()));
    }
    Ok(())
}

pub async fn delete_payment_channel(state: &AppState, id: i64) -> AppResult<()> {
    let active_orders: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE payment_channel_id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
    if active_orders > 0 {
        return Err(AppError::Conflict(
            "支付通道已被订单使用，不能删除".to_string(),
        ));
    }
    let now = crate::time::now_str();
    let affected = sqlx::query(
        "UPDATE payment_channels SET deleted_at = ?, is_active = 0, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("支付通道不存在".to_string()));
    }
    Ok(())
}

pub async fn settings(state: &AppState) -> AppResult<SettingsData> {
    let value_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'site_config'")
            .fetch_optional(&state.pool)
            .await?;
    let smtp_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'smtp_config'")
            .fetch_optional(&state.pool)
            .await?;
    let notification_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'notification_config'")
            .fetch_optional(&state.pool)
            .await?;
    let order_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'order_config'")
            .fetch_optional(&state.pool)
            .await?;
    let theme_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'theme_config'")
            .fetch_optional(&state.pool)
            .await?;
    let captcha_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'captcha_config'")
            .fetch_optional(&state.pool)
            .await?;
    let security_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'security_config'")
            .fetch_optional(&state.pool)
            .await?;
    let smtp = parse_smtp_settings(smtp_json.as_deref());
    let notification = parse_notification_settings(notification_json.as_deref());
    let order = parse_order_settings(
        order_json.as_deref(),
        state.config.site.order_expire_minutes,
    );
    let theme = parse_theme_settings(theme_json.as_deref(), &state.config.site.theme);
    let captcha = parse_captcha_settings(captcha_json.as_deref());
    let security = parse_security_settings(security_json.as_deref());
    if let Some(value_json) = value_json {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&value_json) {
            return Ok(SettingsData {
                name: value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&state.config.site.name)
                    .to_string(),
                logo_text: value
                    .get("logo_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&state.config.site.logo_text)
                    .to_string(),
                keywords: value
                    .get("keywords")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                description: value
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                manage_email: value
                    .get("manage_email")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                notice: value
                    .get("notice")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&state.config.site.notice)
                    .to_string(),
                footer: value
                    .get("footer")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&state.config.site.footer)
                    .to_string(),
                base_url: value
                    .get("base_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&state.config.site.base_url)
                    .to_string(),
                is_open_anti_red: value
                    .get("is_open_anti_red")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                is_open_google_translate: value
                    .get("is_open_google_translate")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                template: theme.template,
                order_expire_minutes: order.order_expire_minutes,
                is_open_search_pwd: order.is_open_search_pwd,
                purchase_rate_window_minutes: order.purchase_rate_window_minutes,
                purchase_rate_max_per_email: order.purchase_rate_max_per_email,
                purchase_rate_max_per_ip: order.purchase_rate_max_per_ip,
                is_open_img_code: captcha.is_open_img_code,
                login_max_attempts: security.login_max_attempts,
                login_lock_minutes: security.login_lock_minutes,
                cookie_secure: security.cookie_secure,
                trust_proxy_hops: security.trust_proxy_hops,
                smtp_enabled: smtp.enabled,
                smtp_host: smtp.host,
                smtp_port: smtp.port,
                smtp_username: smtp.username,
                smtp_password: if smtp.password.is_empty() {
                    String::new()
                } else {
                    String::from("********")
                },
                smtp_from_email: smtp.from_email,
                smtp_from_name: smtp.from_name,
                smtp_encryption: smtp.encryption,
                notify_server_chan_key: mask_value(&notification.server_chan_key),
                notify_telegram_bot_token: mask_value(&notification.telegram_bot_token),
                notify_telegram_chat_id: notification.telegram_chat_id,
                notify_bark_url: mask_value(&notification.bark_url),
                notify_wecom_webhook: mask_value(&notification.wecom_webhook),
                is_open_server_chan: notification.is_open_server_chan,
                is_open_telegram: notification.is_open_telegram,
                is_open_bark: notification.is_open_bark,
                is_open_bark_push_url: notification.is_open_bark_push_url,
                is_open_wecom: notification.is_open_wecom,
                language: value
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("zh-CN")
                    .to_string(),
                img_logo: value
                    .get("img_logo")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
    }
    Ok(SettingsData {
        name: state.config.site.name.clone(),
        logo_text: state.config.site.logo_text.clone(),
        keywords: String::new(),
        description: String::new(),
        manage_email: String::new(),
        notice: state.config.site.notice.clone(),
        footer: state.config.site.footer.clone(),
        base_url: state.config.site.base_url.clone(),
        is_open_anti_red: false,
        is_open_google_translate: false,
        template: theme.template,
        order_expire_minutes: order.order_expire_minutes,
        is_open_search_pwd: order.is_open_search_pwd,
        purchase_rate_window_minutes: order.purchase_rate_window_minutes,
        purchase_rate_max_per_email: order.purchase_rate_max_per_email,
        purchase_rate_max_per_ip: order.purchase_rate_max_per_ip,
        is_open_img_code: captcha.is_open_img_code,
        login_max_attempts: security.login_max_attempts,
        login_lock_minutes: security.login_lock_minutes,
        cookie_secure: security.cookie_secure,
        trust_proxy_hops: security.trust_proxy_hops,
        smtp_enabled: smtp.enabled,
        smtp_host: smtp.host,
        smtp_port: smtp.port,
        smtp_username: smtp.username,
        smtp_password: if smtp.password.is_empty() {
            String::new()
        } else {
            String::from("********")
        },
        smtp_from_email: smtp.from_email,
        smtp_from_name: smtp.from_name,
        smtp_encryption: smtp.encryption,
        notify_server_chan_key: mask_value(&notification.server_chan_key),
        notify_telegram_bot_token: mask_value(&notification.telegram_bot_token),
        notify_telegram_chat_id: notification.telegram_chat_id,
        notify_bark_url: mask_value(&notification.bark_url),
        notify_wecom_webhook: mask_value(&notification.wecom_webhook),
        is_open_server_chan: notification.is_open_server_chan,
        is_open_telegram: notification.is_open_telegram,
        is_open_bark: notification.is_open_bark,
        is_open_bark_push_url: notification.is_open_bark_push_url,
        is_open_wecom: notification.is_open_wecom,
        language: "zh-CN".to_string(),
        img_logo: String::new(),
    })
}

pub async fn save_settings(state: &AppState, form: SettingsForm) -> AppResult<()> {
    if form.name.trim().is_empty() {
        return Err(AppError::BadRequest("站点名称不能为空".to_string()));
    }
    let now = crate::time::now_str();
    let value = serde_json::json!({
        "name": form.name.trim(),
        "logo_text": form.logo_text.trim(),
        "keywords": form.keywords.as_deref().unwrap_or_default().trim(),
        "description": form.description.as_deref().unwrap_or_default().trim(),
        "manage_email": form.manage_email.as_deref().unwrap_or_default().trim(),
        "notice": form.notice,
        "footer": form.footer,
        "base_url": form.base_url.as_deref().unwrap_or_default().trim(),
        "is_open_anti_red": form.is_open_anti_red.is_some(),
        "is_open_google_translate": form.is_open_google_translate.is_some(),
        "language": match form.language.as_deref().unwrap_or("zh-CN").trim() {
            "en-US" => "en-US",
            _ => "zh-CN",
        },
        "img_logo": form.img_logo.as_deref().unwrap_or_default().trim(),
    });
    let template = match form.template.as_deref().unwrap_or("luna").trim() {
        "luna" => "luna",
        "unicorn" => "unicorn",
        "hyper" => "hyper",
        _ => return Err(AppError::BadRequest("主题模板不支持".to_string())),
    };
    let order_expire_minutes = form.order_expire_minutes.unwrap_or(5).max(1);
    let order_value = serde_json::json!({
        "order_expire_minutes": order_expire_minutes,
        "is_open_search_pwd": form.is_open_search_pwd.is_some(),
        "purchase_rate_window_minutes": form.purchase_rate_window_minutes.unwrap_or(0).max(0),
        "purchase_rate_max_per_email": form.purchase_rate_max_per_email.unwrap_or(0).max(0),
        "purchase_rate_max_per_ip": form.purchase_rate_max_per_ip.unwrap_or(0).max(0),
    });
    let theme_value = serde_json::json!({
        "template": template,
    });
    let captcha_value = serde_json::json!({
        "is_open_img_code": form.is_open_img_code.is_some(),
    });
    let security_value = serde_json::json!({
        "login_max_attempts": form.login_max_attempts.unwrap_or(5).clamp(1, 20),
        "login_lock_minutes": form.login_lock_minutes.unwrap_or(10).clamp(1, 1440),
        "cookie_secure": form.cookie_secure.is_some(),
        "trust_proxy_hops": form.trust_proxy_hops.unwrap_or(0).clamp(0, 10),
    });
    let smtp_port = form.smtp_port.unwrap_or(0);
    let smtp_encryption = form.smtp_encryption.trim().to_ascii_lowercase();
    if !matches!(smtp_encryption.as_str(), "starttls" | "tls" | "none") {
        return Err(AppError::BadRequest(
            "SMTP 加密方式必须是 starttls、tls 或 none".to_string(),
        ));
    }
    if form.smtp_enabled.is_some() {
        if form.smtp_host.trim().is_empty()
            || form.smtp_from_email.trim().is_empty()
            || smtp_port <= 0
        {
            return Err(AppError::BadRequest(
                "启用 SMTP 时必须填写 Host、端口和发件邮箱".to_string(),
            ));
        }
        if !form.smtp_from_email.contains('@') {
            return Err(AppError::BadRequest("发件邮箱格式不正确".to_string()));
        }
    }
    let existing_smtp = parse_smtp_settings(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT value_json FROM settings WHERE key = 'smtp_config'",
        )
        .fetch_optional(&state.pool)
        .await?
        .flatten()
        .as_deref(),
    );
    let existing_notification = parse_notification_settings(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT value_json FROM settings WHERE key = 'notification_config'",
        )
        .fetch_optional(&state.pool)
        .await?
        .flatten()
        .as_deref(),
    );
    let smtp_password_to_store = resolve_secret(
        form.smtp_password.trim(),
        &existing_smtp.password,
        &state.secret_box,
    );
    let smtp_value = serde_json::json!({
        "enabled": form.smtp_enabled.is_some(),
        "host": form.smtp_host.trim(),
        "port": smtp_port,
        "username": form.smtp_username.trim(),
        "password": smtp_password_to_store,
        "from_email": form.smtp_from_email.trim(),
        "from_name": form.smtp_from_name.trim(),
        "encryption": smtp_encryption,
    });
    let notification_value = serde_json::json!({
        "server_chan_key": resolve_secret(
            form.notify_server_chan_key.as_deref().unwrap_or_default().trim(),
            &existing_notification.server_chan_key,
            &state.secret_box,
        ),
        "telegram_bot_token": resolve_secret(
            form.notify_telegram_bot_token.as_deref().unwrap_or_default().trim(),
            &existing_notification.telegram_bot_token,
            &state.secret_box,
        ),
        "telegram_chat_id": form.notify_telegram_chat_id.as_deref().unwrap_or_default().trim(),
        "bark_url": resolve_secret(
            form.notify_bark_url.as_deref().unwrap_or_default().trim(),
            &existing_notification.bark_url,
            &state.secret_box,
        ),
        "wecom_webhook": resolve_secret(
            form.notify_wecom_webhook.as_deref().unwrap_or_default().trim(),
            &existing_notification.wecom_webhook,
            &state.secret_box,
        ),
        "is_open_server_chan": form.is_open_server_chan.is_some(),
        "is_open_telegram": form.is_open_telegram.is_some(),
        "is_open_bark": form.is_open_bark.is_some(),
        "is_open_bark_push_url": form.is_open_bark_push_url.is_some(),
        "is_open_wecom": form.is_open_wecom.is_some(),
    });
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO settings(key, value_json, created_at, updated_at)
         VALUES ('site_config', ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(value.to_string())
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO settings(key, value_json, created_at, updated_at)
         VALUES ('smtp_config', ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(smtp_value.to_string())
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO settings(key, value_json, created_at, updated_at)
         VALUES ('notification_config', ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(notification_value.to_string())
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    for (key, value) in [
        ("order_config", order_value.to_string()),
        ("theme_config", theme_value.to_string()),
        ("captcha_config", captcha_value.to_string()),
        ("security_config", security_value.to_string()),
    ] {
        sqlx::query(
            "INSERT INTO settings(key, value_json, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn coupons(state: &AppState, page: PageParams) -> AppResult<CouponsData> {
    let total = scalar_i64(
        state,
        "SELECT COUNT(*) FROM coupons WHERE deleted_at IS NULL",
    )
    .await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT c.id, c.code, c.type, c.value_cents, c.min_amount_cents, c.usage_limit,
                c.used_count, c.is_active,
                GROUP_CONCAT(p.name, '、') AS product_scope,
                GROUP_CONCAT(cp.product_id, ',') AS product_ids,
                MIN(cp.product_id) AS product_id
         FROM coupons c
         LEFT JOIN coupon_products cp ON cp.coupon_id = c.id
         LEFT JOIN products p ON p.id = cp.product_id AND p.deleted_at IS NULL
         WHERE c.deleted_at IS NULL
         GROUP BY c.id
         ORDER BY c.id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    let coupons = rows
        .into_iter()
        .map(|row| {
            let value_cents = row.get::<i64, _>("value_cents");
            let min_amount_cents = row.get::<i64, _>("min_amount_cents");
            let product_scope = row
                .get::<Option<String>, _>("product_scope")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "全部商品".to_string());
            let product_ids = row
                .get::<Option<String>, _>("product_ids")
                .unwrap_or_default()
                .split(',')
                .filter_map(|value| value.parse::<i64>().ok())
                .collect();
            CouponRow {
                id: row.get("id"),
                code: row.get("code"),
                r#type: row.get("type"),
                value_cents,
                value_display: money::format_cents(value_cents),
                min_amount_cents,
                min_amount_display: money::format_cents(min_amount_cents),
                usage_limit: row.get("usage_limit"),
                used_count: row.get("used_count"),
                is_active: row.get("is_active"),
                product_scope,
                product_id: row.get("product_id"),
                product_ids,
            }
        })
        .collect();
    Ok(CouponsData {
        coupons,
        products: product_options(state).await?,
        pagination,
    })
}

pub async fn create_coupon(state: &AppState, form: CouponForm) -> AppResult<()> {
    validate_coupon_form(&form)?;
    let now = crate::time::now_str();
    let mut tx = state.pool.begin().await?;
    let coupon_id = sqlx::query(
        "INSERT INTO coupons(code, type, value_cents, min_amount_cents, usage_limit, used_count, is_active, created_at, updated_at)
         VALUES (?, 'fixed', ?, ?, ?, 0, ?, ?, ?)",
    )
    .bind(form.code.trim().to_uppercase())
    .bind(form.value_cents)
    .bind(form.min_amount_cents.unwrap_or(0).max(0))
    .bind(form.usage_limit.unwrap_or(0).max(0))
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    let product_ids = coupon_product_ids(&form);
    save_coupon_scope(&mut tx, coupon_id, &product_ids).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_coupon(state: &AppState, id: i64, form: CouponForm) -> AppResult<()> {
    validate_coupon_form(&form)?;
    let now = crate::time::now_str();
    let mut tx = state.pool.begin().await?;
    let affected = sqlx::query(
        "UPDATE coupons
         SET code = ?, value_cents = ?, min_amount_cents = ?, usage_limit = ?, is_active = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(form.code.trim().to_uppercase())
    .bind(form.value_cents)
    .bind(form.min_amount_cents.unwrap_or(0).max(0))
    .bind(form.usage_limit.unwrap_or(0).max(0))
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("优惠券不存在".to_string()));
    }
    let product_ids = coupon_product_ids(&form);
    save_coupon_scope(&mut tx, id, &product_ids).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_coupon(state: &AppState, id: i64) -> AppResult<()> {
    soft_delete_by_id(state, "coupons", id).await
}

pub async fn email_templates(state: &AppState, page: PageParams) -> AppResult<EmailTemplatesData> {
    let total = scalar_i64(
        state,
        "SELECT COUNT(*) FROM email_templates WHERE deleted_at IS NULL",
    )
    .await?;
    let pagination = Pagination::from_params(&page, total);
    let rows =
        sqlx::query("SELECT id, token, subject, content, is_system FROM email_templates WHERE deleted_at IS NULL ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(pagination.limit)
            .bind(pagination.offset)
            .fetch_all(&state.pool)
            .await?;
    Ok(EmailTemplatesData {
        templates: rows
            .into_iter()
            .map(|row| EmailTemplateRow {
                id: row.get("id"),
                token: row.get("token"),
                subject: row.get("subject"),
                content: row.get("content"),
                is_system: row.get("is_system"),
            })
            .collect(),
        pagination,
    })
}

pub async fn create_email_template(state: &AppState, form: EmailTemplateForm) -> AppResult<()> {
    validate_email_template_form(&form)?;
    let now = crate::time::now_str();
    sqlx::query(
        "INSERT INTO email_templates(token, subject, content, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(form.token.trim())
    .bind(form.subject.trim())
    .bind(form.content)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn update_email_template(
    state: &AppState,
    id: i64,
    form: EmailTemplateForm,
) -> AppResult<()> {
    validate_email_template_form(&form)?;
    let affected = sqlx::query(
        "UPDATE email_templates SET token = ?, subject = ?, content = ?, updated_at = ? WHERE id = ?",
    )
    .bind(form.token.trim())
    .bind(form.subject.trim())
    .bind(form.content)
    .bind(crate::time::now_str())
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("邮件模板不存在".to_string()));
    }
    Ok(())
}

pub async fn delete_email_template(state: &AppState, id: i64) -> AppResult<()> {
    let is_system: i64 = sqlx::query_scalar("SELECT is_system FROM email_templates WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("邮件模板不存在".to_string()))?;
    if is_system == 1 {
        return Err(AppError::Conflict("系统邮件模板不能删除".to_string()));
    }
    let affected = sqlx::query("UPDATE email_templates SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(crate::time::now_str())
        .bind(crate::time::now_str())
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("邮件模板不存在".to_string()));
    }
    Ok(())
}

pub async fn restore_default_email_templates(state: &AppState) -> AppResult<()> {
    let now = crate::time::now_str();
    let templates = [
        (
            "card_send_user_email",
            "您的订单 {{ order_no }} 已完成",
            "订单号：{{ order_no }}\n订单金额：{{ amount }}\n\n卡密内容：\n{{ fulfillment }}",
        ),
        (
            "manual_send_user_email",
            "您的订单 {{ order_no }} 已处理",
            "订单号：{{ order_no }}\n订单金额：{{ amount }}\n\n发货内容：\n{{ fulfillment }}",
        ),
    ];
    for (token, subject, content) in templates {
        sqlx::query(
            "INSERT INTO email_templates(token, subject, content, is_system, created_at, updated_at)
             VALUES (?, ?, ?, 1, ?, ?)
             ON CONFLICT(token) DO UPDATE SET is_system = 1, deleted_at = NULL, updated_at = excluded.updated_at",
        )
        .bind(token)
        .bind(subject)
        .bind(content)
        .bind(&now)
        .bind(&now)
        .execute(&state.pool)
        .await?;
    }
    Ok(())
}

pub async fn admins(state: &AppState, page: PageParams) -> AppResult<AdminsData> {
    let total = scalar_i64(state, "SELECT COUNT(*) FROM admins").await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT id, username, display_name, COALESCE(role, 'owner') AS role, is_active FROM admins ORDER BY id ASC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    Ok(AdminsData {
        admins: rows
            .into_iter()
            .map(|row| AdminRow {
                id: row.get("id"),
                username: row.get("username"),
                display_name: row.get("display_name"),
                role: row.get("role"),
                is_active: row.get("is_active"),
            })
            .collect(),
        pagination,
    })
}

pub async fn create_admin(state: &AppState, form: AdminForm) -> AppResult<()> {
    let username = form.username.trim();
    let display_name = form.display_name.trim();
    if username.is_empty() || display_name.is_empty() || form.password.len() < 8 {
        return Err(AppError::BadRequest(
            "用户名、显示名不能为空，密码至少 8 位".to_string(),
        ));
    }
    let role = normalize_admin_role(form.role.as_deref())?;
    let hash = password::hash_password(&form.password)?;
    let now = crate::time::now_str();
    sqlx::query(
        "INSERT INTO admins(username, password_hash, display_name, role, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(username)
    .bind(hash)
    .bind(display_name)
    .bind(role)
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn update_admin(state: &AppState, id: i64, form: AdminForm) -> AppResult<()> {
    let display_name = form.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::BadRequest("显示名不能为空".to_string()));
    }
    let role = normalize_admin_role(form.role.as_deref())?;
    let now = crate::time::now_str();
    if form.password.trim().is_empty() {
        let affected = sqlx::query(
            "UPDATE admins SET display_name = ?, role = ?, is_active = ?, updated_at = ? WHERE id = ?",
        )
        .bind(display_name)
        .bind(role)
        .bind(if form.is_active.is_some() { 1 } else { 0 })
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound("管理员不存在".to_string()));
        }
        return Ok(());
    }
    if form.password.len() < 8 {
        return Err(AppError::BadRequest("密码至少 8 位".to_string()));
    }
    let hash = password::hash_password(&form.password)?;
    let affected = sqlx::query(
        "UPDATE admins SET display_name = ?, password_hash = ?, role = ?, is_active = ?, updated_at = ? WHERE id = ?",
    )
    .bind(display_name)
    .bind(hash)
    .bind(role)
    .bind(if form.is_active.is_some() { 1 } else { 0 })
    .bind(&now)
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("管理员不存在".to_string()));
    }
    Ok(())
}

pub async fn uploads(state: &AppState, page: PageParams) -> AppResult<UploadsData> {
    let total = scalar_i64(state, "SELECT COUNT(*) FROM media").await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT id, path, mime, size_bytes, created_at FROM media ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    Ok(UploadsData {
        media: rows
            .into_iter()
            .map(|row| MediaRow {
                id: row.get("id"),
                path: row.get("path"),
                mime: row.get("mime"),
                size_bytes: row.get("size_bytes"),
                created_at: row.get("created_at"),
            })
            .collect(),
        pagination,
    })
}

pub async fn email_test(state: &AppState) -> AppResult<EmailTestData> {
    let smtp_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'smtp_config'")
            .fetch_optional(&state.pool)
            .await?;
    Ok(EmailTestData {
        smtp_enabled: parse_smtp_settings(smtp_json.as_deref()).enabled,
    })
}

pub async fn send_email_test(state: &AppState, form: EmailTestForm) -> AppResult<()> {
    if !form.to.contains('@') || form.title.trim().is_empty() || form.body.trim().is_empty() {
        return Err(AppError::BadRequest(
            "收件人、标题和内容不能为空".to_string(),
        ));
    }
    mail::send_test_email(state, form.to.trim(), form.title.trim(), &form.body)
        .await
        .map_err(AppError::Anyhow)
}

pub async fn record_media(
    state: &AppState,
    path: &str,
    mime: &str,
    size_bytes: i64,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO media(disk, path, mime, size_bytes, created_at) VALUES ('local', ?, ?, ?, ?)",
    )
    .bind(path)
    .bind(mime)
    .bind(size_bytes)
    .bind(crate::time::now_str())
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn jobs(state: &AppState, page: PageParams) -> AppResult<JobsData> {
    let total = scalar_i64(state, "SELECT COUNT(*) FROM jobs").await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT id, kind, status, attempts, max_attempts, run_at, last_error, created_at
         FROM jobs ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    Ok(JobsData {
        jobs: rows
            .into_iter()
            .map(|row| JobRow {
                id: row.get("id"),
                kind: row.get("kind"),
                status: row.get("status"),
                attempts: row.get("attempts"),
                max_attempts: row.get("max_attempts"),
                run_at: row.get("run_at"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
            })
            .collect(),
        pagination,
    })
}

pub async fn retry_job(state: &AppState, id: i64) -> AppResult<()> {
    let affected = sqlx::query(
        "UPDATE jobs SET status = 'pending', attempts = 0, last_error = '', run_at = ?, locked_at = NULL, locked_by = '', updated_at = ?
         WHERE id = ? AND status IN ('dead', 'running')",
    )
    .bind(crate::time::now_str())
    .bind(crate::time::now_str())
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::Conflict(
            "只有 dead/running 任务可以重试".to_string(),
        ));
    }
    Ok(())
}

pub async fn notification_logs(
    state: &AppState,
    page: PageParams,
) -> AppResult<NotificationLogsData> {
    let total = scalar_i64(state, "SELECT COUNT(*) FROM notification_logs").await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT kind, target, status, error, created_at FROM notification_logs ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    Ok(NotificationLogsData {
        logs: rows
            .into_iter()
            .map(|row| NotificationLogRow {
                kind: row.get("kind"),
                target: row.get("target"),
                status: row.get("status"),
                error: row.get("error"),
                created_at: row.get("created_at"),
            })
            .collect(),
        pagination,
    })
}

pub async fn trash(state: &AppState, page: PageParams) -> AppResult<TrashData> {
    let mut rows = Vec::new();
    for (table_name, title_col) in [
        ("categories", "name"),
        ("products", "name"),
        ("coupons", "code"),
        ("payment_channels", "name"),
        ("email_templates", "token"),
        ("card_secrets", "secret"),
        ("orders", "order_no"),
    ] {
        let sql = format!(
            "SELECT id, {title_col} AS title, deleted_at FROM {table_name} WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC LIMIT 100",
        );
        for row in sqlx::query(&sql).fetch_all(&state.pool).await? {
            rows.push(TrashRow {
                table_name: table_name.to_string(),
                id: row.get("id"),
                title: row.get("title"),
                deleted_at: row
                    .get::<Option<String>, _>("deleted_at")
                    .unwrap_or_default(),
            });
        }
    }
    rows.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    let pagination = Pagination::from_params(&page, rows.len() as i64);
    let rows = rows
        .into_iter()
        .skip(pagination.offset as usize)
        .take(pagination.limit as usize)
        .collect();
    Ok(TrashData { rows, pagination })
}

pub async fn restore_trash(state: &AppState, table: &str, id: i64) -> AppResult<()> {
    let sql = match table {
        "categories" => "UPDATE categories SET deleted_at = NULL, updated_at = ? WHERE id = ?",
        "products" => {
            "UPDATE products SET deleted_at = NULL, is_active = 1, updated_at = ? WHERE id = ?"
        }
        "coupons" => {
            "UPDATE coupons SET deleted_at = NULL, is_active = 1, updated_at = ? WHERE id = ?"
        }
        "payment_channels" => {
            "UPDATE payment_channels SET deleted_at = NULL, is_active = 1, updated_at = ? WHERE id = ?"
        }
        "email_templates" => {
            "UPDATE email_templates SET deleted_at = NULL, updated_at = ? WHERE id = ?"
        }
        "card_secrets" => "UPDATE card_secrets SET deleted_at = NULL, updated_at = ? WHERE id = ?",
        "orders" => "UPDATE orders SET deleted_at = NULL, updated_at = ? WHERE id = ?",
        _ => return Err(AppError::BadRequest("不支持的回收站目标".to_string())),
    };
    let affected = sqlx::query(sql)
        .bind(crate::time::now_str())
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("回收站记录不存在".to_string()));
    }
    Ok(())
}

pub async fn audit_logs(state: &AppState, page: PageParams) -> AppResult<AuditLogsData> {
    let total = scalar_i64(state, "SELECT COUNT(*) FROM admin_audit_logs").await?;
    let pagination = Pagination::from_params(&page, total);
    let rows = sqlx::query(
        "SELECT id, admin_id, method, path, action, ip, created_at FROM admin_audit_logs ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.pool)
    .await?;
    Ok(AuditLogsData {
        logs: rows
            .into_iter()
            .map(|row| AuditLogRow {
                id: row.get("id"),
                admin_id: row.get("admin_id"),
                method: row.get("method"),
                path: row.get("path"),
                action: row.get("action"),
                ip: row.get("ip"),
                created_at: row.get("created_at"),
            })
            .collect(),
        pagination,
    })
}

pub async fn cleanup_uploads(state: &AppState) -> AppResult<i64> {
    let rows = sqlx::query("SELECT id, path FROM media")
        .fetch_all(&state.pool)
        .await?;
    let mut removed = 0;
    for row in rows {
        let id = row.get::<i64, _>("id");
        let path = row.get::<String, _>("path");
        let disk_path = state.config.uploads_dir().join(&path);
        if !disk_path.exists() {
            sqlx::query("DELETE FROM media WHERE id = ?")
                .bind(id)
                .execute(&state.pool)
                .await?;
            removed += 1;
        }
    }
    Ok(removed)
}

pub async fn cleanup_runtime(state: &AppState) -> AppResult<()> {
    let now_str = crate::time::now_str();
    sqlx::query("DELETE FROM admin_sessions WHERE expires_at <= ?")
        .bind(&now_str)
        .execute(&state.pool)
        .await?;
    let cutoff_30d = (crate::time::now() - chrono::Duration::days(30)).to_rfc3339();
    let cutoff_90d = (crate::time::now() - chrono::Duration::days(90)).to_rfc3339();
    sqlx::query("DELETE FROM jobs WHERE status = 'succeeded' AND updated_at <= ?")
        .bind(&cutoff_30d)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM jobs WHERE status = 'dead' AND updated_at <= ?")
        .bind(&cutoff_30d)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM captcha_challenges WHERE expires_at <= ?")
        .bind(&now_str)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM admin_login_attempts WHERE created_at <= ?")
        .bind(&cutoff_30d)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE created_at <= ?")
        .bind(&cutoff_90d)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM notification_logs WHERE created_at <= ?")
        .bind(&cutoff_30d)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM api_hook_logs WHERE created_at <= ?")
        .bind(&cutoff_30d)
        .execute(&state.pool)
        .await?;
    Ok(())
}

async fn product_options(state: &AppState) -> AppResult<Vec<ProductOption>> {
    let rows = sqlx::query(
        "SELECT id, name FROM products WHERE deleted_at IS NULL ORDER BY sort_order DESC, id DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ProductOption {
            id: row.get("id"),
            name: row.get("name"),
        })
        .collect())
}

async fn category_options(state: &AppState) -> AppResult<Vec<CategoryRow>> {
    let rows = sqlx::query(
        "SELECT id, name, is_active, sort_order FROM categories WHERE deleted_at IS NULL ORDER BY sort_order DESC, id DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CategoryRow {
            id: row.get("id"),
            name: row.get("name"),
            is_active: row.get("is_active"),
            sort_order: row.get("sort_order"),
        })
        .collect())
}

async fn payment_channel_options(state: &AppState) -> AppResult<Vec<PaymentChannelOption>> {
    let rows =
        sqlx::query("SELECT id, name FROM payment_channels WHERE deleted_at IS NULL ORDER BY sort_order DESC, id DESC")
            .fetch_all(&state.pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|row| PaymentChannelOption {
            id: row.get("id"),
            name: row.get("name"),
        })
        .collect())
}

fn validate_coupon_form(form: &CouponForm) -> AppResult<()> {
    if form.code.trim().is_empty() {
        return Err(AppError::BadRequest("优惠码不能为空".to_string()));
    }
    if form.value_cents <= 0 {
        return Err(AppError::BadRequest("优惠金额必须大于 0".to_string()));
    }
    Ok(())
}

async fn save_coupon_scope(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    coupon_id: i64,
    product_ids: &[i64],
) -> AppResult<()> {
    sqlx::query("DELETE FROM coupon_products WHERE coupon_id = ?")
        .bind(coupon_id)
        .execute(&mut **tx)
        .await?;
    let mut seen = HashSet::new();
    for product_id in product_ids
        .iter()
        .copied()
        .filter(|value| *value > 0 && seen.insert(*value))
    {
        sqlx::query("INSERT INTO coupon_products(coupon_id, product_id) VALUES (?, ?)")
            .bind(coupon_id)
            .bind(product_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn coupon_product_ids(form: &CouponForm) -> Vec<i64> {
    if !form.product_ids.is_empty() {
        form.product_ids.clone()
    } else {
        form.product_id.into_iter().collect()
    }
}

async fn save_product_payment_channels(
    state: &AppState,
    product_id: i64,
    raw: &str,
) -> AppResult<()> {
    let ids = serde_json::from_str::<Vec<i64>>(raw).unwrap_or_default();
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM product_payment_channels WHERE product_id = ?")
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    for id in ids.into_iter().filter(|id| *id > 0) {
        sqlx::query(
            "INSERT OR IGNORE INTO product_payment_channels(product_id, payment_channel_id) VALUES (?, ?)",
        )
        .bind(product_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn validate_email_template_form(form: &EmailTemplateForm) -> AppResult<()> {
    if form.token.trim().is_empty() || form.subject.trim().is_empty() {
        return Err(AppError::BadRequest("模板标识和标题不能为空".to_string()));
    }
    Ok(())
}

fn normalize_admin_role(role: Option<&str>) -> AppResult<&'static str> {
    match role.unwrap_or("owner").trim() {
        "owner" => Ok("owner"),
        "operator" => Ok("operator"),
        "viewer" => Ok("viewer"),
        _ => Err(AppError::BadRequest("管理员角色不支持".to_string())),
    }
}

fn normalize_json_array(raw: Option<&str>) -> AppResult<String> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok("[]".to_string());
    };
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|err| AppError::BadRequest(format!("JSON 格式错误: {err}")))?;
    if !value.is_array() {
        return Err(AppError::BadRequest("配置必须是 JSON 数组".to_string()));
    }
    Ok(value.to_string())
}

fn normalize_client_scope(raw: Option<&str>) -> AppResult<&'static str> {
    match raw.unwrap_or("all").trim() {
        "all" | "" => Ok("all"),
        "pc" => Ok("pc"),
        "mobile" => Ok("mobile"),
        _ => Err(AppError::BadRequest("支付客户端范围不支持".to_string())),
    }
}

fn merge_payment_config(form: &PaymentChannelForm) -> AppResult<String> {
    let mut value: serde_json::Value = serde_json::from_str(
        form.config_json
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("{}"),
    )
    .map_err(|err| AppError::BadRequest(format!("JSON 格式错误: {err}")))?;
    let Some(object) = value.as_object_mut() else {
        return Err(AppError::BadRequest("配置必须是 JSON 对象".to_string()));
    };
    for (key, value) in [
        ("merchant_id", form.merchant_id.as_deref()),
        ("merchant_key", form.merchant_key.as_deref()),
        ("merchant_pem", form.merchant_pem.as_deref()),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            object.insert(key.to_string(), serde_json::json!(value));
        }
    }
    if object.get("pid").is_none() {
        if let Some(value) = form
            .merchant_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            object.insert("pid".to_string(), serde_json::json!(value));
        }
    }
    if object.get("key").is_none() && object.get("token").is_none() {
        if let Some(value) = form
            .merchant_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            object.insert("key".to_string(), serde_json::json!(value));
            object.insert("token".to_string(), serde_json::json!(value));
        }
    }
    if object.get("gateway_url").is_none() {
        if let Some(value) = form
            .merchant_pem
            .as_deref()
            .map(str::trim)
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        {
            object.insert("gateway_url".to_string(), serde_json::json!(value));
        }
    }
    let provider = form.provider_type.trim().to_ascii_lowercase();
    let merchant_id = form
        .merchant_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let merchant_key = form
        .merchant_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let merchant_pem = form
        .merchant_pem
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match provider.as_str() {
        "epay" | "yipay" => {
            insert_if_missing(object, "merchant_id", merchant_id);
            insert_if_missing(object, "merchant_key", merchant_key);
        }
        "tokenpay" => {
            insert_if_missing(object, "notify_secret", merchant_key);
        }
        "epusdt" => {
            insert_if_missing(object, "secret_key", merchant_key);
        }
        "bepusdt" => {
            insert_if_missing(object, "auth_token", merchant_key);
        }
        "dujiaopay" => {
            insert_if_missing(object, "api_key_id", merchant_id);
            insert_if_missing(object, "api_secret", merchant_key);
            insert_if_missing(object, "api_base_url", merchant_pem);
        }
        "okpay" => {
            insert_if_missing(object, "merchant_id", merchant_id);
            insert_if_missing(object, "merchant_token", merchant_key);
        }
        "official" => match form.channel_type.trim().to_ascii_lowercase().as_str() {
            "stripe" => {
                insert_if_missing(object, "secret_key", merchant_key);
                insert_if_missing(object, "api_base_url", merchant_pem);
            }
            "paypal" => {
                insert_if_missing(object, "client_id", merchant_id);
                insert_if_missing(object, "client_secret", merchant_key);
                insert_if_missing(object, "base_url", merchant_pem);
            }
            "alipay" => {
                insert_if_missing(object, "app_id", merchant_id);
                insert_if_missing(object, "private_key", merchant_key);
                insert_if_missing(object, "gateway_url", merchant_pem);
            }
            "wechat" | "wxpay" => {
                insert_if_missing(object, "mchid", merchant_id);
                insert_if_missing(object, "merchant_private_key", merchant_key);
                insert_if_missing(object, "base_url", merchant_pem);
            }
            _ => {}
        },
        _ => {}
    }
    Ok(serde_json::Value::Object(object.clone()).to_string())
}

fn validate_evm_local_config_if_needed(provider_type: &str, config_json: &str) -> AppResult<()> {
    if !crate::services::evm_local_service::is_evm_local_provider(provider_type) {
        return Ok(());
    }
    let config: serde_json::Value = serde_json::from_str(config_json)
        .map_err(|err| AppError::BadRequest(format!("config_json 解析失败: {err}")))?;
    crate::services::evm_local_service::validate_channel_config(&config)
        .map_err(|err| AppError::BadRequest(err.to_string()))
}

/// Dry-run validation used by `POST /admin/api/payment-channels/validate`.
/// Reuses the same field-merge logic as create/update, then asks the matching
/// PaymentProvider to confirm its required-config invariants — without
/// touching the database.
pub fn validate_payment_channel_form(form: &PaymentChannelForm) -> anyhow::Result<()> {
    let provider_type = form.provider_type.trim();
    let channel_type = form.channel_type.trim();
    let interaction_mode = form.interaction_mode.trim();
    if form.name.trim().is_empty() {
        anyhow::bail!("通道名称不能为空");
    }
    if provider_type.is_empty() {
        anyhow::bail!("provider_type 不能为空");
    }
    if channel_type.is_empty() {
        anyhow::bail!("channel_type 不能为空");
    }
    if !matches!(interaction_mode, "redirect" | "qrcode") {
        anyhow::bail!("interaction_mode 仅支持 redirect / qrcode");
    }
    let merged = merge_payment_config(form).map_err(|err| anyhow::anyhow!("{err}"))?;
    let config: serde_json::Value = serde_json::from_str(&merged)
        .map_err(|err| anyhow::anyhow!("config_json 解析失败: {err}"))?;
    if crate::services::evm_local_service::is_evm_local_provider(provider_type) {
        crate::services::evm_local_service::validate_channel_config(&config)?;
        return Ok(());
    }
    let registry = crate::payment::registry::PaymentRegistry::default_registry();
    let provider = registry
        .lookup(provider_type, channel_type)
        .ok_or_else(|| anyhow::anyhow!("未注册的支付提供方 {provider_type}:{channel_type}"))?;
    provider.validate_config(&config, channel_type)?;
    Ok(())
}

fn insert_if_missing(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<&str>,
) {
    if object.get(key).is_none() {
        if let Some(value) = value {
            object.insert(key.to_string(), serde_json::json!(value));
        }
    }
}

#[derive(Debug)]
struct SmtpSettings {
    enabled: bool,
    host: String,
    port: i64,
    username: String,
    password: String,
    from_email: String,
    from_name: String,
    encryption: String,
}

#[derive(Debug)]
struct NotificationSettings {
    server_chan_key: String,
    telegram_bot_token: String,
    telegram_chat_id: String,
    bark_url: String,
    wecom_webhook: String,
    is_open_server_chan: bool,
    is_open_telegram: bool,
    is_open_bark: bool,
    is_open_bark_push_url: bool,
    is_open_wecom: bool,
}

impl NotificationSettings {
    fn any_enabled(&self) -> bool {
        (self.is_open_server_chan && !self.server_chan_key.trim().is_empty())
            || (self.is_open_telegram
                && !self.telegram_bot_token.trim().is_empty()
                && !self.telegram_chat_id.trim().is_empty())
            || (self.is_open_bark && !self.bark_url.trim().is_empty())
            || (self.is_open_wecom && !self.wecom_webhook.trim().is_empty())
    }
}

#[derive(Debug)]
struct OrderSettings {
    order_expire_minutes: i64,
    is_open_search_pwd: bool,
    purchase_rate_window_minutes: i64,
    purchase_rate_max_per_email: i64,
    purchase_rate_max_per_ip: i64,
}

#[derive(Debug)]
struct ThemeSettings {
    template: String,
}

#[derive(Debug)]
struct CaptchaSettings {
    is_open_img_code: bool,
}

#[derive(Debug)]
struct SecuritySettings {
    login_max_attempts: i64,
    login_lock_minutes: i64,
    cookie_secure: bool,
    trust_proxy_hops: i64,
}

fn parse_smtp_settings(raw: Option<&str>) -> SmtpSettings {
    let value = raw
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    SmtpSettings {
        enabled: value
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        host: value
            .get("host")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        port: value
            .get("port")
            .and_then(|value| value.as_i64())
            .unwrap_or(587),
        username: value
            .get("username")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        password: value
            .get("password")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        from_email: value
            .get("from_email")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        from_name: value
            .get("from_name")
            .and_then(|value| value.as_str())
            .unwrap_or("Dujiao Rust")
            .to_string(),
        encryption: value
            .get("encryption")
            .and_then(|value| value.as_str())
            .unwrap_or("starttls")
            .to_string(),
    }
}

fn parse_notification_settings(raw: Option<&str>) -> NotificationSettings {
    let value = raw
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    NotificationSettings {
        server_chan_key: value
            .get("server_chan_key")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        telegram_bot_token: value
            .get("telegram_bot_token")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        telegram_chat_id: value
            .get("telegram_chat_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        bark_url: value
            .get("bark_url")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        wecom_webhook: value
            .get("wecom_webhook")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        is_open_server_chan: value
            .get("is_open_server_chan")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        is_open_telegram: value
            .get("is_open_telegram")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        is_open_bark: value
            .get("is_open_bark")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        is_open_bark_push_url: value
            .get("is_open_bark_push_url")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        is_open_wecom: value
            .get("is_open_wecom")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn parse_order_settings(raw: Option<&str>, default_expire: i64) -> OrderSettings {
    let value = raw
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    OrderSettings {
        order_expire_minutes: value
            .get("order_expire_minutes")
            .and_then(|value| value.as_i64())
            .unwrap_or(default_expire)
            .max(1),
        is_open_search_pwd: value
            .get("is_open_search_pwd")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        purchase_rate_window_minutes: value
            .get("purchase_rate_window_minutes")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
            .max(0),
        purchase_rate_max_per_email: value
            .get("purchase_rate_max_per_email")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
            .max(0),
        purchase_rate_max_per_ip: value
            .get("purchase_rate_max_per_ip")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
            .max(0),
    }
}

fn parse_theme_settings(raw: Option<&str>, default_theme: &str) -> ThemeSettings {
    let value = raw
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let template = value
        .get("template")
        .and_then(|value| value.as_str())
        .unwrap_or(default_theme);
    ThemeSettings {
        template: match template {
            "unicorn" => "unicorn".to_string(),
            "hyper" => "hyper".to_string(),
            _ => "luna".to_string(),
        },
    }
}

fn parse_captcha_settings(raw: Option<&str>) -> CaptchaSettings {
    let value = raw
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    CaptchaSettings {
        is_open_img_code: value
            .get("is_open_img_code")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    }
}

fn mask_value(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        "********".to_string()
    }
}

/// When the admin keeps the masked placeholder (`********`) in the form, preserve the
/// existing ciphertext from settings. Empty input clears the secret. Anything else is
/// freshly encrypted with the current app secret.
fn resolve_secret(
    submitted: &str,
    existing_ciphertext: &str,
    secret_box: &crate::security::secrets::SecretManager,
) -> String {
    if submitted == "********" {
        return existing_ciphertext.to_string();
    }
    if submitted.is_empty() {
        return String::new();
    }
    secret_box.encrypt(submitted)
}

fn parse_security_settings(raw: Option<&str>) -> SecuritySettings {
    let value = raw
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    SecuritySettings {
        login_max_attempts: value
            .get("login_max_attempts")
            .and_then(|value| value.as_i64())
            .unwrap_or(5),
        login_lock_minutes: value
            .get("login_lock_minutes")
            .and_then(|value| value.as_i64())
            .unwrap_or(10),
        cookie_secure: value
            .get("cookie_secure")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        trust_proxy_hops: value
            .get("trust_proxy_hops")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
            .clamp(0, 10),
    }
}

async fn soft_delete_by_id(state: &AppState, table: &str, id: i64) -> AppResult<()> {
    let sql = match table {
        "categories" => {
            "UPDATE categories SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        "coupons" => {
            "UPDATE coupons SET deleted_at = ?, updated_at = ?, is_active = 0 WHERE id = ? AND deleted_at IS NULL"
        }
        _ => return Err(AppError::BadRequest("不支持的删除目标".to_string())),
    };
    let now = crate::time::now_str();
    let affected = sqlx::query(sql)
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("记录不存在".to_string()));
    }
    Ok(())
}
