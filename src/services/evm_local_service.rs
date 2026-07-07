use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};

use crate::{
    error::{AppError, AppResult},
    models, money,
    services::payment_service,
    state::AppState,
    time,
};

const PROVIDER_TYPE: &str = "evm-local";
const DEFAULT_ALCHEMY_NETWORK: &str = "bnb-mainnet";
const DEFAULT_SCAN_INTERVAL_SECS: u64 = 30;
const DEFAULT_CONFIRMATIONS: i64 = 12;
const DEFAULT_AMOUNT_PRECISION: u32 = 6;
const DEFAULT_EXPIRE_MINUTES: i64 = 30;
const DEFAULT_LOG_SCAN_BLOCK_RANGE: i64 = 10;
const DEFAULT_MAX_SCAN_CHUNKS_PER_TICK: i64 = 12;
const DEFAULT_BOOTSTRAP_SCAN_BLOCKS: i64 = 2_000;
const MAX_AMOUNT_BUMPS: i64 = 200;
const PROCESSING_TIMEOUT_SECS: i64 = 600;
const ERC20_TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
const NETWORK_ENV_MAINNET: &str = "mainnet";
const NETWORK_ENV_TESTNET: &str = "testnet";

#[derive(Debug, Clone, Serialize)]
pub struct EvmTokenPreset {
    pub symbol: &'static str,
    pub contract: &'static str,
    pub decimals: u32,
    pub official: bool,
    pub note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvmChainPreset {
    pub id: &'static str,
    pub env: &'static str,
    pub label: &'static str,
    pub alchemy_network: &'static str,
    pub chain_id: i64,
    pub chain_slug: &'static str,
    pub chain_name: &'static str,
    pub scan_host: &'static str,
    pub default_confirmations: i64,
    pub default_log_scan_block_range: i64,
    pub tokens: &'static [EvmTokenPreset],
}

#[derive(Debug, Clone)]
pub struct EvmLocalConfig {
    pub alchemy_api_key: String,
    pub rpc_url: String,
    pub alchemy_network: String,
    pub network_env: String,
    pub chain_id: i64,
    pub chain_slug: String,
    pub chain_name: String,
    pub scan_host: String,
    pub token_symbol: String,
    pub token_contract: String,
    pub token_decimals: u32,
    pub confirmations: i64,
    pub amount_precision: u32,
    pub expire_minutes: i64,
    pub log_scan_block_range: i64,
    pub max_scan_chunks_per_tick: i64,
    pub bootstrap_scan_blocks: i64,
    pub allow_overpay: bool,
    pub overpay_tolerance_scaled: i64,
    pub fiat_per_token: String,
    pub addresses: Vec<String>,
}

const BSC_TOKENS: &[EvmTokenPreset] = &[
    EvmTokenPreset {
        symbol: "USDT",
        contract: "0x55d398326f99059ff775485246999027b3197955",
        decimals: 18,
        official: true,
        note: "BSC mainnet USDT",
    },
    EvmTokenPreset {
        symbol: "USDC",
        contract: "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d",
        decimals: 18,
        official: true,
        note: "BSC mainnet USDC",
    },
];

const BASE_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
    decimals: 6,
    official: true,
    note: "Base mainnet USDC",
}];

const POLYGON_TOKENS: &[EvmTokenPreset] = &[
    EvmTokenPreset {
        symbol: "USDT",
        contract: "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",
        decimals: 6,
        official: true,
        note: "Polygon PoS mainnet USDT",
    },
    EvmTokenPreset {
        symbol: "USDC",
        contract: "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
        decimals: 6,
        official: true,
        note: "Polygon PoS mainnet native USDC",
    },
];

const ARBITRUM_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
    decimals: 6,
    official: true,
    note: "Arbitrum One mainnet native USDC",
}];

const OPTIMISM_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0x0b2c639c533813f4aa9d7837caf62653d097ff85",
    decimals: 6,
    official: true,
    note: "OP Mainnet native USDC",
}];

const ETH_SEPOLIA_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238",
    decimals: 6,
    official: true,
    note: "Circle testnet USDC; no financial value",
}];

const BASE_SEPOLIA_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0x036cbd53842c5426634e7929541ec2318f3dcf7e",
    decimals: 6,
    official: true,
    note: "Circle testnet USDC; no financial value",
}];

const POLYGON_AMOY_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0x41e94eb019c0762f9bfcf9fb1e58725bfb0e7582",
    decimals: 6,
    official: true,
    note: "Circle testnet USDC; no financial value",
}];

const ARBITRUM_SEPOLIA_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d",
    decimals: 6,
    official: true,
    note: "Circle testnet USDC; no financial value",
}];

const OPTIMISM_SEPOLIA_TOKENS: &[EvmTokenPreset] = &[EvmTokenPreset {
    symbol: "USDC",
    contract: "0x5fd84259d66cd46123540766be93dfe6d43130d7",
    decimals: 6,
    official: true,
    note: "Circle testnet USDC; no financial value",
}];

const EMPTY_TOKENS: &[EvmTokenPreset] = &[];

const EVM_CHAIN_PRESETS: &[EvmChainPreset] = &[
    EvmChainPreset {
        id: "bsc-mainnet",
        env: NETWORK_ENV_MAINNET,
        label: "BNB Smart Chain",
        alchemy_network: "bnb-mainnet",
        chain_id: 56,
        chain_slug: "bnb-mainnet",
        chain_name: "BNB Smart Chain",
        scan_host: "https://bscscan.com",
        default_confirmations: 12,
        default_log_scan_block_range: 10,
        tokens: BSC_TOKENS,
    },
    EvmChainPreset {
        id: "base-mainnet",
        env: NETWORK_ENV_MAINNET,
        label: "Base",
        alchemy_network: "base-mainnet",
        chain_id: 8453,
        chain_slug: "base-mainnet",
        chain_name: "Base",
        scan_host: "https://basescan.org",
        default_confirmations: 12,
        default_log_scan_block_range: 200,
        tokens: BASE_TOKENS,
    },
    EvmChainPreset {
        id: "polygon-mainnet",
        env: NETWORK_ENV_MAINNET,
        label: "Polygon PoS",
        alchemy_network: "polygon-mainnet",
        chain_id: 137,
        chain_slug: "polygon-mainnet",
        chain_name: "Polygon PoS",
        scan_host: "https://polygonscan.com",
        default_confirmations: 64,
        default_log_scan_block_range: 200,
        tokens: POLYGON_TOKENS,
    },
    EvmChainPreset {
        id: "arbitrum-mainnet",
        env: NETWORK_ENV_MAINNET,
        label: "Arbitrum One",
        alchemy_network: "arb-mainnet",
        chain_id: 42161,
        chain_slug: "arb-mainnet",
        chain_name: "Arbitrum One",
        scan_host: "https://arbiscan.io",
        default_confirmations: 20,
        default_log_scan_block_range: 200,
        tokens: ARBITRUM_TOKENS,
    },
    EvmChainPreset {
        id: "optimism-mainnet",
        env: NETWORK_ENV_MAINNET,
        label: "OP Mainnet",
        alchemy_network: "opt-mainnet",
        chain_id: 10,
        chain_slug: "opt-mainnet",
        chain_name: "OP Mainnet",
        scan_host: "https://optimistic.etherscan.io",
        default_confirmations: 20,
        default_log_scan_block_range: 200,
        tokens: OPTIMISM_TOKENS,
    },
    EvmChainPreset {
        id: "eth-sepolia",
        env: NETWORK_ENV_TESTNET,
        label: "Ethereum Sepolia",
        alchemy_network: "eth-sepolia",
        chain_id: 11155111,
        chain_slug: "eth-sepolia",
        chain_name: "Ethereum Sepolia",
        scan_host: "https://sepolia.etherscan.io",
        default_confirmations: 3,
        default_log_scan_block_range: 200,
        tokens: ETH_SEPOLIA_TOKENS,
    },
    EvmChainPreset {
        id: "base-sepolia",
        env: NETWORK_ENV_TESTNET,
        label: "Base Sepolia",
        alchemy_network: "base-sepolia",
        chain_id: 84532,
        chain_slug: "base-sepolia",
        chain_name: "Base Sepolia",
        scan_host: "https://sepolia.basescan.org",
        default_confirmations: 3,
        default_log_scan_block_range: 200,
        tokens: BASE_SEPOLIA_TOKENS,
    },
    EvmChainPreset {
        id: "polygon-amoy",
        env: NETWORK_ENV_TESTNET,
        label: "Polygon PoS Amoy",
        alchemy_network: "polygon-amoy",
        chain_id: 80002,
        chain_slug: "polygon-amoy",
        chain_name: "Polygon PoS Amoy",
        scan_host: "https://amoy.polygonscan.com",
        default_confirmations: 6,
        default_log_scan_block_range: 200,
        tokens: POLYGON_AMOY_TOKENS,
    },
    EvmChainPreset {
        id: "arbitrum-sepolia",
        env: NETWORK_ENV_TESTNET,
        label: "Arbitrum Sepolia",
        alchemy_network: "arb-sepolia",
        chain_id: 421614,
        chain_slug: "arb-sepolia",
        chain_name: "Arbitrum Sepolia",
        scan_host: "https://sepolia.arbiscan.io",
        default_confirmations: 3,
        default_log_scan_block_range: 200,
        tokens: ARBITRUM_SEPOLIA_TOKENS,
    },
    EvmChainPreset {
        id: "optimism-sepolia",
        env: NETWORK_ENV_TESTNET,
        label: "OP Sepolia",
        alchemy_network: "opt-sepolia",
        chain_id: 11155420,
        chain_slug: "opt-sepolia",
        chain_name: "OP Sepolia",
        scan_host: "https://sepolia-optimism.etherscan.io",
        default_confirmations: 3,
        default_log_scan_block_range: 200,
        tokens: OPTIMISM_SEPOLIA_TOKENS,
    },
    EvmChainPreset {
        id: "bnb-testnet",
        env: NETWORK_ENV_TESTNET,
        label: "BNB Smart Chain Testnet",
        alchemy_network: "bnb-testnet",
        chain_id: 97,
        chain_slug: "bnb-testnet",
        chain_name: "BNB Smart Chain Testnet",
        scan_host: "https://testnet.bscscan.com",
        default_confirmations: 3,
        default_log_scan_block_range: 10,
        tokens: EMPTY_TOKENS,
    },
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcLog {
    #[serde(default)]
    address: String,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    data: String,
    #[serde(default)]
    block_number: String,
    #[serde(default)]
    transaction_hash: String,
    #[serde(default)]
    log_index: String,
    #[serde(default)]
    removed: bool,
}

#[derive(Debug, Clone)]
struct ObservedTransfer {
    contract_address: String,
    from: String,
    to: String,
    value: String,
    tx_hash: String,
    log_index: String,
    block_number: i64,
}

#[derive(Debug)]
struct TransferScan {
    transfers: Vec<ObservedTransfer>,
    scanned_to: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcReceipt {
    #[serde(default)]
    status: String,
    #[serde(default)]
    block_number: String,
    #[serde(default)]
    logs: Vec<RpcLog>,
}

#[derive(Debug, Clone)]
struct PendingIntent {
    id: i64,
    payment_id: i64,
    channel_id: i64,
    payment_amount_cents: i64,
    payment_currency: String,
    chain_id: i64,
    token_contract: String,
    token_decimals: u32,
    receive_address: String,
    amount_scaled: i64,
    amount_precision: u32,
    scan_from_block: i64,
    last_scanned_block: i64,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct IntentGroupKey {
    channel_id: i64,
    chain_id: i64,
    token_contract: String,
    receive_address: String,
}

pub fn is_evm_local_provider(provider_type: &str) -> bool {
    matches!(
        provider_type.trim().to_ascii_lowercase().as_str(),
        PROVIDER_TYPE | "tokenpay-local" | "tokenpay_local" | "evmlocal" | "evm_local"
    )
}

pub fn chain_presets() -> &'static [EvmChainPreset] {
    EVM_CHAIN_PRESETS
}

pub fn validate_channel_config(config: &Value) -> anyhow::Result<()> {
    let cfg = EvmLocalConfig::from_value(config)?;
    if cfg.addresses.is_empty() {
        anyhow::bail!("evm-local config missing addresses");
    }
    if parse_decimal_scaled(&cfg.fiat_per_token, 8).is_none_or(|v| v <= 0) {
        anyhow::bail!("evm-local config fiat_per_token/rate must be greater than 0");
    }
    Ok(())
}

pub async fn create_payment(
    state: &AppState,
    channel: &crate::models::payment::PaymentChannel,
    config: &Value,
    order: &crate::models::order::Order,
    payment_no: &str,
    _base_url: &str,
) -> AppResult<crate::services::payment_service::PayPageData> {
    let cfg =
        EvmLocalConfig::from_value(config).map_err(|err| AppError::BadRequest(err.to_string()))?;
    let amount_scaled = token_amount_scaled(order.total_amount_cents, &cfg)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let rpc_chain_id = fetch_chain_id(&client, &cfg)
        .await
        .map_err(|err| AppError::BadRequest(format!("Alchemy RPC 链校验失败: {err}")))?;
    if rpc_chain_id != cfg.chain_id {
        return Err(AppError::BadRequest(format!(
            "Alchemy RPC chain_id 不一致: 配置 {}, 实际 {}",
            cfg.chain_id, rpc_chain_id
        )));
    }
    let scan_from_block = latest_block_number(&client, &cfg)
        .await
        .map(|block| block.saturating_add(1))
        .map_err(|err| AppError::BadRequest(format!("Alchemy RPC 区块高度获取失败: {err}")))?;
    let now = time::now_str();
    let expires_at = (time::now() + chrono::Duration::minutes(cfg.expire_minutes)).to_rfc3339();
    let bump_unit = 1_i64;
    let payment_url = format!("/detail-order-sn/{}", order.order_no);

    let mut last_error = None;
    for bump in 0..=MAX_AMOUNT_BUMPS {
        let candidate_scaled = amount_scaled.saturating_add(bump * bump_unit);
        let amount_text = format_scaled(candidate_scaled, cfg.amount_precision);
        for address in ordered_addresses(&cfg.addresses, candidate_scaled) {
            let qr_code = build_qr_text(&cfg, &amount_text, address, &expires_at);
            let provider_payload = json!({
                "provider": PROVIDER_TYPE,
                "backend": "alchemy",
            "network_env": cfg.network_env,
            "chain_id": cfg.chain_id,
                "chain_slug": cfg.chain_slug,
                "chain_name": cfg.chain_name,
                "alchemy_network": cfg.alchemy_network,
                "token": cfg.token_symbol,
                "token_contract": cfg.token_contract,
                "receive_address": address,
                "amount": amount_text,
                "expires_at": expires_at,
                "scan_host": cfg.scan_host
            });

            let mut tx = state.pool.begin().await?;
            let payment_id = sqlx::query(
                "INSERT INTO payments(payment_no, order_id, channel_id, provider_type, channel_type, interaction_mode,
                 amount_cents, currency, status, provider_ref, gateway_order_no, pay_url, qr_code, provider_payload_json,
                 created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(payment_no)
            .bind(order.id)
            .bind(channel.id)
            .bind(PROVIDER_TYPE)
            .bind(&channel.channel_type)
            .bind(&channel.interaction_mode)
            .bind(order.total_amount_cents)
            .bind(&order.currency)
            .bind(models::PAYMENT_PENDING)
            .bind(payment_no)
            .bind(payment_no)
            .bind(&payment_url)
            .bind(&qr_code)
            .bind(provider_payload.to_string())
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?
            .last_insert_rowid();

            let insert_intent = sqlx::query(
                "INSERT INTO evm_payment_intents(payment_id, network_env, chain_id, chain_slug, token_symbol, token_contract,
                 token_decimals, receive_address, amount_scaled, amount_text, amount_precision, scan_from_block,
                 last_scanned_block, status, expires_at, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 'pending', ?, ?, ?)",
            )
            .bind(payment_id)
            .bind(&cfg.network_env)
            .bind(cfg.chain_id)
            .bind(&cfg.chain_slug)
            .bind(&cfg.token_symbol)
            .bind(normalize_address(&cfg.token_contract))
            .bind(cfg.token_decimals as i64)
            .bind(normalize_address(address))
            .bind(candidate_scaled)
            .bind(&amount_text)
            .bind(cfg.amount_precision as i64)
            .bind(scan_from_block)
            .bind(&expires_at)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await;

            match insert_intent {
                Ok(_) => {
                    tx.commit().await?;
                    return Ok(crate::services::payment_service::PayPageData {
                        order_no: order.order_no.clone(),
                        payment_no: payment_no.to_string(),
                        amount_display: money::format_cents(order.total_amount_cents),
                        pay_url: payment_url,
                        qr_code,
                        interaction_mode: channel.interaction_mode.clone(),
                    });
                }
                Err(err) => {
                    tx.rollback().await?;
                    last_error = Some(err);
                }
            }
        }
    }

    Err(AppError::Conflict(format!(
        "没有可用的 EVM 收款金额锁: {}",
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "amount lock exhausted".to_string())
    )))
}

pub async fn manual_confirm_intent(
    state: &AppState,
    intent_id: i64,
    tx_hash: &str,
) -> AppResult<()> {
    let tx_hash = tx_hash.trim();
    if !valid_tx_hash(tx_hash) {
        return Err(AppError::BadRequest("tx_hash 格式错误".to_string()));
    }
    let intent = load_pending_intent_by_id(&state.pool, intent_id).await?;
    let config_json: String =
        sqlx::query_scalar("SELECT config_json FROM payment_channels WHERE id = ?")
            .bind(intent.channel_id)
            .fetch_one(&state.pool)
            .await?;
    let cfg = EvmLocalConfig::from_value(
        &serde_json::from_str::<Value>(&config_json).unwrap_or_else(|_| json!({})),
    )
    .map_err(|err| AppError::BadRequest(err.to_string()))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let receipt = fetch_transaction_receipt(&client, &cfg, tx_hash)
        .await
        .map_err(AppError::Anyhow)?;
    if !receipt.status.is_empty() && receipt.status != "0x1" {
        return Err(AppError::BadRequest("交易执行失败，不能补单".to_string()));
    }
    let block_number = parse_hex_i64(&receipt.block_number)
        .ok_or_else(|| AppError::BadRequest("交易尚未上链或缺少 blockNumber".to_string()))?;
    let latest = latest_block_number(&client, &cfg)
        .await
        .map_err(AppError::Anyhow)?;
    if latest.saturating_sub(block_number).saturating_add(1) < cfg.confirmations {
        return Err(AppError::BadRequest("交易确认数不足".to_string()));
    }
    for log in receipt.logs {
        let Some(transfer) = observed_transfer_from_log(&log) else {
            continue;
        };
        if !transfer_matches(&intent, &transfer, &cfg) {
            continue;
        }
        let log_index = transfer.log_index.trim().to_string();
        let actual_scaled =
            transfer_amount_scaled(&intent, &transfer).unwrap_or(intent.amount_scaled);
        let amount_text = format_scaled(actual_scaled, intent.amount_precision);
        if !claim_transfer(
            state,
            &intent,
            &transfer,
            tx_hash,
            &log_index,
            actual_scaled,
            &amount_text,
        )
        .await
        .map_err(AppError::Anyhow)?
        {
            return Err(AppError::Conflict(
                "该链上交易已被处理或订单状态已变化".to_string(),
            ));
        }
        if let Err(err) = payment_service::apply_success(
            state,
            intent.payment_id,
            intent.payment_amount_cents,
            &intent.payment_currency,
        )
        .await
        {
            release_transfer_claim(state, intent.id, err.to_string())
                .await
                .map_err(AppError::Anyhow)?;
            return Err(err);
        }
        mark_claim_matched(state, &intent, tx_hash, &log_index, &transfer.from)
            .await
            .map_err(AppError::Anyhow)?;
        return Ok(());
    }
    Err(AppError::BadRequest(
        "交易中没有匹配该 intent 的 ERC20 入账".to_string(),
    ))
}

pub fn spawn_watcher(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(err) = run_watcher_once(&state).await {
                tracing::warn!(error = %err, "evm-local watcher tick failed");
            }
            tokio::time::sleep(Duration::from_secs(DEFAULT_SCAN_INTERVAL_SECS)).await;
        }
    });
}

pub async fn run_watcher_once(state: &AppState) -> anyhow::Result<()> {
    reset_stale_processing_intents(&state.pool).await?;
    expire_old_intents(&state.pool).await?;
    let intents = load_pending_intents(&state.pool).await?;
    if intents.is_empty() {
        return Ok(());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let groups = group_pending_intents(intents);
    for (key, group) in groups {
        if let Err(err) = check_intent_group(state, &client, key.clone(), group.clone()).await {
            let now = time::now_str();
            let ids = group.iter().map(|intent| intent.id).collect::<Vec<_>>();
            for intent_id in ids {
                sqlx::query("UPDATE evm_payment_intents SET last_error = ?, last_checked_at = ?, updated_at = ? WHERE id = ?")
                    .bind(err.to_string())
                    .bind(&now)
                    .bind(&now)
                    .bind(intent_id)
                    .execute(&state.pool)
                    .await?;
            }
            tracing::warn!(?key, error = %err, "evm-local intent group check failed");
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    Ok(())
}

fn group_pending_intents(
    intents: Vec<PendingIntent>,
) -> HashMap<IntentGroupKey, Vec<PendingIntent>> {
    let mut groups: HashMap<IntentGroupKey, Vec<PendingIntent>> = HashMap::new();
    for intent in intents {
        groups
            .entry(IntentGroupKey {
                channel_id: intent.channel_id,
                chain_id: intent.chain_id,
                token_contract: normalize_address(&intent.token_contract),
                receive_address: normalize_address(&intent.receive_address),
            })
            .or_default()
            .push(intent);
    }
    for group in groups.values_mut() {
        group.sort_by_key(|intent| intent.id);
    }
    groups
}

async fn check_intent_group(
    state: &AppState,
    client: &reqwest::Client,
    key: IntentGroupKey,
    intents: Vec<PendingIntent>,
) -> anyhow::Result<()> {
    if intents.is_empty() {
        return Ok(());
    }
    let config_json: String =
        sqlx::query_scalar("SELECT config_json FROM payment_channels WHERE id = ?")
            .bind(key.channel_id)
            .fetch_one(&state.pool)
            .await?;
    let cfg = EvmLocalConfig::from_value(
        &serde_json::from_str::<Value>(&config_json).unwrap_or_else(|_| json!({})),
    )?;
    let scan = fetch_transfers_for_group(&state.pool, client, &cfg, &key, &intents).await?;
    let mut matched_intent_ids = Vec::new();
    for transfer in scan.transfers {
        let tx_hash = transfer.tx_hash.trim().to_string();
        if tx_hash.is_empty() {
            continue;
        }
        let log_index = transfer.log_index.trim().to_string();
        let Some(intent) = intents.iter().find(|intent| {
            !matched_intent_ids.contains(&intent.id)
                && transfer.block_number >= effective_scan_from(intent)
                && transfer_matches(intent, &transfer, &cfg)
        }) else {
            continue;
        };
        let actual_scaled =
            transfer_amount_scaled(intent, &transfer).unwrap_or(intent.amount_scaled);
        let amount_text = format_scaled(actual_scaled, intent.amount_precision);
        if !claim_transfer(
            state,
            intent,
            &transfer,
            &tx_hash,
            &log_index,
            actual_scaled,
            &amount_text,
        )
        .await?
        {
            continue;
        }

        if let Err(err) = payment_service::apply_success(
            state,
            intent.payment_id,
            intent.payment_amount_cents,
            &intent.payment_currency,
        )
        .await
        {
            release_transfer_claim(state, intent.id, err.to_string()).await?;
            return Err(err.into());
        }

        mark_claim_matched(state, intent, &tx_hash, &log_index, &transfer.from).await?;
        matched_intent_ids.push(intent.id);
    }
    if let Some(scanned_to) = scan.scanned_to {
        let now = time::now_str();
        for intent in &intents {
            if matched_intent_ids.contains(&intent.id) {
                continue;
            }
            sqlx::query(
                "UPDATE evm_payment_intents
                 SET last_scanned_block = ?, last_checked_at = ?, last_error = '', updated_at = ?
                 WHERE id = ? AND status = 'pending'",
            )
            .bind(scanned_to)
            .bind(&now)
            .bind(&now)
            .bind(intent.id)
            .execute(&state.pool)
            .await?;
        }
    } else {
        let now = time::now_str();
        for intent in &intents {
            if intent.scan_from_block <= 0 && intent.last_scanned_block <= 0 {
                continue;
            }
            sqlx::query(
                "UPDATE evm_payment_intents SET last_checked_at = ?, last_error = '', updated_at = ? WHERE id = ? AND status = 'pending'",
            )
            .bind(&now)
            .bind(&now)
            .bind(intent.id)
            .execute(&state.pool)
            .await?;
        }
    }
    Ok(())
}

async fn claim_transfer(
    state: &AppState,
    intent: &PendingIntent,
    transfer: &ObservedTransfer,
    tx_hash: &str,
    log_index: &str,
    actual_amount_scaled: i64,
    amount_text: &str,
) -> anyhow::Result<bool> {
    let now = time::now_str();
    let mut tx = state.pool.begin().await?;
    let claim = sqlx::query(
        "UPDATE evm_payment_intents
         SET status = 'processing', last_checked_at = ?, updated_at = ?
         WHERE id = ? AND status = 'pending'",
    )
    .bind(&now)
    .bind(&now)
    .bind(intent.id)
    .execute(&mut *tx)
    .await?;
    if claim.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    let seen = sqlx::query(
        "INSERT OR IGNORE INTO evm_seen_transfers(intent_id, status, chain_id, token_contract, tx_hash,
         log_index, from_address, to_address, amount_scaled, amount_text, block_number, tx_time,
         created_at, updated_at)
         VALUES (?, 'processing', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(intent.id)
    .bind(intent.chain_id)
    .bind(normalize_address(&intent.token_contract))
    .bind(tx_hash)
    .bind(log_index)
    .bind(normalize_address(&transfer.from))
    .bind(normalize_address(&transfer.to))
    .bind(actual_amount_scaled)
    .bind(amount_text)
    .bind(transfer.block_number)
    .bind("")
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    if seen.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

async fn release_transfer_claim(
    state: &AppState,
    intent_id: i64,
    error: String,
) -> anyhow::Result<()> {
    let now = time::now_str();
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM evm_seen_transfers WHERE intent_id = ? AND status = 'processing'")
        .bind(intent_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE evm_payment_intents
         SET status = 'pending', last_error = ?, last_checked_at = ?, updated_at = ?
         WHERE id = ? AND status = 'processing'",
    )
    .bind(error)
    .bind(&now)
    .bind(&now)
    .bind(intent_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn mark_claim_matched(
    state: &AppState,
    intent: &PendingIntent,
    tx_hash: &str,
    log_index: &str,
    from_address: &str,
) -> anyhow::Result<()> {
    let mut tx = state.pool.begin().await?;
    let now = time::now_str();
    sqlx::query(
        "UPDATE evm_seen_transfers
         SET status = 'matched', updated_at = ?
         WHERE intent_id = ? AND status = 'processing'",
    )
    .bind(&now)
    .bind(intent.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE evm_payment_intents
         SET status = 'matched', matched_tx_hash = ?, matched_log_index = ?, matched_from_address = ?,
             matched_at = ?, last_checked_at = ?, last_error = '', updated_at = ?
         WHERE id = ? AND status = 'processing'",
    )
    .bind(tx_hash)
    .bind(log_index)
    .bind(normalize_address(from_address))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(intent.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn reset_stale_processing_intents(pool: &SqlitePool) -> anyhow::Result<()> {
    let cutoff = (time::now() - chrono::Duration::seconds(PROCESSING_TIMEOUT_SECS)).to_rfc3339();
    let now = time::now_str();
    let rows = sqlx::query(
        "SELECT id FROM evm_payment_intents
         WHERE status = 'processing' AND updated_at <= ?",
    )
    .bind(&cutoff)
    .fetch_all(pool)
    .await?;
    let intent_ids = rows
        .into_iter()
        .map(|row| row.get::<i64, _>("id"))
        .collect::<Vec<_>>();
    for intent_id in intent_ids {
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM evm_seen_transfers WHERE intent_id = ? AND status = 'processing'")
            .bind(intent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE evm_payment_intents
             SET status = 'pending', last_error = 'processing timeout, retrying', updated_at = ?
             WHERE id = ? AND status = 'processing'",
        )
        .bind(&now)
        .bind(intent_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }
    Ok(())
}

async fn fetch_transfers_for_group(
    pool: &SqlitePool,
    client: &reqwest::Client,
    cfg: &EvmLocalConfig,
    key: &IntentGroupKey,
    intents: &[PendingIntent],
) -> anyhow::Result<TransferScan> {
    let latest = latest_block_number(client, cfg).await?;
    if latest < cfg.confirmations {
        return Ok(TransferScan {
            transfers: Vec::new(),
            scanned_to: None,
        });
    }
    let confirmed_to = latest - cfg.confirmations;
    protect_legacy_intents(pool, intents, confirmed_to).await?;
    let active_intents = intents
        .iter()
        .filter(|intent| intent.scan_from_block > 0 || intent.last_scanned_block > 0)
        .collect::<Vec<_>>();
    if active_intents.is_empty() {
        return Ok(TransferScan {
            transfers: Vec::new(),
            scanned_to: None,
        });
    }
    let mut from_block = active_intents
        .iter()
        .map(|intent| {
            if intent.last_scanned_block > 0 {
                intent.last_scanned_block.saturating_add(1)
            } else {
                intent.scan_from_block
            }
        })
        .min()
        .unwrap_or(confirmed_to.saturating_add(1));
    if from_block > confirmed_to {
        return Ok(TransferScan {
            transfers: Vec::new(),
            scanned_to: None,
        });
    }

    let mut transfers = Vec::new();
    let mut scanned_to = None;
    for _ in 0..cfg.max_scan_chunks_per_tick {
        if from_block > confirmed_to {
            break;
        }
        let to_block = (from_block + cfg.log_scan_block_range - 1).min(confirmed_to);
        let logs = fetch_transfer_logs(client, cfg, key, from_block, to_block).await?;
        scanned_to = Some(to_block);
        for log in logs {
            if let Some(transfer) = observed_transfer_from_log(&log) {
                transfers.push(transfer);
            }
        }
        from_block = to_block.saturating_add(1);
    }
    Ok(TransferScan {
        transfers,
        scanned_to,
    })
}

async fn fetch_transfer_logs(
    client: &reqwest::Client,
    cfg: &EvmLocalConfig,
    key: &IntentGroupKey,
    from_block: i64,
    to_block: i64,
) -> anyhow::Result<Vec<RpcLog>> {
    let params = json!([{
        "fromBlock": hex_block(from_block),
        "toBlock": hex_block(to_block),
        "address": normalize_address(&key.token_contract),
        "topics": [
            ERC20_TRANSFER_TOPIC,
            Value::Null,
            address_to_topic(&key.receive_address)
        ]
    }]);
    let result = alchemy_rpc(client, cfg, "eth_getLogs", params).await?;
    Ok(serde_json::from_value(result)?)
}

async fn protect_legacy_intents(
    pool: &SqlitePool,
    intents: &[PendingIntent],
    confirmed_to: i64,
) -> anyhow::Result<()> {
    let legacy_ids = intents
        .iter()
        .filter(|intent| intent.scan_from_block <= 0 && intent.last_scanned_block <= 0)
        .map(|intent| intent.id)
        .collect::<Vec<_>>();
    if legacy_ids.is_empty() {
        return Ok(());
    }
    let now = time::now_str();
    let scan_from = confirmed_to.saturating_add(1);
    for intent_id in legacy_ids {
        sqlx::query(
            "UPDATE evm_payment_intents
             SET scan_from_block = ?, last_scanned_block = ?, last_error = ?, last_checked_at = ?, updated_at = ?
             WHERE id = ? AND status = 'pending' AND scan_from_block = 0 AND last_scanned_block = 0",
        )
        .bind(scan_from)
        .bind(confirmed_to)
        .bind("legacy intent protected from historical scan; regenerate payment if needed")
        .bind(&now)
        .bind(&now)
        .bind(intent_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn effective_scan_from(intent: &PendingIntent) -> i64 {
    if intent.last_scanned_block > 0 {
        intent.last_scanned_block.saturating_add(1)
    } else {
        intent.scan_from_block.max(1)
    }
}

async fn fetch_chain_id(client: &reqwest::Client, cfg: &EvmLocalConfig) -> anyhow::Result<i64> {
    let result = alchemy_rpc(client, cfg, "eth_chainId", json!([])).await?;
    let raw = result
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("eth_chainId returned non-string result"))?;
    parse_hex_i64(raw).ok_or_else(|| anyhow::anyhow!("invalid eth_chainId result: {raw}"))
}

async fn fetch_transaction_receipt(
    client: &reqwest::Client,
    cfg: &EvmLocalConfig,
    tx_hash: &str,
) -> anyhow::Result<RpcReceipt> {
    let result = alchemy_rpc(client, cfg, "eth_getTransactionReceipt", json!([tx_hash])).await?;
    if result.is_null() {
        anyhow::bail!("transaction receipt not found");
    }
    Ok(serde_json::from_value(result)?)
}

async fn latest_block_number(
    client: &reqwest::Client,
    cfg: &EvmLocalConfig,
) -> anyhow::Result<i64> {
    let result = alchemy_rpc(client, cfg, "eth_blockNumber", json!([])).await?;
    let raw = result
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("eth_blockNumber returned non-string result"))?;
    parse_hex_i64(raw).ok_or_else(|| anyhow::anyhow!("invalid eth_blockNumber result: {raw}"))
}

async fn alchemy_rpc(
    client: &reqwest::Client,
    cfg: &EvmLocalConfig,
    method: &str,
    params: Value,
) -> anyhow::Result<Value> {
    let response = client
        .post(&cfg.rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<RpcResponse>()
        .await?;
    if let Some(error) = response.error {
        anyhow::bail!(
            "alchemy rpc {method} error {}: {}",
            error.code,
            error.message
        );
    }
    response
        .result
        .ok_or_else(|| anyhow::anyhow!("alchemy rpc {method} missing result"))
}

fn transfer_matches(
    intent: &PendingIntent,
    transfer: &ObservedTransfer,
    cfg: &EvmLocalConfig,
) -> bool {
    if !normalize_address(&transfer.contract_address).eq(&normalize_address(&intent.token_contract))
        || !normalize_address(&transfer.to).eq(&normalize_address(&intent.receive_address))
    {
        return false;
    }
    let Some(scaled) = transfer_amount_scaled(intent, transfer) else {
        return false;
    };
    if scaled == intent.amount_scaled {
        return true;
    }
    cfg.allow_overpay
        && scaled > intent.amount_scaled
        && scaled
            <= intent
                .amount_scaled
                .saturating_add(cfg.overpay_tolerance_scaled)
}

fn transfer_amount_scaled(intent: &PendingIntent, transfer: &ObservedTransfer) -> Option<i64> {
    hex_token_value_to_scaled(
        &transfer.value,
        intent.token_decimals,
        intent.amount_precision,
    )
}

async fn load_pending_intents(pool: &SqlitePool) -> anyhow::Result<Vec<PendingIntent>> {
    let now = time::now_str();
    let rows = sqlx::query(
        "SELECT i.id, i.payment_id, p.channel_id, p.amount_cents AS payment_amount_cents,
                p.currency AS payment_currency, i.chain_id, i.token_contract, i.token_decimals,
                i.receive_address, i.amount_scaled, i.amount_precision, i.scan_from_block,
                i.last_scanned_block
         FROM evm_payment_intents i
         JOIN payments p ON p.id = i.payment_id
         WHERE i.status = 'pending' AND i.expires_at > ? AND p.status = ?
         ORDER BY i.id ASC
         LIMIT 100",
    )
    .bind(now)
    .bind(models::PAYMENT_PENDING)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| PendingIntent {
            id: row.get("id"),
            payment_id: row.get("payment_id"),
            channel_id: row.get("channel_id"),
            payment_amount_cents: row.get("payment_amount_cents"),
            payment_currency: row.get("payment_currency"),
            chain_id: row.get("chain_id"),
            token_contract: row.get("token_contract"),
            token_decimals: row.get::<i64, _>("token_decimals") as u32,
            receive_address: row.get("receive_address"),
            amount_scaled: row.get("amount_scaled"),
            amount_precision: row.get::<i64, _>("amount_precision") as u32,
            scan_from_block: row.get("scan_from_block"),
            last_scanned_block: row.get("last_scanned_block"),
        })
        .collect())
}

async fn load_pending_intent_by_id(pool: &SqlitePool, intent_id: i64) -> AppResult<PendingIntent> {
    let row = sqlx::query(
        "SELECT i.id, i.payment_id, p.channel_id, p.amount_cents AS payment_amount_cents,
                p.currency AS payment_currency, i.chain_id, i.token_contract, i.token_decimals,
                i.receive_address, i.amount_scaled, i.amount_precision, i.scan_from_block,
                i.last_scanned_block
         FROM evm_payment_intents i
         JOIN payments p ON p.id = i.payment_id
         WHERE i.id = ? AND i.status = 'pending' AND p.status = ?
         LIMIT 1",
    )
    .bind(intent_id)
    .bind(models::PAYMENT_PENDING)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("EVM payment intent 不存在或不是待支付状态".to_string()))?;
    Ok(PendingIntent {
        id: row.get("id"),
        payment_id: row.get("payment_id"),
        channel_id: row.get("channel_id"),
        payment_amount_cents: row.get("payment_amount_cents"),
        payment_currency: row.get("payment_currency"),
        chain_id: row.get("chain_id"),
        token_contract: row.get("token_contract"),
        token_decimals: row.get::<i64, _>("token_decimals") as u32,
        receive_address: row.get("receive_address"),
        amount_scaled: row.get("amount_scaled"),
        amount_precision: row.get::<i64, _>("amount_precision") as u32,
        scan_from_block: row.get("scan_from_block"),
        last_scanned_block: row.get("last_scanned_block"),
    })
}

pub async fn expire_old_intents(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = time::now_str();
    let rows = sqlx::query(
        "SELECT payment_id FROM evm_payment_intents
         WHERE status = 'pending' AND expires_at <= ?",
    )
    .bind(&now)
    .fetch_all(pool)
    .await?;
    let payment_ids = rows
        .into_iter()
        .map(|row| row.get::<i64, _>("payment_id"))
        .collect::<Vec<_>>();
    sqlx::query(
        "UPDATE evm_payment_intents SET status = 'expired', updated_at = ?
         WHERE status = 'pending' AND expires_at <= ?",
    )
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    for payment_id in payment_ids {
        sqlx::query(
            "UPDATE payments SET status = ?, expired_at = ?, updated_at = ?
             WHERE id = ? AND status = ?",
        )
        .bind(models::PAYMENT_EXPIRED)
        .bind(&now)
        .bind(&now)
        .bind(payment_id)
        .bind(models::PAYMENT_PENDING)
        .execute(pool)
        .await?;
    }
    Ok(())
}

impl EvmLocalConfig {
    fn from_value(config: &Value) -> anyhow::Result<Self> {
        let alchemy_api_key = str_any(config, &["alchemy_api_key", "api_key"]);
        let alchemy_network =
            str_any(config, &["alchemy_network", "network"]).if_empty(DEFAULT_ALCHEMY_NETWORK);
        let configured_rpc_url = str_any(config, &["rpc_url", "alchemy_rpc_url"]);
        if alchemy_api_key.is_empty() && configured_rpc_url.is_empty() {
            anyhow::bail!("evm-local config missing alchemy_api_key or rpc_url");
        }
        let rpc_url = if configured_rpc_url.is_empty() {
            format!(
                "https://{}.g.alchemy.com/v2/{}",
                alchemy_network, alchemy_api_key
            )
        } else {
            configured_rpc_url
        };
        let token_contract = str_any(config, &["token_contract", "contract_address"]);
        if !valid_evm_address(&token_contract) {
            anyhow::bail!("evm-local token_contract is invalid");
        }
        let addresses = parse_addresses(config);
        if addresses.iter().any(|addr| !valid_evm_address(addr)) {
            anyhow::bail!("evm-local addresses contains invalid EVM address");
        }
        let chain_id = int_any(config, &["chain_id", "chainid"]).unwrap_or(56);
        let network_env = normalize_network_env(
            &str_any(config, &["network_env", "env"])
                .if_empty(&infer_network_env(chain_id, &alchemy_network)),
        )?;
        let token_decimals_raw = int_any(config, &["token_decimals", "decimals"]).unwrap_or(18);
        let amount_precision_raw =
            int_any(config, &["amount_precision"]).unwrap_or(DEFAULT_AMOUNT_PRECISION as i64);
        let confirmations = int_any(config, &["confirmations"]).unwrap_or(DEFAULT_CONFIRMATIONS);
        let log_scan_block_range =
            int_any(config, &["log_scan_block_range"]).unwrap_or(DEFAULT_LOG_SCAN_BLOCK_RANGE);
        let max_scan_chunks_per_tick = int_any(config, &["max_scan_chunks_per_tick"])
            .unwrap_or(DEFAULT_MAX_SCAN_CHUNKS_PER_TICK);
        let bootstrap_scan_blocks =
            int_any(config, &["bootstrap_scan_blocks"]).unwrap_or(DEFAULT_BOOTSTRAP_SCAN_BLOCKS);
        let allow_overpay = bool_any(config, &["allow_overpay"]).unwrap_or(false);
        if !(0..=30).contains(&token_decimals_raw) {
            anyhow::bail!("evm-local token_decimals must be <= 30");
        }
        if !(0..=8).contains(&amount_precision_raw) {
            anyhow::bail!("evm-local amount_precision must be between 0 and 8");
        }
        if !(1..=200).contains(&confirmations) {
            anyhow::bail!("evm-local confirmations must be between 1 and 200");
        }
        if !(1..=1_000).contains(&log_scan_block_range) {
            anyhow::bail!("evm-local log_scan_block_range must be between 1 and 1000");
        }
        if !(1..=200).contains(&max_scan_chunks_per_tick) {
            anyhow::bail!("evm-local max_scan_chunks_per_tick must be between 1 and 200");
        }
        if !(1..=1_000_000).contains(&bootstrap_scan_blocks) {
            anyhow::bail!("evm-local bootstrap_scan_blocks must be between 1 and 1000000");
        }
        let token_decimals = token_decimals_raw as u32;
        let amount_precision = amount_precision_raw as u32;
        let overpay_tolerance_scaled = parse_decimal_scaled(
            &str_any(config, &["overpay_tolerance"]).if_empty("0"),
            amount_precision,
        )
        .and_then(|value| i64::try_from(value).ok())
        .unwrap_or(0)
        .max(0);
        Ok(Self {
            alchemy_api_key,
            rpc_url: rpc_url.trim_end_matches('/').to_string(),
            alchemy_network: alchemy_network.clone(),
            network_env,
            chain_id,
            chain_slug: str_any(config, &["chain_slug"]).if_empty(&alchemy_network),
            chain_name: str_any(config, &["chain_name"]).if_empty("BNB Smart Chain"),
            scan_host: str_any(config, &["scan_host"]).if_empty("https://bscscan.com"),
            token_symbol: str_any(config, &["token_symbol", "token"])
                .if_empty("USDT")
                .to_ascii_uppercase(),
            token_contract: normalize_address(&token_contract),
            token_decimals,
            confirmations,
            amount_precision,
            expire_minutes: int_any(config, &["expire_minutes"]).unwrap_or(DEFAULT_EXPIRE_MINUTES),
            log_scan_block_range,
            max_scan_chunks_per_tick,
            bootstrap_scan_blocks,
            allow_overpay,
            overpay_tolerance_scaled,
            fiat_per_token: str_any(config, &["fiat_per_token", "rate"]).if_empty("1"),
            addresses,
        })
    }
}

trait IfEmpty {
    fn if_empty(self, default: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, default: &str) -> String {
        if self.trim().is_empty() {
            default.to_string()
        } else {
            self
        }
    }
}

fn token_amount_scaled(amount_cents: i64, cfg: &EvmLocalConfig) -> AppResult<i64> {
    let precision = cfg.amount_precision;
    let scale = 10_i128.pow(precision);
    let fiat_scaled = (amount_cents as i128)
        .checked_mul(scale)
        .and_then(|value| value.checked_mul(scale))
        .ok_or_else(|| AppError::BadRequest("订单金额过大".to_string()))?;
    let rate_scaled = parse_decimal_scaled(&cfg.fiat_per_token, precision)
        .ok_or_else(|| AppError::BadRequest("fiat_per_token/rate 格式错误".to_string()))?;
    if rate_scaled <= 0 {
        return Err(AppError::BadRequest(
            "fiat_per_token/rate 必须大于 0".to_string(),
        ));
    }
    let denom = rate_scaled
        .checked_mul(100)
        .ok_or_else(|| AppError::BadRequest("汇率过大".to_string()))?;
    let scaled = div_ceil(fiat_scaled, denom);
    i64::try_from(scaled).map_err(|_| AppError::BadRequest("token 金额过大".to_string()))
}

#[cfg(test)]
fn token_value_to_scaled(value: &str, token_decimals: u32, amount_precision: u32) -> Option<i64> {
    let raw = value.trim().parse::<i128>().ok()?;
    let token_scale = 10_i128.checked_pow(token_decimals)?;
    let display_scale = 10_i128.checked_pow(amount_precision)?;
    i64::try_from(raw.checked_mul(display_scale)? / token_scale).ok()
}

fn hex_token_value_to_scaled(
    value: &str,
    token_decimals: u32,
    amount_precision: u32,
) -> Option<i64> {
    let raw = parse_hex_i128(value)?;
    let token_scale = 10_i128.checked_pow(token_decimals)?;
    let display_scale = 10_i128.checked_pow(amount_precision)?;
    i64::try_from(raw.checked_mul(display_scale)? / token_scale).ok()
}

fn observed_transfer_from_log(log: &RpcLog) -> Option<ObservedTransfer> {
    if log.removed || log.topics.len() < 3 {
        return None;
    }
    if !log.topics[0].eq_ignore_ascii_case(ERC20_TRANSFER_TOPIC) {
        return None;
    }
    Some(ObservedTransfer {
        contract_address: normalize_address(&log.address),
        from: topic_to_address(&log.topics[1])?,
        to: topic_to_address(&log.topics[2])?,
        value: log.data.clone(),
        tx_hash: log.transaction_hash.clone(),
        log_index: log.log_index.clone(),
        block_number: parse_hex_i64(&log.block_number)?,
    })
}

fn address_to_topic(address: &str) -> String {
    let address = normalize_address(address);
    let address = address.trim_start_matches("0x");
    format!("0x{:0>64}", address)
}

fn topic_to_address(topic: &str) -> Option<String> {
    let topic = topic.trim();
    let hex = topic.strip_prefix("0x").unwrap_or(topic);
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", &hex[24..]).to_ascii_lowercase())
}

fn hex_block(block: i64) -> String {
    format!("0x{:x}", block.max(0))
}

fn parse_hex_i64(value: &str) -> Option<i64> {
    i64::try_from(parse_hex_i128(value)?).ok()
}

fn parse_hex_i128(value: &str) -> Option<i128> {
    let value = value.trim();
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    i128::from_str_radix(value, 16).ok()
}

fn parse_decimal_scaled(value: &str, precision: u32) -> Option<i128> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') {
        return None;
    }
    let mut parts = value.split('.');
    let whole = parts.next()?.parse::<i128>().ok()?;
    let frac = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return None;
    }
    let mut frac_digits = frac.chars().take(precision as usize).collect::<String>();
    while frac_digits.len() < precision as usize {
        frac_digits.push('0');
    }
    let frac_value = if frac_digits.is_empty() {
        0
    } else {
        frac_digits.parse::<i128>().ok()?
    };
    whole
        .checked_mul(10_i128.checked_pow(precision)?)?
        .checked_add(frac_value)
}

fn div_ceil(a: i128, b: i128) -> i128 {
    if b == 0 { 0 } else { (a + b - 1) / b }
}

fn format_scaled(value: i64, precision: u32) -> String {
    if precision == 0 {
        return value.to_string();
    }
    let scale = 10_i64.pow(precision);
    let whole = value / scale;
    let frac = (value.abs() % scale).to_string();
    format!("{whole}.{:0>width$}", frac, width = precision as usize)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn build_qr_text(
    cfg: &EvmLocalConfig,
    amount_text: &str,
    address: &str,
    expires_at: &str,
) -> String {
    let tx_url = if cfg.scan_host.trim().is_empty() {
        String::new()
    } else {
        format!("\nExplorer: {}", cfg.scan_host.trim_end_matches('/'))
    };
    let env_notice = if cfg.network_env == NETWORK_ENV_TESTNET {
        "\nEnvironment: TESTNET (no financial value)"
    } else {
        ""
    };
    format!(
        "Network: {}\nChain ID: {}\nToken: {} ({})\nAmount: {}\nAddress: {}\nExpires At: {}{}{}",
        cfg.chain_name,
        cfg.chain_id,
        cfg.token_symbol,
        cfg.token_contract,
        amount_text,
        address,
        expires_at,
        env_notice,
        tx_url
    )
}

fn ordered_addresses(addresses: &[String], amount_scaled: i64) -> Vec<&str> {
    if addresses.is_empty() {
        return Vec::new();
    }
    let start = (amount_scaled.unsigned_abs() as usize) % addresses.len();
    (0..addresses.len())
        .filter_map(|offset| addresses.get((start + offset) % addresses.len()))
        .map(String::as_str)
        .collect()
}

fn parse_addresses(config: &Value) -> Vec<String> {
    if let Some(values) = config.get("addresses").and_then(Value::as_array) {
        return values
            .iter()
            .filter_map(Value::as_str)
            .map(normalize_address)
            .filter(|value| !value.is_empty())
            .collect();
    }
    str_any(config, &["addresses", "address", "wallets"])
        .split(|c| matches!(c, '\n' | '\r' | ',' | ';' | ' '))
        .map(normalize_address)
        .filter(|value| !value.is_empty())
        .collect()
}

fn str_any(config: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = config.get(*key) {
            if let Some(s) = value.as_str() {
                if !s.trim().is_empty() {
                    return s.trim().to_string();
                }
            } else if value.is_number() {
                return value.to_string();
            }
        }
    }
    String::new()
}

fn int_any(config: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        config.get(*key).and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
        })
    })
}

fn bool_any(config: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        config.get(*key).and_then(|value| {
            value.as_bool().or_else(|| {
                value
                    .as_str()
                    .and_then(|s| match s.trim().to_ascii_lowercase().as_str() {
                        "1" | "true" | "yes" | "on" => Some(true),
                        "0" | "false" | "no" | "off" => Some(false),
                        _ => None,
                    })
            })
        })
    })
}

fn normalize_network_env(value: &str) -> anyhow::Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | NETWORK_ENV_MAINNET | "main" | "prod" | "production" => {
            Ok(NETWORK_ENV_MAINNET.to_string())
        }
        NETWORK_ENV_TESTNET | "test" | "sandbox" => Ok(NETWORK_ENV_TESTNET.to_string()),
        other => anyhow::bail!("evm-local network_env must be mainnet or testnet, got {other}"),
    }
}

fn infer_network_env(chain_id: i64, alchemy_network: &str) -> String {
    if EVM_CHAIN_PRESETS
        .iter()
        .any(|preset| preset.chain_id == chain_id && preset.env == NETWORK_ENV_TESTNET)
    {
        return NETWORK_ENV_TESTNET.to_string();
    }
    let network = alchemy_network.to_ascii_lowercase();
    if network.contains("sepolia")
        || network.contains("testnet")
        || network.contains("amoy")
        || network.contains("fuji")
        || network.contains("goerli")
        || network.contains("holesky")
        || network.contains("hoodi")
    {
        NETWORK_ENV_TESTNET.to_string()
    } else {
        NETWORK_ENV_MAINNET.to_string()
    }
}

fn normalize_address(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn valid_evm_address(value: &str) -> bool {
    let value = value.trim();
    value.len() == 42
        && value.starts_with("0x")
        && value.chars().skip(2).all(|ch| ch.is_ascii_hexdigit())
}

fn valid_tx_hash(value: &str) -> bool {
    let value = value.trim();
    value.len() == 66
        && value.starts_with("0x")
        && value.chars().skip(2).all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(rate: &str) -> EvmLocalConfig {
        EvmLocalConfig {
            alchemy_api_key: "key".to_string(),
            rpc_url: "https://bnb-mainnet.g.alchemy.com/v2/key".to_string(),
            alchemy_network: DEFAULT_ALCHEMY_NETWORK.to_string(),
            network_env: NETWORK_ENV_MAINNET.to_string(),
            chain_id: 56,
            chain_slug: "bnb-mainnet".to_string(),
            chain_name: "BNB Smart Chain".to_string(),
            scan_host: "https://bscscan.com".to_string(),
            token_symbol: "USDT".to_string(),
            token_contract: "0x55d398326f99059ff775485246999027b3197955".to_string(),
            token_decimals: 18,
            confirmations: 12,
            amount_precision: 6,
            expire_minutes: 30,
            log_scan_block_range: 10,
            max_scan_chunks_per_tick: 12,
            bootstrap_scan_blocks: 2_000,
            allow_overpay: false,
            overpay_tolerance_scaled: 0,
            fiat_per_token: rate.to_string(),
            addresses: vec!["0x0000000000000000000000000000000000000001".to_string()],
        }
    }

    #[test]
    fn calculates_token_amount_from_fiat_rate() {
        assert_eq!(token_amount_scaled(100, &cfg("1")).unwrap(), 1_000_000);
        assert_eq!(token_amount_scaled(725, &cfg("7.25")).unwrap(), 1_000_000);
        assert_eq!(token_amount_scaled(1000, &cfg("7.25")).unwrap(), 1_379_311);
    }

    #[test]
    fn normalizes_raw_erc20_value_to_payment_precision() {
        assert_eq!(
            token_value_to_scaled("1000000000000000000", 18, 6),
            Some(1_000_000)
        );
        assert_eq!(
            hex_token_value_to_scaled("0xde0b6b3a7640000", 18, 6),
            Some(1_000_000)
        );
        assert_eq!(token_value_to_scaled("1234567", 6, 6), Some(1_234_567));
    }

    #[test]
    fn formats_scaled_amounts() {
        assert_eq!(format_scaled(1_000_000, 6), "1");
        assert_eq!(format_scaled(1_379_311, 6), "1.379311");
    }

    #[test]
    fn infers_network_env_from_chain_and_alchemy_slug() {
        assert_eq!(infer_network_env(56, "bnb-mainnet"), "mainnet");
        assert_eq!(infer_network_env(84532, "base-sepolia"), "testnet");
        assert_eq!(infer_network_env(12345, "custom-testnet"), "testnet");
        assert!(normalize_network_env("sandbox").is_ok());
        assert!(normalize_network_env("unknown").is_err());
    }

    #[test]
    fn exposes_circle_usdc_testnet_presets() {
        let base = chain_presets()
            .iter()
            .find(|chain| chain.id == "base-sepolia")
            .unwrap();
        assert_eq!(base.env, NETWORK_ENV_TESTNET);
        assert_eq!(base.chain_id, 84532);
        assert_eq!(base.alchemy_network, "base-sepolia");
        assert_eq!(base.tokens[0].symbol, "USDC");
        assert_eq!(
            base.tokens[0].contract,
            "0x036cbd53842c5426634e7929541ec2318f3dcf7e"
        );
        assert_eq!(base.tokens[0].decimals, 6);
    }

    #[test]
    fn qr_text_marks_testnet_payments() {
        let mut config = cfg("1");
        config.network_env = NETWORK_ENV_TESTNET.to_string();
        config.chain_name = "Base Sepolia".to_string();
        config.chain_id = 84532;
        let text = build_qr_text(
            &config,
            "1",
            "0x0000000000000000000000000000000000000001",
            "2026-06-18T00:00:00Z",
        );
        assert!(text.contains("Base Sepolia"));
        assert!(text.contains("TESTNET"));
        assert!(text.contains("no financial value"));
    }

    #[test]
    fn decodes_erc20_transfer_log() {
        let to = "0x0000000000000000000000000000000000000001";
        let from = "0x0000000000000000000000000000000000000002";
        let log = RpcLog {
            address: "0x55d398326f99059ff775485246999027b3197955".to_string(),
            topics: vec![
                ERC20_TRANSFER_TOPIC.to_string(),
                address_to_topic(from),
                address_to_topic(to),
            ],
            data: "0xde0b6b3a7640000".to_string(),
            block_number: "0x10".to_string(),
            transaction_hash: "0xabc".to_string(),
            log_index: "0x2".to_string(),
            removed: false,
        };
        let transfer = observed_transfer_from_log(&log).unwrap();
        assert_eq!(
            transfer.contract_address,
            "0x55d398326f99059ff775485246999027b3197955"
        );
        assert_eq!(transfer.from, from);
        assert_eq!(transfer.to, to);
        assert_eq!(transfer.block_number, 16);
        assert_eq!(transfer.log_index, "0x2");
    }

    #[test]
    fn matches_overpay_only_when_enabled_and_within_tolerance() {
        let intent = PendingIntent {
            id: 1,
            payment_id: 1,
            channel_id: 1,
            payment_amount_cents: 100,
            payment_currency: "cny".to_string(),
            chain_id: 56,
            token_contract: "0x55d398326f99059ff775485246999027b3197955".to_string(),
            token_decimals: 18,
            receive_address: "0x0000000000000000000000000000000000000001".to_string(),
            amount_scaled: 1_000_000,
            amount_precision: 6,
            scan_from_block: 1,
            last_scanned_block: 0,
        };
        let transfer = ObservedTransfer {
            contract_address: intent.token_contract.clone(),
            from: "0x0000000000000000000000000000000000000002".to_string(),
            to: intent.receive_address.clone(),
            value: "0xde0bfcbf5d6a000".to_string(),
            tx_hash: "0xabc".to_string(),
            log_index: "0x1".to_string(),
            block_number: 10,
        };
        let mut config = cfg("1");
        assert!(!transfer_matches(&intent, &transfer, &config));
        config.allow_overpay = true;
        config.overpay_tolerance_scaled = 10;
        assert!(transfer_matches(&intent, &transfer, &config));
        config.overpay_tolerance_scaled = 1;
        assert!(!transfer_matches(&intent, &transfer, &config));
    }
}
