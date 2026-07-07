use axum::{
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

/// Embeds the Vite `dist/` produced by `web-admin/`.
///
/// Build-time contract: `cargo build` expects `web-admin/dist/` to exist (the
/// project's `build.sh` runs `pnpm -C web-admin build` first). When the
/// directory is missing, the embed simply contains zero files and every SPA
/// request 404s — the binary still runs so JSON-only deployments work.
#[derive(RustEmbed)]
#[folder = "web-admin/dist/"]
struct AdminSpa;

/// Serves a single asset under `/admin/{filename}` (e.g. `index.html`,
/// `favicon.svg`) or `/admin/assets/{hash}.js|.css`.
///
/// SPA history mode rule:
/// - Request matches an embedded file → serve as-is with correct MIME and
///   long-lived `Cache-Control` for hashed `/assets/*`.
/// - Request misses (e.g. `/admin/orders`, `/admin/products/42/cards`) →
///   fall back to `index.html` so vue-router can take over client-side.
/// - Special case: API namespace `/admin/api/*` is mounted separately and
///   never reaches this handler.
pub async fn serve_spa(path: &str) -> Response {
    let rel = path.trim_start_matches('/');
    if let Some(file) = AdminSpa::get(rel) {
        return respond(rel, file.data.into_owned(), is_immutable(rel));
    }
    // SPA fallback to index.html
    match AdminSpa::get("index.html") {
        Some(index) => respond("index.html", index.data.into_owned(), false),
        None => spa_unavailable(),
    }
}

/// Axum handler for `/admin/*path`.
pub async fn admin_spa_handler(Path(path): Path<String>) -> Response {
    serve_spa(&path).await
}

/// Axum handler for the bare `/admin` index — serves `index.html`.
pub async fn admin_spa_index() -> Response {
    serve_spa("index.html").await
}

fn respond(path: &str, body: Vec<u8>, immutable: bool) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut response = body.into_response();
    let headers = response.headers_mut();
    if let Ok(value) = mime.as_ref().parse() {
        headers.insert(header::CONTENT_TYPE, value);
    }
    let cache = if immutable {
        "public, immutable, max-age=31536000"
    } else {
        "no-cache"
    };
    if let Ok(value) = cache.parse() {
        headers.insert(header::CACHE_CONTROL, value);
    }
    response
}

fn is_immutable(path: &str) -> bool {
    // Vite emits hashed filenames under `assets/`; everything else (index.html,
    // favicon, robots.txt) should revalidate.
    path.starts_with("assets/") && path.contains('.')
}

fn spa_unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        r#"<!doctype html><html><head><meta charset="utf-8"><title>SPA not built</title></head>
<body style="font-family:sans-serif;padding:40px;max-width:640px;margin:auto">
<h1>Admin SPA not built</h1>
<p>The Rust binary was compiled without <code>web-admin/dist/</code>.
Run <code>pnpm -C web-admin build</code> (or the top-level <code>./build.sh</code>) and rebuild.</p>
<p>API endpoints under <code>/admin/api/*</code> are still available.</p>
</body></html>"#,
    )
        .into_response()
}
