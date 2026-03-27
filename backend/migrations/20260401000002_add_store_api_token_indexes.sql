CREATE UNIQUE INDEX IF NOT EXISTS idx_store_api_tokens_token_prefix
    ON store_api_tokens (token_prefix);

CREATE INDEX IF NOT EXISTS idx_store_api_tokens_store_active_created_at
    ON store_api_tokens (store_id, created_at DESC)
    WHERE revoked_at IS NULL;
