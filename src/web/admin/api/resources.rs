use axum::{
    Extension,
    body::Body,
    extract::{Multipart, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::{
    error::AppError,
    services::{admin_service, backup_service},
    state::AppState,
    web::admin::api::{ApiError, ApiResponse, ApiResult, middleware::AuthContext},
};

#[derive(Debug, Deserialize, Default)]
pub struct PaginationQuery {
    pub current: Option<i64>,
    pub size: Option<i64>,
}

fn page_params(q: PaginationQuery) -> admin_service::PageParams {
    admin_service::PageParams {
        page: Some(q.current.unwrap_or(1).max(1)),
        per_page: Some(q.size.unwrap_or(20).clamp(1, 200)),
    }
}

fn role_gate(ctx: &AuthContext, mutating: bool, owner_only: bool) -> Result<(), ApiError> {
    if owner_only && ctx.role != "owner" {
        return Err(ApiError::forbidden("仅限 owner 操作"));
    }
    if mutating && ctx.role == "viewer" {
        return Err(ApiError::forbidden("viewer 仅可查看"));
    }
    Ok(())
}

// ---------------- Dashboard ----------------

pub async fn get_dashboard(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
) -> ApiResult<admin_service::DashboardData> {
    Ok(ApiResponse::ok(admin_service::dashboard(&state).await?))
}

// ---------------- Orders ----------------

pub async fn list_orders(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(filter): Query<admin_service::AdminOrdersFilter>,
) -> ApiResult<admin_service::AdminOrdersData> {
    Ok(ApiResponse::ok(
        admin_service::orders(&state, filter).await?,
    ))
}

pub async fn get_order(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<admin_service::AdminOrderData> {
    Ok(ApiResponse::ok(admin_service::order(&state, id).await?))
}

#[derive(Debug, Deserialize)]
pub struct FulfillBody {
    pub payload: String,
}

pub async fn fulfill_order(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(body): axum::Json<FulfillBody>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::fulfill(&state, id, body.payload, ctx.admin_id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn cancel_order(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::cancel_order(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn resend_order_email(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::resend_order_email(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn mark_order_abnormal(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::mark_order_abnormal(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn delete_order(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::soft_delete_order(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn start_order_processing(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::start_order_processing(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmEvmIntentBody {
    pub tx_hash: String,
}

pub async fn confirm_evm_intent(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path((order_id, intent_id)): axum::extract::Path<(i64, i64)>,
    axum::Json(body): axum::Json<ConfirmEvmIntentBody>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::confirm_evm_intent(&state, order_id, intent_id, body.tx_hash).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Categories ----------------

pub async fn list_categories(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::CategoriesData> {
    Ok(ApiResponse::ok(
        admin_service::categories(&state, page_params(q)).await?,
    ))
}

pub async fn create_category(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::CategoryForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::create_category(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn update_category(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(form): axum::Json<admin_service::CategoryForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::update_category(&state, id, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn delete_category(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::delete_category(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Products ----------------

pub async fn list_products(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::ProductsData> {
    Ok(ApiResponse::ok(
        admin_service::products(&state, page_params(q)).await?,
    ))
}

pub async fn create_product(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::ProductForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::create_product(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn update_product(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(form): axum::Json<admin_service::ProductForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::update_product(&state, id, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn delete_product(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::delete_product(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Coupons ----------------

pub async fn list_coupons(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::CouponsData> {
    Ok(ApiResponse::ok(
        admin_service::coupons(&state, page_params(q)).await?,
    ))
}

pub async fn create_coupon(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::CouponForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::create_coupon(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn update_coupon(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(form): axum::Json<admin_service::CouponForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::update_coupon(&state, id, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn delete_coupon(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::delete_coupon(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Payment channels ----------------

pub async fn list_payment_channels(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::PaymentChannelsData> {
    Ok(ApiResponse::ok(
        admin_service::payment_channels(&state, page_params(q)).await?,
    ))
}

pub async fn create_payment_channel(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::PaymentChannelForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::create_payment_channel(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn update_payment_channel(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(form): axum::Json<admin_service::PaymentChannelForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::update_payment_channel(&state, id, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn delete_payment_channel(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::delete_payment_channel(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

#[derive(Debug, serde::Serialize)]
pub struct ValidatePaymentChannelResponse {
    pub ok: bool,
}

pub async fn validate_payment_channel(
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::PaymentChannelForm>,
) -> ApiResult<ValidatePaymentChannelResponse> {
    role_gate(&ctx, true, true)?;
    admin_service::validate_payment_channel_form(&form)
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(ApiResponse::ok(ValidatePaymentChannelResponse { ok: true }))
}

pub async fn evm_payment_presets(
    Extension(_ctx): Extension<AuthContext>,
) -> ApiResult<serde_json::Value> {
    Ok(ApiResponse::ok(serde_json::json!({
        "chains": crate::services::evm_local_service::chain_presets()
    })))
}

// ---------------- Settings ----------------

pub async fn get_settings(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
) -> ApiResult<admin_service::SettingsData> {
    Ok(ApiResponse::ok(admin_service::settings(&state).await?))
}

pub async fn save_settings(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::SettingsForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::save_settings(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Email templates ----------------

pub async fn list_email_templates(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::EmailTemplatesData> {
    Ok(ApiResponse::ok(
        admin_service::email_templates(&state, page_params(q)).await?,
    ))
}

pub async fn create_email_template(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::EmailTemplateForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::create_email_template(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn update_email_template(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(form): axum::Json<admin_service::EmailTemplateForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::update_email_template(&state, id, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn delete_email_template(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::delete_email_template(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn restore_default_email_templates(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::restore_default_email_templates(&state).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Admins ----------------

pub async fn list_admins(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::AdminsData> {
    role_gate(&ctx, false, true)?;
    Ok(ApiResponse::ok(
        admin_service::admins(&state, page_params(q)).await?,
    ))
}

pub async fn create_admin(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::AdminForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::create_admin(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn update_admin(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(form): axum::Json<admin_service::AdminForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::update_admin(&state, id, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Jobs / Notifications / Audit / Trash ----------------

pub async fn list_jobs(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::JobsData> {
    Ok(ApiResponse::ok(
        admin_service::jobs(&state, page_params(q)).await?,
    ))
}

pub async fn retry_job(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::retry_job(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn cleanup_runtime(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    admin_service::cleanup_runtime(&state).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn list_notification_logs(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::NotificationLogsData> {
    Ok(ApiResponse::ok(
        admin_service::notification_logs(&state, page_params(q)).await?,
    ))
}

pub async fn list_audit_logs(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::AuditLogsData> {
    role_gate(&ctx, false, true)?;
    Ok(ApiResponse::ok(
        admin_service::audit_logs(&state, page_params(q)).await?,
    ))
}

pub async fn list_trash(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::TrashData> {
    Ok(ApiResponse::ok(
        admin_service::trash(&state, page_params(q)).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct TrashRestorePath {
    pub table: String,
    pub id: i64,
}

pub async fn restore_trash(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path((table, id)): axum::extract::Path<(String, i64)>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::restore_trash(&state, &table, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Cards / Carmis ----------------

#[derive(Debug, Deserialize)]
pub struct CardsListQuery {
    pub status: Option<String>,
    pub is_loop: Option<String>,
    pub current: Option<i64>,
    pub size: Option<i64>,
}

fn cards_filter(q: &CardsListQuery) -> admin_service::CardsFilter {
    admin_service::CardsFilter {
        status: q.status.clone(),
        is_loop: q.is_loop.clone(),
        page: Some(q.current.unwrap_or(1).max(1)),
        per_page: Some(q.size.unwrap_or(20).clamp(1, 200)),
    }
}

pub async fn list_product_cards(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    axum::extract::Path(product_id): axum::extract::Path<i64>,
    Query(q): Query<CardsListQuery>,
) -> ApiResult<admin_service::CardsData> {
    Ok(ApiResponse::ok(
        admin_service::cards(&state, product_id, cards_filter(&q)).await?,
    ))
}

pub async fn import_product_cards(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(product_id): axum::extract::Path<i64>,
    axum::Json(form): axum::Json<admin_service::ImportCardsForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::import_cards(&state, product_id, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

pub async fn export_product_cards(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    axum::extract::Path(product_id): axum::extract::Path<i64>,
    Query(q): Query<CardsListQuery>,
) -> Result<Response, ApiError> {
    let body = admin_service::export_cards(&state, product_id, cards_filter(&q)).await?;
    let filename = format!("cards-{}.txt", product_id);
    Ok(text_download(filename, body))
}

pub async fn delete_product_card(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path((product_id, card_id)): axum::extract::Path<(i64, i64)>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::delete_card(&state, product_id, card_id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

#[derive(Debug, Deserialize)]
pub struct GlobalCardsListQuery {
    pub product_id: Option<i64>,
    pub status: Option<String>,
    pub is_loop: Option<String>,
    pub keyword: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub current: Option<i64>,
    pub size: Option<i64>,
}

fn global_cards_filter(q: &GlobalCardsListQuery) -> admin_service::GlobalCardsFilter {
    admin_service::GlobalCardsFilter {
        product_id: q.product_id,
        status: q.status.clone(),
        is_loop: q.is_loop.clone(),
        keyword: q.keyword.clone(),
        date_from: q.date_from.clone(),
        date_to: q.date_to.clone(),
        page: Some(q.current.unwrap_or(1).max(1)),
        per_page: Some(q.size.unwrap_or(20).clamp(1, 200)),
    }
}

pub async fn list_global_cards(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<GlobalCardsListQuery>,
) -> ApiResult<admin_service::GlobalCardsData> {
    Ok(ApiResponse::ok(
        admin_service::global_cards(&state, global_cards_filter(&q)).await?,
    ))
}

pub async fn export_global_cards_csv(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<GlobalCardsListQuery>,
) -> Result<Response, ApiError> {
    let body = admin_service::export_global_cards(&state, global_cards_filter(&q)).await?;
    Ok(text_download("cards.txt".to_string(), body))
}

pub async fn delete_global_card(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::delete_global_card(&state, id).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Uploads ----------------

pub async fn list_uploads(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(q): Query<PaginationQuery>,
) -> ApiResult<admin_service::UploadsData> {
    Ok(ApiResponse::ok(
        admin_service::uploads(&state, page_params(q)).await?,
    ))
}

#[derive(Debug, serde::Serialize)]
pub struct UploadedFileResponse {
    pub path: String,
    pub url: String,
    pub mime: String,
    pub size_bytes: i64,
}

pub async fn upload_file(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    mut multipart: Multipart,
) -> ApiResult<UploadedFileResponse> {
    role_gate(&ctx, true, false)?;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::bad_request(format!("上传解析失败: {err}")))?
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
        if !crate::web::admin::is_allowed_image(&ext, &mime) {
            return Err(ApiError::bad_request("只允许上传图片文件"));
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|err| ApiError::bad_request(format!("读取上传文件失败: {err}")))?;
        if bytes.len() > 5 * 1024 * 1024 {
            return Err(ApiError::bad_request("图片不能超过 5MB"));
        }
        let dir = state.config.uploads_dir().join("admin");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|err| ApiError::internal(format!("创建目录失败: {err}")))?;
        let stored = format!("{}.{}", uuid::Uuid::new_v4().simple(), ext);
        let relative_path = format!("admin/{}", stored);
        let disk_path = dir.join(&stored);
        let mut file = tokio::fs::File::create(&disk_path)
            .await
            .map_err(|err| ApiError::internal(format!("创建文件失败: {err}")))?;
        file.write_all(&bytes)
            .await
            .map_err(|err| ApiError::internal(format!("写入文件失败: {err}")))?;
        let size = bytes.len() as i64;
        admin_service::record_media(&state, &relative_path, &mime, size).await?;
        let url = format!("/uploads/{}", relative_path);
        return Ok(ApiResponse::ok(UploadedFileResponse {
            path: relative_path,
            url,
            mime,
            size_bytes: size,
        }));
    }
    Err(ApiError::bad_request("没有找到上传文件"))
}

pub async fn cleanup_uploads(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    let removed = admin_service::cleanup_uploads(&state).await?;
    Ok(ApiResponse::ok(serde_json::json!({ "removed": removed })))
}

// ---------------- Email test ----------------

pub async fn get_email_test(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
) -> ApiResult<admin_service::EmailTestData> {
    Ok(ApiResponse::ok(admin_service::email_test(&state).await?))
}

pub async fn send_email_test(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(form): axum::Json<admin_service::EmailTestForm>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, false)?;
    admin_service::send_email_test(&state, form).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Backup ----------------

pub async fn get_backup_page(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<backup_service::BackupPageData> {
    role_gate(&ctx, false, true)?;
    Ok(ApiResponse::ok(backup_service::page_data(&state).await?))
}

#[derive(Debug, serde::Serialize)]
pub struct BackupCreatedResponse {
    pub filename: String,
}

pub async fn create_backup(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<BackupCreatedResponse> {
    role_gate(&ctx, true, true)?;
    let path = backup_service::create_manual_backup(&state).await?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("backup.gz")
        .to_string();
    Ok(ApiResponse::ok(BackupCreatedResponse { filename }))
}

pub async fn download_backup(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Result<Response, ApiError> {
    role_gate(&ctx, false, true)?;
    let (filename, bytes) = backup_service::read_stored_backup(&state, &filename).await?;
    let mut response = bytes.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, "application/gzip".parse().unwrap());
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );
    Ok(response)
}

pub async fn save_backup_settings(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::Json(config): axum::Json<backup_service::BackupConfig>,
) -> ApiResult<serde_json::Value> {
    role_gate(&ctx, true, true)?;
    backup_service::save_config(&state, config).await?;
    Ok(ApiResponse::ok(serde_json::Value::Null))
}

// ---------------- Orders export ----------------

pub async fn export_orders(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthContext>,
    Query(filter): Query<admin_service::AdminOrdersFilter>,
) -> Result<Response, ApiError> {
    let csv = admin_service::export_orders(&state, filter).await?;
    let mut response = Response::new(Body::from(csv));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/csv; charset=utf-8".parse().unwrap(),
    );
    let stamp = crate::time::now_str().replace(':', "-");
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"orders-{}.csv\"", stamp)
            .parse()
            .unwrap(),
    );
    Ok(response)
}

// ---------------- shared helpers ----------------

fn text_download(filename: String, body: String) -> Response {
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/plain; charset=utf-8".parse().unwrap(),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );
    response
}

impl From<AppError> for Response {
    fn from(value: AppError) -> Self {
        let api_err: ApiError = value.into();
        api_err.into_response()
    }
}

// Provide an IntoResponse impl for direct usage when a handler returns Response
// but needs StatusCode 404 fallback semantics.
#[allow(dead_code)]
fn not_found_response(msg: impl Into<String>) -> Response {
    (StatusCode::NOT_FOUND, msg.into()).into_response()
}
