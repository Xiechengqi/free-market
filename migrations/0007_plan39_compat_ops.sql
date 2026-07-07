ALTER TABLE payment_channels ADD COLUMN pay_check TEXT NOT NULL DEFAULT '';
ALTER TABLE payment_channels ADD COLUMN client_scope TEXT NOT NULL DEFAULT 'all';
ALTER TABLE payment_channels ADD COLUMN handleroute TEXT NOT NULL DEFAULT '';
ALTER TABLE payment_channels ADD COLUMN deleted_at TEXT;

ALTER TABLE products ADD COLUMN payment_channel_ids_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE email_templates ADD COLUMN is_system INTEGER NOT NULL DEFAULT 0;
ALTER TABLE email_templates ADD COLUMN deleted_at TEXT;

CREATE TABLE IF NOT EXISTS captcha_challenges (
    id TEXT PRIMARY KEY,
    answer_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_captcha_challenges_expires ON captcha_challenges(expires_at);

CREATE TABLE IF NOT EXISTS admin_audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    admin_id INTEGER,
    method TEXT NOT NULL DEFAULT '',
    path TEXT NOT NULL DEFAULT '',
    action TEXT NOT NULL DEFAULT '',
    ip TEXT NOT NULL DEFAULT '',
    user_agent TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_audit_created ON admin_audit_logs(created_at);

CREATE TABLE IF NOT EXISTS admin_login_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL,
    ip TEXT NOT NULL DEFAULT '',
    success INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_login_attempts_user_created ON admin_login_attempts(username, created_at);

CREATE TABLE IF NOT EXISTS api_hook_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    http_status INTEGER NOT NULL DEFAULT 0,
    response_body TEXT NOT NULL DEFAULT '',
    error TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(order_id) REFERENCES orders(id),
    FOREIGN KEY(product_id) REFERENCES products(id)
);

CREATE INDEX IF NOT EXISTS idx_api_hook_logs_order ON api_hook_logs(order_id);

CREATE TABLE IF NOT EXISTS product_payment_channels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    payment_channel_id INTEGER NOT NULL,
    UNIQUE(product_id, payment_channel_id),
    FOREIGN KEY(product_id) REFERENCES products(id),
    FOREIGN KEY(payment_channel_id) REFERENCES payment_channels(id)
);
