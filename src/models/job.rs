use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub payload_json: String,
    pub attempts: i64,
    pub max_attempts: i64,
}
