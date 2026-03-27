-- 20260326000007_create_payments.sql
CREATE TABLE IF NOT EXISTS payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    created_by_user_id UUID REFERENCES users(id),
    provider_name TEXT NOT NULL,
    provider_trx_id TEXT,
    provider_rrn TEXT,
    merchant_order_id TEXT,
    custom_ref TEXT,
    gross_amount BIGINT NOT NULL,
    platform_tx_fee_bps INTEGER NOT NULL,
    platform_tx_fee_amount BIGINT NOT NULL,
    store_pending_credit_amount BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('created', 'pending', 'success', 'failed', 'expired')),
    qris_payload TEXT,
    expired_at TIMESTAMPTZ,
    provider_created_at TIMESTAMPTZ,
    provider_finished_at TIMESTAMPTZ,
    finalized_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (store_id, merchant_order_id)
);

CREATE INDEX IF NOT EXISTS idx_payments_store_created ON payments (store_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_payments_store_status_created ON payments (store_id, status, created_at DESC);
