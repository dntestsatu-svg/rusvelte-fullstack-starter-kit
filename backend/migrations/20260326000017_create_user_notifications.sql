-- 20260326000017_create_user_notifications.sql
CREATE TABLE IF NOT EXISTS user_notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    type TEXT NOT NULL, -- Ambiguous type values, no CHECK constraint enforced
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    related_type TEXT,
    related_id UUID,
    status TEXT NOT NULL CHECK (status IN ('unread', 'read')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_user_notifications_user_status_created ON user_notifications (user_id, status, created_at DESC);
