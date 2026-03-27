-- 20260326000018_create_callback_deliveries.sql
CREATE TABLE IF NOT EXISTS callback_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    related_type TEXT NOT NULL CHECK (related_type IN ('payment', 'payout')),
    related_id UUID NOT NULL,
    event_type TEXT NOT NULL, -- Ambiguous event naming, no CHECK constraint enforced
    target_url TEXT NOT NULL,
    signature TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'delivering', 'success', 'failed', 'dead')),
    next_retry_at TIMESTAMPTZ,
    final_failure_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_callback_deliveries_status_retry ON callback_deliveries (status, next_retry_at);
