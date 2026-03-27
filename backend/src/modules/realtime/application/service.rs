use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeEventEnvelope {
    pub event_type: String,
    pub payload: serde_json::Value,
    pub target_user_ids: Vec<Uuid>,
    pub store_id: Option<Uuid>,
}

#[derive(Clone)]
pub struct RealtimeService {
    sender: Arc<broadcast::Sender<RealtimeEventEnvelope>>,
}

impl RealtimeService {
    pub fn new(buffer_size: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer_size);
        Self {
            sender: Arc::new(sender),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeEventEnvelope> {
        self.sender.subscribe()
    }

    pub fn publish_payment_updated(
        &self,
        store_id: Uuid,
        payment_id: Uuid,
        status: &str,
    ) {
        let _ = self.sender.send(RealtimeEventEnvelope {
            event_type: "payment.updated".into(),
            payload: serde_json::json!({
                "payment_id": payment_id,
                "store_id": store_id,
                "status": status,
            }),
            target_user_ids: vec![],
            store_id: Some(store_id),
        });
    }

    pub fn publish_notification_created(
        &self,
        target_user_ids: Vec<Uuid>,
        related_type: Option<&str>,
        related_id: Option<Uuid>,
    ) {
        if target_user_ids.is_empty() {
            return;
        }

        let _ = self.sender.send(RealtimeEventEnvelope {
            event_type: "notification.created".into(),
            payload: serde_json::json!({
                "related_type": related_type,
                "related_id": related_id,
            }),
            target_user_ids,
            store_id: None,
        });
    }
}
