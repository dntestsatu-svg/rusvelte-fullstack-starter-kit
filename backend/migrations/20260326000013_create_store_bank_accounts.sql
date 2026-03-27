-- 20260326000013_create_store_bank_accounts.sql
CREATE TABLE IF NOT EXISTS store_bank_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    owner_user_id UUID NOT NULL REFERENCES users(id),
    bank_code TEXT NOT NULL,
    bank_name TEXT NOT NULL,
    account_holder_name TEXT NOT NULL,
    account_number_encrypted TEXT NOT NULL,
    account_number_last4 TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    verification_status TEXT NOT NULL, -- Ambiguous status values, no CHECK constraint enforced
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_store_bank_accounts_store_default ON store_bank_accounts (store_id, is_default);
