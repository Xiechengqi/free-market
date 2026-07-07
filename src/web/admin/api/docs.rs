use pulldown_cmark::{Options, Parser, html};
use serde::Serialize;
use std::sync::OnceLock;

use crate::web::admin::api::{ApiResponse, ApiResult};

/// Source markdown is embedded at compile time so the rendered HTML is part
/// of the binary and does not depend on the working directory at runtime.
/// Edit `docs/README.md` and rebuild to update.
const DOCS_SOURCE: &str = include_str!("../../../../docs/README.md");

static RENDERED: OnceLock<String> = OnceLock::new();

fn rendered_html() -> &'static str {
    RENDERED.get_or_init(|| {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);

        let parser = Parser::new_ext(DOCS_SOURCE, opts);
        let mut out = String::with_capacity(DOCS_SOURCE.len() * 2);
        html::push_html(&mut out, parser);
        out
    })
}

#[derive(Debug, Serialize)]
pub struct DocsResponse {
    pub html: &'static str,
}

/// GET /admin/api/docs — public.
/// Returns the HTML rendered from `docs/README.md` at compile time. The SPA
/// `/docs` route wraps this in a Ruan-Yifeng-blog style chrome (white card,
/// centered max-width, serif-leaning typography).
pub async fn docs() -> ApiResult<DocsResponse> {
    Ok(ApiResponse::ok(DocsResponse {
        html: rendered_html(),
    }))
}
