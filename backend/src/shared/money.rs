use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize, PartialEq)]
pub enum MoneyError {
    #[error("Amount must be greater than zero")]
    InvalidAmount,
    #[error("Amount exceeds withdrawable balance")]
    ExceedsBalance,
    #[error("Net disbursed amount must be greater than zero")]
    NegativeNet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentFeeBreakdown {
    pub gross_amount: i64,
    pub platform_fee_bps: u32,
    pub platform_fee_amount: i64,
    pub store_pending_amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawBreakdown {
    pub requested_amount: i64,
    pub platform_fee_bps: u32,
    pub platform_fee_amount: i64,
    pub provider_fee_amount: i64,
    pub net_disbursed_amount: i64,
}

pub fn percentage_fee(amount: i64, bps: i64) -> i64 {
    // Math: (amount * bps) / 10000 with floor to ensure integer safety
    (amount * bps) / 10000
}

pub fn payment_fee_breakdown(gross: i64) -> PaymentFeeBreakdown {
    let bps = 300; // 3.00%
    let platform_fee = percentage_fee(gross, bps as i64);
    let store_pending = gross - platform_fee;

    PaymentFeeBreakdown {
        gross_amount: gross,
        platform_fee_bps: bps,
        platform_fee_amount: platform_fee,
        store_pending_amount: store_pending,
    }
}

pub fn payment_success_breakdown(gross: i64) -> PaymentFeeBreakdown {
    payment_fee_breakdown(gross)
}

pub fn withdraw_breakdown(requested: i64, provider_fee: i64) -> WithdrawBreakdown {
    let bps = 1200; // 12.00%
    let platform_fee = percentage_fee(requested, bps as i64);
    let net = requested - platform_fee - provider_fee;

    WithdrawBreakdown {
        requested_amount: requested,
        platform_fee_bps: bps,
        platform_fee_amount: platform_fee,
        provider_fee_amount: provider_fee,
        net_disbursed_amount: net,
    }
}

pub fn validate_payout_guard(
    breakdown: &WithdrawBreakdown,
    withdrawable: i64,
) -> Result<(), MoneyError> {
    if breakdown.requested_amount <= 0 {
        return Err(MoneyError::InvalidAmount);
    }
    if breakdown.requested_amount > withdrawable {
        return Err(MoneyError::ExceedsBalance);
    }
    if breakdown.net_disbursed_amount <= 0 {
        return Err(MoneyError::NegativeNet);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentage_fee() {
        assert_eq!(percentage_fee(100_000, 300), 3_000);
        assert_eq!(percentage_fee(1_000_000, 1200), 120_000);
        assert_eq!(percentage_fee(50, 1200), 6);
    }

    #[test]
    fn test_payment_breakdown() {
        let b = payment_fee_breakdown(100_000);
        assert_eq!(b.platform_fee_amount, 3_000);
        assert_eq!(b.store_pending_amount, 97_000);

        let uneven = payment_fee_breakdown(10_001);
        assert_eq!(uneven.platform_fee_amount, 300);
        assert_eq!(uneven.store_pending_amount, 9_701);
    }

    #[test]
    fn test_withdraw_breakdown() {
        let b = withdraw_breakdown(1_000_000, 5_000);
        assert_eq!(b.platform_fee_amount, 120_000);
        assert_eq!(b.provider_fee_amount, 5_000);
        assert_eq!(b.net_disbursed_amount, 875_000);
    }

    #[test]
    fn test_payout_guards() {
        let b = withdraw_breakdown(1_000_000, 5_000);

        // Success
        assert!(validate_payout_guard(&b, 1_000_000).is_ok());

        // Exceeds balance
        assert_eq!(
            validate_payout_guard(&b, 500_000),
            Err(MoneyError::ExceedsBalance)
        );

        // Invalid amount
        let b_zero = withdraw_breakdown(0, 5_000);
        assert_eq!(
            validate_payout_guard(&b_zero, 1_000_000),
            Err(MoneyError::InvalidAmount)
        );

        // Negative net (fees exceed requested)
        let b_small = withdraw_breakdown(10_000, 15_000);
        assert_eq!(
            validate_payout_guard(&b_small, 100_000),
            Err(MoneyError::NegativeNet)
        );
    }
}
