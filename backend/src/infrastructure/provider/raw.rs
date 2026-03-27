use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct GenerateQrisProviderRequest<'a> {
    pub username: &'a str,
    pub amount: i64,
    pub uuid: &'a str,
    pub expire: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_ref: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateQrisProviderResponse {
    pub status: bool,
    pub data: Option<String>,
    pub trx_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckPaymentStatusProviderRequest<'a> {
    pub uuid: &'a str,
    pub client: &'a str,
    pub client_key: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CheckPaymentStatusProviderResponse {
    Success(CheckPaymentStatusSuccessResponse),
    Failure(ProviderBooleanFailureResponse),
}

#[derive(Debug, Deserialize)]
pub struct CheckPaymentStatusSuccessResponse {
    pub amount: i64,
    pub merchant_id: String,
    pub trx_id: String,
    pub rrn: Option<String>,
    pub status: String,
    pub created_at: Option<DateTime<Utc>>,
    pub finish_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct InquiryBankProviderRequest<'a> {
    pub client: &'a str,
    pub client_key: &'a str,
    pub uuid: &'a str,
    pub amount: i64,
    pub bank_code: &'a str,
    pub account_number: &'a str,
    #[serde(rename = "type")]
    pub transfer_type: i32,
}

#[derive(Debug, Deserialize)]
pub struct InquiryBankSuccessEnvelope {
    pub status: bool,
    pub data: Option<InquiryBankSuccessData>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InquiryBankSuccessData {
    pub account_number: String,
    pub account_name: String,
    pub bank_code: String,
    pub bank_name: String,
    pub partner_ref_no: String,
    pub vendor_ref_no: Option<String>,
    pub amount: i64,
    pub fee: i64,
    pub inquiry_id: i64,
}

#[derive(Debug, Serialize)]
pub struct TransferProviderRequest<'a> {
    pub client: &'a str,
    pub client_key: &'a str,
    pub uuid: &'a str,
    pub amount: i64,
    pub bank_code: &'a str,
    pub account_number: &'a str,
    #[serde(rename = "type")]
    pub transfer_type: i32,
    pub inquiry_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct TransferProviderResponse {
    pub status: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckDisbursementStatusProviderRequest<'a> {
    pub client: &'a str,
    pub uuid: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CheckDisbursementStatusProviderResponse {
    Success(CheckDisbursementStatusSuccessResponse),
    Failure(ProviderBooleanFailureResponse),
}

#[derive(Debug, Deserialize)]
pub struct CheckDisbursementStatusSuccessResponse {
    pub amount: i64,
    pub fee: i64,
    pub partner_ref_no: String,
    pub merchant_uuid: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct GetBalanceProviderRequest<'a> {
    pub client: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum GetBalanceProviderResponse {
    Success(GetBalanceSuccessResponse),
    Failure(ProviderBooleanFailureResponse),
}

#[derive(Debug, Deserialize)]
pub struct GetBalanceSuccessResponse {
    pub status: String,
    pub pending_balance: i64,
    pub settle_balance: i64,
}

#[derive(Debug, Deserialize)]
pub struct ProviderBooleanFailureResponse {
    pub status: bool,
    pub error: Option<String>,
}
