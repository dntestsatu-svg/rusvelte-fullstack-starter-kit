-- 20260326000011_create_store_balance_ledger_entries.sql
CREATE TABLE IF NOT EXISTS store_balance_ledger_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    related_type TEXT NOT NULL,
    related_id UUID,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('payment_success_credit_pending', 'settlement_move_pending_to_settled', 'payout_reserve_settled', 'payout_success_debit_settled', 'payout_failed_release_reserve', 'manual_adjustment')),
    amount BIGINT NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('credit', 'debit')),
    balance_bucket TEXT NOT NULL CHECK (balance_bucket IN ('pending', 'settled', 'reserved')),
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
