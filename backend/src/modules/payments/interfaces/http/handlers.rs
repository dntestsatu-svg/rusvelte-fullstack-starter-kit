use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::bootstrap::state::AppState;
use crate::infrastructure::security::limiter::SlidingWindowLimiter;
use crate::modules::payments::application::idempotency::PaymentIdempotencyLookup;
use crate::modules::payments::application::service::CreateClientPaymentRequest;
use crate::modules::payments::domain::entity::{
    ClientPaymentDetail, ClientPaymentStatusView, CLIENT_PAYMENT_CREATE_RATE_LIMIT,
    CLIENT_PAYMENT_CREATE_RATE_WINDOW_SECONDS,
};
use crate::modules::store_tokens::domain::entity::StoreApiTokenAuthContext;
use crate::shared::error::AppError;

#[derive(Debug, Serialize)]
pub struct ClientPaymentDetailResponse {
    pub payment: ClientPaymentDetail,
}

#[derive(Debug, Serialize)]
pub struct ClientPaymentStatusResponse {
    pub payment: ClientPaymentStatusView,
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
