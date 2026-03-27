use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::modules::settlements::domain::entity::{
    ProcessSettlementCommand, ProcessedSettlement,
};

#[derive(Debug, Error)]
pub enum SettlementProcessError {
    #[error("store not found: {0}")]
    StoreNotFound(Uuid),
    #[error("insufficient pending balance")]
    InsufficientPendingBalance { available: i64, requested: i64 },
    #[error("unexpected settlement write error: {0}")]
    Unexpected(#[from] anyhow::Error),
}

impl Clone for SettlementProcessError {
    fn clone(&self) -> Self {
        match self {
            Self::StoreNotFound(store_id) => Self::StoreNotFound(*store_id),
            Self::InsufficientPendingBalance {
                available,
                requested,
            } => Self::InsufficientPendingBalance {
                available: *available,
                requested: *requested,
            },
            Self::Unexpected(error) => Self::Unexpected(anyhow::anyhow!(error.to_string())),
        }
    }
}

#[async_trait]
pub trait SettlementRepository: Send + Sync {
    async fn process_settlement(
        &self,
        command: ProcessSettlementCommand,
    ) -> Result<ProcessedSettlement, SettlementProcessError>;
}
