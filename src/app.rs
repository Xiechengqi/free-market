use axum::Router;

use crate::{state::AppState, web};

pub fn router(state: AppState) -> Router {
    web::router::router(state)
}
