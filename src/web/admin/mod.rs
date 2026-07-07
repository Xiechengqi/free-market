pub mod api;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

use crate::state::AppState;

pub async fn uploaded_file(State(state): State<AppState>, Path(path): Path<String>) -> Response {
    if path.contains("..") || path.starts_with('/') {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let disk_path = state.config.uploads_dir().join(&path);
    match tokio::fs::read(&disk_path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&disk_path).first_or_octet_stream();
            let mut response = bytes.into_response();
            if let Ok(value) = mime.as_ref().parse() {
                response.headers_mut().insert(header::CONTENT_TYPE, value);
            }
            response
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn is_allowed_image(ext: &str, mime: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp") && mime.starts_with("image/")
}
