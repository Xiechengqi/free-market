CREATE TABLE IF NOT EXISTS admin_refresh_tokens (
    jti TEXT PRIMARY KEY,
    admin_id INTEGER NOT NULL,
    issued_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_admin_refresh_tokens_admin
    ON admin_refresh_tokens(admin_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_admin_refresh_tokens_expires
    ON admin_refresh_tokens(expires_at);
