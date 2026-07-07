ALTER TABLE orders ADD COLUMN coupon_ret_back INTEGER NOT NULL DEFAULT 0;

ALTER TABLE products ADD COLUMN sales_volume INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_orders_coupon_ret_back ON orders(coupon_ret_back);
