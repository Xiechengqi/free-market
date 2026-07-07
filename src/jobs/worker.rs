use std::time::Duration;

use serde_json::Value;

use crate::{
    mail,
    models::job::Job,
    notification,
    services::{api_hook_service, backup_service, order_service, secret_rotation_service},
    state::AppState,
    time,
};

pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(err) = run_once(&state).await {
                tracing::warn!(error = %err, "worker tick failed");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

async fn run_once(state: &AppState) -> anyhow::Result<()> {
    backup_service::run_scheduled_if_due(state).await?;
    secret_rotation_service::rotate_if_due(state).await?;

    let now = time::now_str();
    let job: Option<Job> = sqlx::query_as(
        "SELECT id, kind, payload_json, attempts, max_attempts
         FROM jobs
         WHERE status = 'pending' AND run_at <= ?
         ORDER BY run_at ASC, id ASC
         LIMIT 1",
    )
    .bind(&now)
    .fetch_optional(&state.pool)
    .await?;
    let Some(job) = job else {
        return Ok(());
    };
    let affected = sqlx::query(
        "UPDATE jobs SET status = 'running', locked_at = ?, locked_by = ?, updated_at = ?
         WHERE id = ? AND status = 'pending'",
    )
    .bind(&now)
    .bind(&state.worker_id)
    .bind(&now)
    .bind(job.id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if affected != 1 {
        return Ok(());
    }

    let result = handle_job(state, &job).await;
    match result {
        Ok(()) => {
            sqlx::query("UPDATE jobs SET status = 'succeeded', updated_at = ? WHERE id = ?")
                .bind(time::now_str())
                .bind(job.id)
                .execute(&state.pool)
                .await?;
        }
        Err(err) => {
            let attempts = job.attempts + 1;
            let status = if attempts >= job.max_attempts {
                "dead"
            } else {
                "pending"
            };
            let run_at = (time::now()
                + chrono::Duration::seconds(2_i64.pow(attempts.min(5) as u32)))
            .to_rfc3339();
            sqlx::query(
                "UPDATE jobs SET status = ?, attempts = ?, last_error = ?, run_at = ?, updated_at = ? WHERE id = ?",
            )
            .bind(status)
            .bind(attempts)
            .bind(err.to_string())
            .bind(run_at)
            .bind(time::now_str())
            .bind(job.id)
            .execute(&state.pool)
            .await?;
        }
    }
    Ok(())
}

async fn handle_job(state: &AppState, job: &Job) -> anyhow::Result<()> {
    match job.kind.as_str() {
        "order_timeout_cancel" => {
            let payload: Value = serde_json::from_str(&job.payload_json)?;
            let order_id = payload["order_id"].as_i64().unwrap_or_default();
            order_service::cancel_expired_order(state, order_id).await?;
        }
        "order_status_email" => {
            let payload: Value = serde_json::from_str(&job.payload_json)?;
            mail::record_order_email_job(state, &payload).await?;
        }
        "admin_notification" => {
            let payload: Value = serde_json::from_str(&job.payload_json)?;
            notification::send_admin_notification(state, &payload).await?;
        }
        "api_hook" => {
            let payload: Value = serde_json::from_str(&job.payload_json)?;
            api_hook_service::send_api_hook(state, &payload).await?;
        }
        other => tracing::warn!(kind = other, "unknown job kind ignored"),
    }
    Ok(())
}
