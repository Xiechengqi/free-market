CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_categories_active_sort ON categories(is_active, sort_order);

CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    short_description TEXT NOT NULL DEFAULT '',
    keywords TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    image_path TEXT NOT NULL DEFAULT '',
    retail_price_cents INTEGER NOT NULL DEFAULT 0,
    price_cents INTEGER NOT NULL DEFAULT 0,
    wholesale_prices_json TEXT NOT NULL DEFAULT '[]',
    fulfillment_type TEXT NOT NULL DEFAULT 'auto',
    manual_form_schema_json TEXT NOT NULL DEFAULT '[]',
    manual_stock_total INTEGER NOT NULL DEFAULT 0,
    manual_stock_locked INTEGER NOT NULL DEFAULT 0,
    manual_stock_sold INTEGER NOT NULL DEFAULT 0,
    buy_limit_num INTEGER NOT NULL DEFAULT 0,
    buy_prompt TEXT NOT NULL DEFAULT '',
    api_hook TEXT NOT NULL DEFAULT '',
    is_active INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY(category_id) REFERENCES categories(id)
);

CREATE INDEX IF NOT EXISTS idx_products_category_active_sort ON products(category_id, is_active, sort_order);

CREATE TABLE IF NOT EXISTS product_skus (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    sku_code TEXT NOT NULL DEFAULT 'DEFAULT',
    spec_values_json TEXT NOT NULL DEFAULT '{}',
    price_cents INTEGER NOT NULL DEFAULT 0,
    manual_stock_total INTEGER NOT NULL DEFAULT 0,
    manual_stock_locked INTEGER NOT NULL DEFAULT 0,
    manual_stock_sold INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    UNIQUE(product_id, sku_code),
    FOREIGN KEY(product_id) REFERENCES products(id)
);

CREATE TABLE IF NOT EXISTS card_secret_batches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    sku_id INTEGER NOT NULL DEFAULT 0,
    name TEXT NOT NULL,
    total_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY(product_id) REFERENCES products(id)
);

CREATE TABLE IF NOT EXISTS card_secrets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    sku_id INTEGER NOT NULL DEFAULT 0,
    batch_id INTEGER,
    secret TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'available',
    is_loop INTEGER NOT NULL DEFAULT 0,
    order_id INTEGER,
    reserved_at TEXT,
    used_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY(product_id) REFERENCES products(id),
    FOREIGN KEY(batch_id) REFERENCES card_secret_batches(id)
);

CREATE INDEX IF NOT EXISTS idx_card_secret_reserve ON card_secrets(product_id, sku_id, status, id);
CREATE INDEX IF NOT EXISTS idx_card_secret_order ON card_secrets(order_id);
