use async_trait::async_trait;

use crate::shared::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateQrisRequest {
    pub username: String,
    pub amount: i64,
    pub expire_seconds: i64,
    pub custom_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedQris {
    pub provider_trx_id: String,
    pub qris_payload: String,
}

#[async_trait]
pub trait QrisProviderGateway: Send + Sync {
    async fn generate_qris(&self, request: GenerateQrisRequest) -> Result<GeneratedQris, AppError>;
}
