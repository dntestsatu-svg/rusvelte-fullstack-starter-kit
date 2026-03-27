-- 20260401000001_seed_dev_user.sql
-- Idempotent seed for exactly 1 dev user
INSERT INTO users (id, name, email, password_hash, role, status)
SELECT 
    '00000000-0000-0000-0000-000000000000',
    'Developer',
    'dev@justqiu.com',
    -- Hashed 'dev12345' using Argon2id
    '$argon2id$v=19$m=19456,t=2,p=1$fijzBv5di6YBIjdA5ZcjFg$dTClkV8ejmXmxr/ZaKbqKlEkQtleanDApN6knkJW8hI', 
    'dev',
    'active'
WHERE NOT EXISTS (SELECT 1 FROM users WHERE role = 'dev')
ON CONFLICT (email) DO NOTHING;
