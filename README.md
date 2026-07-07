<h4 align="right"><a href="README_EN.md">English</a> | <strong>简体中文</strong></h4>

<p align="center">
  <img src="web-admin/src/assets/svg-icon/logo.svg" alt="dujiao-rust logo" width="72" height="72">
</p>

<h1 align="center">dujiao-rust</h1>

<p align="center"><strong>基于 Rust 的单二进制订单/卡密发卡系统，保留独角数卡前台体验，无需 Redis 或 MySQL。</strong></p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-axum-000000?style=flat-square&logo=rust">
  <img alt="Frontend" src="https://img.shields.io/badge/storefront-SSR%20%2B%20SPA-2563eb?style=flat-square">
  <img alt="Runtime" src="https://img.shields.io/badge/runtime-single%20binary-16a34a?style=flat-square">
  <img alt="Storage" src="https://img.shields.io/badge/storage-SQLite-0f766e?style=flat-square">
</p>

`dujiao-rust` 将 [独角数卡](https://github.com/assimon/dujiaoka) 的核心卖卡链路重构为 Rust 实现：前台沿用 `luna` / `hyper` / `unicorn` 三套主题的服务端渲染页面，后台为内嵌 Vue SPA，支付、卡密、履约与设置全部落在同一个进程和 SQLite 数据库中。

典型购买链路：

```text
https://shop.example.com/buy/<id>
  -> minijinja 前台（主题 + 静态资源）
  -> POST /create-order（预占卡密 + 创建支付单）
  -> 支付网关回调 / 链上确认
  -> 自动发卡 + 邮件/通知
  -> /detail-order-sn/<order_no> 查单
```

项目面向 **单机自托管、稳定卖卡** 场景：一个 binary + 一个 SQLite 文件 + `uploads/` 目录即可运行。内嵌 SQLite job worker 处理订单过期与清理，不依赖 Redis、MySQL、消息队列或外部数据库。当前目标是有序的商品/卡密/订单/支付/履约闭环；不追求完整复刻 Dcat Admin，也不实现钱包、会员等级、分销、上游采购、Telegram 登录或 OAuth。

## 特性

- 单二进制部署：`dujiao-rust` 同时承载前台 SSR、支付回调、内嵌 worker 与管理后台 API；admin SPA 构建后通过 `rust-embed` 编入 binary。
- 前台体验对齐独角数卡：`luna` / `hyper` / `unicorn` 三套主题、原路由习惯与购买流程；商品展示、下单、账单、支付、查单页面由 `minijinja` 渲染。
- 卡密状态机：`available` → `reserved` → `used`，下单先预占、超时释放、支付后履约，降低超卖风险。
- 支付 Provider 注册表：统一 `payments` 表与幂等回调校验，内置 `epay`、`tokenpay`、`epusdt`、`bepusdt`、`dujiaopay`、`okpay`、`evm-local`、官方支付宝/微信/Stripe/PayPal 等通道。
- 管理后台：商品/分类/卡密、订单导出与详情、支付通道、优惠码、邮件模板、上传资源、备份与计划任务、SMTP/多通道通知、系统设置。
- 人机防护：内置算术图形验证码（`/captcha/:id`）、邮箱/IP 下单频控、登录失败锁定；不支持 Geetest 极验。
- 设置持久化：`settings` 表保存站点、订单、主题、验证码、安全、SMTP、通知等 JSON 配置；敏感字段用 `data/app.secret` 本地加密。
- 备份运维：`/admin/backup` 在线一致性 SQLite 快照（gzip），默认每周计划备份；`/healthz` 与 `--healthcheck` 供探活。
- 反向代理友好：识别 `X-Forwarded-Proto` / `X-Forwarded-For`，可配置信任反代层数；推荐 Cloudflare Tunnel 或橙云代理前置，应用侧走 HTTP。

## 快速开始

先构建 admin SPA 并编译二进制：

```bash
./build.sh --release
```

准备配置并启动：

```bash
cp config.example.toml config.toml
# 首次运行前请修改 admin.bootstrap_password（默认 admin123456 会拒绝自动初始化）
./target/release/dujiao-rust
```

首次初始化：

```text
http://127.0.0.1:8080/install
```

完成 owner 账号创建后，进入管理后台：

```text
http://127.0.0.1:8080/admin
```

必做配置：

1. `/admin/settings` — 设置对外 `base_url`、主题、SMTP、通知、验证码与安全项。
2. `/admin/payment-channels` — 添加支付通道（详见 [运维与支付配置](docs/README.md)）。
3. `/admin/products` / `/admin/cards` — 创建商品并导入卡密。

Docker Compose（可选）：

```bash
docker compose up -d
```

会在宿主机暴露 `8080`；TLS 建议在 Cloudflare 侧终止，详见下文部署说明。

## 本地验证

开发和发布前建议执行：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
./build.sh --release
```

基础 smoke：

```bash
curl -s http://127.0.0.1:8080/healthz
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8080/
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8080/admin/
```

集成测试覆盖并发不超卖、支付金额校验、回调幂等、优惠码退款、取消释放卡密等场景，见 [`tests/order_flow.rs`](tests/order_flow.rs) 与 [`tests/frontend_routes.rs`](tests/frontend_routes.rs)。

## 运维能力

管理后台默认位于 `/admin`，首次安装位于 `/install`。登录后可完成：

- 商品与分类管理，卡密批量导入/导出，上传图片资源。
- 订单列表、详情、导出 CSV、人工补发、取消、标记异常、重发邮件。
- 支付通道增删改、配置校验、沙箱/实付验收前的启停控制。
- 优惠码、邮件模板、站点/订单/主题/验证码/安全/SMTP/通知设置。
- SQLite 备份与下载、计划备份策略、邮件测试发送。
- 内嵌 worker 任务与过期订单/会话/验证码清理（无需额外进程）。

### 生产部署要点

TLS **不由本程序终止**。推荐前置 Cloudflare，应用在绑定端口上提供 HTTP；检测到 `X-Forwarded-Proto: https` 时自动为 Cookie 打上 `Secure`。

在 `/admin/settings → 验证码与安全` 中建议：

- **Cookie Secure** ✓（`localhost` 或 `X-Forwarded-Proto: http` 时会自动降级）
- **信任反代层数** = `1`（Cloudflare 在 `X-Forwarded-For` 前追加一跳）

部署方式二选一：

**A) Docker Compose + Cloudflare Tunnel / 橙云代理**

```bash
docker compose up -d
cloudflared tunnel run --url http://127.0.0.1:8080 dujiao
```

**B) systemd + Cloudflare**

```bash
sudo cp target/release/dujiao-rust /opt/dujiao-rust/
sudo cp deploy/dujiao-rust.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now dujiao-rust
```

备份与恢复：

```bash
# 在 /admin/backup 创建并下载 backup.sqlite.gz

systemctl stop dujiao-rust
zcat backup.sqlite.gz > /opt/dujiao-rust/data/dujiao.db
systemctl start dujiao-rust
```

迁移或恢复时请一并保留 `data/app.secret`、`data/dujiao.db`、`uploads/`；丢失密钥会导致加密的 SMTP/通知配置需重新填写。

## 关键配置

静态配置写在 `config.toml`；运行时站点/订单/支付等业务配置保存在 SQLite `settings` 表，可通过 `/admin/settings` 修改。数据库路径决定 `uploads/`、`data/backups/`、`data/app.secret` 等派生路径的父目录。

| 领域 | 配置 |
| --- | --- |
| 监听 | `[server] host`、`port` |
| 数据库 | `[database] path`（默认旁路生成 `uploads/` 与 `app.secret`） |
| 站点默认值 | `[site] name`、`base_url`、`theme`、`order_expire_minutes` |
| 管理员 | `[admin] route_prefix`、`bootstrap_username`、`bootstrap_password`、`app_secret` |
| 环境变量 | `DUJIAO_CONFIG`、`DUJIAO_ALLOW_DEFAULT_ADMIN`、`RUST_LOG` |
| 反代与安全 | `trust_proxy_hops`、`cookie_secure`、登录失败锁定、限购窗口（admin 设置） |
| 验证码 | `is_open_img_code`（算术图形验证码，无 Geetest） |
| 支付 | Provider / Channel / `pay_check` / 回调 URL（admin 支付通道） |

`bootstrap_password` 仍为默认值 `admin123456` 时，程序**拒绝**自动创建管理员，必须通过 `/install` 完成初始化。设置页不回显密文；字段显示 `********` 时重新保存会保留原密文。

## 文档

- [运维与支付配置](docs/README.md) — 部署、备份、健康检查、各支付通道配置指南
- [设计与实施记录](PLAN.md) — 重构目标、数据模型与阶段性实施日志
- [systemd 单元示例](deploy/dujiao-rust.service)
- [配置样例](config.example.toml)
