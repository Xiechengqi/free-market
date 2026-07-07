use serde::Serialize;

#[allow(dead_code)]
pub fn cents_from_decimal_str(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.split('.');
    let yuan = parts.next()?.parse::<i64>().ok()?;
    let cents_part = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return None;
    }
    let mut cents = cents_part.chars().take(2).collect::<String>();
    while cents.len() < 2 {
        cents.push('0');
    }
    let cents = cents.parse::<i64>().ok()?;
    Some(yuan.saturating_mul(100).saturating_add(cents))
}

pub fn format_cents(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, (cents.abs() % 100))
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct PriceView {
    pub cents: i64,
    pub display: String,
}

impl From<i64> for PriceView {
    fn from(cents: i64) -> Self {
        Self {
            cents,
            display: format_cents(cents),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_to_cents() {
        assert_eq!(cents_from_decimal_str("95"), Some(9500));
        assert_eq!(cents_from_decimal_str("95.5"), Some(9550));
        assert_eq!(cents_from_decimal_str("95.05"), Some(9505));
        assert_eq!(cents_from_decimal_str(""), None);
        assert_eq!(cents_from_decimal_str("1.2.3"), None);
    }

    #[test]
    fn formats_cents() {
        assert_eq!(format_cents(9500), "95.00");
        assert_eq!(format_cents(5), "0.05");
    }
}
