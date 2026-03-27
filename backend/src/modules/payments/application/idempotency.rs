use std::sync::Arc;

use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Error as SqlxError;
use uuid::Uuid;

use crate::modules::payments::domain::entity::{
    NewPaymentIdempotencyRecord, PaymentIdempotencyRecord, PaymentIdempotencyStatus,
};
use crate::modules::payments::domain::repository::PaymentIdempotencyRepository;
use crate::shared::error::AppError;

#[derive(Debug, Clone, PartialEq)]
pub struct CachedIdempotentResponse {
    pub status_code: u16,
    pub body_json: serde_json::Value,
    pub payment_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaymentIdempotencyLookup {
    Missing,
    Pending,
    Mismatch,
    Cached(CachedIdempotentResponse),
}

pub struct PaymentIdempotencyService {
    repository: Arc<dyn PaymentIdempotencyRepository>,
}

impl PaymentIdempotencyService {
    pub fn new(repository: Arc<dyn PaymentIdempotencyRepository>) -> Self {
        Self { repository }
    }

    pub fn hash_request<T>(&self, request: &T) -> Result<String, AppError>
    where
        T: Serialize,
    {
        let payload = serde_json::to_vec(request)
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;
        let digest = Sha256::digest(payload);
        Ok(digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>())
    }

    pub async fn lookup(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<PaymentIdempotencyLookup, AppError> {
        let Some(record) = self
            .repository
            .find_by_key(store_id, idempotency_key)
            .await?
        else {
            return Ok(PaymentIdempotencyLookup::Missing);
        };

        if record.request_hash != request_hash {
            return Ok(PaymentIdempotencyLookup::Mismatch);
        }

        match record.status {
            PaymentIdempotencyStatus::Pending => Ok(PaymentIdempotencyLookup::Pending),
            PaymentIdempotencyStatus::Completed => {
                let status_code = record.response_status_code.ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "Completed idempotency record is missing response status"
                    ))
                })?;
                let body_json = record.response_body_json.ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "Completed idempotency record is missing response body"
                    ))
                })?;

                Ok(PaymentIdempotencyLookup::Cached(CachedIdempotentResponse {
                    status_code: status_code as u16,
                    body_json,
                    payment_id: record.payment_id,
                }))
            }
        }
    }

    pub async fn reserve(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        let insert = self
            .repository
            .insert_pending(NewPaymentIdempotencyRecord {
                id: Uuid::new_v4(),
                store_id,
                idempotency_key: idempotency_key.to_string(),
                request_hash: request_hash.to_string(),
                status: PaymentIdempotencyStatus::Pending,
                created_at: now,
                updated_at: now,
            })
            .await;

        match insert {
            Ok(_) => Ok(()),
            Err(error) if is_unique_conflict(&error) => Err(AppError::Conflict(
                "Idempotency key is already being processed".into(),
            )),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn complete(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
        response_status_code: u16,
        response_body_json: serde_json::Value,
        payment_id: Option<Uuid>,
    ) -> Result<PaymentIdempotencyRecord, AppError> {
        self.repository
            .complete(
                store_id,
                idempotency_key,
                i32::from(response_status_code),
                response_body_json,
                payment_id,
                Utc::now(),
            )
            .await
            .map_err(Into::into)
    }
}

fn is_unique_conflict(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<SqlxError>()
        .and_then(|sqlx_error| match sqlx_error {
            SqlxError::Database(database_error) => {
                database_error.code().map(|code| code == "23505")
            }
            _ => None,
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;

    #[derive(Default)]
    struct MockPaymentIdempotencyRepository {
        records: Mutex<HashMap<(Uuid, String), PaymentIdempotencyRecord>>,
    }

    #[async_trait]
    impl PaymentIdempotencyRepository for MockPaymentIdempotencyRepository {
        async fn find_by_key(
            &self,
            store_id: Uuid,
            idempotency_key: &str,
        ) -> anyhow::Result<Option<PaymentIdempotencyRecord>> {
            Ok(self
                .records
                .lock()
                .unwrap()
                .get(&(store_id, idempotency_key.to_string()))
                .cloned())
        }

        async fn insert_pending(
            &self,
            record: NewPaymentIdempotencyRecord,
        ) -> anyhow::Result<PaymentIdempotencyRecord> {
            let key = (record.store_id, record.idempotency_key.clone());
            let mut records = self.records.lock().unwrap();
            if records.contains_key(&key) {
                return Err(SqlxError::Protocol("duplicate idempotency key".into()).into());
            }

            let stored = PaymentIdempotencyRecord {
                id: record.id,
                store_id: record.store_id,
                idempotency_key: record.idempotency_key,
                request_hash: record.request_hash,
                status: record.status,
                response_status_code: None,
                response_body_json: None,
                payment_id: None,
                created_at: record.created_at,
                completed_at: None,
                updated_at: record.updated_at,
            };
            records.insert(key, stored.clone());
            Ok(stored)
        }

        async fn complete(
            &self,
            store_id: Uuid,
            idempotency_key: &str,
            response_status_code: i32,
            response_body_json: serde_json::Value,
            payment_id: Option<Uuid>,
            completed_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<PaymentIdempotencyRecord> {
            let key = (store_id, idempotency_key.to_string());
            let mut records = self.records.lock().unwrap();
            let record = records
                .get_mut(&key)
                .ok_or_else(|| anyhow::anyhow!("missing idempotency record"))?;
            record.status = PaymentIdempotencyStatus::Completed;
            record.response_status_code = Some(response_status_code);
            record.response_body_json = Some(response_body_json);
            record.payment_id = payment_id;
            record.completed_at = Some(completed_at);
            record.updated_at = completed_at;
            Ok(record.clone())
        }
    }

    #[test]
    fn hash_request_is_stable() {
        let service =
            PaymentIdempotencyService::new(Arc::new(MockPaymentIdempotencyRepository::default()));
        let payload = serde_json::json!({
            "amount": 10000,
            "expire_seconds": 300,
        });

        let first = service.hash_request(&payload).unwrap();
        let second = service.hash_request(&payload).unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn lookup_returns_cached_pending_and_mismatch_states() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockPaymentIdempotencyRepository::default());
        let service = PaymentIdempotencyService::new(repository.clone());

        service
            .reserve(store_id, "same-key", "hash-a")
            .await
            .unwrap();
        let pending = service
            .lookup(store_id, "same-key", "hash-a")
            .await
            .unwrap();
        assert_eq!(pending, PaymentIdempotencyLookup::Pending);

        service
            .complete(
                store_id,
                "same-key",
                201,
                serde_json::json!({ "payment": { "id": "1" } }),
                Some(Uuid::new_v4()),
            )
            .await
            .unwrap();

        let cached = service
            .lookup(store_id, "same-key", "hash-a")
            .await
            .unwrap();
        assert!(matches!(cached, PaymentIdempotencyLookup::Cached(_)));

        let mismatch = service
            .lookup(store_id, "same-key", "hash-b")
            .await
            .unwrap();
        assert_eq!(mismatch, PaymentIdempotencyLookup::Mismatch);
    }
}
