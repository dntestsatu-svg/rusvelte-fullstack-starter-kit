-- 20260326000009_create_provider_webhook_events.sql
CREATE TABLE IF NOT EXISTS provider_webhook_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_name TEXT NOT NULL,
    webhook_kind TEXT NOT NULL CHECK (webhook_kind IN ('payment', 'payout', 'unknown')),
    merchant_id TEXT,
    provider_trx_id TEXT,
    partner_ref_no TEXT,
    payload_json JSONB NOT NULL,
    is_verified BOOLEAN NOT NULL DEFAULT FALSE,
    verification_reason TEXT,
    is_processed BOOLEAN NOT NULL DEFAULT FALSE,
    processing_result TEXT,
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_provider_webhook_merchant_created ON provider_webhook_events (merchant_id, created_at DESC);
