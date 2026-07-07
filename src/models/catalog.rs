use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub is_active: i64,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Product {
    pub id: i64,
    pub category_id: i64,
    pub slug: String,
    pub name: String,
    pub short_description: String,
    pub description_html: String,
    pub image_path: String,
    pub retail_price_cents: i64,
    pub price_cents: i64,
    pub wholesale_prices_json: String,
    pub fulfillment_type: String,
    pub manual_form_schema_json: String,
    pub manual_stock_total: i64,
    pub manual_stock_locked: i64,
    pub manual_stock_sold: i64,
    pub buy_limit_num: i64,
    pub buy_prompt: String,
    pub is_active: i64,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProductWithStock {
    pub id: i64,
    pub category_id: i64,
    pub name: String,
    pub short_description: String,
    pub image_path: String,
    pub image_url: String,
    pub price_cents: i64,
    pub price_display: String,
    pub fulfillment_type: String,
    pub stock: i64,
    pub sold: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryWithProducts {
    pub id: i64,
    pub name: String,
    pub products: Vec<ProductWithStock>,
}
