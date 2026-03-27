-- 20260326000019_create_callback_attempts.sql
CREATE TABLE IF NOT EXISTS callback_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    callback_delivery_id UUID NOT NULL REFERENCES callback_deliveries(id),
    attempt_number INTEGER NOT NULL,
    request_headers_json JSONB NOT NULL,
    request_body_json JSONB NOT NULL,
    response_status INTEGER,
    response_body_excerpt TEXT,
    error_message TEXT,
    duration_ms INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
