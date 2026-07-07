CREATE TABLE IF NOT EXISTS coupons (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL DEFAULT 'fixed',
    value_cents INTEGER NOT NULL DEFAULT 0,
    min_amount_cents INTEGER NOT NULL DEFAULT 0,
    usage_limit INTEGER NOT NULL DEFAULT 0,
    used_count INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS coupon_products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    coupon_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    UNIQUE(coupon_id, product_id),
    FOREIGN KEY(coupon_id) REFERENCES coupons(id),
    FOREIGN KEY(product_id) REFERENCES products(id)
);

CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_no TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'CNY',
    guest_email TEXT NOT NULL,
    guest_password TEXT NOT NULL DEFAULT '',
    client_ip TEXT NOT NULL DEFAULT '',
    original_amount_cents INTEGER NOT NULL DEFAULT 0,
    coupon_discount_cents INTEGER NOT NULL DEFAULT 0,
    wholesale_discount_cents INTEGER NOT NULL DEFAULT 0,
    total_amount_cents INTEGER NOT NULL DEFAULT 0,
    coupon_id INTEGER,
    payment_channel_id INTEGER,
    legacy_info TEXT NOT NULL DEFAULT '',
    expires_at TEXT NOT NULL,
    paid_at TEXT,
    canceled_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(coupon_id) REFERENCES coupons(id)
);

CREATE INDEX IF NOT EXISTS idx_orders_status_expires ON orders(status, expires_at);
CREATE INDEX IF NOT EXISTS idx_orders_email_created ON orders(guest_email, created_at);

CREATE TABLE IF NOT EXISTS order_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    sku_id INTEGER NOT NULL DEFAULT 0,
    product_name TEXT NOT NULL,
    unit_price_cents INTEGER NOT NULL,
    quantity INTEGER NOT NULL,
    total_price_cents INTEGER NOT NULL,
    fulfillment_type TEXT NOT NULL,
    manual_form_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    FOREIGN KEY(order_id) REFERENCES orders(id),
    FOREIGN KEY(product_id) REFERENCES products(id)
);

CREATE INDEX IF NOT EXISTS idx_order_items_order ON order_items(order_id);

CREATE TABLE IF NOT EXISTS coupon_usages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    coupon_id INTEGER NOT NULL,
    order_id INTEGER NOT NULL,
    discount_cents INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'reserved',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(coupon_id, order_id),
    FOREIGN KEY(coupon_id) REFERENCES coupons(id),
    FOREIGN KEY(order_id) REFERENCES orders(id)
);

CREATE TABLE IF NOT EXISTS payment_channels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    channel_type TEXT NOT NULL,
    interaction_mode TEXT NOT NULL DEFAULT 'redirect',
    config_json TEXT NOT NULL DEFAULT '{}',
    is_active INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_no TEXT NOT NULL UNIQUE,
    order_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    provider_type TEXT NOT NULL,
    channel_type TEXT NOT NULL,
    interaction_mode TEXT NOT NULL,
    amount_cents INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'CNY',
    status TEXT NOT NULL,
    provider_ref TEXT NOT NULL DEFAULT '',
    gateway_order_no TEXT NOT NULL DEFAULT '',
    pay_url TEXT NOT NULL DEFAULT '',
    qr_code TEXT NOT NULL DEFAULT '',
    provider_payload_json TEXT NOT NULL DEFAULT '{}',
    paid_at TEXT,
    expired_at TEXT,
    callback_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(order_id) REFERENCES orders(id),
    FOREIGN KEY(channel_id) REFERENCES payment_channels(id)
);

CREATE INDEX IF NOT EXISTS idx_payments_order_status ON payments(order_id, status);
CREATE INDEX IF NOT EXISTS idx_payments_gateway_order_no ON payments(gateway_order_no);
CREATE INDEX IF NOT EXISTS idx_payments_provider_ref ON payments(provider_ref);

CREATE TABLE IF NOT EXISTS fulfillments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id INTEGER NOT NULL UNIQUE,
    type TEXT NOT NULL,
    status TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '',
    logistics_json TEXT NOT NULL DEFAULT '{}',
    delivered_by INTEGER,
    delivered_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(order_id) REFERENCES orders(id)
);

CREATE TABLE IF NOT EXISTS email_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token TEXT NOT NULL UNIQUE,
    subject TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
