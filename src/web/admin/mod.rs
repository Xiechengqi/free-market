use axum::{
    Extension,
    extract::{ConnectInfo, Form, Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::AsyncWriteExt;

pub mod api;

use crate::{
    error::{AppError, AppResult},
    security::session,
    services::{admin_service, backup_service},
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct LoginPageData {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct FulfillForm {
    pub payload: String,
}

#[derive(Debug, Deserialize)]
pub struct BackupSettingsForm {
    pub enabled: Option<String>,
    pub weekday: u32,
    pub hour: u32,
    pub keep_files: usize,
}

pub async fn login_page(State(state): State<AppState>) -> AppResult<Html<String>> {
    Ok(Html(state.views.render(
        "admin/login.html",
        &state.config.site,
        LoginPageData {
            error: String::new(),
        },
    )?))
}

pub async fn login(
    State(state): State<AppState>,
    connect: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> AppResult<Response> {
    let ip = crate::security::net::client_ip(&state, &headers, connect.as_ref()).await;
    let secure = crate::security::csrf::should_use_secure_cookie(&state, &headers).await;
    if session::login_blocked(&state, form.username.trim(), &ip).await? {
        return Err(AppError::BadRequest(
            "登录失败次数过多，请稍后再试".to_string(),
        ));
    }
    let token = session::create_session(&state, form.username.trim(), &form.password)
        .await?
        .ok_or_else(|| AppError::BadRequest("用户名或密码错误".to_string()));
    let token = match token {
        Ok(token) => {
            let _ = session::record_login_attempt(&state, form.username.trim(), &ip, true).await;
            token
        }
        Err(err) => {
            let _ = session::record_login_attempt(&state, form.username.trim(), &ip, false).await;
            return Err(err);
        }
    };
    let mut out = HeaderMap::new();
    out.insert(
        header::SET_COOKIE,
        session::session_cookie(state.admin_prefix(), secure, &token)
            .parse()
            .map_err(|err| AppError::Anyhow(anyhow::anyhow!("invalid cookie header: {err}")))?,
    );
    Ok((out, Redirect::to(state.admin_prefix())).into_response())
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = session::extract_session_token(&headers) {
        session::destroy_session(&state, &token).await?;
    }
    let mut response = Redirect::to(&state.admin_url("/login")).into_response();
    let secure = crate::security::csrf::should_use_secure_cookie(&state, &headers).await;
    response.headers_mut().insert(
        header::SET_COOKIE,
        session::expired_session_cookie(state.admin_prefix(), secure)
            .parse()
            .map_err(|err| AppError::Anyhow(anyhow::anyhow!("invalid cookie header: {err}")))?,
    );
    Ok(response)
}

pub async fn dashboard(State(state): State<AppState>) -> AppResult<Html<String>> {
    let data = admin_service::dashboard(&state).await?;
    Ok(Html(state.views.render(
        "admin/dashboard.html",
        &state.config.site,
        data,
    )?))
}

pub async fn orders(
    State(state): State<AppState>,
    Query(filter): Query<admin_service::AdminOrdersFilter>,
) -> AppResult<Html<String>> {
    let data = admin_service::orders(&state, filter).await?;
    Ok(Html(state.views.render(
        "admin/orders.html",
        &state.config.site,
        data,
    )?))
}

pub async fn order(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let data = admin_service::order(&state, id).await?;
    Ok(Html(state.views.render(
        "admin/order.html",
        &state.config.site,
        data,
    )?))
}

pub async fn fulfill(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Extension(admin): Extension<crate::security::session::AdminContext>,
    Form(form): Form<FulfillForm>,
) -> AppResult<Redirect> {
    admin_service::fulfill(&state, id, form.payload, admin.id).await?;
    Ok(Redirect::to(&format!(
        "{}/orders/{}",
        state.admin_prefix(),
        id
    )))
}

pub async fn cancel_order(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::cancel_order(&state, id).await?;
    Ok(Redirect::to(&format!(
        "{}/orders/{}",
        state.admin_prefix(),
        id
    )))
}

pub async fn resend_order_email(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::resend_order_email(&state, id).await?;
    Ok(Redirect::to(&format!(
        "{}/orders/{}",
        state.admin_prefix(),
        id
    )))
}

pub async fn mark_order_abnormal(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::mark_order_abnormal(&state, id).await?;
    Ok(Redirect::to(&format!(
        "{}/orders/{}",
        state.admin_prefix(),
        id
    )))
}

pub async fn delete_order(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::soft_delete_order(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/orders")))
}

pub async fn start_order_processing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::start_order_processing(&state, id).await?;
    Ok(Redirect::to(&format!(
        "{}/orders/{}",
        state.admin_prefix(),
        id
    )))
}

pub async fn export_orders(
    State(state): State<AppState>,
    Query(filter): Query<admin_service::AdminOrdersFilter>,
) -> AppResult<Response> {
    let csv = admin_service::export_orders(&state, filter).await?;
    let mut response = Response::new(axum::body::Body::from(csv));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/csv; charset=utf-8".parse().unwrap(),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!(
            "attachment; filename=\"orders-{}.csv\"",
            crate::time::now_str().replace(':', "-")
        )
        .parse()
        .unwrap(),
    );
    Ok(response)
}

pub async fn categories(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::categories(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/categories.html",
        &state.config.site,
        data,
    )?))
}

pub async fn create_category(
    State(state): State<AppState>,
    Form(form): Form<admin_service::CategoryForm>,
) -> AppResult<Redirect> {
    admin_service::create_category(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/categories")))
}

pub async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<admin_service::CategoryForm>,
) -> AppResult<Redirect> {
    admin_service::update_category(&state, id, form).await?;
    Ok(Redirect::to(&state.admin_url("/categories")))
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::delete_category(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/categories")))
}

pub async fn products(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::products(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/products.html",
        &state.config.site,
        data,
    )?))
}

pub async fn create_product(
    State(state): State<AppState>,
    Form(form): Form<admin_service::ProductForm>,
) -> AppResult<Redirect> {
    admin_service::create_product(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/products")))
}

pub async fn update_product(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<admin_service::ProductForm>,
) -> AppResult<Redirect> {
    admin_service::update_product(&state, id, form).await?;
    Ok(Redirect::to(&state.admin_url("/products")))
}

pub async fn delete_product(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::delete_product(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/products")))
}

pub async fn cards(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
    Query(filter): Query<admin_service::CardsFilter>,
) -> AppResult<Html<String>> {
    let data = admin_service::cards(&state, product_id, filter).await?;
    Ok(Html(state.views.render(
        "admin/cards.html",
        &state.config.site,
        data,
    )?))
}

pub async fn export_cards(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
    Query(filter): Query<admin_service::CardsFilter>,
) -> AppResult<Response> {
    let body = admin_service::export_cards(&state, product_id, filter).await?;
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/plain; charset=utf-8".parse().map_err(|err| {
            AppError::Anyhow(anyhow::anyhow!("invalid content-type header: {err}"))
        })?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"cards-{}.txt\"", product_id)
            .parse()
            .map_err(|err| {
                AppError::Anyhow(anyhow::anyhow!("invalid disposition header: {err}"))
            })?,
    );
    Ok(response)
}

pub async fn import_cards(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
    Form(form): Form<admin_service::ImportCardsForm>,
) -> AppResult<Redirect> {
    admin_service::import_cards(&state, product_id, form).await?;
    Ok(Redirect::to(&format!(
        "/admin/products/{}/cards",
        product_id
    )))
}

pub async fn delete_card(
    State(state): State<AppState>,
    Path((product_id, card_id)): Path<(i64, i64)>,
) -> AppResult<Redirect> {
    admin_service::delete_card(&state, product_id, card_id).await?;
    Ok(Redirect::to(&format!(
        "/admin/products/{}/cards",
        product_id
    )))
}

pub async fn global_cards(
    State(state): State<AppState>,
    Query(filter): Query<admin_service::GlobalCardsFilter>,
) -> AppResult<Html<String>> {
    let data = admin_service::global_cards(&state, filter).await?;
    Ok(Html(state.views.render(
        "admin/global_cards.html",
        &state.config.site,
        data,
    )?))
}

pub async fn export_global_cards(
    State(state): State<AppState>,
    Query(filter): Query<admin_service::GlobalCardsFilter>,
) -> AppResult<Response> {
    let body = admin_service::export_global_cards(&state, filter).await?;
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/plain; charset=utf-8".parse().map_err(|err| {
            AppError::Anyhow(anyhow::anyhow!("invalid content-type header: {err}"))
        })?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"cards.txt\""
            .parse()
            .map_err(|err| {
                AppError::Anyhow(anyhow::anyhow!("invalid disposition header: {err}"))
            })?,
    );
    Ok(response)
}

pub async fn delete_global_card(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::delete_global_card(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/cards")))
}

pub async fn payment_channels(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::payment_channels(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/payment_channels.html",
        &state.config.site,
        data,
    )?))
}

pub async fn create_payment_channel(
    State(state): State<AppState>,
    Form(form): Form<admin_service::PaymentChannelForm>,
) -> AppResult<Redirect> {
    admin_service::create_payment_channel(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/payment-channels")))
}

pub async fn update_payment_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<admin_service::PaymentChannelForm>,
) -> AppResult<Redirect> {
    admin_service::update_payment_channel(&state, id, form).await?;
    Ok(Redirect::to(&state.admin_url("/payment-channels")))
}

pub async fn delete_payment_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::delete_payment_channel(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/payment-channels")))
}

pub async fn settings(State(state): State<AppState>) -> AppResult<Html<String>> {
    let data = admin_service::settings(&state).await?;
    Ok(Html(state.views.render(
        "admin/settings.html",
        &state.config.site,
        data,
    )?))
}

pub async fn save_settings(
    State(state): State<AppState>,
    Form(form): Form<admin_service::SettingsForm>,
) -> AppResult<Redirect> {
    admin_service::save_settings(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/settings")))
}

pub async fn coupons(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::coupons(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/coupons.html",
        &state.config.site,
        data,
    )?))
}

pub async fn create_coupon(
    State(state): State<AppState>,
    Form(form): Form<admin_service::CouponForm>,
) -> AppResult<Redirect> {
    admin_service::create_coupon(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/coupons")))
}

pub async fn update_coupon(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<admin_service::CouponForm>,
) -> AppResult<Redirect> {
    admin_service::update_coupon(&state, id, form).await?;
    Ok(Redirect::to(&state.admin_url("/coupons")))
}

pub async fn delete_coupon(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::delete_coupon(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/coupons")))
}

pub async fn email_templates(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::email_templates(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/email_templates.html",
        &state.config.site,
        data,
    )?))
}

pub async fn create_email_template(
    State(state): State<AppState>,
    Form(form): Form<admin_service::EmailTemplateForm>,
) -> AppResult<Redirect> {
    admin_service::create_email_template(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/email-templates")))
}

pub async fn update_email_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<admin_service::EmailTemplateForm>,
) -> AppResult<Redirect> {
    admin_service::update_email_template(&state, id, form).await?;
    Ok(Redirect::to(&state.admin_url("/email-templates")))
}

pub async fn delete_email_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Redirect> {
    admin_service::delete_email_template(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/email-templates")))
}

pub async fn restore_default_email_templates(State(state): State<AppState>) -> AppResult<Redirect> {
    admin_service::restore_default_email_templates(&state).await?;
    Ok(Redirect::to(&state.admin_url("/email-templates")))
}

pub async fn email_test(State(state): State<AppState>) -> AppResult<Html<String>> {
    let data = admin_service::email_test(&state).await?;
    Ok(Html(state.views.render(
        "admin/email_test.html",
        &state.config.site,
        data,
    )?))
}

pub async fn send_email_test(
    State(state): State<AppState>,
    Form(form): Form<admin_service::EmailTestForm>,
) -> AppResult<Redirect> {
    admin_service::send_email_test(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/email-test")))
}

pub async fn admins(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::admins(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/admins.html",
        &state.config.site,
        data,
    )?))
}

pub async fn create_admin(
    State(state): State<AppState>,
    Form(form): Form<admin_service::AdminForm>,
) -> AppResult<Redirect> {
    admin_service::create_admin(&state, form).await?;
    Ok(Redirect::to(&state.admin_url("/admins")))
}

pub async fn update_admin(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<admin_service::AdminForm>,
) -> AppResult<Redirect> {
    admin_service::update_admin(&state, id, form).await?;
    Ok(Redirect::to(&state.admin_url("/admins")))
}

pub async fn uploads(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::uploads(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/uploads.html",
        &state.config.site,
        data,
    )?))
}

pub async fn jobs(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::jobs(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/jobs.html",
        &state.config.site,
        data,
    )?))
}

pub async fn retry_job(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Redirect> {
    admin_service::retry_job(&state, id).await?;
    Ok(Redirect::to(&state.admin_url("/jobs")))
}

pub async fn notification_logs(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::notification_logs(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/notification_logs.html",
        &state.config.site,
        data,
    )?))
}

pub async fn trash(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::trash(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/trash.html",
        &state.config.site,
        data,
    )?))
}

pub async fn restore_trash(
    State(state): State<AppState>,
    Path((table, id)): Path<(String, i64)>,
) -> AppResult<Redirect> {
    admin_service::restore_trash(&state, &table, id).await?;
    Ok(Redirect::to(&state.admin_url("/trash")))
}

pub async fn audit_logs(
    State(state): State<AppState>,
    Query(page): Query<admin_service::PageParams>,
) -> AppResult<Html<String>> {
    let data = admin_service::audit_logs(&state, page).await?;
    Ok(Html(state.views.render(
        "admin/audit_logs.html",
        &state.config.site,
        data,
    )?))
}

pub async fn backup(State(state): State<AppState>) -> AppResult<Html<String>> {
    let data = backup_service::page_data(&state).await?;
    Ok(Html(state.views.render(
        "admin/backup.html",
        &state.config.site,
        data,
    )?))
}

pub async fn create_backup(State(state): State<AppState>) -> AppResult<Redirect> {
    backup_service::create_manual_backup(&state).await?;
    Ok(Redirect::to(&state.admin_url("/backup")))
}

pub async fn download_backup_file(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> AppResult<Response> {
    let (filename, compressed) = backup_service::read_stored_backup(&state, &filename).await?;
    backup_response(filename, compressed)
}

fn backup_response(filename: String, compressed: Vec<u8>) -> AppResult<Response> {
    let mut response = compressed.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/gzip".parse().map_err(|err| {
            AppError::Anyhow(anyhow::anyhow!("invalid content-type header: {err}"))
        })?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"")
            .parse()
            .map_err(|err| {
                AppError::Anyhow(anyhow::anyhow!("invalid disposition header: {err}"))
            })?,
    );
    Ok(response)
}

pub async fn save_backup_settings(
    State(state): State<AppState>,
    Form(form): Form<BackupSettingsForm>,
) -> AppResult<Redirect> {
    backup_service::save_config(
        &state,
        backup_service::BackupConfig {
            enabled: form.enabled.is_some(),
            weekday: form.weekday,
            hour: form.hour,
            keep_files: form.keep_files,
        },
    )
    .await?;
    Ok(Redirect::to(&state.admin_url("/backup")))
}

pub async fn cleanup_uploads(State(state): State<AppState>) -> AppResult<Redirect> {
    let _ = admin_service::cleanup_uploads(&state).await?;
    Ok(Redirect::to(&state.admin_url("/uploads")))
}

pub async fn cleanup_runtime(State(state): State<AppState>) -> AppResult<Redirect> {
    admin_service::cleanup_runtime(&state).await?;
    Ok(Redirect::to(&state.admin_url("/jobs")))
}

pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Redirect> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::BadRequest(format!("上传解析失败: {err}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().unwrap_or("upload.bin").to_string();
        let mime = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !is_allowed_image(&ext, &mime) {
            return Err(AppError::BadRequest("只允许上传图片文件".to_string()));
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|err| AppError::BadRequest(format!("读取上传文件失败: {err}")))?;
        if bytes.len() > 5 * 1024 * 1024 {
            return Err(AppError::BadRequest("图片不能超过 5MB".to_string()));
        }
        let dir = state.config.uploads_dir().join("admin");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        let stored = format!("{}.{}", uuid::Uuid::new_v4().simple(), ext);
        let relative_path = format!("admin/{}", stored);
        let disk_path = dir.join(&stored);
        let mut file = tokio::fs::File::create(&disk_path)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        file.write_all(&bytes)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        admin_service::record_media(&state, &relative_path, &mime, bytes.len() as i64).await?;
        return Ok(Redirect::to(&state.admin_url("/uploads")));
    }
    Err(AppError::BadRequest("没有找到上传文件".to_string()))
}

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
