-- 20260326000014_create_store_payout_requests.sql
CREATE TABLE IF NOT EXISTS store_payout_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    bank_account_id UUID NOT NULL REFERENCES store_bank_accounts(id),
    requested_by_user_id UUID NOT NULL REFERENCES users(id),
    requested_amount BIGINT NOT NULL,
    platform_withdraw_fee_bps INTEGER NOT NULL,
    platform_withdraw_fee_amount BIGINT NOT NULL,
    provider_withdraw_fee_amount BIGINT NOT NULL,
    net_disbursed_amount BIGINT NOT NULL,
    provider_partner_ref_no TEXT,
    provider_inquiry_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('previewed', 'pending_provider', 'processing', 'success', 'failed', 'cancelled')),
    failure_reason TEXT,
    provider_transaction_date TIMESTAMPTZ,
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_store_payout_requests_store_status_created ON store_payout_requests (store_id, status, created_at DESC);
