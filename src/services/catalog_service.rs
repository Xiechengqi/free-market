use base64::{Engine as _, engine::general_purpose::STANDARD};
use qrcode::{QrCode, render::svg};
use serde::Serialize;
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    models::catalog::{Category, CategoryWithProducts, Product, ProductWithStock},
    money,
    services::{
        captcha_service,
        pricing_service::{self, WholesaleTier},
        settings_service,
    },
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct HomeData {
    pub categories: Vec<CategoryWithProducts>,
}

#[derive(Debug, Serialize)]
pub struct BuyProduct {
    pub id: i64,
    pub name: String,
    pub short_description: String,
    pub description_html: String,
    pub image_url: String,
    pub price_display: String,
    pub retail_price_display: String,
    pub show_retail_price: bool,
    pub stock: i64,
    pub fulfillment_label: String,
    pub fulfillment_tip_class: String,
    pub buy_limit_num: i64,
    pub buy_prompt: String,
    pub wholesale_tiers: Vec<WholesaleTier>,
    pub manual_fields: Vec<ManualFieldView>,
}

#[derive(Debug, Serialize)]
pub struct ManualFieldView {
    pub field: String,
    pub label: String,
    pub required: bool,
}

#[derive(Debug, Serialize)]
pub struct BuyPayChannel {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub channel_type: String,
    pub pay_check: String,
    pub badge: String,
    pub selected: bool,
}

#[derive(Debug, Serialize)]
pub struct BuyData {
    pub product: BuyProduct,
    pub default_channel_id: i64,
    pub payways: Vec<BuyPayChannel>,
    pub mobile_order_qr_data_url: String,
    pub captcha_enabled: bool,
    pub captcha_id: String,
    pub captcha_question: String,
    pub captcha_image_url: String,
}

pub async fn home_data(state: &AppState) -> AppResult<HomeData> {
    let categories: Vec<Category> = sqlx::query_as(
        "SELECT id, name, is_active, sort_order
         FROM categories
         WHERE is_active = 1 AND deleted_at IS NULL
         ORDER BY sort_order DESC, id ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut result = Vec::new();
    for category in categories {
        let products = products_by_category(state, category.id).await?;
        result.push(CategoryWithProducts {
            id: category.id,
            name: category.name,
            products,
        });
    }
    Ok(HomeData { categories: result })
}

pub async fn products_by_category(
    state: &AppState,
    category_id: i64,
) -> AppResult<Vec<ProductWithStock>> {
    let rows = sqlx::query(
        "SELECT p.id, p.category_id, p.name, p.short_description, p.image_path, p.price_cents,
                p.fulfillment_type, p.sales_volume,
                CASE WHEN p.fulfillment_type = 'auto'
                    THEN COALESCE(SUM(CASE WHEN cs.status = 'available' THEN 1 ELSE 0 END), 0)
                    ELSE MAX(p.manual_stock_total - p.manual_stock_locked, 0)
                END AS stock,
                p.sales_volume AS sold
         FROM products p
         LEFT JOIN card_secrets cs ON cs.product_id = p.id AND cs.deleted_at IS NULL
         WHERE p.category_id = ? AND p.is_active = 1 AND p.deleted_at IS NULL
         GROUP BY p.id
         ORDER BY p.sort_order DESC, p.id ASC",
    )
    .bind(category_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let price_cents = row.get::<i64, _>("price_cents");
            let image_path = row.get::<String, _>("image_path");
            ProductWithStock {
                id: row.get("id"),
                category_id: row.get("category_id"),
                name: row.get("name"),
                short_description: row.get("short_description"),
                image_url: image_url(&image_path),
                image_path,
                price_cents,
                price_display: money::format_cents(price_cents),
                fulfillment_type: row.get("fulfillment_type"),
                stock: row.get("stock"),
                sold: row.get("sold"),
            }
        })
        .collect())
}

pub async fn product_for_buy(
    state: &AppState,
    id: i64,
    user_agent: &str,
    buy_url: &str,
) -> AppResult<BuyData> {
    let product: Product = sqlx::query_as(
        "SELECT id, category_id, slug, name, short_description, description_html, image_path,
                retail_price_cents, price_cents, wholesale_prices_json, fulfillment_type,
                manual_form_schema_json, manual_stock_total, manual_stock_locked, manual_stock_sold, buy_limit_num,
                buy_prompt, is_active, sort_order
         FROM products
         WHERE id = ? AND is_active = 1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("商品不存在".to_string()))?;

    let stock = if product.fulfillment_type == "auto" {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM card_secrets WHERE product_id = ? AND status = 'available' AND deleted_at IS NULL",
        )
        .bind(product.id)
        .fetch_one(&state.pool)
        .await?
    } else {
        (product.manual_stock_total - product.manual_stock_locked).max(0)
    };

    let client = client_scope_from_user_agent(user_agent);
    let bound_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM product_payment_channels WHERE product_id = ?")
            .bind(product.id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
    let channel_rows = sqlx::query(
        "SELECT pc.id, pc.name, pc.provider_type, pc.channel_type, pc.pay_check, pc.client_scope
         FROM payment_channels pc
         LEFT JOIN product_payment_channels ppc
           ON ppc.payment_channel_id = pc.id AND ppc.product_id = ?
         WHERE pc.is_active = 1 AND pc.deleted_at IS NULL
           AND (? = 0 OR ppc.id IS NOT NULL)
           AND (pc.client_scope = 'all' OR pc.client_scope = ?)
         ORDER BY pc.sort_order DESC, pc.id ASC",
    )
    .bind(product.id)
    .bind(bound_count)
    .bind(client)
    .fetch_all(&state.pool)
    .await?;
    let default_channel_id = channel_rows
        .first()
        .map(|row| row.get::<i64, _>("id"))
        .unwrap_or(1);
    let payways = channel_rows
        .into_iter()
        .enumerate()
        .map(|(idx, row)| {
            let provider = row.get::<String, _>("provider_type");
            let channel_type = row.get::<String, _>("channel_type");
            let pay_check = row.get::<String, _>("pay_check").trim().to_string();
            let icon_key = payment_icon_key(&provider, &channel_type, &pay_check);
            BuyPayChannel {
                id: row.get("id"),
                name: row.get("name"),
                provider_type: provider,
                channel_type,
                badge: payment_badge(&icon_key),
                pay_check: icon_key,
                selected: idx == 0,
            }
        })
        .collect();

    let retail_price_display = money::format_cents(product.retail_price_cents);
    let image_url = image_url(&product.image_path);
    let (fulfillment_label, fulfillment_tip_class) =
        if product.fulfillment_type == crate::models::FULFILLMENT_AUTO {
            ("自动发货".to_string(), "tips-green".to_string())
        } else {
            ("人工处理".to_string(), "tips-yellow".to_string())
        };

    let captcha_config = settings_service::captcha_config(state).await;
    let captcha = if captcha_config.is_open_img_code {
        Some(captcha_service::create_challenge(state).await?)
    } else {
        None
    };
    Ok(BuyData {
        product: BuyProduct {
            id: product.id,
            name: product.name,
            short_description: product.short_description,
            description_html: product.description_html,
            image_url,
            price_display: money::format_cents(product.price_cents),
            retail_price_display,
            show_retail_price: product.retail_price_cents > 0,
            stock,
            fulfillment_label,
            fulfillment_tip_class,
            buy_limit_num: product.buy_limit_num,
            buy_prompt: product.buy_prompt,
            wholesale_tiers: pricing_service::tiers(&product.wholesale_prices_json),
            manual_fields: parse_manual_fields(&product.manual_form_schema_json),
        },
        default_channel_id,
        payways,
        mobile_order_qr_data_url: qr_data_url(buy_url)?,
        captcha_enabled: captcha.is_some(),
        captcha_id: captcha
            .as_ref()
            .map(|item| item.id.clone())
            .unwrap_or_default(),
        captcha_question: captcha
            .as_ref()
            .map(|item| item.question.clone())
            .unwrap_or_default(),
        captcha_image_url: captcha
            .as_ref()
            .map(|item| item.image_url.clone())
            .unwrap_or_default(),
    })
}

fn client_scope_from_user_agent(user_agent: &str) -> &'static str {
    let ua = user_agent.to_ascii_lowercase();
    if ua.contains("mobile")
        || ua.contains("android")
        || ua.contains("iphone")
        || ua.contains("ipad")
        || ua.contains("micromessenger")
    {
        "mobile"
    } else {
        "pc"
    }
}

fn payment_icon_key(provider: &str, channel_type: &str, pay_check: &str) -> String {
    let explicit = pay_check.trim().to_ascii_lowercase();
    if !explicit.is_empty() {
        return normalize_payment_icon_key(&explicit);
    }
    let provider = provider.trim().to_ascii_lowercase();
    let channel = channel_type.trim().to_ascii_lowercase();
    match provider.as_str() {
        "noop" => "other".to_string(),
        "epay" | "yipay" => normalize_payment_icon_key(&channel),
        "tokenpay" => "tokenpay".to_string(),
        "epusdt" => "epusdt".to_string(),
        "bepusdt" => match channel.as_str() {
            "trx" => "trx".to_string(),
            "usdc-trc20" => "usdc".to_string(),
            _ => "usdt".to_string(),
        },
        "dujiaopay" => "dujiaopay".to_string(),
        "okpay" => match channel.as_str() {
            "trx" => "trx".to_string(),
            _ => "okpay".to_string(),
        },
        "official" => normalize_payment_icon_key(&channel),
        _ => normalize_payment_icon_key(&channel),
    }
}

fn normalize_payment_icon_key(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "wechat" | "wepay" | "wx" | "mwx" | "vwx" => "wxpay".to_string(),
        "ali" | "alipay" | "aliweb" | "aliwap" | "zfb" | "mzfb" | "vzfb" => "alipay".to_string(),
        "qq" | "mqq" => "qqpay".to_string(),
        "paypal" => "paypal".to_string(),
        "stripe" => "stripe".to_string(),
        "tokenpay" => "tokenpay".to_string(),
        "epusdt" => "epusdt".to_string(),
        "bepusdt" => "bepusdt".to_string(),
        "dujiaopay" => "dujiaopay".to_string(),
        "okpay" => "okpay".to_string(),
        "usdt" | "usdt-trc20" | "tron-usdt" => "usdt".to_string(),
        "usdc" | "usdc-trc20" => "usdc".to_string(),
        "trx" | "tron-trx" => "trx".to_string(),
        _ => value.trim().to_ascii_lowercase(),
    }
}

fn payment_badge(icon_key: &str) -> String {
    match icon_key {
        "alipay" => "Ali".to_string(),
        "wxpay" => "Wx".to_string(),
        "qqpay" => "QQ".to_string(),
        "paypal" => "PP".to_string(),
        "stripe" => "S".to_string(),
        "tokenpay" => "TP".to_string(),
        "epusdt" => "EUS".to_string(),
        "bepusdt" => "BUS".to_string(),
        "dujiaopay" => "DJP".to_string(),
        "okpay" => "OK".to_string(),
        "usdt" => "USDT".to_string(),
        "usdc" => "USDC".to_string(),
        "trx" => "TRX".to_string(),
        _ => "Pay".to_string(),
    }
}

fn image_url(path: &str) -> String {
    if path.trim().is_empty() {
        "/assets/common/images/default.jpg".to_string()
    } else if path.starts_with('/') || path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("/uploads/{}", path.trim_start_matches("uploads/"))
    }
}

fn qr_data_url(content: &str) -> AppResult<String> {
    let code = QrCode::new(content.as_bytes())
        .map_err(|err| AppError::Anyhow(anyhow::anyhow!("generate mobile order qr: {err}")))?;
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(300, 300)
        .dark_color(svg::Color("#515151"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(svg.as_bytes())
    ))
}

fn parse_manual_fields(raw: &str) -> Vec<ManualFieldView> {
    #[derive(serde::Deserialize)]
    struct RawField {
        field: String,
        label: String,
        #[serde(default)]
        required: bool,
    }

    serde_json::from_str::<Vec<RawField>>(raw.trim())
        .unwrap_or_default()
        .into_iter()
        .filter(|field| !field.field.trim().is_empty() && !field.label.trim().is_empty())
        .map(|field| ManualFieldView {
            field: field.field,
            label: field.label,
            required: field.required,
        })
        .collect()
}
