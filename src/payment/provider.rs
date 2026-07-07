use std::collections::HashMap;

use async_trait::async_trait;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentInput {
    pub payment_no: String,
    pub order_no: String,
    pub subject: String,
    pub amount_cents: i64,
    pub currency: String,
    pub return_url: String,
    pub notify_url: String,
    pub client_ip: String,
    pub channel_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentResult {
    pub provider_ref: String,
    pub pay_url: String,
    pub qr_code: String,
    pub payload: Value,
    pub amount_sent: String,
    pub currency_sent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentCallback {
    pub payment_no: String,
    pub provider_ref: String,
    pub status: PaymentStatus,
    pub amount_cents: i64,
    pub currency: String,
    pub paid_at: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    Pending,
    Success,
    Failed,
    Expired,
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    fn provider_type(&self) -> &'static str;

    fn validate_config(&self, _config: &Value, _channel_type: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult>;

    async fn verify_callback(
        &self,
        _config: &Value,
        _form: &HashMap<String, String>,
        _body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        anyhow::bail!(
            "{} does not support form/json callback",
            self.provider_type()
        )
    }

    async fn parse_webhook(
        &self,
        _config: &Value,
        _headers: &HeaderMap,
        _body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        anyhow::bail!("{} does not support webhook", self.provider_type())
    }

    #[allow(dead_code)]
    async fn query_payment(
        &self,
        _config: &Value,
        _provider_ref: &str,
    ) -> anyhow::Result<PaymentCallback> {
        anyhow::bail!("{} does not support query/capture", self.provider_type())
    }
}

pub fn payment_result(
    provider_ref: impl Into<String>,
    pay_url: impl Into<String>,
    qr_code: impl Into<String>,
    payload: Value,
) -> CreatePaymentResult {
    CreatePaymentResult {
        provider_ref: provider_ref.into(),
        pay_url: pay_url.into(),
        qr_code: qr_code.into(),
        payload,
        amount_sent: String::new(),
        currency_sent: String::new(),
    }
}

pub fn callback_success(
    payment_no: impl Into<String>,
    provider_ref: impl Into<String>,
    amount_cents: i64,
    currency: impl Into<String>,
    payload: Value,
) -> PaymentCallback {
    PaymentCallback {
        payment_no: payment_no.into(),
        provider_ref: provider_ref.into(),
        status: PaymentStatus::Success,
        amount_cents,
        currency: currency.into(),
        paid_at: None,
        payload,
    }
}

pub fn require_config(config: &Value, keys: &[&str]) -> anyhow::Result<()> {
    for key in keys {
        if str_config(config, key).trim().is_empty() {
            anyhow::bail!("payment config missing {key}");
        }
    }
    Ok(())
}

pub fn str_config(config: &Value, key: &str) -> String {
    config
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn str_config_any(config: &Value, keys: &[&str]) -> String {
    keys.iter()
        .map(|key| str_config(config, key))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

pub fn amount_yuan(amount_cents: i64) -> String {
    crate::money::format_cents(amount_cents)
}

pub fn yuan_to_cents(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let negative = value.starts_with('-');
    let value = value.trim_start_matches('-');
    let mut parts = value.splitn(2, '.');
    let yuan = parts.next()?.parse::<i64>().ok()?;
    let cents_raw = parts.next().unwrap_or("0");
    let cents = format!("{:0<2}", cents_raw.chars().take(2).collect::<String>())
        .chars()
        .take(2)
        .collect::<String>()
        .parse::<i64>()
        .ok()?;
    let total = yuan.saturating_mul(100).saturating_add(cents);
    Some(if negative { -total } else { total })
}

pub fn first_param<'a>(params: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    for key in keys {
        if let Some(value) = params.get(*key).filter(|value| !value.trim().is_empty()) {
            return Some(value);
        }
        if let Some((_, value)) = params
            .iter()
            .find(|(name, value)| name.eq_ignore_ascii_case(key) && !value.trim().is_empty())
        {
            return Some(value);
        }
    }
    None
}

pub fn sorted_md5_sign(params: &[(String, String)], key: &str) -> String {
    let mut items = params
        .iter()
        .filter(|(name, value)| {
            !matches!(
                name.as_str(),
                "sign" | "sign_type" | "signature" | "Signature"
            ) && !value.trim().is_empty()
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let raw = items
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{:x}", md5::compute(format!("{raw}{key}")))
}

pub fn sorted_md5_sign_value_suffix(params: &[(String, String)], suffix: &str) -> String {
    let mut items = params
        .iter()
        .filter(|(name, value)| {
            !matches!(
                name.as_str(),
                "sign" | "sign_type" | "signature" | "Signature"
            ) && !value.trim().is_empty()
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let raw = items
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{:x}", md5::compute(format!("{raw}{suffix}")))
}

pub fn url_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char)
            }
            b' ' => output.push('+'),
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

pub fn parse_json_object(body: &[u8]) -> anyhow::Result<Value> {
    let value: Value = serde_json::from_slice(body)?;
    if !value.is_object() {
        anyhow::bail!("callback body must be a JSON object");
    }
    Ok(value)
}

pub fn flatten_json(value: &Value) -> HashMap<String, String> {
    let mut output = HashMap::new();
    flatten_json_into("", value, &mut output);
    output
}

fn flatten_json_into(prefix: &str, value: &Value, output: &mut HashMap<String, String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}[{key}]")
                };
                flatten_json_into(&full_key, value, output);
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                flatten_json_into(&format!("{prefix}[{index}]"), value, output);
            }
        }
        Value::String(text) => {
            output.insert(prefix.to_string(), text.trim().to_string());
        }
        Value::Number(number) => {
            output.insert(prefix.to_string(), number.to_string());
        }
        Value::Bool(flag) => {
            output.insert(prefix.to_string(), flag.to_string());
        }
        Value::Null => {}
    }
}

pub fn json_string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

pub fn json_i64(value: &Value, path: &[&str]) -> Option<i64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_i64()
        .or_else(|| current.as_str().and_then(|value| value.trim().parse().ok()))
}

pub fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
