-- 20260326000016_create_provider_balance_snapshots.sql
CREATE TABLE IF NOT EXISTS provider_balance_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_pending_balance BIGINT NOT NULL,
    provider_settle_balance BIGINT NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
