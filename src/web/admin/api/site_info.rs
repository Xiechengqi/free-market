use axum::extract::State;
use serde::Serialize;

use crate::{
    services::settings_service,
    state::AppState,
    web::admin::api::{ApiResponse, ApiResult},
};

#[derive(Debug, Serialize)]
pub struct SiteInfo {
    pub name: String,
    #[serde(rename = "logoText")]
    pub logo_text: String,
    /// Operator-overridden brand image URL (settings.img_logo, e.g.
    /// `/uploads/admin/abc.png`). Empty string means the SPA should fall back
    /// to its bundled default logo.
    #[serde(rename = "imgLogo")]
    pub img_logo: String,
    pub language: String,
    pub footer: String,
}

/// GET /admin/api/site-info — public.
/// Returns the runtime site config so the SPA can render the brand
/// (login splash, header title, document.title, watermark, footer) from a
/// single source of truth.
pub async fn site_info(State(state): State<AppState>) -> ApiResult<SiteInfo> {
    let site = settings_service::runtime_site_config(&state).await;
    Ok(ApiResponse::ok(SiteInfo {
        name: site.name,
        logo_text: site.logo_text,
        img_logo: site.img_logo,
        language: site.language,
        footer: site.footer,
    }))
}
