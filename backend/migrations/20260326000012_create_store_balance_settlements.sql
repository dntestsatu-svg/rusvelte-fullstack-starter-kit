-- 20260326000012_create_store_balance_settlements.sql
CREATE TABLE IF NOT EXISTS store_balance_settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    amount BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('processed')),
    processed_by_user_id UUID NOT NULL REFERENCES users(id),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
