use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PaymentChannel {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub channel_type: String,
    pub interaction_mode: String,
    pub config_json: String,
    pub is_active: i64,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Payment {
    pub id: i64,
    pub payment_no: String,
    pub order_id: i64,
    pub channel_id: i64,
    pub provider_type: String,
    pub channel_type: String,
    pub interaction_mode: String,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub provider_ref: String,
    pub gateway_order_no: String,
    pub pay_url: String,
    pub qr_code: String,
    pub provider_payload_json: String,
    pub paid_at: Option<String>,
    pub expired_at: Option<String>,
    pub callback_at: Option<String>,
}
