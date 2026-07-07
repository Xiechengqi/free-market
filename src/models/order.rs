use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Order {
    pub id: i64,
    pub order_no: String,
    pub status: String,
    pub currency: String,
    pub guest_email: String,
    pub guest_password: String,
    pub client_ip: String,
    pub original_amount_cents: i64,
    pub coupon_discount_cents: i64,
    pub wholesale_discount_cents: i64,
    pub total_amount_cents: i64,
    pub coupon_id: Option<i64>,
    pub payment_channel_id: Option<i64>,
    pub legacy_info: String,
    pub expires_at: String,
    pub paid_at: Option<String>,
    pub canceled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub sku_id: i64,
    pub product_name: String,
    pub unit_price_cents: i64,
    pub quantity: i64,
    pub total_price_cents: i64,
    pub fulfillment_type: String,
    pub manual_form_json: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Fulfillment {
    pub id: i64,
    pub order_id: i64,
    pub r#type: String,
    pub status: String,
    pub payload: String,
    pub delivered_at: Option<String>,
}
