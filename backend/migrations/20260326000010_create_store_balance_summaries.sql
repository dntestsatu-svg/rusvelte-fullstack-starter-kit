-- 20260326000010_create_store_balance_summaries.sql
CREATE TABLE IF NOT EXISTS store_balance_summaries (
    store_id UUID PRIMARY KEY REFERENCES stores(id),
    pending_balance BIGINT NOT NULL DEFAULT 0,
    settled_balance BIGINT NOT NULL DEFAULT 0,
    reserved_settled_balance BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
