use std::sync::Arc;

use axum::{
    body::Body,
    extract::{FromRef, FromRequestParts, State},
    http::{header, request::Parts, Request, StatusCode},
    middleware::Next,
    response::Response,
    RequestPartsExt,
};

use crate::bootstrap::state::AppState;
use crate::modules::store_tokens::application::service::StoreTokenService;
use crate::modules::store_tokens::domain::entity::StoreApiTokenAuthContext;

#[axum::async_trait]
impl<S> FromRequestParts<S> for StoreApiTokenAuthContext
where
    S: Send + Sync,
    Arc<StoreTokenService>: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let service = parts
            .extract_with_state::<State<Arc<StoreTokenService>>, S>(state)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Service unavailable".to_string(),
                )
            })?;
        let token = extract_bearer_token(parts.headers.get(header::AUTHORIZATION))
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Missing bearer token".to_string()))?;

        service.0.resolve_bearer_token(token).await.map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid store API token".to_string(),
            )
        })
    }
}

pub async fn store_client_auth_middleware(
    State(service): State<Arc<StoreTokenService>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = extract_bearer_token(req.headers().get(header::AUTHORIZATION))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let context = service
        .resolve_bearer_token(token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(context);
    Ok(next.run(req).await)
}

pub fn extract_bearer_token(header_value: Option<&axum::http::HeaderValue>) -> Option<&str> {
    let value = header_value?.to_str().ok()?;
    value.strip_prefix("Bearer ")
}

impl FromRef<AppState> for Arc<StoreTokenService> {
    fn from_ref(state: &AppState) -> Self {
        state.store_token_service.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Json, Router,
    };
    use chrono::Utc;
    use serde_json::{json, Value};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;
    use crate::modules::store_tokens::application::service::{
        CreateStoreApiTokenRequest, StoreTokenService,
    };
    use crate::modules::store_tokens::domain::entity::{
        is_store_api_token_expired, NewStoreApiTokenRecord, StoreApiTokenRecord,
    };
    use crate::modules::store_tokens::domain::repository::StoreTokenRepository;
    use crate::shared::audit::{AuditLogEntry, AuditLogRepository};
    use crate::shared::auth::{AuthenticatedUser, PlatformRole, StoreRole};

    #[derive(Default)]
    struct MockStoreTokenRepository {
        stores: Mutex<HashSet<Uuid>>,
        tokens: Mutex<Vec<StoreApiTokenRecord>>,
    }

    #[async_trait]
    impl StoreTokenRepository for MockStoreTokenRepository {
        async fn store_exists(&self, store_id: Uuid) -> anyhow::Result<bool> {
            Ok(self.stores.lock().unwrap().contains(&store_id))
        }

        async fn list_active_tokens(
            &self,
            store_id: Uuid,
        ) -> anyhow::Result<Vec<StoreApiTokenRecord>> {
            let now = Utc::now();
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .iter()
                .filter(|token| {
                    token.store_id == store_id
                        && token.revoked_at.is_none()
                        && !is_store_api_token_expired(token.expires_at, now)
                })
                .cloned()
                .collect())
        }

        async fn insert_token(
            &self,
            token: NewStoreApiTokenRecord,
        ) -> anyhow::Result<StoreApiTokenRecord> {
            let record = StoreApiTokenRecord {
                id: token.id,
                store_id: token.store_id,
                name: token.name,
                token_prefix: token.token_prefix,
                token_hash: token.token_hash,
                last_used_at: None,
                expires_at: token.expires_at,
                revoked_at: None,
                created_by: token.created_by,
                created_at: token.created_at,
            };
            self.tokens.lock().unwrap().push(record.clone());
            Ok(record)
        }

        async fn find_token_by_lookup_prefix(
            &self,
            token_prefix: &str,
        ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .iter()
                .find(|token| token.token_prefix == token_prefix)
                .cloned())
        }

        async fn find_token_by_id(
            &self,
            store_id: Uuid,
            token_id: Uuid,
        ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .iter()
                .find(|token| token.store_id == store_id && token.id == token_id)
                .cloned())
        }

        async fn mark_token_revoked(
            &self,
            store_id: Uuid,
            token_id: Uuid,
            revoked_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<bool> {
            let mut tokens = self.tokens.lock().unwrap();
            let Some(token) = tokens
                .iter_mut()
                .find(|token| token.store_id == store_id && token.id == token_id)
            else {
                return Ok(false);
            };

            if token.revoked_at.is_some() {
                return Ok(false);
            }

            token.revoked_at = Some(revoked_at);
            Ok(true)
        }

        async fn touch_last_used_at(
            &self,
            token_id: Uuid,
            last_used_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<()> {
            if let Some(token) = self
                .tokens
                .lock()
                .unwrap()
                .iter_mut()
                .find(|token| token.id == token_id)
            {
                token.last_used_at = Some(last_used_at);
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockAuditRepository;

    #[async_trait]
    impl AuditLogRepository for MockAuditRepository {
        async fn insert(&self, _entry: AuditLogEntry) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn owner_actor(store_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: std::collections::HashMap::from([(store_id, StoreRole::Owner)]),
        }
    }

    fn build_service() -> (Arc<StoreTokenService>, Arc<MockStoreTokenRepository>, Uuid) {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreTokenRepository::default());
        repository.stores.lock().unwrap().insert(store_id);
        let audit_repository = Arc::new(MockAuditRepository);
        let service = Arc::new(StoreTokenService::new(repository.clone(), audit_repository));

        (service, repository, store_id)
    }

    fn protected_router(service: Arc<StoreTokenService>) -> Router {
        Router::new()
            .route("/probe", get(protected_probe))
            .layer(middleware::from_fn_with_state(
                service.clone(),
                store_client_auth_middleware,
            ))
            .with_state(service)
    }

    async fn protected_probe(context: StoreApiTokenAuthContext) -> Json<Value> {
        Json(json!({
            "store_id": context.store_id,
            "token_id": context.token_id,
            "token_identifier": context.token_identifier,
        }))
    }

    #[tokio::test]
    async fn bearer_middleware_resolves_store_context() {
        let (service, repository, store_id) = build_service();
        let actor = owner_actor(store_id);
        let created = service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Client Token".into(),
                },
                &actor,
            )
            .await
            .unwrap();
        let app = protected_router(service.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/probe")
                    .header(
                        "Authorization",
                        format!("Bearer {}", created.plaintext_token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["store_id"], json!(store_id));
        assert_eq!(
            payload["token_identifier"],
            json!(created.token.display_prefix)
        );

        let stored = repository
            .find_token_by_lookup_prefix(
                &crate::modules::store_tokens::application::service::extract_lookup_prefix(
                    &created.plaintext_token,
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(stored.last_used_at.is_some());
    }

    #[tokio::test]
    async fn revoked_token_is_rejected_by_bearer_middleware() {
        let (service, _repository, store_id) = build_service();
        let actor = owner_actor(store_id);
        let created = service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Revoked Client Token".into(),
                },
                &actor,
            )
            .await
            .unwrap();

        let listed = service.list_tokens(store_id, &actor).await.unwrap();
        let token_id = listed[0].id;
        service
            .revoke_token(store_id, token_id, &actor)
            .await
            .unwrap();

        let app = protected_router(service);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/probe")
                    .header(
                        "Authorization",
                        format!("Bearer {}", created.plaintext_token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
