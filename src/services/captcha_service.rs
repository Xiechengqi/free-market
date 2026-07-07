use rand::Rng;
use sha2::{Digest, Sha256};

use crate::{error::AppResult, state::AppState, time};

#[derive(Debug, serde::Serialize)]
pub struct CaptchaChallenge {
    pub id: String,
    pub question: String,
    pub image_url: String,
}

pub async fn create_challenge(state: &AppState) -> AppResult<CaptchaChallenge> {
    let (a, b) = {
        let mut rng = rand::thread_rng();
        (rng.gen_range(2..=9), rng.gen_range(2..=9))
    };
    let id = uuid::Uuid::new_v4().simple().to_string();
    let answer = (a + b).to_string();
    let now = time::now_str();
    let expires_at = (time::now() + chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        "INSERT INTO captcha_challenges(id, answer_hash, expires_at, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(hash_answer(&id, &answer))
    .bind(expires_at)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    Ok(CaptchaChallenge {
        id: id.clone(),
        question: format!("{a} + {b} = ?"),
        image_url: format!("/captcha/{id}.svg"),
    })
}

pub async fn verify(state: &AppState, id: &str, answer: &str) -> AppResult<bool> {
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT answer_hash, expires_at, used_at FROM captcha_challenges WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((answer_hash, expires_at, used_at)) = row else {
        return Ok(false);
    };
    if used_at.is_some() {
        return Ok(false);
    }
    if time::parse_rfc3339(&expires_at).is_some_and(|expires_at| expires_at <= time::now()) {
        return Ok(false);
    }
    let ok = answer_hash == hash_answer(id, answer.trim());
    if ok {
        sqlx::query("UPDATE captcha_challenges SET used_at = ? WHERE id = ?")
            .bind(time::now_str())
            .bind(id)
            .execute(&state.pool)
            .await?;
    }
    Ok(ok)
}

pub async fn svg(state: &AppState, id: &str) -> AppResult<Option<String>> {
    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM captcha_challenges WHERE id = ? AND used_at IS NULL")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    if exists.is_none() {
        return Ok(None);
    }
    Ok(Some(format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='120' height='36'><rect width='120' height='36' fill='#f8fafc'/><text x='12' y='24' font-size='18' font-family='Arial' fill='#111827'>验证码</text></svg>"
    )))
}

fn hash_answer(id: &str, answer: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update(b":");
    hasher.update(answer.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}
