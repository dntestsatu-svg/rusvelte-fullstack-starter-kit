use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use std::sync::Arc;
use crate::modules::auth::application::service::AuthService;
use crate::modules::auth::application::dto::{LoginRequest, SessionContext};
use crate::infrastructure::security::csrf::generate_csrf_token;
use uuid::Uuid;

pub async fn login(
    State(service): State<Arc<AuthService>>,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<(CookieJar, Json<crate::modules::auth::application::dto::LoginResponse>), (StatusCode, String)> {
    let res = service.login(payload).await.map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    
    // Find the session_id from the logic - I need to adjust LoginResponse to include session_id 
    // OR just fetch it from the service state if it returns it. 
    // Actually, I'll modify LoginResponse to carry it for a moment or just make Login return (Response, Uuid)
    
    // For now, I'll re-resolve it or just trust the service to return it.
    // Let's assume the service login returns it. I'll fix dto.rs and service.rs to return session_id.
    
    // Wait, let's just make the service return (LoginResponse, Uuid)
    Ok((jar, Json(res)))
}

// I'll fix service.rs to return session_id in LoginResponse.
// Actually, I'll just write the handlers assuming the service is updated.
// I'll update dto and service in a moment.

pub async fn logout(
    State(service): State<Arc<AuthService>>,
    jar: CookieJar,
) -> Result<CookieJar, StatusCode> {
    if let Some(cookie) = jar.get("session_id") {
        if let Ok(id) = Uuid::parse_str(cookie.value()) {
            let _ = service.logout(id).await;
        }
    }
    
    let mut cookie = Cookie::from("session_id");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_max_age(time::Duration::ZERO);
    
    Ok(jar.remove(cookie))
}

pub async fn me(
    axum::extract::Extension(ctx): axum::extract::Extension<SessionContext>,
) -> Json<crate::modules::auth::application::dto::UserProfile> {
    Json(ctx.user)
}

pub async fn get_csrf(
    axum::extract::Extension(ctx): axum::extract::Extension<SessionContext>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ 
        "status": "ok",
        "csrf_token": ctx.csrf_token 
    }))
}
