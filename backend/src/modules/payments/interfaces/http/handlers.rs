use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::bootstrap::state::AppState;
use crate::infrastructure::security::limiter::SlidingWindowLimiter;
use crate::modules::payments::application::idempotency::PaymentIdempotencyLookup;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::payments::application::service::{
    CreateClientPaymentRequest, DashboardPaymentListFilters, PaymentWebhookInput,
    PayoutWebhookInput, ProviderWebhookInput, UnknownWebhookInput,
};
use crate::modules::payments::domain::entity::{
    ClientPaymentDetail, ClientPaymentStatusView, DashboardPaymentDetail,
    DashboardPaymentDistribution, DashboardPaymentSummary, PaymentStatus, PaymentWebhookStatus,
    CLIENT_PAYMENT_CREATE_RATE_LIMIT,
    CLIENT_PAYMENT_CREATE_RATE_WINDOW_SECONDS,
};
use crate::modules::store_tokens::domain::entity::StoreApiTokenAuthContext;
use crate::shared::error::AppError;
use crate::shared::pagination::PaginationParams;

#[derive(Debug, Serialize)]
pub struct ClientPaymentDetailResponse {
    pub payment: ClientPaymentDetail,
}

#[derive(Debug, Serialize)]
pub struct ClientPaymentStatusResponse {
    pub payment: ClientPaymentStatusView,
}

#[derive(Debug, Deserialize)]
pub struct DashboardPaymentListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DashboardPaymentListResponse {
    pub payments: Vec<DashboardPaymentSummary>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Serialize)]
pub struct DashboardPaymentDetailResponse {
    pub payment: DashboardPaymentDetail,
}

#[derive(Debug, Serialize)]
pub struct DashboardPaymentDistributionResponse {
    pub distribution: DashboardPaymentDistribution,
}

#[derive(Debug, Serialize)]
pub struct ProviderWebhookResponse {
    pub status: bool,
}

#[derive(Debug, Deserialize)]
struct PaymentWebhookPayload {
    amount: i64,
    terminal_id: String,
    merchant_id: String,
    trx_id: String,
    rrn: Option<String>,
    custom_ref: Option<String>,
    _vendor: Option<String>,
    status: String,
    created_at: Option<String>,
    finish_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PayoutWebhookPayload {
    amount: i64,
    partner_ref_no: String,
    status: String,
    transaction_date: Option<String>,
    merchant_id: String,
}

pub async fn create_qris_payment(
    State(state): State<AppState>,
    Extension(context): Extension<StoreApiTokenAuthContext>,
    headers: HeaderMap,
    payload: Result<Json<CreateClientPaymentRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let payload = payload
        .map(|Json(body)| body)
        .map_err(|_| AppError::BadRequest("Invalid request body".into()))?;
    let idempotency_key = extract_idempotency_key(&headers)?;
    let normalized_request = payload.normalize()?;
    let request_hash = state
        .payment_idempotency_service
        .hash_request(&normalized_request)?;

    match state
        .payment_idempotency_service
        .lookup(context.store_id, idempotency_key, &request_hash)
        .await?
    {
        PaymentIdempotencyLookup::Cached(cached) => {
            return build_json_response(cached.status_code, cached.body_json);
        }
        PaymentIdempotencyLookup::Mismatch => {
            return Err(AppError::Conflict(
                "Idempotency key cannot be reused with a different request body".into(),
            ));
        }
        PaymentIdempotencyLookup::Pending => {
            return Err(AppError::Conflict(
                "Idempotent request is already being processed".into(),
            ));
        }
        PaymentIdempotencyLookup::Missing => {}
    }

    enforce_create_payment_rate_limit(&state, &context).await?;

    if let Err(error) = state
        .payment_idempotency_service
        .reserve(context.store_id, idempotency_key, &request_hash)
        .await
    {
        if matches!(error, AppError::Conflict(_)) {
            return match state
                .payment_idempotency_service
                .lookup(context.store_id, idempotency_key, &request_hash)
                .await?
            {
                PaymentIdempotencyLookup::Cached(cached) => {
                    build_json_response(cached.status_code, cached.body_json)
                }
                PaymentIdempotencyLookup::Mismatch => Err(AppError::Conflict(
                    "Idempotency key cannot be reused with a different request body".into(),
                )),
                PaymentIdempotencyLookup::Pending | PaymentIdempotencyLookup::Missing => Err(
                    AppError::Conflict("Idempotent request is already being processed".into()),
                ),
            };
        }

        return Err(error);
    }

    match state
        .payment_service
        .create_qris_payment(&context, normalized_request)
        .await
    {
        Ok(payment) => {
            let response_body = ClientPaymentDetailResponse { payment };
            let response_json = serde_json::to_value(&response_body)
                .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;
            state
                .payment_idempotency_service
                .complete(
                    context.store_id,
                    idempotency_key,
                    StatusCode::CREATED.as_u16(),
                    response_json.clone(),
                    Some(response_body.payment.id),
                )
                .await?;

            build_json_response(StatusCode::CREATED.as_u16(), response_json)
        }
        Err(error) => {
            let response_json = error.body_json();
            let status_code = error.status_code().as_u16();
            state
                .payment_idempotency_service
                .complete(
                    context.store_id,
                    idempotency_key,
                    status_code,
                    response_json,
                    None,
                )
                .await?;

            Err(error)
        }
    }
}

pub async fn get_payment(
    State(state): State<AppState>,
    Extension(context): Extension<StoreApiTokenAuthContext>,
    Path(payment_id): Path<Uuid>,
) -> Result<Json<ClientPaymentDetailResponse>, AppError> {
    let payment = state
        .payment_service
        .get_payment_detail(context.store_id, payment_id)
        .await?;

    Ok(Json(ClientPaymentDetailResponse { payment }))
}

pub async fn get_payment_status(
    State(state): State<AppState>,
    Extension(context): Extension<StoreApiTokenAuthContext>,
    Path(payment_id): Path<Uuid>,
) -> Result<Json<ClientPaymentStatusResponse>, AppError> {
    let payment = state
        .payment_service
        .get_payment_status(context.store_id, payment_id)
        .await?;

    Ok(Json(ClientPaymentStatusResponse { payment }))
}

pub async fn list_dashboard_payments(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Query(query): Query<DashboardPaymentListQuery>,
) -> Result<Json<DashboardPaymentListResponse>, AppError> {
    let actor = resolve_dashboard_actor(&state, ctx).await?;
    let params = query.pagination.normalize();
    let filters = DashboardPaymentListFilters {
        search: query.search,
        status: query
            .status
            .as_deref()
            .map(parse_dashboard_status)
            .transpose()?,
    };
    let total = state
        .payment_service
        .count_dashboard_payments(filters.clone(), &actor)
        .await?;
    let payments = state
        .payment_service
        .list_dashboard_payments(
            params.per_page as i64,
            params.offset() as i64,
            filters,
            &actor,
        )
        .await?;

    Ok(Json(DashboardPaymentListResponse {
        payments,
        total,
        page: params.page,
        per_page: params.per_page,
    }))
}

pub async fn get_dashboard_payment(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(payment_id): Path<Uuid>,
) -> Result<Json<DashboardPaymentDetailResponse>, AppError> {
    let actor = resolve_dashboard_actor(&state, ctx).await?;
    let payment = state
        .payment_service
        .get_dashboard_payment(payment_id, &actor)
        .await?;

    Ok(Json(DashboardPaymentDetailResponse { payment }))
}

pub async fn get_dashboard_payment_distribution(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
) -> Result<Json<DashboardPaymentDistributionResponse>, AppError> {
    let actor = resolve_dashboard_actor(&state, ctx).await?;
    let distribution = state
        .payment_service
        .get_dashboard_payment_distribution(&actor)
        .await?;

    Ok(Json(DashboardPaymentDistributionResponse { distribution }))
}

pub async fn provider_webhook(
    State(state): State<AppState>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<(StatusCode, Json<ProviderWebhookResponse>), AppError> {
    let raw_payload = payload
        .map(|Json(body)| body)
        .map_err(|_| AppError::BadRequest("Invalid webhook body".into()))?;
    let normalized = parse_provider_webhook(raw_payload)?;
    let result = state
        .payment_service
        .process_provider_webhook(normalized, &state.config.external_api_uuid)
        .await?;

    if result.publish_payment_event {
        if let Some(payment) = &result.payment {
            state.realtime_service.publish_payment_updated(
                payment.store_id,
                payment.id,
                &payment.status.to_string(),
            );
        }
    }

    if result.publish_notification_event {
        if let Some(payment) = &result.payment {
            state.realtime_service.publish_notification_created(
                result.notification_user_ids.clone(),
                Some("payment"),
                Some(payment.id),
            );
        }
    }

    Ok((
        StatusCode::OK,
        Json(ProviderWebhookResponse {
            status: result.response_status,
        }),
    ))
}

async fn enforce_create_payment_rate_limit(
    state: &AppState,
    context: &StoreApiTokenAuthContext,
) -> Result<(), AppError> {
    let key = format!("client_payment_create:store:{}", context.store_id);
    let mut conn = state
        .redis
        .get()
        .await
        .map_err(|error| AppError::Redis(format!("Could not get Redis connection: {error}")))?;
    let allowed = SlidingWindowLimiter::is_allowed(
        &mut *conn,
        &key,
        CLIENT_PAYMENT_CREATE_RATE_LIMIT,
        CLIENT_PAYMENT_CREATE_RATE_WINDOW_SECONDS,
    )
    .await
    .map_err(|error| AppError::Redis(format!("Rate limit check failed: {error}")))?;

    if !allowed {
        return Err(AppError::TooManyRequests(
            "Create payment rate limit exceeded".into(),
        ));
    }

    Ok(())
}

fn extract_idempotency_key(headers: &HeaderMap) -> Result<&str, AppError> {
    headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("Idempotency-Key header is required".into()))
}

fn build_json_response(
    status_code: u16,
    body_json: serde_json::Value,
) -> Result<Response, AppError> {
    let status = StatusCode::from_u16(status_code).map_err(|error| {
        AppError::Internal(anyhow::anyhow!(format!(
            "Invalid cached response status code: {error}"
        )))
    })?;

    Ok((status, Json(body_json)).into_response())
}

async fn resolve_dashboard_actor(
    state: &AppState,
    ctx: Option<Extension<SessionContext>>,
) -> Result<crate::shared::auth::AuthenticatedUser, AppError> {
    let session = ctx
        .map(|Extension(session)| session)
        .ok_or_else(|| AppError::Unauthorized("Session required".into()))?;
    let platform_role = crate::shared::auth::PlatformRole::from_str(&session.user.role)
        .map_err(|_| AppError::Unauthorized("Invalid session role".into()))?;

    state
        .user_service
        .build_actor(session.user.id, platform_role)
        .await
}

fn parse_dashboard_status(value: &str) -> Result<PaymentStatus, AppError> {
    PaymentStatus::from_str(value)
        .map_err(|_| AppError::BadRequest("Invalid payment status filter".into()))
}

fn parse_provider_webhook(payload: Value) -> Result<ProviderWebhookInput, AppError> {
    if payload.get("trx_id").is_some() && payload.get("terminal_id").is_some() {
        if let Ok(parsed) = serde_json::from_value::<PaymentWebhookPayload>(payload.clone()) {
            if let Ok(status) = parse_payment_webhook_status(&parsed.status) {
                return Ok(ProviderWebhookInput::Payment(PaymentWebhookInput {
                    merchant_id: parsed.merchant_id,
                    provider_trx_id: parsed.trx_id,
                    terminal_id: parsed.terminal_id,
                    custom_ref: parsed.custom_ref,
                    rrn: parsed.rrn,
                    amount: parsed.amount,
                    status,
                    provider_created_at: parse_provider_timestamp(parsed.created_at.as_deref()),
                    provider_finished_at: parse_provider_timestamp(parsed.finish_at.as_deref()),
                    raw_payload: payload,
                }));
            }
        }
    }

    if payload.get("partner_ref_no").is_some() {
        if let Ok(parsed) = serde_json::from_value::<PayoutWebhookPayload>(payload.clone()) {
            let _ = parsed.amount;
            let _ = parsed.status;
            let _ = parsed.transaction_date;

            return Ok(ProviderWebhookInput::Payout(PayoutWebhookInput {
                merchant_id: Some(parsed.merchant_id),
                partner_ref_no: Some(parsed.partner_ref_no),
                raw_payload: payload,
            }));
        }
    }

    Ok(ProviderWebhookInput::Unknown(UnknownWebhookInput {
        merchant_id: payload
            .get("merchant_id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        provider_trx_id: payload
            .get("trx_id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        partner_ref_no: payload
            .get("partner_ref_no")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        raw_payload: payload,
    }))
}

fn parse_payment_webhook_status(value: &str) -> Result<PaymentWebhookStatus, AppError> {
    match value.trim().to_lowercase().as_str() {
        "success" => Ok(PaymentWebhookStatus::Success),
        "failed" => Ok(PaymentWebhookStatus::Failed),
        "expired" => Ok(PaymentWebhookStatus::Expired),
        _ => Err(AppError::BadRequest("Unsupported payment webhook status".into())),
    }
}

fn parse_provider_timestamp(value: Option<&str>) -> Option<DateTime<Utc>> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Some(parsed.with_timezone(&Utc));
    }

    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|parsed| DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc))
}
