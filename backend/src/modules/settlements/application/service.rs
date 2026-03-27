use std::sync::Arc;

use crate::modules::settlements::domain::entity::{
    ProcessSettlementCommand, ProcessedSettlement, SettlementRequest,
};
use crate::modules::settlements::domain::repository::{
    SettlementProcessError, SettlementRepository,
};
use crate::shared::auth::{has_capability, AuthenticatedUser, Capability};
use crate::shared::error::AppError;

pub struct SettlementService {
    repository: Arc<dyn SettlementRepository>,
}

impl SettlementService {
    pub fn new(repository: Arc<dyn SettlementRepository>) -> Self {
        Self { repository }
    }

    pub async fn process_settlement(
        &self,
        request: SettlementRequest,
        actor: &AuthenticatedUser,
    ) -> Result<ProcessedSettlement, AppError> {
        if !has_capability(actor, Capability::SettlementCreate, None) {
            return Err(AppError::Forbidden(
                "You do not have permission to process settlements".into(),
            ));
        }

        if request.amount <= 0 {
            return Err(AppError::BadRequest(
                "Settlement amount must be greater than zero".into(),
            ));
        }

        self.repository
            .process_settlement(ProcessSettlementCommand {
                store_id: request.store_id,
                amount: request.amount,
                notes: normalize_notes(request.notes),
                processed_by_user_id: actor.user_id,
                processed_at: chrono::Utc::now(),
            })
            .await
            .map_err(map_process_error)
    }
}

fn normalize_notes(notes: Option<String>) -> Option<String> {
    notes.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn map_process_error(error: SettlementProcessError) -> AppError {
    match error {
        SettlementProcessError::StoreNotFound(store_id) => {
            AppError::NotFound(format!("Store {store_id} was not found"))
        }
        SettlementProcessError::InsufficientPendingBalance { available, requested } => {
            AppError::Conflict(format!(
                "Settlement amount exceeds pending balance. Available: {available}, requested: {requested}"
            ))
        }
        SettlementProcessError::Unexpected(error) => AppError::Internal(error),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use anyhow::anyhow;
    use async_trait::async_trait;
    use uuid::Uuid;
    use super::*;
    use crate::modules::balances::domain::entity::StoreBalanceSnapshot;
    use crate::modules::settlements::domain::entity::{
        SettlementRecord, SettlementStatus,
    };
    use crate::shared::auth::PlatformRole;

    #[derive(Default)]
    struct MockSettlementRepository {
        commands: Mutex<Vec<ProcessSettlementCommand>>,
        result: Mutex<Option<Result<ProcessedSettlement, SettlementProcessError>>>,
    }

    #[async_trait]
    impl SettlementRepository for MockSettlementRepository {
        async fn process_settlement(
            &self,
            command: ProcessSettlementCommand,
        ) -> Result<ProcessedSettlement, SettlementProcessError> {
            self.commands.lock().unwrap().push(command.clone());
            self.result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| {
                    Ok(ProcessedSettlement {
                        settlement: SettlementRecord {
                            id: Uuid::new_v4(),
                            store_id: command.store_id,
                            amount: command.amount,
                            status: SettlementStatus::Processed,
                            processed_by_user_id: command.processed_by_user_id,
                            notes: command.notes,
                            created_at: command.processed_at,
                        },
                        balance: StoreBalanceSnapshot {
                            store_id: command.store_id,
                            pending_balance: 0,
                            settled_balance: command.amount,
                            reserved_settled_balance: 0,
                            withdrawable_balance: command.amount,
                            updated_at: command.processed_at,
                        },
                        notification_user_ids: vec![],
                    })
                })
        }
    }

    fn dev_actor() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Dev,
            memberships: HashMap::new(),
        }
    }

    fn superadmin_actor() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Superadmin,
            memberships: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn rejects_non_positive_amount() {
        let service = SettlementService::new(Arc::new(MockSettlementRepository::default()));

        let error = service
            .process_settlement(
                SettlementRequest {
                    store_id: Uuid::new_v4(),
                    amount: 0,
                    notes: None,
                },
                &dev_actor(),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn rejects_non_dev_actor() {
        let service = SettlementService::new(Arc::new(MockSettlementRepository::default()));

        let error = service
            .process_settlement(
                SettlementRequest {
                    store_id: Uuid::new_v4(),
                    amount: 1_000,
                    notes: None,
                },
                &superadmin_actor(),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn maps_insufficient_pending_to_conflict() {
        let repository = Arc::new(MockSettlementRepository::default());
        repository.result.lock().unwrap().replace(Err(
            SettlementProcessError::InsufficientPendingBalance {
                available: 500,
                requested: 1_000,
            },
        ));
        let service = SettlementService::new(repository);

        let error = service
            .process_settlement(
                SettlementRequest {
                    store_id: Uuid::new_v4(),
                    amount: 1_000,
                    notes: None,
                },
                &dev_actor(),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Conflict(_)));
    }

    #[tokio::test]
    async fn trims_notes_before_repository_call() {
        let repository = Arc::new(MockSettlementRepository::default());
        let service = SettlementService::new(repository.clone());

        let _ = service
            .process_settlement(
                SettlementRequest {
                    store_id: Uuid::new_v4(),
                    amount: 2_000,
                    notes: Some("  processed by dev  ".into()),
                },
                &dev_actor(),
            )
            .await
            .unwrap();

        let commands = repository.commands.lock().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].notes.as_deref(), Some("processed by dev"));
    }

    #[tokio::test]
    async fn maps_unexpected_errors_to_internal() {
        let repository = Arc::new(MockSettlementRepository::default());
        repository
            .result
            .lock()
            .unwrap()
            .replace(Err(SettlementProcessError::Unexpected(anyhow!(
                "broken write path"
            ))));
        let service = SettlementService::new(repository);

        let error = service
            .process_settlement(
                SettlementRequest {
                    store_id: Uuid::new_v4(),
                    amount: 2_000,
                    notes: None,
                },
                &dev_actor(),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Internal(_)));
    }
}
