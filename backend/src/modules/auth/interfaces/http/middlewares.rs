use crate::infrastructure::security::csrf::verify_csrf_token;
use crate::infrastructure::security::limiter::SlidingWindowLimiter;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::auth::application::service::AuthService;
use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{request::Parts, Method, Request, StatusCode},
    middleware::Next,
    response::Response,
    RequestPartsExt,
};
use axum_extra::extract::CookieJar;
use std::sync::Arc;
use uuid::Uuid;

#[axum::async_trait]
impl<S> FromRequestParts<S> for SessionContext
where
    S: Send + Sync,
    Arc<AuthService>: axum::extract::FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Missing cookies".to_string()))?;

        let session_id = jar
            .get("session_id")
            .map(|c| c.value().to_string())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    "Missing session cookie".to_string(),
                )
            })?;

        let session_uuid = Uuid::parse_str(&session_id)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid session id".to_string()))?;

        // In Axum 0.7, we can extract State explicitly from the parts and state
        let service = parts
            .extract_with_state::<State<Arc<AuthService>>, S>(state)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Service unavailable".to_string(),
                )
            })?;

        let context = service
            .0
            .resolve_session(session_uuid)
            .await
            .map_err(|e: anyhow::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    "Session expired or invalid".to_string(),
                )
            })?;

        Ok(context)
    }
}

pub async fn csrf_middleware(
    ctx: Option<axum::Extension<SessionContext>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = req.method();
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let token = req
        .headers()
        .get("X-CSRF-Token")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // If no context, we can't verify CSRF
    let ctx = match ctx {
        Some(axum::Extension(c)) => c,
        None => return Err(StatusCode::FORBIDDEN),
    };

    match token {
        Some(t) if verify_csrf_token(&t, &ctx.csrf_token) => Ok(next.run(req).await),
        _ => Err(StatusCode::FORBIDDEN),
    }
}

pub async fn session_middleware(
    State(service): State<Arc<AuthService>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let jar = CookieJar::from_headers(req.headers());

    let session_id = jar.get("session_id").map(|c| c.value().to_string());

    if let Some(id) = session_id {
        if let Ok(uuid) = Uuid::parse_str(&id) {
            if let Ok(Some(ctx)) = service.resolve_session(uuid).await {
                req.extensions_mut().insert(ctx);
            }
        }
    }

    Ok(next.run(req).await)
}

pub async fn rate_limit_middleware(
    State(service): State<Arc<AuthService>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // In a real app with Axum, we'd use ConnectInfo or similar to get the IP.
    // For this implementation, we'll use a placeholder or session-based limit if session exists.
    let key = "global_ip";

    // We'll use the services redis pool
    let mut conn = service
        .get_redis_conn()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 100 requests per 60 seconds for general routes
    let allowed = SlidingWindowLimiter::is_allowed(&mut *conn, key, 100, 60)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !allowed {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(next.run(req).await)
}
