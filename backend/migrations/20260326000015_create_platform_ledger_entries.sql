-- 20260326000015_create_platform_ledger_entries.sql
CREATE TABLE IF NOT EXISTS platform_ledger_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    related_type TEXT NOT NULL, -- Ambiguous mapping, no CHECK constraint enforced
    related_id UUID,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('payment_platform_fee_income', 'payout_platform_fee_income', 'manual_adjustment')),
    amount BIGINT NOT NULL,
    direction TEXT NOT NULL, -- Ambiguous direction values, no CHECK constraint enforced
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
