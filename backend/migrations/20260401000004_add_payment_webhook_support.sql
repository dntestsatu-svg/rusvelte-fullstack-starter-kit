ALTER TABLE payments
    ADD COLUMN IF NOT EXISTS provider_terminal_id TEXT;

UPDATE payments p
SET provider_terminal_id = s.provider_username
FROM stores s
WHERE p.store_id = s.id
  AND p.provider_terminal_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_payments_provider_name_trx_id_unique
    ON payments (provider_name, provider_trx_id)
    WHERE provider_trx_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_provider_webhook_events_provider_trx_created
    ON provider_webhook_events (provider_trx_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_store_balance_ledger_payment_success_unique
    ON store_balance_ledger_entries (store_id, related_type, related_id, entry_type)
    WHERE related_type = 'payment'
      AND related_id IS NOT NULL
      AND entry_type = 'payment_success_credit_pending';

CREATE UNIQUE INDEX IF NOT EXISTS idx_platform_ledger_payment_fee_unique
    ON platform_ledger_entries (related_type, related_id, entry_type)
    WHERE related_type = 'payment'
      AND related_id IS NOT NULL
      AND entry_type = 'payment_platform_fee_income';

CREATE UNIQUE INDEX IF NOT EXISTS idx_callback_deliveries_payment_event_unique
    ON callback_deliveries (store_id, related_type, related_id, event_type)
    WHERE related_type = 'payment';
