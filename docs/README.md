# freeMarket

基于 Rust 的单二进制订单/卡密系统，技术栈：`axum` + `sqlx + SQLite (WAL)` + `minijinja`。不依赖 Redis、MySQL、消息队列 —— 一个二进制 + 一个 SQLite 文件 + 一个 uploads 目录。

## 快速开始

```bash
# 1. 编译 release 二进制
./build.sh                         # debug 构建；生产环境请用 cargo build --release
# 或构建 Docker 镜像
docker build -t free-market:latest .

# 2. 准备配置文件（参考 config.example.toml）
cp config.example.toml config.toml
# 重要：首次运行前请修改 admin.bootstrap_password（B2 防护）
# 程序会自动创建 data/app.secret 用于加密敏感设置

# 3. 启动
./target/release/free-market
# 然后访问 http://127.0.0.1:8080/install 创建第一个管理员
```

## 生产部署

TLS 终端**不归**本程序负责。推荐方案：前置 Cloudflare，程序在内网走 HTTP。程序会通过 `X-Forwarded-Proto: https` 检测 HTTPS，并自动给 Cookie 打上 `Secure` 标记。

下面两种部署方式**二选一**。

### A) Docker Compose + Cloudflare

```bash
docker compose up -d
```

会在宿主机暴露 `8080` 端口。然后：

1. **Cloudflare Tunnel**（`cloudflared`）—— 推荐：无需开放公网端口，无防火墙缺口。
   ```bash
   cloudflared tunnel create freemarket
   cloudflared tunnel route dns freemarket your-domain.example.com
   cloudflared tunnel run --url http://127.0.0.1:8080 freemarket
   ```
2. **Cloudflare 代理 DNS**（橙色云朵）：A 记录指向服务器 IP 并启用代理。SSL/TLS 模式设为 **Full**（Cloudflare ↔ 源站走内网 HTTP，浏览器 ↔ Cloudflare 走 HTTPS）。

然后在 `/admin/settings → 验证码与安全` 中设置：

- **Cookie Secure** ✓ （保持启用 —— `localhost` 与 `X-Forwarded-Proto: http` 时程序会自动降级）
- **信任反代层数** = `1`（Cloudflare 会在 `X-Forwarded-For` 前追加一跳）

人机校验仅支持内置算术图形验证码（`/captcha/:id`）及邮箱/IP 下单频控，不支持 Geetest 极验。

### B) systemd 单元 + Cloudflare 隧道

```bash
sudo cp target/release/free-market /opt/free-market/
sudo cp deploy/free-market.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now free-market
```

再把 `cloudflared` 作为另一个 systemd 单元运行，指向 `http://127.0.0.1:8080`。不需要 nginx，也不需要在服务器上管理 TLS 证书。

## 首次启动必做项

1. 访问 `http://your-host/install` —— 当 `bootstrap_password` 仍是默认值 `admin123456` 时，程序**拒绝**自动初始化管理员。使用 `/install` 创建一个强密码的 owner。若想强制保留旧的自动初始化行为（不推荐），设置 `FREEMARKET_ALLOW_DEFAULT_ADMIN=1`。
2. 进入 `/admin/settings`：
   - 设置对外可访问的 `site.base_url`（用于邮件链接与支付 return_url）
   - 把「信任反代层数」设为与反向代理层数一致（nginx 直接前置时为 1）
   - 在 HTTPS 后面运行时开启 `Cookie Secure`
   - 配置 SMTP、通知通道等
3. 在 `/admin/payment-channels` 中添加支付通道。详细配置见下文「支付通道配置指南」。

## 配置参考

| 文件 | 用途 |
| --- | --- |
| `config.toml` | 静态配置（监听 host/port、数据库路径、admin 路由前缀、bootstrap 管理员） |
| `data/freemarket.db` | SQLite 数据库（WAL 模式 + 外键开启）。可在 `/admin/backup` 安全备份（一致性 SQLite 快照 gzip 流） |
| `data/app.secret` | 自动生成的本地加密密钥，用于加密敏感设置。请与数据库、uploads 一并保留 |
| `uploads/` | 商品图片及管理员上传的资源 |
| `logs/` | 应用日志（同时输出到 stdout） |

环境变量：

| 变量 | 用途 |
| --- | --- |
| `FREEMARKET_CONFIG` | TOML 配置路径（默认 `./config.toml`） |
| `FREEMARKET_ENABLE_INSTALL` | 即使设置了强密码，也强制跳过自动初始化 |
| `FREEMARKET_ALLOW_DEFAULT_ADMIN` | 默认密码下重新启用自动初始化（不推荐） |
| `RUST_LOG` | tracing 过滤器，例如 `free_market=debug,tower_http=info` |

## 备份与恢复

在 `/admin/backup` 可从后台界面发起手工备份，并在备份历史表中下载。
默认开启计划备份：每周一服务器 UTC 时间 08:00 执行，保留 `data/backups/` 下最新的 7 份。

```bash
# 在 UI 备份后，从 /admin/backup/files/<filename> 下载所选备份文件

# 离线恢复
systemctl stop free-market
zcat backup.sqlite.gz > /opt/free-market/data/freemarket.db
systemctl start free-market
```

数据库中的敏感字段通过 `data/app.secret` 中自动生成的本地密钥加密。迁移或恢复时，请将 `data/app.secret`、`data/freemarket.db`、`uploads/` 一并保留；丢失密钥可能导致加密的 SMTP、通知设置需要重新填写。

## 健康检查

```bash
curl -s https://your-host/healthz
# {"db":"ok","status":"ok","uptime_secs":1234,"version":"0.1.0","worker":"worker-…"}

./free-market --healthcheck
# 配置的 SQLite 数据库可达时退出码为 0
```

`status: degraded` + `db: down` 表示 SQLite ping 失败；程序仍保持运行，由负载均衡器决定如何处理。
容器健康检查使用 `--healthcheck`，运行镜像不需要安装 curl 或 wget。

## 运行测试

```bash
cargo test                         # 单元测试 + 集成测试
cargo test --test order_flow       # 只跑集成测试
```

六个集成测试覆盖：并发不超卖、金额不一致拒付、成功回调幂等、优惠码退款幂等、取消订单释放卡密、循环卡密单次限购 1 张。详见 [`tests/order_flow.rs`](tests/order_flow.rs)。

## 运维注意事项

- 你只需要跑这**一个**二进制。SQLite job worker 是内嵌的，无需额外进程。
- `Cookie Secure` 默认 `true`（生产安全）。本地纯 HTTP 调试时可在 `/admin/settings` 中关掉。
- `trust_proxy_hops = 0` 表示「忽略 `X-Forwarded-For`」 —— 没有反向代理时这是正确值。
- 登录表单按 `(用户名, 客户端 IP)` 维度限流。可通过「安全 → 登录失败次数 / 锁定分钟」调节。
- 设置页**永远不会**把密文回显；保存时如果字段仍显示 `********`，密文会原样保留。
- 所有 admin 路由都在 `admin.route_prefix`（默认 `/admin`）下。改成非常规路径可以挡掉简单扫描器。
- 真实支付通道必须经过沙箱或小额实付验收后才允许上生产；仅完成代码注册并不够。

---

# 支付通道配置指南

本节说明 `free-market` 后台支付通道的技术框架和接入配置方式。面向部署和运营配置，不展开代码实现细节。

## 1. 支付通道框架

`free-market` 使用统一的支付通道模型管理不同支付平台。每个通道由三部分组成：

| 概念 | 说明 |
| --- | --- |
| Provider | 支付服务类型，例如 `epay`、`tokenpay`、`epusdt`。 |
| Channel | 同一 Provider 下的具体支付渠道，例如 `alipay`、`wxpay`、`usdt`。 |
| Config | 该通道的商户参数、网关地址、密钥、币种等配置。 |

后台入口：

```text
/admin/payment-channels
```

前台下单时，系统会根据商品、客户端范围和通道启用状态展示可用支付方式。

## 2. 通用字段说明

新增或编辑支付通道时，后台会显示以下字段：

| 字段 | 说明 |
| --- | --- |
| 支付类型 | 先选择 `模拟`、`内置` 或 `外部`。模拟只用于本地联调；内置是由 free-market 自己处理的收款能力；外部是第三方支付网关。 |
| 支付方式 | 在所选支付类型下选择具体方式。当前内置方式为 `基于 Alchemy ERC20 Token`，对应 Provider `evm-local`。 |
| 显示名称 | 前台展示的支付方式名称，例如"支付宝"、"USDT-TRC20"。 |
| 支付提供方 | Provider 类型。当前常用值：`noop`、`epay`、`tokenpay`、`evm-local`、`epusdt`、`bepusdt`、`freemarketpay`、`okpay`、`official`。 |
| 渠道类型 | 具体支付渠道。常见值：`alipay`、`wxpay`、`qqpay`、`usdt`、`trx`、`stripe`、`paypal`、`wechat`、`test`。 |
| 旧支付标识 `pay_check` | 兼容旧版前台图标和旧支付标识，建议填写 `alipay`、`wxpay`、`qqpay`、`usdt` 等。 |
| 客户端范围 | `全部`、`PC`、`移动端`。用于控制不同设备看到的支付方式。 |
| 旧 handleroute | 兼容旧支付路由习惯，例如 `pay/yipay`、`pay/tokenpay`、`pay/epusdt`。 |
| 交互方式 | `跳转` 或 `二维码`。跳转类打开支付页面，二维码类显示扫码或支付内容。 |
| 商户 ID | 支付平台分配的商户号、PID、用户 ID、币种等，按 Provider 要求填写。 |
| 商户 Key/Token | 支付平台密钥、Token 或签名密钥。 |
| 网关地址/Pem | 支付网关 URL，或部分平台所需的 Pem/网关参数。 |
| 配置 JSON | Provider 专属配置。高级配置以 JSON 对象填写。 |
| 启用 | 开启后前台才会展示该通道。 |

说明：

- `商户 ID`、`商户 Key/Token`、`网关地址/Pem` 会辅助生成或补充配置 JSON。
- 若配置 JSON 已显式填写同名字段，以配置 JSON 为准。
- 配置 JSON 必须是合法 JSON 对象。
- 支付密钥属于敏感信息，不应截图、外发或写入公开文档。

## 3. 客户端范围

| 选项 | 适用场景 |
| --- | --- |
| 全部 | PC 和移动端都展示。 |
| PC | 仅桌面浏览器展示。 |
| 移动端 | 手机浏览器、微信、移动设备展示。 |

建议：

- 支付宝 H5、微信 H5、移动钱包类通道可设为"移动端"。
- 扫码支付、电脑网页支付可设为"PC"或"全部"。
- 同一 Provider 可以创建多个通道，分别服务 PC 和移动端。

## 4. 支付方式图标与前台展示

前台 Luna 主题会根据 `pay_check` 决定支付按钮图标和展示习惯。

常用建议：

| 支付类型 | 推荐 `pay_check` |
| --- | --- |
| 支付宝 | `alipay` |
| 微信支付 | `wxpay` |
| QQ 钱包 | `qqpay` |
| USDT / 虚拟币 | `usdt` |
| 其他支付 | `other` |

若 `pay_check` 未命中已知图标，前台会使用通用支付图标。

## 5. 模拟支付 Noop

仅用于本地联调和首次部署的业务链路验证，不接入真实支付平台。生产环境必须禁用。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `noop` |
| 渠道类型 | `test`（任意值都会被接受） |
| 旧支付标识 | `other` |
| 配置 JSON | `{}` |

启动后下单 → 支付页会显示一个「模拟支付成功」按钮，点击直接走支付成功回调，自动发卡、邮件通知、订单状态推进的整条链路都会跑一遍。

适用场景：

- 验证 SMTP / 通知通道是否真的可用。
- 自动发卡 / 重复购买限制 / 优惠码退款的端到端验证。
- 上线前最后一次冒烟。

## 6. Epay / Yipay 通道

适用于兼容易支付协议（彩虹易支付、码支付、各类二开易支付）的网关。一个易支付站点可以同时开 `alipay` / `wxpay` / `qqpay` 三条通道，分别建一条记录。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `epay`（或 `yipay`，效果完全一致） |
| 渠道类型 | `alipay` / `wxpay` / `qqpay` |
| 旧支付标识 | 同渠道类型 |
| 交互方式 | 跳转 |

接入准备（易支付商户后台）：

1. 申请商户：拿到 **商户 ID（PID）**、**商户密钥（KEY）**。
2. 在「网址列表」/「同步异步通知」中预先填写本站回调地址：
   - 异步通知：`https://你的域名/payment/callback/epay/alipay`（按渠道类型替换）
   - 同步跳转：`https://你的域名/pay/epay/return_url`
3. v2 协议（RSA 签名）需要在商户后台生成商户公私钥对，下载 PEM 格式的私钥。

完整配置字段（来自 `EpayProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `gateway_url` | ✓ | — | 易支付网关根地址，例 `https://pay.example.com`，**不带末尾斜杠** |
| `merchant_id` / `pid` | ✓ | — | 商户 ID（PID），两个 key 任填其一 |
| `merchant_key` / `key` | v1 必填 | — | v1 MD5 签名密钥 |
| `private_key` | v2 必填 | — | v2 RSA 商户私钥，PKCS8 或 PKCS1 PEM 均可 |
| `epay_version` | | `v1` | `v1`（MD5）或 `v2`（RSA） |
| `submit_path` | | v1 `/submit.php`，v2 `/api/pay/submit` | 创建订单接口路径 |
| `method` | | `web` | v2 专用，部分网关支持 `web` / `wap` / `qr` |
| `device` | | `pc` | v1 专用，`pc` / `mobile` |

v1（默认，最常见）配置示例：

```json
{
  "gateway_url": "https://pay.example.com",
  "pid": "10001",
  "key": "your-epay-merchant-key"
}
```

v2（RSA 签名）配置示例：

```json
{
  "gateway_url": "https://pay.example.com",
  "epay_version": "v2",
  "merchant_id": "10001",
  "private_key": "-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----",
  "merchant_key": "fallback-md5-key-for-callback"
}
```

> v2 异步回调验签当前回退到 MD5（与多数易支付二开版本一致），所以 v2 仍建议同时保留 `merchant_key`。

回调地址（推荐使用 `/payment/callback/...` 新路径，旧路径仅做兼容）：

| 类型 | 地址 |
| --- | --- |
| 异步通知（推荐） | `/payment/callback/epay/{channel_type}` |
| 异步通知（兼容旧路由） | `/pay/yipay/notify_url`、`/pay/epay/notify_url` |
| 同步返回 | `/pay/epay/return_url` |

常见错误：

- **签名错误**：`gateway_url` 末尾带了 `/submit.php`（应只填到域名根）；`pid`/`key` 与易支付后台不一致；商户密钥前后有空格。
- **拒绝回调**：易支付后台未配置本站异步通知地址，或站点不能被外网访问。
- **金额不一致**：易支付有些版本会用 `total_fee`（分），有些用 `money`（元）；本程序两者都识别。

## 7. TokenPay 通道

适用于 TokenPay 系（含 TokenPay 原版、ChinaPay、虚拟币聚合）支付网关。通常用于 USDT / TRX 收款。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `tokenpay` |
| 渠道类型 | `usdt` / `trx` / 平台约定值 |
| 旧支付标识 | `usdt` |
| 交互方式 | 跳转 或 二维码 |

接入准备：

1. 拿到 TokenPay 商户的 **回调验签密钥（NotifySecret / Token）**。
2. 在商户后台 / 配置文件中预先登记异步通知地址：`https://你的域名/payment/callback/tokenpay/{channel_type}`。

完整配置字段（来自 `TokenPayProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `gateway_url` | ✓ | — | TokenPay 网关地址 |
| `notify_secret` / `token` / `key` | ✓ | — | 回调验签密钥，三个 key 任填其一 |
| `currency` | | `USDT` | TokenPay 端币种，例 `USDT`、`USDT_TRC20` |
| `create_path` | | `/CreateOrder` | 创建订单接口路径 |
| `base_currency` | | `CNY` | 回调记录用的法币币种 |

配置示例：

```json
{
  "gateway_url": "https://tokenpay.example.com",
  "notify_secret": "your-tokenpay-secret",
  "currency": "USDT",
  "base_currency": "CNY"
}
```

回调地址：

| 类型 | 地址 |
| --- | --- |
| 异步通知（推荐） | `/payment/callback/tokenpay/{channel_type}` |
| 异步通知（兼容旧路由） | `/pay/tokenpay/notify_url` |

请求 / 回调字段（参考）：创建订单提交 `OutOrderId`、`OrderUserKey`、`ActualAmount`、`Currency`、`NotifyUrl`、`RedirectUrl` 加 `Signature`；回调接受 JSON 或表单，`Status=1` 视为成功。

常见错误：

- **缺 pay url**：TokenPay 部分网关返回字段是 `data` 或 `info.PaymentUrl` / `info.QrCodeLink`，三者本程序都识别；若仍报 `tokenpay response missing pay url`，说明上游返回的 JSON 结构不是这三种，需要确认对方文档。
- **回调签名错**：商户后台密钥与 `notify_secret` 不一致；或上游使用了非标准 MD5 排序方式。

能力边界：

- TokenPay 原版插件的 `refund` 方法明确返回“不支持发起退款，请手动操作”。当前本通道按 TokenPay 的创建订单、回调、查单模型接入，不提供自动退款。
- TokenPay 项目内的链上转账能力主要服务 TRON 动态地址归集、手续费转账和能量租用，不等同于商户订单退款。
- 如需退款，建议在后台订单备注或外部财务流程中记录人工退款 tx_hash；不要把 TokenPay 的归集私钥能力直接扩展成自动退款。

## 8. EVM 本地收款

内置 ERC20 收款能力，使用 Alchemy JSON-RPC 的 `eth_getLogs` 监听 ERC20 `Transfer` 事件，不依赖外部 TokenPay/Epusdt 进程，也不依赖区块浏览器 API。首期只监听自有静态地址池，不保存私钥、不自动归集。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `evm-local` |
| 渠道类型 | `bsc-usdt` / `bsc-usdc` / `base-usdc` / `polygon-usdt` / `polygon-usdc` / `arbitrum-usdc` / `optimism-usdc` / `eth-sepolia-usdc` / `base-sepolia-usdc` / `polygon-amoy-usdc` / `arbitrum-sepolia-usdc` / `optimism-sepolia-usdc` / `evm-erc20` |
| 旧支付标识 | `usdt` / `usdc` |
| 交互方式 | 二维码（推荐） |

完整配置字段：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `network_env` | | `mainnet` | 网络环境：`mainnet` 或 `testnet`；未填时会按 `chain_id` / `alchemy_network` 推断 |
| `evm_chain_preset` | | — | 后台表单辅助字段，选择链预设后自动填充链参数 |
| `evm_token_preset` | | — | 后台表单辅助字段，选择 Token 预设后自动填充合约和精度 |
| `alchemy_api_key` / `api_key` | ✓ | — | Alchemy API Key；若直接配置 `rpc_url` 可不填 |
| `rpc_url` | | `https://{alchemy_network}.g.alchemy.com/v2/{alchemy_api_key}` | 自定义 JSON-RPC 地址 |
| `alchemy_network` | ✓ | `bnb-mainnet` | Alchemy network slug |
| `addresses` | ✓ | — | 收款地址池，支持数组或换行字符串 |
| `fiat_per_token` / `rate` | ✓ | `1` | 单个 Token 对应订单法币价格，订单金额除以该值为应付 Token 数量 |
| `chain_id` | ✓ | `56` | EVM chain id；创建支付时会用 `eth_chainId` 校验 RPC |
| `chain_slug` | | `bnb-mainnet` | 内部链标识 |
| `chain_name` | | `BNB Smart Chain` | 支付页展示名称 |
| `scan_host` | | `https://bscscan.com` | 区块浏览器根地址，仅用于展示 |
| `token_symbol` | ✓ | `USDT` | Token 符号 |
| `token_contract` | ✓ | BSC USDT 合约 | ERC20 合约地址 |
| `token_decimals` | | `18` | ERC20 decimals |
| `confirmations` | | `12` | 最低确认数 |
| `amount_precision` | | `6` | 支付识别金额小数位，最大 8 |
| `allow_overpay` | | `false` | 是否允许过付 |
| `overpay_tolerance` | | `0` | 允许过付时的 Token 容差，例如 `0.01` |
| `expire_minutes` | | `30` | 本地支付意图有效期 |
| `log_scan_block_range` | | `10` | 单次 `eth_getLogs` 扫描区块数；BNB 免费层建议保持 10 |
| `max_scan_chunks_per_tick` | | `12` | 每个地址分组每轮最多扫描分片数 |
| `bootstrap_scan_blocks` | | `2000` | 保留配置项；新订单会从创建支付时的最新 block 之后开始扫描 |

BSC USDT 示例：

```json
{
  "alchemy_api_key": "your-alchemy-key",
  "alchemy_network": "bnb-mainnet",
  "chain_id": 56,
  "chain_slug": "bnb-mainnet",
  "chain_name": "BNB Smart Chain",
  "scan_host": "https://bscscan.com",
  "token_symbol": "USDT",
  "token_contract": "0x55d398326f99059ff775485246999027b3197955",
  "token_decimals": 18,
  "confirmations": 12,
  "log_scan_block_range": 10,
  "allow_overpay": false,
  "fiat_per_token": "7.25",
  "addresses": [
    "0x0000000000000000000000000000000000000001"
  ]
}
```

BSC USDC 合约：`0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d`。

测试网支持：

| 链 | `alchemy_network` | `chain_id` | 内置 Token |
| --- | --- | --- | --- |
| Ethereum Sepolia | `eth-sepolia` | `11155111` | Circle 测试网 USDC |
| Base Sepolia | `base-sepolia` | `84532` | Circle 测试网 USDC |
| Polygon PoS Amoy | `polygon-amoy` | `80002` | Circle 测试网 USDC |
| Arbitrum Sepolia | `arb-sepolia` | `421614` | Circle 测试网 USDC |
| OP Sepolia | `opt-sepolia` | `11155420` | Circle 测试网 USDC |
| BNB Smart Chain Testnet | `bnb-testnet` | `97` | 自定义 ERC20 |

测试网 USDC 合约来自 Circle 官方 USDC Contract Addresses 页面。测试网 USDC 没有真实价值，只用于联调。USDT 测试网没有内置官方默认合约；如需测试 USDT，请在后台选择“自定义 ERC20 / 测试 USDT”并填写自己的测试 Token 合约。

运行要求：

- `server.run_worker = true`，后台 watcher 才会扫描链上入账。
- Alchemy 不同链、不同方法的免费额度和 block range 限制以 Alchemy 当前套餐为准；BNB Smart Chain 的 `eth_getLogs` 免费层建议保持 `log_scan_block_range = 10`。
- 收款地址必须是用户自己控制的钱包地址；系统不会保存私钥，也不会自动归集。
- 测试网通道会在支付 intent 中快照 `network_env=testnet`，订单详情会显示“测试网”标识，二维码文本会标注 TESTNET / no financial value。
- 本功能只做链上入账识别，不迁移 TokenPay 的 TRON、私钥地址生成、自动归集、能量租用、独立网关 API 或自动退款。
- 建议配置多个静态收款地址。同金额订单会优先遍历地址池，所有地址都被占用后才微调最小识别金额。
- watcher 会按 `channel + chain + token_contract + receive_address` 分组扫描，降低同地址多订单的 Alchemy RPC 消耗。
- 已存在但没有 `scan_from_block` 的旧 intent 会被保护为从当前已确认高度之后开始扫描，避免历史同金额交易被误匹配；如升级前已有未支付的 EVM 订单，建议让用户重新生成支付单。
- 订单详情页的“EVM 本地收款”区域可查看扫描状态、错误、匹配交易，也可输入 `tx_hash` 校验并人工补单。手动确认会校验交易回执、链 ID、合约地址、收款地址、金额和确认数。
- 默认要求链上实际到账金额与订单识别金额精确匹配。开启 `allow_overpay` 后，系统只在 `expected_amount <= paid_amount <= expected_amount + overpay_tolerance` 范围内认定成功。
- 因为系统不保存收款地址私钥，所以不会也不能自动退款。后续如需退款流程，应优先设计为“管理员线下退款后填写 tx_hash，系统校验链上交易并留痕”的半自动模式。

常见错误：

- **支付一直未确认**：检查 `alchemy_api_key` / `rpc_url` 是否能访问该 `chain_id`、合约地址是否正确、确认数是否过高。
- **金额不匹配**：检查 `fiat_per_token` 和订单币种是否一致；本地按 `订单金额 / fiat_per_token` 计算 Token 数量。
- **重复金额冲突**：系统会自动微调最小识别单位，仍失败说明地址池和金额空间不足。
- **用户多付**：默认不会成功；如业务允许过付，可开启 `allow_overpay` 并设置 `overpay_tolerance`。

## 9. Epusdt 通道

适用于 Epusdt 原版及兼容 GMPay 协议的 USDT 收款网关。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `epusdt` |
| 渠道类型 | `usdt` |
| 旧支付标识 | `usdt` |
| 交互方式 | 二维码（默认）或 跳转 |

接入准备：

1. 部署 Epusdt 网关（自建或第三方），拿到网关地址与 **Token**。
2. 在 Epusdt 后台配置回调地址：`https://你的域名/payment/callback/epusdt/usdt`。

完整配置字段（来自 `EpusdtProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `gateway_url` | ✓ | — | Epusdt 网关地址 |
| `secret_key` / `token` / `key` | ✓ | — | 验签密钥 |
| `pid` | | 空 | 部分版本需要的合作伙伴 ID |
| `currency` | | `cny` | 法币币种 |
| `token` | | `usdt` | 上游字段名 `token`（币种枚举，与配置 key 同名，注意区别） |
| `network` | | `trc20` | 链网络 |
| `create_path` | | `/payments/gmpay/v1/order/create-transaction` | 创建订单接口 |
| `pay_url_template` | | — | 若上游使用静态跳转模板，可改用模板模式 |

模板变量（用于 `pay_url_template`）：`{payment_no}`、`{order_no}`、`{amount}`。

标准接入示例（API 模式）：

```json
{
  "gateway_url": "https://epusdt.example.com",
  "secret_key": "your-epusdt-token",
  "currency": "cny",
  "network": "trc20"
}
```

模板跳转示例（旧版 Epusdt）：

```json
{
  "gateway_url": "https://epusdt.example.com",
  "secret_key": "your-epusdt-token",
  "pay_url_template": "https://epusdt.example.com/pay/{payment_no}?amount={amount}"
}
```

API 模式下，下单成功后用户会跳转到 `<gateway_url>/pay/checkout-counter/{trade_id}`。

回调地址：

| 类型 | 地址 |
| --- | --- |
| 异步通知（推荐） | `/payment/callback/epusdt/{channel_type}` |
| 异步通知（兼容旧路由） | `/pay/epusdt/notify_url` |

常见错误：

- **`missing trade_id`**：上游返回 JSON 不含 `data.trade_id` 也不含顶层 `trade_id`，说明网关版本不匹配，请确认走的是 GMPay 兼容协议。
- **回调签名错**：密钥前后空格、或上游切换到 HMAC-SHA256 签名（本程序按 MD5 排序签名校验）。

## 10. Bepusdt 通道

适用于 BEpusdt 协议的多链 USDT / USDC / TRX 收款网关。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `bepusdt` |
| 渠道类型 | `usdt` / `usdt-trc20` / `usdc-trc20` / `trx` |
| 旧支付标识 | `usdt` |

`channel_type` 会自动映射到 BEpusdt 端的 `trade_type`：

| 渠道类型 | trade_type |
| --- | --- |
| `usdt` / `usdt-trc20` | `usdt.trc20` |
| `usdc-trc20` | `usdc.trc20` |
| `trx` | `tron.trx` |

如需覆盖映射，可在 JSON 中显式设置 `trade_type` 字段。

完整配置字段（来自 `BepusdtProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `gateway_url` | ✓ | — | BEpusdt 网关地址 |
| `auth_token` / `token` / `key` | ✓ | — | API Token（用于签名） |
| `trade_type` | | 按渠道类型映射 | 上游 trade_type，覆盖默认映射 |
| `fiat` | | `CNY` | 法币币种 |

请求接口固定为 `POST {gateway_url}/api/v1/order/create-transaction`，签名采用 MD5 排序加 `Token` 后缀。

配置示例：

```json
{
  "gateway_url": "https://bepusdt.example.com",
  "auth_token": "your-bepusdt-token",
  "fiat": "CNY"
}
```

回调地址：

| 类型 | 地址 |
| --- | --- |
| 异步通知（推荐） | `/payment/callback/bepusdt/{channel_type}` |
| 异步通知（兼容旧路由） | `/pay/bepusdt/notify_url` |

## 11. Okpay 通道

适用于 OKPAY 商户系统（默认网关 `https://api.okaypay.me/shop`），收 USDT / TRX。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `okpay` |
| 渠道类型 | `usdt` 或 `trx` |
| 旧支付标识 | `usdt` |

完整配置字段（来自 `OkpayProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `merchant_id` | ✓ | — | OKPAY 商户 ID |
| `merchant_token` / `token` / `key` | ✓ | — | 商户 Token（签名密钥） |
| `gateway_url` | | `https://api.okaypay.me/shop` | 网关地址，自建时填写自己的 |

请求接口固定为 `POST {gateway_url}/payLink`，使用 `form-urlencoded` 提交，签名规则参见代码。

配置示例：

```json
{
  "merchant_id": "your-okpay-merchant-id",
  "merchant_token": "your-okpay-merchant-token"
}
```

回调地址：

| 类型 | 地址 |
| --- | --- |
| 异步通知（推荐） | `/payment/callback/okpay/{channel_type}` |
| 异步通知（兼容旧路由） | `/pay/okpay/notify_url` |

回调 form 字段会被识别 `data[unique_id]` / `data[status]` / `data[payment_status]`（OKPAY 嵌套 form 风格），无需用户处理。

## 12. FreeMarketPay 通道

FreeMarketPay 是 free-market 推荐的多链虚拟币收款 SaaS。Provider 固定为 `freemarketpay`，渠道类型必须使用平台定义的 `token_id`。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `freemarketpay` |
| 渠道类型（=`token_id`） | 见下表 |
| 旧支付标识 | `usdt`（或留空） |

支持的 `token_id` → 链映射（来自 `resolve_freemarketpay_chain`）：

| token_id | 链 |
| --- | --- |
| `tron-trx` / `tron-usdt` | TRON |
| `ethereum-eth` / `ethereum-usdt` / `ethereum-usdc` | Ethereum |
| `bsc-bnb` / `bsc-usdt` | BSC |
| `polygon-usdc` / `polygon-usdt0` | Polygon |
| `base-usdc` | Base |
| `arbitrum-usdc` / `arbitrum-usdt0` | Arbitrum |
| `plasma-usdt0` | Plasma |
| `x-layer-usdt0` | X Layer |
| `solana-usdc` / `solana-usdt` | Solana |
| `aptos-usdc` / `aptos-usdt` | Aptos |

完整配置字段（来自 `FreeMarketPayProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `api_base_url` | ✓ | `https://www.freemarketpay.com` | FreeMarketPay API 根地址；自托管时改成自己的入口 |
| `api_key_id` | ✓ | — | API Key ID |
| `api_secret` | ✓ | — | API Secret（用于请求签名，HMAC-SHA256） |
| `webhook_secret` | ✓ | — | Webhook 校验密钥（独立于 api_secret） |
| `token_id` | | 取渠道类型 | 强制指定 token_id |
| `chain` | | 按 token_id 映射 | 覆盖默认链 |
| `fiat_currency` | | 订单币种或 `CNY` | 法币币种 |

请求接口固定为 `POST {api_base_url}/v1/orders`，使用 `Idempotency-Key`（即支付号）幂等。

Webhook 校验：

- 必须携带 `DJP-Webhook-Timestamp`、`DJP-Webhook-Signature` 两个 Header。
- 容忍时差 ±300 秒。
- 签名 = `HMAC_SHA256(webhook_secret, timestamp + "." + raw_body)`，签名 Header 形如 `sha256=<hex>`。

配置示例：

```json
{
  "api_base_url": "https://www.freemarketpay.com",
  "api_key_id": "your-key-id",
  "api_secret": "your-api-secret",
  "webhook_secret": "your-webhook-secret",
  "fiat_currency": "CNY"
}
```

回调地址：

| 类型 | 地址 |
| --- | --- |
| 异步通知（推荐） | `/payment/callback/freemarketpay/{token_id}` |
| 异步通知（兼容旧路由） | `/pay/freemarketpay/notify_url` |

事件类型：`order.paid` → 成功；`order.expired` → 过期；`order.canceled` → 失败；其他 → 待支付。

## 13. 官方 Stripe 通道

Provider 为 `official`，渠道类型 `stripe`。基于 Stripe Checkout Session。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `official` |
| 渠道类型 | `stripe` |
| 旧支付标识 | `other` 或 `stripe` |

接入准备（Stripe Dashboard）：

1. 「Developers → API keys」获取 **Secret key**（`sk_live_xxx` 或 `sk_test_xxx`）。
2. 「Developers → Webhooks → Add endpoint」：
   - URL：`https://你的域名/pay/stripe/notify_url`
   - Events：勾选 `checkout.session.completed`、`checkout.session.expired`、`checkout.session.async_payment_succeeded`、`checkout.session.async_payment_failed`、`payment_intent.succeeded`、`payment_intent.payment_failed`、`payment_intent.canceled`。
   - 创建后展开 endpoint，复制 **Signing secret**（`whsec_xxx`）作为 `webhook_secret`。

完整配置字段（来自 `StripeProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `secret_key` | ✓ | — | Stripe API Secret Key |
| `webhook_secret` | ✓ | — | Webhook Signing Secret（用于校验 `Stripe-Signature`） |
| `api_base_url` | | `https://api.stripe.com` | 极少修改 |
| `success_url` | | 订单 return_url | 支付成功跳转 URL（可含 `{CHECKOUT_SESSION_ID}` 模板） |
| `cancel_url` | | 订单 return_url | 取消跳转 URL |
| `payment_method_types` | | `["card"]` | 可选 `card` / `alipay` / `wechat_pay` / `link` 等 |
| `target_currency` | | 订单币种 | 提交给 Stripe 的币种，例 `usd` / `eur`（影响金额换算） |

> 零小数币种（JPY、KRW、VND 等）金额会被自动除以 100；常规币种保持「分」直接提交。

配置示例：

```json
{
  "secret_key": "sk_live_xxxxxxxxxxxxxxxx",
  "webhook_secret": "whsec_xxxxxxxxxxxxxxxx",
  "payment_method_types": ["card", "alipay", "wechat_pay"],
  "target_currency": "usd"
}
```

回调地址：`https://你的域名/pay/stripe/notify_url`

## 14. 官方 PayPal 通道

Provider 为 `official`，渠道类型 `paypal`。基于 PayPal Orders v2 API + Capture。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `official` |
| 渠道类型 | `paypal` |

接入准备（PayPal Developer）：

1. 创建 App，记下 **Client ID** 与 **Client Secret**。
2. 选择运行环境：
   - 沙箱：`https://api-m.sandbox.paypal.com`
   - 生产：`https://api-m.paypal.com`
3. （可选但**强烈推荐**）创建 Webhook：
   - URL：`https://你的域名/pay/paypal/notify_url`
   - Events：`PAYMENT.CAPTURE.COMPLETED`、`PAYMENT.CAPTURE.DENIED`、`PAYMENT.CAPTURE.FAILED`、`CHECKOUT.ORDER.COMPLETED`、`CHECKOUT.ORDER.DENIED`。
   - 记下 **Webhook ID** 作为 `webhook_id`。

> 不配 `webhook_id` 时程序**不会**校验 Webhook 签名，依赖 PayPal 推送+主动 capture 的金额校验；生产环境必须配置。

完整配置字段（来自 `PaypalProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `client_id` | ✓ | — | PayPal App Client ID |
| `client_secret` | ✓ | — | PayPal App Client Secret |
| `base_url` | | `https://api-m.sandbox.paypal.com` | 沙箱默认；**生产必须改为** `https://api-m.paypal.com` |
| `webhook_id` | | — | 留空则跳过 Webhook 签名校验 |
| `target_currency` | | 订单币种 | 提交给 PayPal 的币种，PayPal 不支持 CNY |
| `return_url` | | 订单 return_url | 用户授权后跳转 URL |
| `cancel_url` | | 订单 return_url | 用户取消跳转 URL |
| `user_action` | | `PAY_NOW` | `PAY_NOW` / `CONTINUE` |
| `shipping_preference` | | `NO_SHIPPING` | `NO_SHIPPING` / `SET_PROVIDED_ADDRESS` / `GET_FROM_FILE` |

配置示例（生产）：

```json
{
  "client_id": "AYxxxxxxxxxxxxxxxxx",
  "client_secret": "ELxxxxxxxxxxxxxxxxx",
  "base_url": "https://api-m.paypal.com",
  "webhook_id": "8XX12345AB678901C",
  "target_currency": "USD"
}
```

回调地址：`https://你的域名/pay/paypal/notify_url`

注意：PayPal 不支持 CNY 入账，必须配 `target_currency` 为 `USD` / `EUR` / `HKD` 等。

## 15. 官方支付宝通道

Provider 为 `official`，渠道类型 `alipay`。基于支付宝开放平台 OpenAPI（不是易支付）。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `official` |
| 渠道类型 | `alipay` |
| 旧支付标识 | `alipay` |

接入准备（支付宝开放平台）：

1. 创建网页支付能力的应用，获取 **APPID**。
2. 生成 RSA2 商户公私钥对（`alipay_keytool` 或 OpenSSL），将商户公钥上传到开放平台。
3. 下载 **支付宝公钥**（用于回调验签）。
4. 在应用网关中配置同步与异步通知地址：
   - 异步通知：`https://你的域名/payment/callback/alipay/alipay`
   - 同步返回：`https://你的域名/pay/alipay/return_url`

完整配置字段（来自 `AlipayProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `app_id` | ✓ | — | 开放平台 APPID |
| `private_key` | ✓ | — | 商户 RSA 私钥（PKCS8 或 PKCS1 PEM） |
| `alipay_public_key` | | — | 支付宝公钥；**强烈建议填写**，否则回调不验签 |
| `gateway_url` | | `https://openapi.alipay.com/gateway.do` | 沙箱使用 `https://openapi.alipaydev.com/gateway.do` |
| `sign_type` | | `RSA2` | 推荐 RSA2 |
| `interaction_mode` | | 空（=PC 跳转） | `wap`（H5）/ `qr` / `qrcode`（扫码） |

`interaction_mode` 决定调用支付宝哪个接口：

| 模式 | 支付宝 method |
| --- | --- |
| 默认（PC） | `alipay.trade.page.pay` |
| `wap` | `alipay.trade.wap.pay` |
| `qr` / `qrcode` | `alipay.trade.precreate` |

配置示例：

```json
{
  "app_id": "2021000123456789",
  "private_key": "-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----",
  "alipay_public_key": "-----BEGIN PUBLIC KEY-----\nMIIB...\n-----END PUBLIC KEY-----",
  "sign_type": "RSA2"
}
```

回调地址：

| 类型 | 地址 |
| --- | --- |
| 异步通知（推荐） | `/payment/callback/alipay/alipay` |
| 异步通知（兼容旧路由） | `/pay/alipay/notify_url` |
| 同步返回 | `/pay/alipay/return_url` |

## 16. 官方微信支付通道

Provider 为 `official`，渠道类型 `wechat` 或 `wxpay`。基于微信支付 APIv3（Native + H5）。

| 配置项 | 取值 |
| --- | --- |
| 支付提供方 | `official` |
| 渠道类型 | `wechat` / `wxpay` |
| 旧支付标识 | `wxpay` |

接入准备（微信支付商户平台）：

1. 申请微信支付 Native / H5 产品并审核通过。
2. 拿到 **AppID（公众号或小程序）** 与 **mchid（商户号）**。
3. 在商户平台「账户中心 → API 安全」中：
   - 申请 **APIv3 密钥**（32 字节字符串）。
   - 申请商户证书（`apiclient_cert.pem` + `apiclient_key.pem`），记下 **证书序列号**。
4. 在「产品中心 → 开发配置」中预先填写支付回调通知地址：`https://你的域名/pay/wechat/notify_url`。

完整配置字段（来自 `WechatPayProvider`）：

| 字段 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `appid` | ✓ | — | 公众号 / 小程序 AppID |
| `mchid` | ✓ | — | 商户号 |
| `merchant_serial_no` | ✓ | — | 商户 API 证书序列号 |
| `merchant_private_key` | ✓ | — | 商户私钥（`apiclient_key.pem` 内容） |
| `api_v3_key` | | — | APIv3 密钥，用于解密回调资源；**生产必填** |
| `base_url` | | `https://api.mch.weixin.qq.com` | 极少修改 |
| `trade_type` | | 空（=Native 扫码） | `h5` / `wap` 走 H5 接口 |
| `h5_type` | | `Wap` | H5 场景类型，移动浏览器一般保持 `Wap` |

调用接口：

| 模式 | 接口路径 |
| --- | --- |
| Native 扫码（默认） | `POST /v3/pay/transactions/native`，返回 `code_url` |
| H5 | `POST /v3/pay/transactions/h5`，返回 `h5_url` |

> 如果渠道类型本身是 `h5` / `wap`，或配置中 `trade_type=h5`，自动走 H5。

配置示例（Native）：

```json
{
  "appid": "wx1234567890abcdef",
  "mchid": "1610000000",
  "merchant_serial_no": "AB12CD34EF56GH78IJ90KL12MN34OP56QR78ST90",
  "merchant_private_key": "-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----",
  "api_v3_key": "your-32-byte-apiv3-key"
}
```

回调地址：`https://你的域名/pay/wechat/notify_url`

> 微信支付要求回调通知地址必须 **HTTPS 公网可达**，且使用与商户 API 一致的证书。Cloudflare Tunnel + 站点配 HTTPS 已满足此要求。

## 17. 商品绑定支付通道

支付通道默认是全局可用。商品也可以单独限定可用通道。

后台路径：

```text
/admin/products
```

商品表单中的"可用支付通道 ID JSON"用于限定通道：

```json
[1, 2, 3]
```

规则：

| 配置 | 效果 |
| --- | --- |
| `[]` | 使用全部已启用通道。 |
| `[1,2]` | 该商品仅展示 ID 为 1、2 的支付通道。 |

建议：

- 普通商品可保持 `[]`。
- 高风险或特定币种商品建议绑定指定通道。
- 测试通道不要绑定到正式商品。

## 18. 回调地址配置

支付平台通常需要配置异步通知地址和同步返回地址。

推荐使用完整域名：

```text
https://your-domain.com/payment/callback/{provider_type}/{channel_type}
```

常见示例：

| Provider | Channel | 异步通知地址 |
| --- | --- | --- |
| Epay 支付宝 | `alipay` | `/payment/callback/epay/alipay` |
| Epay 微信 | `wxpay` | `/payment/callback/epay/wxpay` |
| TokenPay USDT | `usdt` | `/payment/callback/tokenpay/usdt` |
| EVM 本地收款 | `bsc-usdt` 等 | 无外部异步通知；由内置 watcher 通过 Alchemy 扫描确认 |
| Epusdt USDT | `usdt` | `/payment/callback/epusdt/usdt` |

兼容旧路由：

```text
/pay/epay/notify_url
/pay/yipay/notify_url
/pay/tokenpay/notify_url
/pay/epusdt/notify_url
```

同步返回地址通常配置：

```text
/pay/{provider}/return_url
```

## 19. 接入检查清单

新增支付通道后，建议按以下顺序验证：

1. 后台支付通道已启用。
2. 商品未限制该通道，或已在商品支付通道白名单中加入该通道 ID。
3. 前台购买页能看到对应支付按钮。
4. 创建订单后能进入支付页或跳转第三方支付页。
5. 第三方支付平台能访问异步通知地址。
6. 支付成功后订单状态变为已支付或已完成。
7. 自动发货商品能正常发卡。
8. 后台订单详情能看到支付流水。

## 20. 常见问题

### 前台看不到支付方式

检查：

- 支付通道是否启用。
- 客户端范围是否匹配当前设备。
- 商品是否设置了支付通道白名单。
- `pay_check` 是否填写了合适的支付标识。

### 支付跳转失败

检查：

- `gateway_url` 是否正确。
- 商户 ID、密钥是否正确。
- 第三方平台是否允许当前域名发起支付。
- 订单金额是否满足平台最低支付金额。

### 支付成功但订单不变更

检查：

- 第三方平台异步通知地址是否填写正确。
- 服务器公网域名是否可访问。
- 通道密钥是否与平台一致。
- 回调金额和订单金额是否一致。
- 后台订单详情是否已有支付流水。

### 签名错误

检查：

- `key` 或 `token` 是否填写正确。
- 网关是否使用标准 MD5 签名规则。
- 是否存在多余空格或错误换行。
- 特殊网关需要按其文档调整配置。

## 21. 生产配置建议

- 生产环境关闭 Noop 测试通道。
- 每种真实支付方式单独建一个通道，避免多个渠道共用同一配置。
- 支付密钥定期轮换，并限制后台访问权限。
- 先用小额订单验证完整支付和回调链路。
- 对 USDT 等虚拟币通道，明确币种、链类型、到账确认规则。
- 不要在公告、商品描述或公开文档中暴露商户密钥。

---

# 支付通道生产验收清单

本节用于记录支付通道从"代码已接入"到"允许生产启用"的验收状态。代码级 Provider 注册不等于真实收款可用；每个计划上线的通道都需要完成沙箱或小额实付验证。

## 验收原则

每个真实支付通道上线前至少完成：

| 项目 | 要求 |
| --- | --- |
| 创建支付 | 后台配置通道后，前台订单能成功生成支付单并跳转或展示二维码 |
| 成功回调 | 网关异步通知能正确验签、核对金额和币种，并将订单推进到发货流程 |
| 金额不一致 | 篡改或异常金额必须被拒绝，订单保持未支付或异常状态 |
| 重复回调 | 同一成功通知重复到达时必须幂等，不重复发卡、不重复扣优惠券 |
| 失败/过期 | 网关失败、取消、过期事件不应把订单误标为已支付 |
| 同步返回 | 用户支付后返回站点时能进入订单详情或等待页 |

## 通道矩阵

| Provider | Channel 示例 | 验收环境 | 当前状态 | 生产建议 |
| --- | --- | --- | --- | --- |
| `noop` | `test` | 本地 | 仅用于测试 | 禁止生产真实售卖启用 |
| `epay` / `yipay` | `alipay` / `wxpay` / `qqpay` | 沙箱或小额实付 | 待验收 | 验证后启用 |
| `tokenpay` | `usdt` / `trx` | 沙箱或小额实付 | 待验收 | 验证后启用 |
| `evm-local` | `bsc-usdt` / `bsc-usdc` / `base-usdc` / 测试网 USDC 等 | Alchemy + 测试网转账 + 主网小额实付 | 待验收 | 验证主网/测试网标识、地址池、确认数、超付策略和人工 tx_hash 补单后启用 |
| `epusdt` | `usdt` | 沙箱或小额实付 | 待验收 | 验证后启用 |
| `bepusdt` | `usdt` / `usdc` / `trx` | 沙箱或小额实付 | 待验收 | 验证后启用 |
| `freemarketpay` | `token_id` | 沙箱或小额实付 | 待验收 | 验证后启用 |
| `okpay` | `usdt` | 沙箱或小额实付 | 待验收 | 验证后启用 |
| `official` | `stripe` | Stripe test mode | 待验收 | 验证后启用 |
| `official` | `paypal` | PayPal sandbox | 待验收 | 验证后启用 |
| `official` | `alipay` | 支付宝沙箱或小额实付 | 待验收 | 验证后启用 |
| `official` | `wechat` / `wxpay` | 微信支付沙箱或小额实付 | 待验收 | 验证后启用 |

## 单通道记录模板

复制以下模板，为每个计划上线的通道保留一份脱敏记录。

```text
Provider:
Channel:
通道名称:
验收环境:
验收日期:
验收人:

配置确认:
- merchant/app id:
- key/secret/webhook secret:
- gateway/base url:
- notify_url:
- return_url:

测试结果:
- 创建支付:
- 成功回调:
- 金额不一致拒绝:
- 重复回调幂等:
- 失败/过期事件:
- 同步返回:

脱敏样例:
- 创建支付响应:
- 成功回调 body/header:
- 失败回调 body/header:

结论:
- [ ] 允许生产启用
- [ ] 仅允许测试环境
- [ ] 暂停使用，原因:
```

## 上线开关建议

- 没完成本节验收的真实通道，不应在生产后台设置为启用。
- `noop` 通道只用于本地联调，生产环境应禁用。
- 同一 Provider 的不同网关实现可能存在字段差异，不能用一个网关的验收结果替代另一个网关。
- 支付回调域名必须使用生产 `site.base_url`，并确保公网可访问。

---

## License

见 [LICENSE](LICENSE)（继承上游 Laravel 项目的协议）。
