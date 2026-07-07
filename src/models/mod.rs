pub mod catalog;
pub mod job;
pub mod order;
pub mod payment;

pub const ORDER_PENDING_PAYMENT: &str = "pending_payment";
pub const ORDER_PAID: &str = "paid";
pub const ORDER_FULFILLING: &str = "fulfilling";
pub const ORDER_COMPLETED: &str = "completed";
pub const ORDER_CANCELED: &str = "canceled";
pub const ORDER_ABNORMAL: &str = "abnormal";
pub const ORDER_FAILED: &str = "failed";

pub const PAYMENT_PENDING: &str = "pending";
pub const PAYMENT_SUCCESS: &str = "success";
pub const PAYMENT_FAILED: &str = "failed";
pub const PAYMENT_EXPIRED: &str = "expired";

pub const CARD_AVAILABLE: &str = "available";
#[allow(dead_code)]
pub const CARD_RESERVED: &str = "reserved";
#[allow(dead_code)]
pub const CARD_USED: &str = "used";

pub const FULFILLMENT_AUTO: &str = "auto";
pub const FULFILLMENT_MANUAL: &str = "manual";
pub const FULFILLMENT_DELIVERED: &str = "delivered";
