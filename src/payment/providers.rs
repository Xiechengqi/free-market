use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use async_trait::async_trait;
use axum::http::HeaderMap;
use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, Mac};
use rand::RngCore;
use reqwest::Client;
use rsa::{
    RsaPrivateKey, RsaPublicKey,
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey},
    pkcs1v15::{SigningKey, VerifyingKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use signature::{SignatureEncoding, Signer, Verifier};

use crate::payment::provider::{
    CreatePaymentInput, CreatePaymentResult, PaymentCallback, PaymentProvider, PaymentStatus,
    amount_yuan, callback_success, first_param, flatten_json, hex_lower, json_i64, json_string,
    parse_json_object, payment_result, require_config, sorted_md5_sign,
    sorted_md5_sign_value_suffix, str_config, str_config_any, url_encode, yuan_to_cents,
};

const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 15;

pub struct NoopProvider;

#[async_trait]
impl PaymentProvider for NoopProvider {
    fn provider_type(&self) -> &'static str {
        "noop"
    }

    async fn create_payment(
        &self,
        _config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        Ok(payment_result(
            format!("NOOP-{}", input.payment_no),
            input.return_url,
            "",
            json!({ "provider": "noop" }),
        ))
    }
}

pub struct EpayProvider;

#[async_trait]
impl PaymentProvider for EpayProvider {
    fn provider_type(&self) -> &'static str {
        "epay"
    }

    fn validate_config(&self, config: &Value, channel_type: &str) -> anyhow::Result<()> {
        let version = epay_version(config);
        if !matches!(
            resolve_epay_type(channel_type).as_str(),
            "alipay" | "wxpay" | "qqpay"
        ) {
            anyhow::bail!("epay unsupported channel_type {channel_type}");
        }
        require_config(config, &["gateway_url"])?;
        if str_config_any(config, &["merchant_id", "pid"]).is_empty() {
            anyhow::bail!("epay config missing merchant_id/pid");
        }
        if version == "v2" {
            require_config(config, &["private_key"])?;
        } else if str_config_any(config, &["merchant_key", "key"]).is_empty() {
            anyhow::bail!("epay config missing merchant_key/key");
        }
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let gateway = str_config(config, "gateway_url")
            .trim_end_matches('/')
            .to_string();
        let merchant_id = str_config_any(config, &["merchant_id", "pid"]);
        let payment_type = resolve_epay_type(&input.channel_type);
        let money = amount_yuan(input.amount_cents);
        let version = epay_version(config);
        let mut params = vec![
            ("notify_url".to_string(), input.notify_url.clone()),
            ("return_url".to_string(), input.return_url.clone()),
            ("name".to_string(), input.subject.clone()),
            ("money".to_string(), money),
        ];
        if version == "v2" {
            params.extend([
                ("mch_id".to_string(), merchant_id),
                ("out_trade_no".to_string(), input.payment_no.clone()),
                ("type".to_string(), payment_type.clone()),
                ("method".to_string(), config_str_or(config, "method", "web")),
                ("timestamp".to_string(), unix_ts().to_string()),
            ]);
            let content = build_sign_content(&params);
            let sign = rsa_sign_base64(&str_config(config, "private_key"), &content)?;
            params.push(("sign".to_string(), sign));
            params.push(("sign_type".to_string(), "RSA".to_string()));
            let path = config_str_or(config, "submit_path", "/api/pay/submit");
            let pay_url = format!("{}{}?{}", gateway, path, encode_query(&params));
            Ok(payment_result(
                input.payment_no,
                pay_url,
                "",
                json!({ "provider": "epay", "version": "v2", "type": payment_type }),
            ))
        } else {
            params.extend([
                ("pid".to_string(), merchant_id),
                ("type".to_string(), payment_type.clone()),
                ("out_trade_no".to_string(), input.payment_no.clone()),
                ("sitename".to_string(), "free-market".to_string()),
                ("clientip".to_string(), input.client_ip),
                ("device".to_string(), config_str_or(config, "device", "pc")),
            ]);
            let key = str_config_any(config, &["merchant_key", "key"]);
            let sign = sorted_md5_sign(&params, &key);
            params.push(("sign".to_string(), sign));
            params.push(("sign_type".to_string(), "MD5".to_string()));
            let path = config_str_or(config, "submit_path", "/submit.php");
            let pay_url = format!("{}{}?{}", gateway, path, encode_query(&params));
            Ok(payment_result(
                input.payment_no,
                pay_url,
                "",
                json!({ "provider": "epay", "version": "v1", "type": payment_type }),
            ))
        }
    }

    async fn verify_callback(
        &self,
        config: &Value,
        form: &HashMap<String, String>,
        _body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        let payment_no = first_param(form, &["out_trade_no", "trade_no"])
            .ok_or_else(|| anyhow::anyhow!("epay callback missing out_trade_no"))?;
        let status = first_param(form, &["trade_status", "status"]).unwrap_or("TRADE_SUCCESS");
        if !matches!(status, "TRADE_SUCCESS" | "TRADE_FINISHED" | "success" | "1") {
            anyhow::bail!("epay callback status is not success");
        }
        let sign = first_param(form, &["sign"])
            .ok_or_else(|| anyhow::anyhow!("epay callback missing sign"))?;
        let params = form
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        let expected = if epay_version(config) == "v2" {
            // v2 callback variants are inconsistent across gateways. If platform_public_key is
            // absent, fall back to strict merchant-key MD5 for compatibility with common Yipay.
            let key = str_config_any(config, &["merchant_key", "key"]);
            sorted_md5_sign(&params, &key)
        } else {
            let key = str_config_any(config, &["merchant_key", "key"]);
            sorted_md5_sign(&params, &key)
        };
        if !expected.eq_ignore_ascii_case(sign) {
            anyhow::bail!("epay callback signature mismatch");
        }
        let amount = first_param(form, &["money", "total_fee"])
            .and_then(yuan_to_cents)
            .ok_or_else(|| anyhow::anyhow!("epay callback amount invalid"))?;
        Ok(callback_success(
            payment_no,
            first_param(form, &["trade_no"]).unwrap_or(payment_no),
            amount,
            "CNY",
            json!(form),
        ))
    }
}

pub struct TokenPayProvider;

#[async_trait]
impl PaymentProvider for TokenPayProvider {
    fn provider_type(&self) -> &'static str {
        "tokenpay"
    }

    fn validate_config(&self, config: &Value, _channel_type: &str) -> anyhow::Result<()> {
        require_config(config, &["gateway_url"])?;
        if str_config_any(config, &["notify_secret", "token", "key"]).is_empty() {
            anyhow::bail!("tokenpay config missing notify_secret/token/key");
        }
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let gateway = str_config(config, "gateway_url")
            .trim_end_matches('/')
            .to_string();
        let secret = str_config_any(config, &["notify_secret", "token", "key"]);
        let currency = config_str_or(config, "currency", "USDT");
        let amount = amount_yuan(input.amount_cents);
        let mut payload = json!({
            "OutOrderId": input.payment_no,
            "OrderUserKey": input.order_no,
            "ActualAmount": amount,
            "Currency": currency,
            "NotifyUrl": input.notify_url,
            "RedirectUrl": input.return_url
        });
        let params = json_to_pairs(&payload);
        payload["Signature"] = json!(sorted_md5_sign(&params, &secret));
        let path = config_str_or(config, "create_path", "/CreateOrder");
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!("{gateway}{path}"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let pay_url = response
            .get("data")
            .and_then(Value::as_str)
            .or_else(|| json_string(&response, &["info", "PaymentUrl"]))
            .or_else(|| json_string(&response, &["info", "QrCodeLink"]))
            .unwrap_or_default()
            .to_string();
        if pay_url.is_empty() {
            anyhow::bail!("tokenpay response missing pay url");
        }
        let provider_ref = json_string(&response, &["info", "Id"])
            .unwrap_or(&input.payment_no)
            .to_string();
        Ok(payment_result(provider_ref, pay_url, "", response))
    }

    async fn verify_callback(
        &self,
        config: &Value,
        form: &HashMap<String, String>,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        let raw = if form.is_empty() && !body.is_empty() {
            flatten_json(&parse_json_object(body)?)
        } else {
            form.clone()
        };
        let sign = first_param(&raw, &["Signature", "signature"])
            .ok_or_else(|| anyhow::anyhow!("tokenpay callback missing signature"))?;
        let secret = str_config_any(config, &["notify_secret", "token", "key"]);
        let params = raw
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        let expected = sorted_md5_sign(&params, &secret);
        if !expected.eq_ignore_ascii_case(sign) {
            anyhow::bail!("tokenpay callback signature mismatch");
        }
        let status = first_param(&raw, &["Status", "status"]).unwrap_or("1");
        if status != "1" {
            anyhow::bail!("tokenpay callback status is not success");
        }
        let payment_no = first_param(&raw, &["OutOrderId", "out_order_id"])
            .ok_or_else(|| anyhow::anyhow!("tokenpay callback missing order id"))?;
        let amount = first_param(&raw, &["Amount", "ActualAmount", "amount", "actual_amount"])
            .and_then(yuan_to_cents)
            .ok_or_else(|| anyhow::anyhow!("tokenpay callback amount invalid"))?;
        Ok(callback_success(
            payment_no,
            first_param(&raw, &["Id", "id"]).unwrap_or(payment_no),
            amount,
            config_str_or(config, "base_currency", "CNY"),
            json!(raw),
        ))
    }
}

pub struct EpusdtProvider;

#[async_trait]
impl PaymentProvider for EpusdtProvider {
    fn provider_type(&self) -> &'static str {
        "epusdt"
    }

    fn validate_config(&self, config: &Value, _channel_type: &str) -> anyhow::Result<()> {
        require_config(config, &["gateway_url"])?;
        if str_config_any(config, &["secret_key", "token", "key"]).is_empty() {
            anyhow::bail!("epusdt config missing secret_key/token/key");
        }
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let gateway = str_config(config, "gateway_url")
            .trim_end_matches('/')
            .to_string();
        let money = amount_yuan(input.amount_cents);
        if let Some(template) = config.get("pay_url_template").and_then(Value::as_str) {
            let pay_url = template
                .replace("{payment_no}", &input.payment_no)
                .replace("{order_no}", &input.order_no)
                .replace("{amount}", &money);
            return Ok(payment_result(
                input.payment_no,
                pay_url,
                "",
                json!({ "provider": "epusdt", "mode": "template" }),
            ));
        }
        let secret = str_config_any(config, &["secret_key", "token", "key"]);
        let mut payload = json!({
            "pid": str_config(config, "pid"),
            "order_id": input.payment_no,
            "currency": config_str_or(config, "currency", "cny"),
            "token": config_str_or(config, "token", "usdt"),
            "network": config_str_or(config, "network", "trc20"),
            "amount": money,
            "notify_url": input.notify_url,
            "redirect_url": input.return_url,
            "name": input.subject
        });
        let params = json_to_pairs(&payload);
        payload["signature"] = json!(sorted_md5_sign(&params, &secret));
        let path = config_str_or(
            config,
            "create_path",
            "/payments/gmpay/v1/order/create-transaction",
        );
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!("{gateway}{path}"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let trade_id = json_string(&response, &["data", "trade_id"])
            .or_else(|| response.get("trade_id").and_then(Value::as_str))
            .ok_or_else(|| anyhow::anyhow!("epusdt response missing trade_id"))?
            .to_string();
        Ok(payment_result(
            trade_id.clone(),
            format!("{gateway}/pay/checkout-counter/{trade_id}"),
            "",
            response,
        ))
    }

    async fn verify_callback(
        &self,
        config: &Value,
        form: &HashMap<String, String>,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        signed_json_or_form_callback(config, form, body, "epusdt", &["order_id"], &["trade_id"])
            .await
    }
}

pub struct BepusdtProvider;

#[async_trait]
impl PaymentProvider for BepusdtProvider {
    fn provider_type(&self) -> &'static str {
        "bepusdt"
    }

    fn validate_config(&self, config: &Value, channel_type: &str) -> anyhow::Result<()> {
        require_config(config, &["gateway_url"])?;
        if str_config_any(config, &["auth_token", "token", "key"]).is_empty() {
            anyhow::bail!("bepusdt config missing auth_token/token/key");
        }
        if !resolve_bepusdt_trade_type(channel_type).is_empty()
            || !str_config(config, "trade_type").is_empty()
        {
            Ok(())
        } else {
            anyhow::bail!("bepusdt unsupported channel_type {channel_type}");
        }
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let gateway = str_config(config, "gateway_url")
            .trim_end_matches('/')
            .to_string();
        let token = str_config_any(config, &["auth_token", "token", "key"]);
        let trade_type = config_str_or(
            config,
            "trade_type",
            &resolve_bepusdt_trade_type(&input.channel_type),
        );
        let mut payload = json!({
            "order_id": input.payment_no,
            "amount": amount_yuan(input.amount_cents),
            "notify_url": input.notify_url,
            "redirect_url": input.return_url,
            "trade_type": trade_type,
            "fiat": config_str_or(config, "fiat", "CNY"),
            "name": input.subject
        });
        payload["signature"] = json!(sorted_md5_sign_value_suffix(
            &json_to_pairs(&payload),
            &token
        ));
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!("{gateway}/api/v1/order/create-transaction"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let source = response.get("data").unwrap_or(&response);
        let pay_url = source
            .get("payment_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if pay_url.is_empty() {
            anyhow::bail!("bepusdt response missing payment_url");
        }
        let provider_ref = source
            .get("trade_id")
            .and_then(Value::as_str)
            .unwrap_or(&input.payment_no)
            .to_string();
        Ok(payment_result(
            provider_ref,
            pay_url.clone(),
            pay_url,
            response,
        ))
    }

    async fn verify_callback(
        &self,
        config: &Value,
        form: &HashMap<String, String>,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        signed_json_or_form_callback(config, form, body, "bepusdt", &["order_id"], &["trade_id"])
            .await
    }
}

pub struct OkpayProvider;

#[async_trait]
impl PaymentProvider for OkpayProvider {
    fn provider_type(&self) -> &'static str {
        "okpay"
    }

    fn validate_config(&self, config: &Value, channel_type: &str) -> anyhow::Result<()> {
        if !matches!(channel_type.to_ascii_lowercase().as_str(), "usdt" | "trx") {
            anyhow::bail!("okpay unsupported channel_type {channel_type}");
        }
        require_config(config, &["merchant_id"])?;
        if str_config_any(config, &["merchant_token", "token", "key"]).is_empty() {
            anyhow::bail!("okpay config missing merchant_token/token/key");
        }
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let gateway = config_str_or(config, "gateway_url", "https://api.okaypay.me/shop")
            .trim_end_matches('/')
            .to_string();
        let merchant_id = str_config(config, "merchant_id");
        let token = str_config_any(config, &["merchant_token", "token", "key"]);
        let coin = match input.channel_type.to_ascii_lowercase().as_str() {
            "trx" => "TRX",
            _ => "USDT",
        };
        let mut payload = vec![
            ("unique_id".to_string(), input.payment_no.clone()),
            ("amount".to_string(), amount_yuan(input.amount_cents)),
            ("return_url".to_string(), input.return_url),
            ("callback_url".to_string(), input.notify_url),
            ("coin".to_string(), coin.to_string()),
            ("id".to_string(), merchant_id),
        ];
        let sign = okpay_sign(&payload, &token);
        payload.push(("sign".to_string(), sign));
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!("{gateway}/payLink"))
            .form(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let source = response
            .get("data")
            .and_then(|value| {
                if value.is_array() {
                    value.as_array().and_then(|items| items.first())
                } else {
                    Some(value)
                }
            })
            .unwrap_or(&response);
        let pay_url = source
            .get("pay_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if pay_url.is_empty() {
            anyhow::bail!("okpay response missing pay_url");
        }
        let provider_ref = source
            .get("order_id")
            .and_then(Value::as_str)
            .unwrap_or(&input.payment_no)
            .to_string();
        Ok(payment_result(provider_ref, pay_url, "", response))
    }

    async fn verify_callback(
        &self,
        config: &Value,
        form: &HashMap<String, String>,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        let raw = if form.is_empty() && !body.is_empty() {
            flatten_json(&parse_json_object(body)?)
        } else {
            form.clone()
        };
        let token = str_config_any(config, &["merchant_token", "token", "key"]);
        let sign =
            first_param(&raw, &["sign"]).ok_or_else(|| anyhow::anyhow!("okpay missing sign"))?;
        let params = raw
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        if !okpay_sign(&params, &token).eq_ignore_ascii_case(sign) {
            anyhow::bail!("okpay signature mismatch");
        }
        let status = first_param(&raw, &["data[status]", "status"]).unwrap_or("success");
        let pay_status =
            first_param(&raw, &["data[payment_status]", "data[status]"]).unwrap_or("1");
        if !status.eq_ignore_ascii_case("success") || pay_status != "1" {
            anyhow::bail!("okpay callback status is not success");
        }
        let payment_no = first_param(&raw, &["data[unique_id]", "unique_id"])
            .ok_or_else(|| anyhow::anyhow!("okpay missing unique_id"))?;
        let amount = first_param(&raw, &["data[amount]", "amount"])
            .and_then(yuan_to_cents)
            .ok_or_else(|| anyhow::anyhow!("okpay amount invalid"))?;
        Ok(callback_success(
            payment_no,
            first_param(&raw, &["data[order_id]", "order_id"]).unwrap_or(payment_no),
            amount,
            "CNY",
            json!(raw),
        ))
    }
}

pub struct FreeMarketPayProvider;

const FREEMARKETPAY_DEFAULT_BASE_URL: &str = "https://www.freemarketpay.com";

#[async_trait]
impl PaymentProvider for FreeMarketPayProvider {
    fn provider_type(&self) -> &'static str {
        "freemarketpay"
    }

    fn validate_config(&self, config: &Value, channel_type: &str) -> anyhow::Result<()> {
        require_config(config, &["api_key_id", "api_secret", "webhook_secret"])?;
        let token_id = if channel_type.trim().is_empty() {
            str_config(config, "token_id")
        } else {
            channel_type.trim().to_ascii_lowercase()
        };
        if resolve_freemarketpay_chain(&token_id).is_empty() {
            anyhow::bail!("freemarketpay unsupported token_id {token_id}");
        }
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let base = config_str_or(config, "api_base_url", FREEMARKETPAY_DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .to_string();
        let token_id = if input.channel_type.trim().is_empty() {
            str_config(config, "token_id")
        } else {
            input.channel_type.trim().to_ascii_lowercase()
        };
        let chain = config_str_or(config, "chain", &resolve_freemarketpay_chain(&token_id));
        let payload = json!({
            "chain": chain,
            "token_id": token_id,
            "fiat_currency": config_str_or(config, "fiat_currency", &input.currency).to_ascii_uppercase(),
            "fiat_amount": amount_yuan(input.amount_cents),
            "merchant_order_id": input.payment_no,
            "success_url": input.return_url,
            "cancel_url": input.return_url,
            "metadata": { "order_no": input.order_no }
        });
        let body = serde_json::to_vec(&payload)?;
        let ts = unix_ts();
        let nonce = random_hex(16);
        let headers = freemarketpay_sign_headers(
            &str_config(config, "api_secret"),
            &str_config(config, "api_key_id"),
            "POST",
            "/v1/orders",
            "",
            &body,
            ts,
            &nonce,
        );
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?;
        let mut request = client
            .post(format!("{base}/v1/orders"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Idempotency-Key", &input.payment_no)
            .body(body);
        for (key, value) in headers {
            request = request.header(key, value);
        }
        let response = request
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let source = response.get("data").unwrap_or(&response);
        let checkout_url = source
            .get("checkout_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if checkout_url.is_empty() {
            anyhow::bail!("freemarketpay response missing checkout_url");
        }
        let provider_ref = source
            .get("order_id")
            .and_then(Value::as_str)
            .unwrap_or(&input.payment_no)
            .to_string();
        Ok(payment_result(provider_ref, checkout_url, "", response))
    }

    async fn parse_webhook(
        &self,
        config: &Value,
        headers: &HeaderMap,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        let timestamp = header_str(headers, "DJP-Webhook-Timestamp")
            .ok_or_else(|| anyhow::anyhow!("freemarketpay webhook missing timestamp"))?;
        let signature = header_str(headers, "DJP-Webhook-Signature")
            .ok_or_else(|| anyhow::anyhow!("freemarketpay webhook missing signature"))?;
        let ts = timestamp.parse::<i64>()?;
        let now = unix_ts() as i64;
        if (now - ts).abs() > 300 {
            anyhow::bail!("freemarketpay webhook timestamp outside tolerance");
        }
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(str_config(config, "webhook_secret").as_bytes())?;
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(body);
        let expected = hex_lower(&mac.finalize().into_bytes());
        let got = signature.trim().trim_start_matches("sha256=");
        if !expected.eq_ignore_ascii_case(got) {
            anyhow::bail!("freemarketpay webhook signature mismatch");
        }
        let value = parse_json_object(body)?;
        let event_type = value
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if event_type != "order.paid" {
            let status = match event_type {
                "order.expired" => PaymentStatus::Expired,
                "order.canceled" => PaymentStatus::Failed,
                _ => PaymentStatus::Pending,
            };
            return Ok(PaymentCallback {
                payment_no: json_string(&value, &["data", "merchant_order_id"])
                    .unwrap_or_default()
                    .to_string(),
                provider_ref: json_string(&value, &["data", "order_id"])
                    .unwrap_or_default()
                    .to_string(),
                status,
                amount_cents: 0,
                currency: config_str_or(config, "fiat_currency", "CNY"),
                paid_at: json_string(&value, &["data", "paid_at"]).map(str::to_string),
                payload: value,
            });
        }
        let payment_no = json_string(&value, &["data", "merchant_order_id"])
            .unwrap_or_default()
            .to_string();
        let provider_ref = json_string(&value, &["data", "order_id"])
            .unwrap_or_default()
            .to_string();
        Ok(callback_success(
            payment_no,
            provider_ref,
            0,
            config_str_or(config, "fiat_currency", "CNY"),
            value,
        ))
    }
}

pub struct StripeProvider;

#[async_trait]
impl PaymentProvider for StripeProvider {
    fn provider_type(&self) -> &'static str {
        "official"
    }

    fn validate_config(&self, config: &Value, _channel_type: &str) -> anyhow::Result<()> {
        require_config(config, &["secret_key", "webhook_secret"])?;
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let base = config_str_or(config, "api_base_url", "https://api.stripe.com");
        let currency =
            config_str_or(config, "target_currency", &input.currency).to_ascii_lowercase();
        let amount = if is_zero_decimal_currency(&currency) {
            input.amount_cents / 100
        } else {
            input.amount_cents
        };
        let success_url = config_url_or(config, "success_url", &input.return_url);
        let cancel_url = config_url_or(config, "cancel_url", &input.return_url);
        let mut form = vec![
            ("mode".to_string(), "payment".to_string()),
            ("success_url".to_string(), success_url),
            ("cancel_url".to_string(), cancel_url),
            ("client_reference_id".to_string(), input.payment_no.clone()),
            ("line_items[0][quantity]".to_string(), "1".to_string()),
            (
                "line_items[0][price_data][currency]".to_string(),
                currency.clone(),
            ),
            (
                "line_items[0][price_data][unit_amount]".to_string(),
                amount.to_string(),
            ),
            (
                "line_items[0][price_data][product_data][name]".to_string(),
                input.subject,
            ),
            ("metadata[payment_no]".to_string(), input.payment_no.clone()),
            (
                "payment_intent_data[metadata][payment_no]".to_string(),
                input.payment_no.clone(),
            ),
        ];
        let methods = config.get("payment_method_types").and_then(Value::as_array);
        if let Some(methods) = methods {
            for method in methods.iter().filter_map(Value::as_str) {
                form.push(("payment_method_types[]".to_string(), method.to_string()));
            }
        } else {
            form.push(("payment_method_types[]".to_string(), "card".to_string()));
        }
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!(
                "{}/v1/checkout/sessions",
                base.trim_end_matches('/')
            ))
            .bearer_auth(str_config(config, "secret_key"))
            .form(&form)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let session_id = response
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let url = response
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if session_id.is_empty() || url.is_empty() {
            anyhow::bail!("stripe response missing id/url");
        }
        Ok(payment_result(session_id, url, "", response))
    }

    async fn parse_webhook(
        &self,
        config: &Value,
        headers: &HeaderMap,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        verify_stripe_signature(&str_config(config, "webhook_secret"), headers, body)?;
        let value = parse_json_object(body)?;
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let object = value.pointer("/data/object").unwrap_or(&value);
        let payment_no = object
            .get("client_reference_id")
            .and_then(Value::as_str)
            .or_else(|| json_string(object, &["metadata", "payment_no"]))
            .or_else(|| json_string(object, &["metadata", "order_no"]))
            .unwrap_or_default();
        let provider_ref = object.get("id").and_then(Value::as_str).unwrap_or_default();
        let status = match event_type {
            "checkout.session.completed"
            | "checkout.session.async_payment_succeeded"
            | "payment_intent.succeeded" => PaymentStatus::Success,
            "checkout.session.expired" => PaymentStatus::Expired,
            "checkout.session.async_payment_failed"
            | "payment_intent.payment_failed"
            | "payment_intent.canceled" => PaymentStatus::Failed,
            _ => PaymentStatus::Pending,
        };
        let amount_cents = object
            .get("amount_total")
            .or_else(|| object.get("amount_received"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let currency = object
            .get("currency")
            .and_then(Value::as_str)
            .unwrap_or("CNY")
            .to_ascii_uppercase();
        Ok(PaymentCallback {
            payment_no: payment_no.to_string(),
            provider_ref: provider_ref.to_string(),
            status,
            amount_cents,
            currency,
            paid_at: None,
            payload: value,
        })
    }
}

pub struct PaypalProvider;

#[async_trait]
impl PaymentProvider for PaypalProvider {
    fn provider_type(&self) -> &'static str {
        "official"
    }

    fn validate_config(&self, config: &Value, _channel_type: &str) -> anyhow::Result<()> {
        require_config(config, &["client_id", "client_secret"])?;
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let base = config_str_or(config, "base_url", "https://api-m.sandbox.paypal.com");
        let token = paypal_access_token(config).await?;
        let currency =
            config_str_or(config, "target_currency", &input.currency).to_ascii_uppercase();
        let payload = json!({
            "intent": "CAPTURE",
            "purchase_units": [{
                "invoice_id": input.payment_no,
                "amount": {
                    "currency_code": currency,
                    "value": amount_yuan(input.amount_cents)
                },
                "description": input.subject
            }],
            "application_context": {
                "return_url": config_url_or(config, "return_url", &input.return_url),
                "cancel_url": config_url_or(config, "cancel_url", &input.return_url),
                "user_action": config_str_or(config, "user_action", "PAY_NOW"),
                "shipping_preference": config_str_or(config, "shipping_preference", "NO_SHIPPING")
            }
        });
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!("{}/v2/checkout/orders", base.trim_end_matches('/')))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let order_id = response
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let approval_url = response
            .get("links")
            .and_then(Value::as_array)
            .and_then(|links| {
                links.iter().find_map(|link| {
                    (link.get("rel").and_then(Value::as_str) == Some("approve"))
                        .then(|| link.get("href").and_then(Value::as_str))
                        .flatten()
                })
            })
            .unwrap_or_default()
            .to_string();
        if order_id.is_empty() || approval_url.is_empty() {
            anyhow::bail!("paypal response missing order id/approval url");
        }
        Ok(payment_result(order_id, approval_url, "", response))
    }

    async fn parse_webhook(
        &self,
        config: &Value,
        headers: &HeaderMap,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        if !str_config(config, "webhook_id").is_empty() {
            verify_paypal_webhook(config, headers, body).await?;
        }
        let value = parse_json_object(body)?;
        let event_type = value
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let resource = value.get("resource").unwrap_or(&value);
        let payment_no = resource
            .get("invoice_id")
            .and_then(Value::as_str)
            .or_else(|| {
                resource
                    .pointer("/purchase_units/0/invoice_id")
                    .and_then(Value::as_str)
            })
            .or_else(|| json_string(resource, &["supplementary_data", "related_ids", "order_id"]))
            .unwrap_or_default();
        let provider_ref = resource
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = match event_type {
            "PAYMENT.CAPTURE.COMPLETED" | "CHECKOUT.ORDER.COMPLETED" => PaymentStatus::Success,
            "PAYMENT.CAPTURE.DENIED"
            | "PAYMENT.CAPTURE.DECLINED"
            | "PAYMENT.CAPTURE.FAILED"
            | "CHECKOUT.ORDER.DENIED" => PaymentStatus::Failed,
            _ => PaymentStatus::Pending,
        };
        let amount = json_string(resource, &["amount", "value"])
            .and_then(yuan_to_cents)
            .unwrap_or(0);
        let currency = json_string(resource, &["amount", "currency_code"]).unwrap_or("CNY");
        Ok(PaymentCallback {
            payment_no: payment_no.to_string(),
            provider_ref: provider_ref.to_string(),
            status,
            amount_cents: amount,
            currency: currency.to_string(),
            paid_at: resource
                .get("update_time")
                .and_then(Value::as_str)
                .map(str::to_string),
            payload: value,
        })
    }

    async fn query_payment(
        &self,
        config: &Value,
        provider_ref: &str,
    ) -> anyhow::Result<PaymentCallback> {
        let base = config_str_or(config, "base_url", "https://api-m.sandbox.paypal.com");
        let token = paypal_access_token(config).await?;
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!(
                "{}/v2/checkout/orders/{}/capture",
                base.trim_end_matches('/'),
                provider_ref
            ))
            .bearer_auth(token)
            .json(&json!({}))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let purchase = response
            .pointer("/purchase_units/0/payments/captures/0")
            .unwrap_or(&response);
        Ok(PaymentCallback {
            payment_no: response
                .pointer("/purchase_units/0/invoice_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            provider_ref: purchase
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(provider_ref)
                .to_string(),
            status: if purchase.get("status").and_then(Value::as_str) == Some("COMPLETED") {
                PaymentStatus::Success
            } else {
                PaymentStatus::Pending
            },
            amount_cents: json_string(purchase, &["amount", "value"])
                .and_then(yuan_to_cents)
                .unwrap_or(0),
            currency: json_string(purchase, &["amount", "currency_code"])
                .unwrap_or("CNY")
                .to_string(),
            paid_at: purchase
                .get("update_time")
                .and_then(Value::as_str)
                .map(str::to_string),
            payload: response,
        })
    }
}

pub struct AlipayProvider;

#[async_trait]
impl PaymentProvider for AlipayProvider {
    fn provider_type(&self) -> &'static str {
        "official"
    }

    fn validate_config(&self, config: &Value, _channel_type: &str) -> anyhow::Result<()> {
        require_config(config, &["app_id", "private_key"])?;
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let gateway = config_str_or(
            config,
            "gateway_url",
            "https://openapi.alipay.com/gateway.do",
        );
        let payment_no = input.payment_no.clone();
        let method = match str_config(config, "interaction_mode").as_str() {
            "wap" => "alipay.trade.wap.pay",
            "qr" | "qrcode" => "alipay.trade.precreate",
            _ => "alipay.trade.page.pay",
        };
        let biz_content = json!({
            "out_trade_no": payment_no,
            "total_amount": amount_yuan(input.amount_cents),
            "subject": input.subject,
            "product_code": if method == "alipay.trade.wap.pay" { "QUICK_WAP_WAY" } else { "FAST_INSTANT_TRADE_PAY" }
        });
        let mut params = vec![
            ("app_id".to_string(), str_config(config, "app_id")),
            ("method".to_string(), method.to_string()),
            ("format".to_string(), "JSON".to_string()),
            ("charset".to_string(), "utf-8".to_string()),
            (
                "sign_type".to_string(),
                config_str_or(config, "sign_type", "RSA2"),
            ),
            (
                "timestamp".to_string(),
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            ),
            ("version".to_string(), "1.0".to_string()),
            ("notify_url".to_string(), input.notify_url),
            ("return_url".to_string(), input.return_url),
            ("biz_content".to_string(), biz_content.to_string()),
        ];
        let content = build_sign_content(&params);
        params.push((
            "sign".to_string(),
            rsa_sign_base64(&str_config(config, "private_key"), &content)?,
        ));
        Ok(payment_result(
            payment_no,
            format!("{}?{}", gateway, encode_query(&params)),
            "",
            json!({ "provider": "alipay", "method": method }),
        ))
    }

    async fn verify_callback(
        &self,
        config: &Value,
        form: &HashMap<String, String>,
        _body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        let public_key = str_config(config, "alipay_public_key");
        if !public_key.is_empty() {
            let sign = first_param(form, &["sign"])
                .ok_or_else(|| anyhow::anyhow!("alipay callback missing sign"))?;
            let params = form
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>();
            let content = build_sign_content(&params);
            rsa_verify_base64(&public_key, &content, sign)?;
        }
        let status = first_param(form, &["trade_status"]).unwrap_or_default();
        if !matches!(status, "TRADE_SUCCESS" | "TRADE_FINISHED") {
            anyhow::bail!("alipay callback status is not success");
        }
        let payment_no = first_param(form, &["out_trade_no"])
            .ok_or_else(|| anyhow::anyhow!("alipay missing out_trade_no"))?;
        let amount = first_param(
            form,
            &["total_amount", "receipt_amount", "buyer_pay_amount"],
        )
        .and_then(yuan_to_cents)
        .ok_or_else(|| anyhow::anyhow!("alipay amount invalid"))?;
        Ok(callback_success(
            payment_no,
            first_param(form, &["trade_no"]).unwrap_or(payment_no),
            amount,
            "CNY",
            json!(form),
        ))
    }
}

pub struct WechatPayProvider;

#[async_trait]
impl PaymentProvider for WechatPayProvider {
    fn provider_type(&self) -> &'static str {
        "official"
    }

    fn validate_config(&self, config: &Value, _channel_type: &str) -> anyhow::Result<()> {
        require_config(
            config,
            &[
                "appid",
                "mchid",
                "merchant_serial_no",
                "merchant_private_key",
            ],
        )?;
        Ok(())
    }

    async fn create_payment(
        &self,
        config: &Value,
        input: CreatePaymentInput,
    ) -> anyhow::Result<CreatePaymentResult> {
        let base = config_str_or(config, "base_url", "https://api.mch.weixin.qq.com");
        let h5 = str_config(config, "trade_type").eq_ignore_ascii_case("h5")
            || input.channel_type.eq_ignore_ascii_case("h5")
            || input.channel_type.eq_ignore_ascii_case("wap");
        let path = if h5 {
            "/v3/pay/transactions/h5"
        } else {
            "/v3/pay/transactions/native"
        };
        let mut payload = json!({
            "appid": str_config(config, "appid"),
            "mchid": str_config(config, "mchid"),
            "description": input.subject,
            "out_trade_no": input.payment_no,
            "notify_url": input.notify_url,
            "amount": { "total": input.amount_cents, "currency": input.currency }
        });
        if h5 {
            payload["scene_info"] = json!({
                "payer_client_ip": input.client_ip,
                "h5_info": { "type": config_str_or(config, "h5_type", "Wap") }
            });
        }
        let body = payload.to_string();
        let auth = wechat_authorization(config, "POST", path, "", &body)?;
        let response = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()?
            .post(format!("{base}{path}"))
            .header("Authorization", auth)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        let pay_url = if h5 {
            response.get("h5_url").and_then(Value::as_str)
        } else {
            response.get("code_url").and_then(Value::as_str)
        }
        .unwrap_or_default()
        .to_string();
        if pay_url.is_empty() {
            anyhow::bail!("wechatpay response missing h5_url/code_url");
        }
        Ok(payment_result(
            input.payment_no,
            pay_url.clone(),
            if h5 { "" } else { &pay_url },
            response,
        ))
    }

    async fn parse_webhook(
        &self,
        config: &Value,
        _headers: &HeaderMap,
        body: &[u8],
    ) -> anyhow::Result<PaymentCallback> {
        let value = parse_json_object(body)?;
        let decrypted;
        let resource = if let Some(plaintext) = value
            .get("resource")
            .and_then(|resource| resource.get("plaintext"))
        {
            plaintext
        } else if let Some(resource) = value.get("resource") {
            decrypted = wechat_decrypt_resource(config, resource)?;
            &decrypted
        } else {
            &value
        };
        let trade_state = resource
            .get("trade_state")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = if trade_state == "SUCCESS" {
            PaymentStatus::Success
        } else {
            PaymentStatus::Pending
        };
        Ok(PaymentCallback {
            payment_no: resource
                .get("out_trade_no")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            provider_ref: resource
                .get("transaction_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            status,
            amount_cents: json_i64(resource, &["amount", "total"]).unwrap_or(0),
            currency: json_string(resource, &["amount", "currency"])
                .unwrap_or("CNY")
                .to_string(),
            paid_at: resource
                .get("success_time")
                .and_then(Value::as_str)
                .map(str::to_string),
            payload: value,
        })
    }
}

async fn signed_json_or_form_callback(
    config: &Value,
    form: &HashMap<String, String>,
    body: &[u8],
    provider: &str,
    payment_keys: &[&str],
    ref_keys: &[&str],
) -> anyhow::Result<PaymentCallback> {
    let raw = if form.is_empty() && !body.is_empty() {
        flatten_json(&parse_json_object(body)?)
    } else {
        form.clone()
    };
    let secret = str_config_any(config, &["secret_key", "auth_token", "token", "key"]);
    let sign = first_param(&raw, &["signature", "sign", "Signature"])
        .ok_or_else(|| anyhow::anyhow!("{provider} callback missing signature"))?;
    let expected = sorted_md5_sign_value_suffix(
        &raw.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        &secret,
    );
    if !expected.eq_ignore_ascii_case(sign) {
        anyhow::bail!("{provider} callback signature mismatch");
    }
    let status = first_param(&raw, &["status", "Status"]).unwrap_or("2");
    if !matches!(status, "2" | "success" | "SUCCESS") {
        anyhow::bail!("{provider} callback status is not success");
    }
    let payment_no = first_param(&raw, payment_keys)
        .ok_or_else(|| anyhow::anyhow!("{provider} callback missing order id"))?;
    let amount = first_param(&raw, &["amount", "Amount"])
        .and_then(yuan_to_cents)
        .ok_or_else(|| anyhow::anyhow!("{provider} callback amount invalid"))?;
    Ok(callback_success(
        payment_no,
        first_param(&raw, ref_keys).unwrap_or(payment_no),
        amount,
        config_str_or(config, "fiat", "CNY"),
        json!(raw),
    ))
}

fn epay_version(config: &Value) -> String {
    config_str_or(config, "epay_version", "v1").to_ascii_lowercase()
}

fn resolve_epay_type(channel_type: &str) -> String {
    match channel_type.trim().to_ascii_lowercase().as_str() {
        "wechat" | "wxpay" => "wxpay".to_string(),
        "alipay" => "alipay".to_string(),
        "qqpay" => "qqpay".to_string(),
        _ => String::new(),
    }
}

fn resolve_bepusdt_trade_type(channel_type: &str) -> String {
    match channel_type.trim().to_ascii_lowercase().as_str() {
        "usdt" | "usdt-trc20" => "usdt.trc20".to_string(),
        "usdc-trc20" => "usdc.trc20".to_string(),
        "trx" => "tron.trx".to_string(),
        _ => String::new(),
    }
}

fn resolve_freemarketpay_chain(token_id: &str) -> String {
    match token_id.trim().to_ascii_lowercase().as_str() {
        "tron-trx" | "tron-usdt" => "tron",
        "ethereum-eth" | "ethereum-usdt" | "ethereum-usdc" => "ethereum",
        "bsc-bnb" | "bsc-usdt" => "bsc",
        "polygon-usdc" | "polygon-usdt0" => "polygon",
        "base-usdc" => "base",
        "arbitrum-usdc" | "arbitrum-usdt0" => "arbitrum",
        "plasma-usdt0" => "plasma",
        "x-layer-usdt0" => "x-layer",
        "solana-usdc" | "solana-usdt" => "solana",
        "aptos-usdc" | "aptos-usdt" => "aptos",
        _ => "",
    }
    .to_string()
}

fn config_str_or(config: &Value, key: &str, fallback: &str) -> String {
    let value = str_config(config, key);
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn config_url_or(config: &Value, key: &str, fallback: &str) -> String {
    let value = str_config(config, key);
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn json_to_pairs(value: &Value) -> Vec<(String, String)> {
    flatten_json(value).into_iter().collect()
}

fn encode_query(params: &[(String, String)]) -> String {
    params
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(key, value)| format!("{}={}", url_encode(key), url_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn build_sign_content(params: &[(String, String)]) -> String {
    let mut items = params
        .iter()
        .filter(|(key, value)| key != "sign" && !value.trim().is_empty())
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn rsa_sign_base64(private_key_pem: &str, content: &str) -> anyhow::Result<String> {
    let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(private_key_pem))?;
    let signing_key = SigningKey::<Sha256>::new(private_key);
    let signature = signing_key.sign(content.as_bytes());
    Ok(general_purpose::STANDARD.encode(signature.to_vec()))
}

fn rsa_verify_base64(public_key_pem: &str, content: &str, signature: &str) -> anyhow::Result<()> {
    let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(public_key_pem))?;
    let signature_bytes = general_purpose::STANDARD.decode(signature.trim())?;
    let signature = rsa::pkcs1v15::Signature::try_from(signature_bytes.as_slice())?;
    let verifying_key = VerifyingKey::<Sha256>::new(public_key);
    verifying_key.verify(content.as_bytes(), &signature)?;
    Ok(())
}

fn okpay_sign(params: &[(String, String)], token: &str) -> String {
    let mut items = params
        .iter()
        .filter(|(key, value)| !key.eq_ignore_ascii_case("sign") && !value.trim().is_empty())
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let query = items
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!(
        "{:x}",
        md5::compute(format!("{query}&token={}", token.trim()))
    )
    .to_ascii_uppercase()
}

fn freemarketpay_sign_headers(
    secret: &str,
    key_id: &str,
    method: &str,
    path: &str,
    query: &str,
    body: &[u8],
    timestamp: u64,
    nonce: &str,
) -> HashMap<String, String> {
    let body_hash = Sha256::digest(body);
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        path,
        query,
        hex_lower(&body_hash),
        timestamp,
        nonce
    );
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).expect("hmac accepts any key");
    mac.update(canonical.as_bytes());
    HashMap::from([
        ("DJP-Key-ID".to_string(), key_id.to_string()),
        ("DJP-Timestamp".to_string(), timestamp.to_string()),
        ("DJP-Nonce".to_string(), nonce.to_string()),
        (
            "DJP-Signature".to_string(),
            hex_lower(&mac.finalize().into_bytes()),
        ),
    ])
}

fn verify_stripe_signature(secret: &str, headers: &HeaderMap, body: &[u8]) -> anyhow::Result<()> {
    let header = header_str(headers, "stripe-signature")
        .ok_or_else(|| anyhow::anyhow!("stripe-signature missing"))?;
    let mut timestamp = "";
    let mut signatures = Vec::new();
    for part in header.split(',') {
        let mut pair = part.splitn(2, '=');
        match (
            pair.next().unwrap_or_default(),
            pair.next().unwrap_or_default(),
        ) {
            ("t", value) => timestamp = value,
            ("v1", value) => signatures.push(value),
            _ => {}
        }
    }
    if timestamp.is_empty() || signatures.is_empty() {
        anyhow::bail!("stripe signature header invalid");
    }
    let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(body));
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())?;
    mac.update(signed_payload.as_bytes());
    let expected = hex_lower(&mac.finalize().into_bytes());
    if !signatures
        .iter()
        .any(|sig| expected.eq_ignore_ascii_case(sig))
    {
        anyhow::bail!("stripe webhook signature mismatch");
    }
    Ok(())
}

async fn paypal_access_token(config: &Value) -> anyhow::Result<String> {
    let base = config_str_or(config, "base_url", "https://api-m.sandbox.paypal.com");
    let response = Client::builder()
        .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
        .build()?
        .post(format!("{}/v1/oauth2/token", base.trim_end_matches('/')))
        .basic_auth(
            str_config(config, "client_id"),
            Some(str_config(config, "client_secret")),
        )
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    response
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("paypal access token missing"))
}

async fn verify_paypal_webhook(
    config: &Value,
    headers: &HeaderMap,
    body: &[u8],
) -> anyhow::Result<()> {
    let base = config_str_or(config, "base_url", "https://api-m.sandbox.paypal.com");
    let token = paypal_access_token(config).await?;
    let event: Value = serde_json::from_slice(body)?;
    let payload = json!({
        "auth_algo": header_str(headers, "PAYPAL-AUTH-ALGO").unwrap_or_default(),
        "cert_url": header_str(headers, "PAYPAL-CERT-URL").unwrap_or_default(),
        "transmission_id": header_str(headers, "PAYPAL-TRANSMISSION-ID").unwrap_or_default(),
        "transmission_sig": header_str(headers, "PAYPAL-TRANSMISSION-SIG").unwrap_or_default(),
        "transmission_time": header_str(headers, "PAYPAL-TRANSMISSION-TIME").unwrap_or_default(),
        "webhook_id": str_config(config, "webhook_id"),
        "webhook_event": event
    });
    let response = Client::builder()
        .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
        .build()?
        .post(format!(
            "{}/v1/notifications/verify-webhook-signature",
            base.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    if response.get("verification_status").and_then(Value::as_str) != Some("SUCCESS") {
        anyhow::bail!("paypal webhook verification failed");
    }
    Ok(())
}

fn wechat_authorization(
    config: &Value,
    method: &str,
    path: &str,
    query: &str,
    body: &str,
) -> anyhow::Result<String> {
    let timestamp = unix_ts().to_string();
    let nonce = random_hex(16);
    let canonical = format!("{method}\n{path}{query}\n{timestamp}\n{nonce}\n{body}\n");
    let signature = rsa_sign_base64(&str_config(config, "merchant_private_key"), &canonical)?;
    Ok(format!(
        "WECHATPAY2-SHA256-RSA2048 mchid=\"{}\",nonce_str=\"{}\",signature=\"{}\",timestamp=\"{}\",serial_no=\"{}\"",
        str_config(config, "mchid"),
        nonce,
        signature,
        timestamp,
        str_config(config, "merchant_serial_no")
    ))
}

fn wechat_decrypt_resource(config: &Value, resource: &Value) -> anyhow::Result<Value> {
    let key = str_config(config, "api_v3_key");
    if key.as_bytes().len() != 32 {
        anyhow::bail!("wechatpay api_v3_key must be 32 bytes");
    }
    let nonce = resource
        .get("nonce")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("wechatpay resource missing nonce"))?;
    let ciphertext = resource
        .get("ciphertext")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("wechatpay resource missing ciphertext"))?;
    let associated_data = resource
        .get("associated_data")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())?;
    let ciphertext = general_purpose::STANDARD.decode(ciphertext)?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(nonce.as_bytes()),
            Payload {
                msg: &ciphertext,
                aad: associated_data.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("wechatpay resource decrypt failed"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .or_else(|| {
            headers
                .iter()
                .find(|(key, _)| key.as_str().eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        })
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
}

fn random_hex(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex_lower(&bytes)
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_zero_decimal_currency(currency: &str) -> bool {
    matches!(
        currency.to_ascii_uppercase().as_str(),
        "BIF"
            | "CLP"
            | "DJF"
            | "GNF"
            | "JPY"
            | "KMF"
            | "KRW"
            | "MGA"
            | "PYG"
            | "RWF"
            | "UGX"
            | "VND"
            | "VUV"
            | "XAF"
            | "XOF"
            | "XPF"
    )
}
