CREATE TABLE IF NOT EXISTS purchase_rate (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL DEFAULT '',
    client_ip TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_purchase_rate_email_created ON purchase_rate(email, created_at);
CREATE INDEX IF NOT EXISTS idx_purchase_rate_ip_created ON purchase_rate(client_ip, created_at);
