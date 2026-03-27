-- 20260326000003_create_store_members.sql
CREATE TABLE IF NOT EXISTS store_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id UUID NOT NULL REFERENCES stores(id),
    user_id UUID NOT NULL REFERENCES users(id),
    store_role TEXT NOT NULL CHECK (store_role IN ('owner', 'manager', 'staff', 'viewer')),
    status TEXT NOT NULL, -- Ambiguous status values, no CHECK constraint enforced
    invited_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (store_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_store_members_user_status ON store_members (user_id, status);
