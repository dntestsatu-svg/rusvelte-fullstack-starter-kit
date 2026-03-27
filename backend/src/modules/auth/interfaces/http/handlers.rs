use crate::modules::auth::application::dto::{LoginRequest, SessionContext};
use crate::modules::auth::application::service::AuthService;
use axum::{extract::State, http::StatusCode, Json};
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::CookieJar;
use std::sync::Arc;
use uuid::Uuid;

pub async fn login(
    State(service): State<Arc<AuthService>>,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<
    (
        CookieJar,
        Json<crate::modules::auth::application::dto::LoginResponse>,
    ),
    (StatusCode, String),
> {
    let res = service
        .login(payload)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let mut session_cookie = Cookie::new("session_id", res.session_id.to_string());
    session_cookie.set_http_only(true);
    session_cookie.set_same_site(SameSite::Lax);
    session_cookie.set_path("/");

    let mut csrf_cookie = Cookie::new("XSRF-TOKEN", res.csrf_token.clone());
    csrf_cookie.set_http_only(false);
    csrf_cookie.set_same_site(SameSite::Lax);
    csrf_cookie.set_path("/");

    Ok((jar.add(session_cookie).add(csrf_cookie), Json(res)))
}

pub async fn logout(
    State(service): State<Arc<AuthService>>,
    jar: CookieJar,
) -> Result<CookieJar, StatusCode> {
    if let Some(cookie) = jar.get("session_id") {
        if let Ok(id) = Uuid::parse_str(cookie.value()) {
            let _ = service.logout(id).await;
        }
    }

    let mut session_cookie = Cookie::from("session_id");
    session_cookie.set_path("/");
    session_cookie.set_http_only(true);
    session_cookie.set_same_site(SameSite::Lax);
    session_cookie.set_max_age(time::Duration::ZERO);

    let mut csrf_cookie = Cookie::from("XSRF-TOKEN");
    csrf_cookie.set_path("/");
    csrf_cookie.set_same_site(SameSite::Lax);
    csrf_cookie.set_max_age(time::Duration::ZERO);

    Ok(jar.remove(session_cookie).remove(csrf_cookie))
}

pub async fn me(
    ctx: Option<axum::extract::Extension<SessionContext>>,
) -> Result<Json<crate::modules::auth::application::dto::UserProfile>, (StatusCode, String)> {
    match ctx {
        Some(axum::extract::Extension(c)) => Ok(Json(c.user)),
        None => Err((StatusCode::UNAUTHORIZED, "Session required".to_string())),
    }
}

pub async fn get_csrf(
    ctx: Option<axum::extract::Extension<SessionContext>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match ctx {
        Some(axum::extract::Extension(c)) => Ok(Json(serde_json::json!({
            "status": "ok",
            "csrf_token": c.csrf_token
        }))),
        None => Err((StatusCode::UNAUTHORIZED, "Session required".to_string())),
    }
}
