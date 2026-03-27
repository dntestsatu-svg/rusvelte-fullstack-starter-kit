-- 20260326000006_create_contact_thread_messages.sql
CREATE TABLE IF NOT EXISTS contact_thread_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id UUID NOT NULL REFERENCES contact_threads(id),
    sender_type TEXT NOT NULL CHECK (sender_type IN ('guest', 'staff')),
    sender_user_id UUID REFERENCES users(id),
    body TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
