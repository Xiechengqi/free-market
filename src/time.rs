use chrono::{DateTime, Duration, Utc};

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

pub fn now_str() -> String {
    now().to_rfc3339()
}

pub fn add_minutes(minutes: i64) -> String {
    (now() + Duration::minutes(minutes)).to_rfc3339()
}

pub fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
