CREATE TABLE IF NOT EXISTS evm_payment_intents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_id INTEGER NOT NULL UNIQUE,
    chain_id INTEGER NOT NULL,
    chain_slug TEXT NOT NULL,
    token_symbol TEXT NOT NULL,
    token_contract TEXT NOT NULL,
    token_decimals INTEGER NOT NULL,
    receive_address TEXT NOT NULL,
    amount_scaled INTEGER NOT NULL,
    amount_text TEXT NOT NULL,
    amount_precision INTEGER NOT NULL,
    status TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    matched_tx_hash TEXT NOT NULL DEFAULT '',
    matched_log_index TEXT NOT NULL DEFAULT '',
    matched_from_address TEXT NOT NULL DEFAULT '',
    matched_at TEXT,
    last_checked_at TEXT,
    last_error TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(payment_id) REFERENCES payments(id)
);

CREATE INDEX IF NOT EXISTS idx_evm_intents_status_expires
    ON evm_payment_intents(status, expires_at);
CREATE INDEX IF NOT EXISTS idx_evm_intents_lookup
    ON evm_payment_intents(chain_id, token_contract, receive_address, amount_scaled, status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_evm_intents_pending_lock
    ON evm_payment_intents(chain_id, token_contract, receive_address, amount_scaled, amount_precision)
    WHERE status IN ('pending', 'processing');

CREATE TABLE IF NOT EXISTS evm_seen_transfers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    intent_id INTEGER NOT NULL,
    status TEXT NOT NULL,
    chain_id INTEGER NOT NULL,
    token_contract TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    log_index TEXT NOT NULL DEFAULT '',
    from_address TEXT NOT NULL DEFAULT '',
    to_address TEXT NOT NULL,
    amount_scaled INTEGER NOT NULL,
    amount_text TEXT NOT NULL,
    block_number INTEGER NOT NULL DEFAULT 0,
    tx_time TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(intent_id) REFERENCES evm_payment_intents(id),
    UNIQUE(chain_id, token_contract, tx_hash, log_index)
);
