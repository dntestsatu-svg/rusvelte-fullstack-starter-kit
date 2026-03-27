use std::convert::Infallible;
use std::str::FromStr;

use async_stream::stream;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Extension,
};

use crate::bootstrap::state::AppState;
use crate::modules::auth::application::dto::SessionContext;
use crate::shared::auth::{has_capability, Capability, PlatformRole};
use crate::shared::error::AppError;

pub async fn stream_events(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>>, AppError> {
    let session = ctx
        .map(|Extension(session)| session)
        .ok_or_else(|| AppError::Unauthorized("Session required".into()))?;
    let platform_role = PlatformRole::from_str(&session.user.role)
        .map_err(|_| AppError::Unauthorized("Invalid session role".into()))?;
    let actor = state
        .user_service
        .build_actor(session.user.id, platform_role)
        .await?;
    let mut receiver = state.realtime_service.subscribe();
    let current_user_id = session.user.id;

    let event_stream = stream! {
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    if !should_deliver(&actor, current_user_id, &envelope) {
                        continue;
                    }

                    yield Ok(Event::default()
                        .event(envelope.event_type)
                        .data(envelope.payload.to_string()));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15))))
}

fn should_deliver(
    actor: &crate::shared::auth::AuthenticatedUser,
    current_user_id: uuid::Uuid,
    envelope: &crate::modules::realtime::application::service::RealtimeEventEnvelope,
) -> bool {
    if envelope
        .target_user_ids
        .iter()
        .any(|user_id| *user_id == current_user_id)
    {
        return true;
    }

    if let Some(store_id) = envelope.store_id {
        return match envelope.event_type.as_str() {
            "store.balance.updated" => has_capability(actor, Capability::BalanceRead, Some(store_id)),
            _ => has_capability(actor, Capability::PaymentRead, Some(store_id)),
        };
    }

    false
}
