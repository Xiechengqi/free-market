ALTER TABLE admins ADD COLUMN role TEXT NOT NULL DEFAULT 'owner';

CREATE INDEX IF NOT EXISTS idx_order_items_product ON order_items(product_id);
CREATE INDEX IF NOT EXISTS idx_orders_payment_channel ON orders(payment_channel_id);
CREATE INDEX IF NOT EXISTS idx_orders_created_at ON orders(created_at);
CREATE INDEX IF NOT EXISTS idx_notification_logs_created ON notification_logs(created_at);
