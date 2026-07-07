<h4 align="right"><strong>English</strong> | <a href="README.md">简体中文</a></h4>

<p align="center">
  <img src="web-admin/src/assets/svg-icon/logo.svg" alt="free-market logo" width="72" height="72">
</p>

<h1 align="center">freeMarket</h1>

<p align="center"><strong>A single-binary Rust order/card-secret storefront on SQLite, without Redis or MySQL.</strong></p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-axum-000000?style=flat-square&logo=rust">
  <img alt="Frontend" src="https://img.shields.io/badge/storefront-SSR%20%2B%20SPA-2563eb?style=flat-square">
  <img alt="Runtime" src="https://img.shields.io/badge/runtime-single%20binary-16a34a?style=flat-square">
  <img alt="Storage" src="https://img.shields.io/badge/storage-SQLite-0f766e?style=flat-square">
</p>

`freeMarket` (binary `free-market`) is a Rust order/card-secret system: the public storefront uses server-rendered `luna` / `hyper` / `unicorn` themes, the admin UI is an embedded Vue SPA, and payments, card secrets, fulfillment, and settings all live in one process and one SQLite database.

Typical purchase flow:

```text
https://shop.example.com/buy/<id>
  -> minijinja storefront (theme + static assets)
  -> POST /create-order (reserve cards + create payment)
  -> payment provider callback / on-chain confirmation
  -> auto fulfillment + email/notification
  -> /detail-order-sn/<order_no> order lookup
```

The project targets **single-node self-hosted, stable card sales**: one binary, one SQLite file, and an `uploads/` directory. An embedded SQLite job worker handles order expiry and cleanup. No Redis, MySQL, message queue, or external database is required. The current scope is a clean product/card/order/payment/fulfillment loop. It does not aim to fully replicate Dcat Admin, and it does not implement wallets, membership tiers, distribution, upstream procurement, Telegram login, or OAuth.

## Features

- Single-binary deployment: `free-market` serves the SSR storefront, payment callbacks, embedded worker, and admin APIs. The admin SPA is built and embedded into the binary via `rust-embed`.
- Mature storefront themes: `luna` / `hyper` / `unicorn` with full purchase flow rendered via `minijinja`.
- Card-secret state machine: `available` → `reserved` → `used`, with reserve-on-order, timeout release, and post-payment fulfillment to reduce oversell risk.
- Payment provider registry: unified `payments` table and idempotent callback validation. Built-in channels include `epay`, `tokenpay`, `epusdt`, `bepusdt`, `freemarketpay`, `okpay`, `evm-local`, and official Alipay/WeChat/Stripe/PayPal integrations.
- Admin dashboard: products/categories/card secrets, order export/detail, payment channels, coupons, email templates, uploads, backup/scheduling, SMTP/multi-channel notifications, and system settings.
- Anti-abuse: built-in arithmetic image captcha (`/captcha/:id`), email/IP purchase rate limits, and login lockout. Geetest is not supported.
- Persistent settings: the `settings` table stores site/order/theme/captcha/security/SMTP/notification JSON. Sensitive values are encrypted with the local key in `data/app.secret`.
- Backup operations: online consistent SQLite snapshots (gzip) from `/admin/backup`, with weekly scheduled backups by default. `/healthz` and `--healthcheck` are available for probes.
- Reverse-proxy friendly: recognizes `X-Forwarded-Proto` / `X-Forwarded-For` with configurable trusted proxy hops. Cloudflare Tunnel or orange-cloud proxy in front with plain HTTP on the app side is the recommended setup.

## Quick Start

Build the admin SPA and binary:

```bash
./build.sh --release
```

Prepare config and run:

```bash
cp config.example.toml config.toml
# Change admin.bootstrap_password before first run (default admin123456 blocks auto-init)
./target/release/free-market
```

First-time setup:

```text
http://127.0.0.1:8080/install
```

After creating the owner account, open the admin UI:

```text
http://127.0.0.1:8080/admin
```

Required setup:

1. `/admin/settings` — public `base_url`, theme, SMTP, notifications, captcha/security.
2. `/admin/payment-channels` — add payment channels (see [Operations & payment guide](docs/README.md)).
3. `/admin/products` / `/admin/cards` — create products and import card secrets.

Optional Docker Compose:

```bash
docker compose up -d
```

This exposes port `8080` on the host. Terminate TLS at Cloudflare; see deployment notes below.

## Local Validation

Recommended checks before development handoff or release:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
./build.sh --release
```

Basic smoke:

```bash
curl -s http://127.0.0.1:8080/healthz
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8080/
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8080/admin/
```

Integration tests cover concurrent oversell safety, payment amount validation, callback idempotency, coupon refunds, cancel/release behavior, and more. See [`tests/order_flow.rs`](tests/order_flow.rs) and [`tests/frontend_routes.rs`](tests/frontend_routes.rs).

## Operations

The admin dashboard lives at `/admin`; first-time installation is at `/install`. After login, operators can:

- Manage products/categories, import/export card secrets, and upload images.
- View order lists/details, export CSV, manually fulfill, cancel, mark abnormal orders, and resend emails.
- Create/update payment channels, validate configs, and enable channels only after sandbox or small-amount acceptance.
- Manage coupons, email templates, and site/order/theme/captcha/security/SMTP/notification settings.
- Create/download SQLite backups, configure scheduled backups, and send test emails.
- Run the embedded worker for order expiry and cleanup of sessions/captcha/logs without extra processes.

### Production deployment notes

TLS termination is **not** handled by this app. Put Cloudflare in front and serve plain HTTP on the bound port. When `X-Forwarded-Proto: https` is detected, cookies are marked `Secure` automatically.

Recommended values in `/admin/settings → 验证码与安全`:

- **Cookie Secure** ✓ (auto-downgrades for `localhost` / `X-Forwarded-Proto: http`)
- **Trust proxy hops** = `1` (Cloudflare prepends one hop in `X-Forwarded-For`)

Pick one deployment path:

**A) Docker Compose + Cloudflare Tunnel / proxied DNS**

```bash
docker compose up -d
cloudflared tunnel run --url http://127.0.0.1:8080 freemarket
```

**B) systemd + Cloudflare**

```bash
sudo cp target/release/free-market /opt/free-market/
sudo cp deploy/free-market.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now free-market
```

Backup and restore:

```bash
# Create and download backup.sqlite.gz from /admin/backup

systemctl stop free-market
zcat backup.sqlite.gz > /opt/free-market/data/freemarket.db
systemctl start free-market
```

Preserve `data/app.secret`, `data/freemarket.db`, and `uploads/` together when migrating or restoring. Losing the secret key means encrypted SMTP/notification settings may need to be re-entered.

## Key Configuration

Static config lives in `config.toml`. Runtime business settings are stored in the SQLite `settings` table and editable from `/admin/settings`. The database path determines sibling paths such as `uploads/`, `data/backups/`, and `data/app.secret`.

| Area | Settings |
| --- | --- |
| Listen | `[server] host`, `port` |
| Database | `[database] path` (also anchors `uploads/` and `app.secret`) |
| Site defaults | `[site] name`, `base_url`, `theme`, `order_expire_minutes` |
| Admin | `[admin] route_prefix`, `bootstrap_username`, `bootstrap_password`, `app_secret` |
| Environment | `FREEMARKET_CONFIG`, `FREEMARKET_ALLOW_DEFAULT_ADMIN`, `RUST_LOG` |
| Proxy/security | `trust_proxy_hops`, `cookie_secure`, login lockout, purchase rate limits (admin settings) |
| Captcha | `is_open_img_code` (arithmetic image captcha; no Geetest) |
| Payments | provider / channel / `pay_check` / callback URLs (admin payment channels) |

While `bootstrap_password` remains the well-known default `admin123456`, the binary **refuses** to auto-create an admin and requires `/install`. The settings page never echoes ciphertext; saving a field that still shows `********` preserves the existing secret.

## Documentation

- [Operations & payment guide](docs/README.md) — deployment, backup, health checks, provider configuration
- [Design & implementation log](PLAN.md) — architecture, data model, and milestone notes
- [systemd unit example](deploy/free-market.service)
- [Config sample](config.example.toml)

## Acknowledgements

Storefront themes and UX patterns originate from the open-source project [Dujiaoka (独角数卡)](https://github.com/assimon/dujiaoka). freeMarket reimplements trading, payments, and admin flows in Rust; it is not an official dujiaoka fork.
