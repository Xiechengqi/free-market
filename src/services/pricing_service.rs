use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct WholesaleTier {
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub unit_price_display: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceQuote {
    pub unit_price_cents: i64,
    pub original_amount_cents: i64,
    pub wholesale_discount_cents: i64,
    pub payable_before_coupon_cents: i64,
}

#[derive(Debug, Deserialize)]
struct RawTier {
    quantity: Option<i64>,
    number: Option<i64>,
    unit_price_cents: Option<i64>,
    price_cents: Option<i64>,
    price: Option<serde_json::Value>,
}

pub fn quote(base_unit_price_cents: i64, quantity: i64, wholesale_raw: &str) -> PriceQuote {
    let quantity = quantity.max(1);
    let original = base_unit_price_cents.saturating_mul(quantity);
    let unit = select_wholesale_unit_price(base_unit_price_cents, quantity, wholesale_raw)
        .unwrap_or(base_unit_price_cents);
    let discounted = unit.saturating_mul(quantity);
    PriceQuote {
        unit_price_cents: unit,
        original_amount_cents: original,
        wholesale_discount_cents: (original - discounted).max(0),
        payable_before_coupon_cents: discounted.min(original),
    }
}

pub fn tiers(wholesale_raw: &str) -> Vec<WholesaleTier> {
    parse_tiers(wholesale_raw)
        .into_iter()
        .map(|(quantity, unit_price_cents)| WholesaleTier {
            quantity,
            unit_price_cents,
            unit_price_display: crate::money::format_cents(unit_price_cents),
        })
        .collect()
}

fn select_wholesale_unit_price(
    base_unit_price_cents: i64,
    quantity: i64,
    wholesale_raw: &str,
) -> Option<i64> {
    parse_tiers(wholesale_raw)
        .into_iter()
        .filter(|(tier_quantity, unit_price)| {
            *tier_quantity > 0 && *unit_price > 0 && *unit_price < base_unit_price_cents
        })
        .filter(|(tier_quantity, _)| quantity >= *tier_quantity)
        .max_by_key(|(tier_quantity, _)| *tier_quantity)
        .map(|(_, unit_price)| unit_price)
}

fn parse_tiers(wholesale_raw: &str) -> Vec<(i64, i64)> {
    let raw = wholesale_raw.trim();
    if raw.is_empty() || raw == "[]" {
        return Vec::new();
    }
    if let Ok(items) = serde_json::from_str::<Vec<RawTier>>(raw) {
        let mut tiers = items
            .into_iter()
            .filter_map(|item| {
                let quantity = item.quantity.or(item.number)?;
                let price = item
                    .unit_price_cents
                    .or(item.price_cents)
                    .or_else(|| item.price.and_then(value_to_cents))?;
                Some((quantity, price))
            })
            .collect::<Vec<_>>();
        tiers.sort_by_key(|(quantity, _)| *quantity);
        return tiers;
    }

    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let parts = line
                .split(['=', '|', ',', ':'])
                .map(str::trim)
                .collect::<Vec<_>>();
            if parts.len() < 2 {
                return None;
            }
            let quantity = parts[0].parse::<i64>().ok()?;
            let price = decimal_yuan_to_cents(parts[1]).or_else(|| parts[1].parse::<i64>().ok())?;
            Some((quantity, price))
        })
        .collect()
}

fn value_to_cents(value: serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Some(value)
            } else {
                number.as_f64().map(|value| (value * 100.0).round() as i64)
            }
        }
        serde_json::Value::String(value) => decimal_yuan_to_cents(&value),
        _ => None,
    }
}

fn decimal_yuan_to_cents(value: &str) -> Option<i64> {
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
    let result = yuan.saturating_mul(100).saturating_add(cents);
    Some(if negative { -result } else { result })
}
