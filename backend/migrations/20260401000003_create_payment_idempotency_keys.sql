CREATE TABLE IF NOT EXISTS payment_idempotency_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    idempotency_key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'completed')),
    response_status_code INTEGER,
    response_body_json JSONB,
    payment_id UUID REFERENCES payments(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (store_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_payment_idempotency_store_created
    ON payment_idempotency_keys (store_id, created_at DESC);
