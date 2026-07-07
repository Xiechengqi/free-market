ALTER TABLE evm_payment_intents ADD COLUMN scan_from_block INTEGER NOT NULL DEFAULT 0;
ALTER TABLE evm_payment_intents ADD COLUMN last_scanned_block INTEGER NOT NULL DEFAULT 0;
