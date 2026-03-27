CREATE UNIQUE INDEX IF NOT EXISTS idx_store_bank_accounts_single_default
ON store_bank_accounts (store_id)
WHERE is_default = TRUE;
