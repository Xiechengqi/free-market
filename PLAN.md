# freeMarket 重构规划

## 1. 目标

将当前独角数卡项目重构为 Rust 实现，形态为“前后端一体、单 binary 部署、SQLite 持久化、无 Redis/外部数据库依赖”，同时完整保留当前项目的用户前台视觉、路由习惯和购买体验。

核心目标：

- 保持 `/data/projects/dujiaoka` 当前前台页面、主题、静态资源、主要路由和购买流程。
- 后端架构吸收 `/data/projects/dujiao-next` 的更优设计：分层服务、独立支付表、支付 Provider、卡密状态机、履约记录、显式设置表、强幂等回调。
- 发布物优先做到一个 Rust binary + 一个 SQLite 数据库文件 + 上传目录 + 日志目录。
- 不依赖 Redis、MySQL、PostgreSQL、消息队列服务或其他第三方数据库。
- 首版优先保证“稳定卖卡”：下单、占库存、支付回调、自动发卡、订单查询、后台管理、邮件通知。

首版非目标：

- 不追求一次性 100% 复刻当前 Laravel/Dcat Admin 的所有边缘功能。
- 不直接执行 Blade 模板；Rust 版本需要把 Blade 页面迁移到 Rust 模板引擎，但保持 DOM/CSS/JS 和视觉体验。
- 暂缓钱包、会员等级、分销、上游采购、Telegram 登录、OAuth/OIDC、复杂 RBAC、复杂对账中心。

## 2. 源项目解读

### 2.1 当前 Laravel 项目

路径：`/data/projects/dujiaoka`

技术形态：

- Laravel 6 / PHP。
- Dcat Admin 后台。
- Blade 模板，包含 `unicorn`、`luna`、`hyper` 等前台主题。
- MySQL 表结构位于 `database/sql/install.sql`。
- 默认配置中 Redis 承担缓存和队列。

主要优势：

- 前台体验成熟，页面和路由已经被用户验证。
- 业务模型集中：商品分组、商品、卡密、优惠券、订单、支付渠道、邮件模板、系统设置。
- 适合迁移为 Rust 服务端渲染，不需要强行改成 SPA。

主要问题：

- 支付逻辑散落在多个控制器和服务中，扩展和测试成本高。
- 订单、支付、发货信息耦合在订单表及其字段中，缺少独立支付流水和履约记录。
- 卡密状态偏粗，未显式建模“已预占但未付款”的状态，极端并发和超时释放需要加强。
- 默认依赖 Redis 队列处理订单过期等异步任务，不符合单文件/单机轻部署目标。
- 系统设置依赖缓存，不如持久化 `settings` 表清晰。
- 支付回调的幂等、金额校验、渠道校验、状态流转还可以更严格。

关键参考文件：

- `/data/projects/dujiaoka/routes/common/web.php`
- `/data/projects/dujiaoka/routes/common/pay.php`
- `/data/projects/dujiaoka/app/Service/OrderService.php`
- `/data/projects/dujiaoka/app/Service/OrderProcessService.php`
- `/data/projects/dujiaoka/database/sql/install.sql`
- `/data/projects/dujiaoka/resources/views`
- `/data/projects/dujiaoka/public/assets`

### 2.2 Go 增强后端项目

路径：`/data/projects/dujiao-next`

技术形态：

- Go / Gin / GORM。
- 支持 SQLite/PostgreSQL。
- Redis/asynq 作为队列。
- 使用 repository / service / payment provider 分层。

值得采用的设计：

- repository + service + provider 分层比当前 Laravel 的控制器分散逻辑更清晰。
- 独立 `payments` 表，支付状态和订单状态解耦。
- 支付 Provider Registry / Adapter 机制，适合 Rust trait 化。
- 回调校验更完整：渠道、订单号、金额、币种、幂等状态。
- `settings` 表持久化配置，支持 JSON 值和规范化。
- 商品和 SKU 分离，为后续多规格扩展留空间。
- 卡密状态包含 `available` / `reserved` / `used`，能支撑先占库存、超时释放、支付后发货。
- 使用 `fulfillments` 表记录发货结果，而不是把所有发货内容塞进订单信息字段。
- 订单状态常量和状态流转更显式。
- 对 SQLite 的 WAL、busy timeout、事务边界有更强意识。
- 测试覆盖思路更完整。

不适合直接照搬的部分：

- Redis/asynq 不符合本次“无 Redis”目标，需要改为 SQLite jobs + 内嵌 worker。
- Go 项目模型更大，部分功能超出首版稳定卖卡目标。
- Go 项目偏 API + SPA 思路，而本次要求前台完全保持当前项目，需要服务端渲染前台页面。

关键参考文件：

- `/data/projects/dujiao-next/internal/models/db.go`
- `/data/projects/dujiao-next/internal/models/order.go`
- `/data/projects/dujiao-next/internal/models/payment.go`
- `/data/projects/dujiao-next/internal/models/card_secret.go`
- `/data/projects/dujiao-next/internal/models/fulfillment.go`
- `/data/projects/dujiao-next/internal/service/order_service.go`
- `/data/projects/dujiao-next/internal/service/payment_service.go`
- `/data/projects/dujiao-next/internal/service/payment_service_callback.go`
- `/data/projects/dujiao-next/internal/payment/provider/types.go`
- `/data/projects/dujiao-next/internal/payment/provider/registry.go`
- `/data/projects/dujiao-next/internal/queue/client.go`
- `/data/projects/dujiao-next/internal/worker/asynq_worker.go`

## 3. 两种实现路线的功能差异

| 对比项 | 100% 复刻当前项目 | “单文件部署、无 Redis、SQLite、稳定卖卡” |
|---|---|---|
| 总体目标 | 最大限度复制 Laravel 独角数卡所有功能和后台体验 | 优先稳定售卖卡密，保留前台体验，后端按 Rust/SQLite 重新设计 |
| 前台页面 | 完整复刻 Blade 主题和所有页面细节 | 保持当前前台视觉、路由、购买链路，模板迁移到 Rust |
| 后台 | 需要复刻 Dcat Admin 的大量表格、表单、权限和扩展体验 | 自建 Rust 后台，只实现商品、卡密、订单、支付、设置等必要工作流 |
| 数据库 | 尽量沿用 MySQL 表语义，兼容旧字段 | SQLite-first，表结构重建，提供迁移映射 |
| Redis | 为了完全复刻，需要保留或模拟缓存/队列语义 | 完全移除 Redis，使用 SQLite jobs 和内嵌 worker |
| 队列 | 复刻 Laravel queue 行为，复杂度高 | `jobs` 表 + 内嵌 worker + 懒清理兜底 |
| 支付 | 复刻当前控制器/插件分散逻辑 | 独立 `payments` 表 + provider trait + 强幂等回调 |
| 卡密库存 | 复刻 `未售出/已售出` 语义 | 使用 `available/reserved/used`，下单先预占，超时释放 |
| 履约记录 | 兼容订单字段里的发货信息 | 独立 `fulfillments` 表记录发货内容、时间、状态 |
| 系统设置 | 复刻缓存式设置读取 | `settings` 表持久化 JSON 设置 |
| 并发卖卡 | 需要在旧语义上补锁，容易变形 | 从数据模型开始支持事务占卡，降低超卖风险 |
| 部署复杂度 | 可能仍需要 PHP-FPM/MySQL/Redis 等多组件概念 | 一个 binary + SQLite 文件即可运行 |
| 迁移难度 | 功能迁移面最大，周期最长 | 数据迁移需要映射，但功能边界更可控 |
| 代码复杂度 | 为复刻历史行为引入大量兼容层 | 代码更聚焦，业务状态更清晰 |
| 用户侧变化 | 最小 | 前台尽量无感，后台会变为新的 Rust 后台 |
| 首版上线风险 | 功能面过大，延期和隐藏 bug 风险高 | 功能收敛，核心交易链路更容易测透 |

结论：不建议以“100% 复刻”为首版目标。Rust 版本应采用第二种路线：前台体验尽量完整保持，后台和核心交易链路按更稳的数据模型重建。

## 4. 相同实现的取优原则

当当前 Laravel 项目和 Go 增强后端存在相同或相近实现时，Rust 版本按以下原则选型：

| 领域 | Laravel 当前实现 | Go 增强实现 | Rust 版本选择 |
|---|---|---|---|
| 路由和前台体验 | 更贴近现有用户习惯 | 偏 API/SPA | 选 Laravel 前台路由和页面体验 |
| 业务分层 | 控制器/服务混合较多 | repository/service/provider 清晰 | 选 Go 的分层方式 |
| 商品展示 | 适配当前主题 | 适合 API 输出 | 选 Laravel 的展示语义，底层模型参考 Go |
| 商品/SKU | 商品模型较直接 | 商品/SKU 分离 | 首版一商品一默认 SKU，结构预留多 SKU |
| 订单表 | 订单承载大量信息 | 订单、支付、履约拆分 | 选 Go 的拆分方式 |
| 支付接入 | 当前生态和路由经验多 | Provider Registry 更清晰 | 路由兼容 Laravel，内部采用 Go 的 Provider 设计 |
| 支付回调 | 能工作但分散 | 校验和幂等更强 | 选 Go 的回调校验模型 |
| 卡密状态 | 简单售出状态 | available/reserved/used | 选 Go 的卡密状态机 |
| 发货记录 | 订单字段承载 | fulfillments 表 | 选 Go 的履约记录 |
| 设置 | 缓存读取 | settings 表 | 选 Go 的持久设置表 |
| 队列 | Redis queue | Redis/asynq | 两者都不直接采用，改 SQLite jobs |
| 管理后台 | Dcat Admin 功能完整 | API 管理思路 | 不复用 Dcat，做 Rust 后台，流程参考当前后台 |
| 测试 | 覆盖较少 | 覆盖更完整 | 采用 Go 项目的测试纪律 |

## 5. Rust 技术栈建议

推荐基线：

- HTTP 框架：`axum`。
- 模板引擎：`minijinja` 或 `tera`，优先选择更适合机械迁移 Blade 的方案。
- 数据库：`sqlx` + SQLite。
- 迁移：`sqlx::migrate!` 或自研极轻量嵌入式 migration runner。
- 静态资源内嵌：`rust-embed` 或 `include_dir`。
- 异步运行时：`tokio`。
- HTTP 客户端：`reqwest`。
- 邮件：`lettre`。
- 密码哈希：`argon2`，导入旧后台密码时可提供 bcrypt 兼容。
- Session：签名/加密 Cookie，或 SQLite session 表。
- 金额：优先用整数分存储；如需小数，使用定点 decimal，禁止浮点数参与金额计算。
- 日志：`tracing` + `tracing-subscriber` + 滚动文件日志。

SQLite 运行参数：

- 开启 WAL。
- 设置 `busy_timeout`。
- `synchronous=NORMAL`。
- 控制写路径，保持事务短小。
- 库存、订单、支付、履约状态变更必须在显式事务中完成。
- 网络请求不能放在数据库事务内。

## 6. 部署形态

目标目录：

```text
free-market/
  free-market              # 编译后的 binary
  config.toml              # 可选运行配置
  data/
    freemarket.db              # SQLite 数据库
  uploads/
    products/
    admin/
  logs/
    app.log
```

binary 启动职责：

- 自动创建 `data`、`uploads`、`logs` 目录。
- 初始化 SQLite，执行 migration。
- 首次启动通过配置/env 或 Web 初始化页创建管理员。
- 直接服务内嵌静态资源。
- 提供前台路由和后台路由。
- 默认在同一进程内启动 jobs worker。

后续可选模式：

- `--mode all`
- `--mode web`
- `--mode worker`
- `admin reset-password`
- `admin create-user`

## 7. 前台保持方案

必须兼容的前台路由：

- `GET /`
- `GET /buy/{id}`
- `POST /create-order`
- `GET /bill/{orderSN}`
- `GET /detail-order-sn/{orderSN}`
- `GET /order-search`
- `GET /check-order-status/{orderSN}`
- `POST /search-order-by-sn`
- `POST /search-order-by-email`
- `POST /search-order-by-browser`
- `GET /pay-gateway/{handle}/{payway}/{orderSN}`

必须保留的页面概念：

- 首页商品列表。
- 商品详情/购买页。
- 订单结算/支付方式选择页。
- 二维码或跳转支付页。
- 订单详情页。
- 订单搜索页。
- 错误页。
- 启用时保留 QQ/微信内置浏览器提示页。

模板迁移策略：

- Blade 模板迁移到 Rust 模板，不在 Rust 中运行 Blade。
- 尽量保持 CSS class、DOM 结构、图片路径和 JavaScript 行为。
- `public/assets` 中的静态资源编译进 binary。
- 商品上传图片、后台上传文件保留在 `uploads/`，不内嵌。
- 如周期允许，保留 `unicorn`、`luna`、`hyper` 多主题；首版也可以先锁定当前启用主题。

## 8. 数据模型规划

### 8.1 核心表

建议表：

- `admins`
- `admin_sessions`，如果采用纯 Cookie session 可不建
- `settings`
- `categories`
- `products`
- `product_skus`
- `card_secrets`
- `card_secret_batches`
- `coupons`
- `coupon_products`
- `coupon_usages`
- `orders`
- `order_items`
- `payments`
- `payment_channels`
- `fulfillments`
- `email_templates`
- `jobs`
- `job_attempts`
- `notification_logs`
- `media`

### 8.2 当前项目到 Rust 的映射

| 当前表/数据 | Rust 表 | 说明 |
|---|---|---|
| `goods_group` | `categories` | 保留名称、排序、状态 |
| `goods` | `products` + 默认 `product_skus` | 首版对外仍可表现为单商品 |
| `carmis` | `card_secrets` | `status=1` 转 `available`，`status=2` 转 `used` |
| `coupons` | `coupons` | 支持固定金额优惠，预留百分比 |
| `coupons_goods` | `coupon_products` | 商品可用范围 |
| `orders` | `orders` + `order_items` + `payments` + `fulfillments` | 拆分订单、支付、发货 |
| `pays` | `payment_channels` | `pay_check` 转为 provider/channel 配置 |
| `emailtpls` | `email_templates` | 保留模板标识 |
| cache `system-setting` | `settings` | 改为持久化 JSON 设置 |

### 8.3 状态设计

订单状态：

- `pending_payment`
- `paid`
- `fulfilling`
- `delivered`
- `completed`
- `canceled`
- `failed`
- `abnormal`
- `partially_refunded`
- `refunded`

支付状态：

- `initiated`
- `pending`
- `success`
- `failed`
- `expired`

履约状态：

- `pending`
- `delivered`
- `failed`

卡密状态：

- `available`
- `reserved`
- `used`

人工库存字段：

- `manual_stock_total`
- `manual_stock_locked`
- `manual_stock_sold`

## 9. 核心业务流程

### 9.1 创建订单

流程：

1. 校验商品、数量、邮箱、查询密码、优惠券、验证码。
2. 解析商品/SKU 和上架状态。
3. 计算单价、批发价、优惠券、总价。
4. 开启 SQLite 事务。
5. 创建 `orders`。
6. 创建 `order_items`。
7. 自动发卡商品在下单时预占卡密：从 `available` 更新为 `reserved`，写入 `order_id` 和 `reserved_at`。
8. 人工处理商品如有有限库存，则增加 `manual_stock_locked`。
9. 写入 `coupon_usages` 并增加优惠券使用次数。
10. 写入 `order_timeout_cancel` job。
11. 提交事务。
12. 写入浏览器订单 Cookie，兼容当前前台查询体验。

关键变化：

- 不等支付成功后再选卡密。
- 下单时先预占库存，订单超时或取消时释放。

### 9.2 创建支付

流程：

1. 校验订单存在且为 `pending_payment`。
2. 校验支付渠道启用，且允许当前商品使用。
3. 如存在未过期 pending payment，优先复用。
4. 创建 `payments` 记录。
5. 调用支付 provider adapter。
6. 保存支付链接、二维码、平台流水号、网关订单号、provider 原始响应摘要。
7. 渲染兼容当前前台的跳转页或二维码页。

### 9.3 支付回调

流程：

1. 根据路由和渠道找到 provider adapter。
2. 验证签名。
3. 解析为统一回调结构。
4. 通过网关订单号、支付 ID 或订单号定位 `payments`。
5. 校验渠道、订单号、金额、币种和终态幂等。
6. 在事务中更新支付状态、订单状态和库存状态。
7. 提交后投递自动发货、邮件、后台通知、API hook job。

强制要求：

- 回调不能只信任订单号。
- 金额不一致必须拒绝。
- 已成功支付的重复成功回调必须幂等返回成功，不得重复发卡。
- 失败回调不能覆盖已成功支付状态。

### 9.4 自动发卡

流程：

1. 加载订单，确认已支付且可自动发货。
2. 开启事务。
3. 读取该订单预占的卡密。
4. 如预占不足，根据策略决定是否补占可用卡密；默认应视为异常，避免静默错发。
5. 将卡密标记为 `used`。
6. 写入 `fulfillments`。
7. 更新订单为 `completed`。
8. 提交事务。
9. 发送发货邮件。

### 9.5 人工发货

流程：

1. 管理员打开已付款/待履约订单。
2. 填写发货内容。
3. 开启事务。
4. 创建 `fulfillments`。
5. 更新订单为 `delivered` 或 `completed`。
6. 扣减人工锁定库存并增加已售库存。
7. 提交事务。
8. 发送用户邮件和后台通知。

### 9.6 订单超时

采用主动任务 + 懒清理双保险。

主动任务：

- SQLite worker 执行到期的 `order_timeout_cancel` job。
- 仅取消仍处于 `pending_payment` 的订单。

懒清理：

- 查询订单时如果发现 pending 且超过 `expires_at`，先执行取消事务再返回。

取消事务：

1. 确认订单仍为 `pending_payment`。
2. 更新订单为 `canceled`。
3. 释放 `reserved` 卡密为 `available`。
4. 释放人工锁定库存。
5. 回滚优惠券使用记录和使用次数。
6. 将 pending payment 标记为 `expired`。

## 10. SQLite Jobs 替代 Redis

### 10.1 `jobs` 表

字段：

- `id`
- `kind`
- `payload_json`
- `status`：`pending` / `running` / `succeeded` / `failed` / `dead`
- `run_at`
- `attempts`
- `max_attempts`
- `last_error`
- `locked_at`
- `locked_by`
- `created_at`
- `updated_at`

### 10.2 首版任务类型

- `order_timeout_cancel`
- `order_auto_fulfill`
- `order_status_email`
- `admin_notification`
- `api_hook`
- `payment_reconcile_once`，可选

### 10.3 Worker 行为

- 每 1 到 5 秒轮询到期任务。
- 使用原子 `UPDATE ... WHERE status='pending' AND run_at <= now` 抢占任务。
- 失败后指数退避重试。
- 超过最大次数后标记为 `dead`。
- `job_attempts` 记录调试信息。
- 所有 job handler 必须幂等。

## 11. 支付 Provider 设计

内部采用 Rust trait，参考 Go 项目的 provider registry。

```rust
trait PaymentProvider {
    fn provider_type(&self) -> &'static str;
    fn validate_config(&self, config: &serde_json::Value, channel_type: &str) -> Result<()>;
    async fn create_payment(&self, input: CreatePaymentInput) -> Result<CreatePaymentResult>;
}

trait CallbackVerifier {
    async fn verify_callback(&self, input: CallbackInput) -> Result<CallbackResult>;
}

trait PaymentQuerier {
    async fn query_payment(&self, input: QueryPaymentInput) -> Result<QueryPaymentResult>;
}
```

Provider Registry：

- key 为 `(provider_type, channel_type)`。
- 先精确匹配。
- 再 fallback 到 `(provider_type, "")`。

首批支付渠道应按实际生产需要选择。建议优先级：

1. Epay/Yipay 类支付。
2. TokenPay 或 Epusdt，若需要虚拟币支付。
3. 官方支付宝/微信，仅在确实有官方商户资料时接入。
4. Stripe/PayPal 放到后续。

## 12. 后台范围

不直接移植 Dcat Admin。

Rust 后台首版实现：

- 登录/退出。
- 修改密码。
- Dashboard 概览。
- 分类管理。
- 商品管理。
- 卡密列表、导入、导出。
- 优惠券管理。
- 订单列表、详情、状态操作。
- 人工发货。
- 支付渠道管理。
- 邮件模板管理。
- 系统设置。
- 上传文件管理。

权限策略：

- 首版单超级管理员足够。
- 后续如有需要再增加轻量角色。
- 管理后台路径应可配置，降低被扫描风险。

## 13. 设置系统

使用持久化 `settings` 表，值为结构化 JSON。

建议设置 key：

- `site_config`
- `theme_config`
- `order_config`
- `smtp_config`
- `captcha_config`
- `notification_config`
- `payment_callback_routes`
- `security_config`

要求：

- 服务端统一默认值。
- 读取时规范化，缺字段自动补默认。
- 后台保存时校验。
- 敏感配置按需加密或至少避免前台泄露。

## 14. 安全要求

最低要求：

- 服务端渲染表单必须有 CSRF。
- Session Cookie 设置 `HttpOnly`、`SameSite`，生产环境启用 `Secure`。
- 无 Redis 限流：单进程内存限流 + 可选 SQLite 登录封禁记录。
- 管理员密码使用 Argon2。
- 启动时检查弱密钥并给出强警告。
- 支付回调必须校验签名、金额、渠道、币种和状态。
- 上传文件校验扩展名、MIME、大小和图片尺寸。
- 用户前台不得暴露成本价、provider 原始 payload、内部错误栈。
- 日志中避免记录完整密钥、完整卡密和支付密钥。

## 15. 测试策略

采用 Go 增强项目的测试纪律。

单元测试：

- 价格计算。
- 批发价计算。
- 优惠券校验和次数限制。
- 表单校验。
- 商品购买限制。
- 支付渠道配置解析。
- 支付回调签名验证。
- 订单状态流转。

集成测试：

- 自动发卡商品下单时预占卡密。
- 人工商品下单时锁定人工库存。
- 订单超时释放卡密和库存。
- 支付成功回调幂等。
- 金额不一致回调被拒绝。
- 自动发卡只消费该订单预占卡密。
- 人工发货写入履约记录。
- SQLite 并发下单不超卖。

迁移测试：

- 当前 MySQL 导出导入 Rust SQLite。
- `carmis` 到 `card_secrets`。
- `orders` 到 `orders/order_items/payments/fulfillments`。
- `pays` 到 `payment_channels`。
- 源表行数和目标表行数报告。

## 16. 实施阶段

### Phase 0：规格冻结

交付物：

- 路由兼容清单。
- 数据映射文档。
- 首版必须支持的支付渠道清单。
- 首版主题范围。
- 后台最小可用工作流。

### Phase 1：项目基础

交付物：

- Rust workspace。
- 配置加载。
- 日志。
- SQLite 连接和 migrations。
- 内嵌静态资源服务。
- 健康检查接口。
- 首次管理员初始化。

### Phase 2：商品和前台渲染

交付物：

- 分类。
- 商品。
- 默认 SKU。
- 主题选择。
- 首页。
- 购买页。
- 订单搜索页外壳。

### Phase 3：订单、库存、优惠券

交付物：

- 创建订单。
- 订单价格预览/计算。
- 卡密预占。
- 人工库存锁定。
- 优惠券使用。
- 浏览器订单 Cookie 兼容。
- 订单详情渲染。

### Phase 4：SQLite Jobs

交付物：

- `jobs` 表。
- 内嵌 worker。
- 订单超时取消。
- 自动发货任务。
- 邮件任务。
- API hook 任务。

### Phase 5：支付

交付物：

- 支付渠道模型和后台 CRUD。
- Provider Registry。
- 首个实际支付 provider。
- 创建支付。
- 兼容当前前台的二维码/跳转渲染。
- 回调校验和幂等。

### Phase 6：发货和邮件

交付物：

- 自动发货。
- 人工发货。
- `fulfillments` 记录。
- 邮件模板。
- SMTP 设置。
- 用户发货邮件。
- 管理员订单通知。

### Phase 7：后台

交付物：

- Dashboard。
- 商品 CRUD。
- 卡密导入/导出。
- 优惠券 CRUD。
- 订单列表/详情。
- 人工发货。
- 系统设置。
- 上传管理。

### Phase 8：迁移工具

交付物：

- 从当前独角数卡 SQL/MySQL 导出导入。
- dry-run 校验。
- 汇总报告。
- 可重复、可回滚的导入策略。

### Phase 9：加固发布

交付物：

- 并发测试。
- 支付回调测试。
- 前台截图冒烟测试。
- 备份/恢复文档。
- 生产配置文档。
- release 打包。

## 17. 首版功能边界

必须有：

- 当前用户前台体验。
- 分类/商品列表。
- 商品详情和购买流程。
- 自动发卡。
- 人工商品订单处理。
- 固定金额优惠券。
- 批发价。
- 按订单号、邮箱、浏览器 Cookie 查单。
- 选定支付渠道。
- 支付回调幂等。
- SQLite jobs worker。
- 后台商品、卡密、订单、支付、设置管理。
- SMTP 邮件。

应该有：

- 多前台主题，至少保留当前启用主题。
- 图片验证码。
- QQ/微信内置浏览器提示。
- API hook。
- 简单管理员通知。
- 基础 Dashboard 统计。

暂缓：

- 钱包。
- 会员等级。
- 分销。
- 上游采购。
- 公开用户账号体系。
- Telegram 登录/OIDC。
- 复杂 RBAC。
- 复杂对账中心。
- 多数据库支持。

## 18. 验收标准

功能验收：

- 全新 binary 不依赖任何外部服务即可启动。
- SQLite 自动初始化。
- 管理员可以创建分类、商品、卡密、优惠券、支付渠道。
- 用户可以通过当前前台风格创建订单。
- 自动发卡商品下单时预占卡密。
- 未付款订单过期释放卡密。
- 支付回调只能成功处理一次，不能重复发卡。
- 自动发卡生成履约记录并将卡密标记为 used。
- 订单详情用当前前台风格展示发货内容。
- 人工商品订单可在后台完成。
- 系统设置重启后仍然存在。

运维验收：

- 不需要 Redis。
- 不需要 MySQL/PostgreSQL。
- binary 直接服务内嵌前台静态资源。
- 上传文件外置，便于备份。
- 日志包含 request id、订单号、支付号等排障字段。

质量验收：

- 测试覆盖价格、优惠券、库存预占、支付回调、订单超时、自动发货。
- SQLite 并发下单测试证明不会超卖。
- 支付 provider 使用固定样例 payload 测试。
- 迁移 dry-run 输出源数据和目标数据行数。

## 19. 关键风险

支付 provider 风险：

- Rust 生态中的支付 SDK 可能不如 PHP/Go 完整。
- 规避：使用原始 HTTP + 签名逻辑实现 adapter，并用固定样例测试。

模板迁移风险：

- Rust 不能直接运行 Blade。
- 规避：保持 DOM/CSS/JS，机械迁移模板，并对关键页面做截图测试。

SQLite 写竞争：

- SQLite 单写者模型在高并发写入时需要谨慎。
- 规避：短事务、WAL、busy timeout、原子更新、事务内不做网络请求。

后台重建成本：

- Dcat Admin 不能直接复用。
- 规避：首版只做必要后台工作流，不追求 Dcat 全部体验。

队列替代风险：

- Redis/asynq 的语义需要由 SQLite jobs 重新承接。
- 规避：jobs 表 + 幂等 handler + 懒订单过期兜底。

数据迁移风险：

- 旧订单、旧卡密、旧支付渠道字段语义可能存在历史脏数据。
- 规避：先 dry-run，生成异常报告，再执行导入。

## 20. 立即下一步

1. 确认首版必须支持的支付渠道。
2. 确认首版是否必须同时支持 `unicorn`、`luna`、`hyper`，还是先支持当前启用主题。
3. 起草 SQLite schema migrations。
4. 做最小原型验证：
   - 启动 axum 服务。
   - 从内嵌资源渲染首页。
   - 初始化 SQLite。
   - 通过 seed 创建一个分类、商品、卡密。
5. 在接入支付前，优先完成订单创建、库存预占、订单超时释放。

## 21. 实现前锁定决策

为了降低首版实现发散，开始写代码前先锁定以下决策：

| 决策项 | 首版选择 | 原因 |
|---|---|---|
| 前台主题 | 优先迁移 `luna`，再评估 `unicorn` / `hyper` | 当前截图和项目资源显示 `luna` 适合作为第一验收主题，范围可控 |
| 后台形态 | Rust server-rendered admin | 不复用 Dcat Admin，避免 PHP 依赖 |
| 数据库 | SQLite only | 符合单 binary/无外部数据库目标 |
| ORM/SQL | `sqlx` typed query + 少量 query builder | SQLite 事务和并发语义需要明确控制 |
| 模板 | `minijinja` | 适合服务端渲染、模板继承和静态资源内嵌 |
| 静态资源 | `rust-embed` 或 `include_dir` | 编进 binary，满足单文件部署 |
| Session | 签名 Cookie + 可选 SQLite session | 首版单实例，Cookie 足够；后台可加服务端 session 表 |
| 金额 | 整数分 `i64` | 避免浮点误差，兼容当前两位小数 |
| 队列 | SQLite `jobs` + 内嵌 worker | 替代 Redis/asynq 和 Laravel queue |
| 首个支付 | Epay/Yipay 类 provider | 当前独角数卡生态最常见，接口相对简单 |
| 初始管理员 | 首次启动 setup 页 + CLI 重置命令 | 单 binary 部署友好 |

如后续明确生产环境必须支持其他支付渠道，可把支付 provider 顺序调整，但不应影响订单、库存、履约模型。

## 22. Rust 工程结构规划

建议首版使用单 crate，后续功能膨胀后再拆 workspace。

```text
/data/projects/free-market/
  Cargo.toml
  PLAN.md
  README.md
  config.example.toml
  migrations/
    0001_core.sql
    0002_catalog.sql
    0003_order_payment_fulfillment.sql
    0004_jobs_notifications.sql
    0005_seed_templates.sql
  assets/
    luna/
      css/
      js/
      img/
    common/
  templates/
    luna/
      layouts/
      static_pages/
      errors/
    admin/
      layouts/
      pages/
      partials/
    email/
  src/
    main.rs
    app.rs
    config.rs
    error.rs
    state.rs
    time.rs
    money.rs
    security/
      mod.rs
      csrf.rs
      password.rs
      session.rs
      rate_limit.rs
    db/
      mod.rs
      migrate.rs
      sqlite.rs
    models/
      mod.rs
      admin.rs
      catalog.rs
      coupon.rs
      order.rs
      payment.rs
      fulfillment.rs
      card_secret.rs
      setting.rs
      job.rs
    repositories/
      mod.rs
      admin_repo.rs
      category_repo.rs
      product_repo.rs
      sku_repo.rs
      card_secret_repo.rs
      coupon_repo.rs
      order_repo.rs
      payment_repo.rs
      fulfillment_repo.rs
      setting_repo.rs
      job_repo.rs
    services/
      mod.rs
      catalog_service.rs
      pricing_service.rs
      order_service.rs
      payment_service.rs
      fulfillment_service.rs
      coupon_service.rs
      setting_service.rs
      notification_service.rs
      migration_service.rs
    payment/
      mod.rs
      provider.rs
      registry.rs
      epay.rs
      noop.rs
    jobs/
      mod.rs
      worker.rs
      handlers.rs
      payloads.rs
    web/
      mod.rs
      router.rs
      extractors.rs
      middleware.rs
      frontend/
        mod.rs
        home.rs
        order.rs
        payment.rs
      admin/
        mod.rs
        auth.rs
        dashboard.rs
        products.rs
        card_secrets.rs
        coupons.rs
        orders.rs
        payments.rs
        settings.rs
    view/
      mod.rs
      assets.rs
      render.rs
    mail/
      mod.rs
      template.rs
      smtp.rs
    import/
      mod.rs
      dujiaoka_mysql.rs
      report.rs
  tests/
    pricing_test.rs
    order_flow_test.rs
    payment_callback_test.rs
    sqlite_concurrency_test.rs
```

目录职责：

- `models`：数据库行结构和业务枚举，不放复杂业务逻辑。
- `repositories`：只负责 SQL 查询和事务内数据读写。
- `services`：承载业务编排、状态流转和跨仓储事务。
- `web`：axum handler，做请求解析、调用 service、渲染模板。
- `payment`：统一支付 provider trait 和具体 provider adapter。
- `jobs`：SQLite 队列抢占、重试、任务处理。
- `view`：模板环境、资源 URL、主题渲染上下文。
- `mail`：邮件模板替换、SMTP 发送。
- `import`：旧独角数卡数据导入和 dry-run 报告。

## 23. Cargo 依赖规划

首版依赖建议：

```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "compression-full", "fs", "set-header"] }
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "sqlite", "chrono", "uuid", "json", "migrate"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
minijinja = { version = "2", features = ["loader"] }
rust-embed = "8"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
rand = "0.8"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
argon2 = "0.5"
password-hash = "0.5"
cookie = "0.18"
time = "0.3"
hmac = "0.12"
sha2 = "0.10"
base64 = "0.22"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
lettre = { version = "0.11", default-features = false, features = ["tokio1-rustls-tls", "smtp-transport", "builder"] }
```

说明：

- 版本在真正初始化工程时以当时 `cargo` 可解析版本为准。
- `sqlx` 采用 SQLite feature，不启用 Postgres/MySQL。
- 支付签名使用 `hmac`、`sha2`、`base64`，不依赖第三方支付 SDK。
- 若 `minijinja` 迁移 Blade 时遇到语法阻力，再评估 `tera`。

## 24. Migration 详细顺序

### 24.1 `0001_core.sql`

包含：

- `schema_migrations`
- `admins`
- `admin_sessions`，如果采用服务端 session
- `settings`
- `media`

关键约束：

- `admins.username` 唯一。
- `settings.key` 唯一。
- `settings.value_json` 必须为 JSON 文本，应用层校验。

### 24.2 `0002_catalog.sql`

包含：

- `categories`
- `products`
- `product_skus`
- `card_secret_batches`
- `card_secrets`

关键索引：

- `categories(is_active, sort_order)`
- `products(category_id, is_active, sort_order)`
- `product_skus(product_id, sku_code)` 唯一
- `card_secrets(product_id, sku_id, status, id)`
- `card_secrets(order_id)`

卡密占用必须使用事务内两步：

```sql
SELECT id
FROM card_secrets
WHERE product_id = ? AND sku_id = ? AND status = 'available'
ORDER BY id
LIMIT ?;

UPDATE card_secrets
SET status = 'reserved', order_id = ?, reserved_at = ?, updated_at = ?
WHERE id IN (...) AND status = 'available';
```

更新行数必须等于购买数量，否则回滚。

### 24.3 `0003_order_payment_fulfillment.sql`

包含：

- `coupons`
- `coupon_products`
- `coupon_usages`
- `orders`
- `order_items`
- `payment_channels`
- `payments`
- `fulfillments`
- `email_templates`

关键索引：

- `orders.order_no` 唯一。
- `orders(status, expires_at)`。
- `orders(guest_email, created_at)`。
- `order_items(order_id)`。
- `payments(order_id, status)`。
- `payments(gateway_order_no)`。
- `payments(provider_ref)`。
- `fulfillments(order_id)` 唯一或按首版一订单一履约记录处理。
- `coupon_usages(coupon_id, order_id)` 唯一。

### 24.4 `0004_jobs_notifications.sql`

包含：

- `jobs`
- `job_attempts`
- `notification_logs`

关键索引：

- `jobs(status, run_at)`
- `jobs(kind, status)`
- `jobs(locked_at)`
- `job_attempts(job_id, created_at)`

### 24.5 `0005_seed_templates.sql`

写入：

- 默认 `site_config`、`theme_config`、`order_config`、`smtp_config`。
- 默认邮件模板：自动发卡、人工订单通知、订单失败通知。
- 默认 `luna` 主题配置。

## 25. 首版数据表字段草案

字段类型以 SQLite 为准，时间统一存 ISO8601 字符串或 Unix timestamp；建议应用层统一 `chrono::DateTime<Utc>`。

### 25.1 `products`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | INTEGER PK | 商品 ID |
| `category_id` | INTEGER | 分类 |
| `slug` | TEXT UNIQUE | 预留 SEO/稳定链接 |
| `name` | TEXT | 对应旧 `gd_name` |
| `short_description` | TEXT | 对应旧 `gd_description` |
| `keywords` | TEXT | 对应旧 `gd_keywords` |
| `description_html` | TEXT | 对应旧 `description` |
| `image_path` | TEXT | 商品图 |
| `retail_price_cents` | INTEGER | 零售价 |
| `price_cents` | INTEGER | 实售价 |
| `wholesale_prices_json` | TEXT | 批发价阶梯 |
| `fulfillment_type` | TEXT | `auto` / `manual` |
| `manual_form_schema_json` | TEXT | 人工输入项 |
| `manual_stock_total` | INTEGER | 人工库存 |
| `manual_stock_locked` | INTEGER | 已锁人工库存 |
| `manual_stock_sold` | INTEGER | 已售人工库存 |
| `buy_limit_num` | INTEGER | 单次限购 |
| `buy_prompt` | TEXT | 购买提示 |
| `api_hook` | TEXT | 兼容旧回调配置 |
| `is_active` | INTEGER | 是否上架 |
| `sort_order` | INTEGER | 排序 |
| `created_at` | TEXT | 创建时间 |
| `updated_at` | TEXT | 更新时间 |
| `deleted_at` | TEXT NULL | 软删除 |

### 25.2 `orders`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | INTEGER PK | 订单 ID |
| `order_no` | TEXT UNIQUE | 对应旧 `order_sn` |
| `status` | TEXT | 显式订单状态 |
| `currency` | TEXT | 默认 `CNY` |
| `guest_email` | TEXT | 下单邮箱 |
| `guest_password_hash` | TEXT | 查询密码哈希，兼容期也可保存 legacy 明文校验字段 |
| `client_ip` | TEXT | 下单 IP |
| `original_amount_cents` | INTEGER | 原始金额 |
| `coupon_discount_cents` | INTEGER | 优惠券优惠 |
| `wholesale_discount_cents` | INTEGER | 批发价优惠 |
| `total_amount_cents` | INTEGER | 应付金额 |
| `coupon_id` | INTEGER NULL | 优惠券 |
| `payment_channel_id` | INTEGER NULL | 用户选择渠道 |
| `legacy_info` | TEXT | 旧人工输入/兼容显示 |
| `expires_at` | TEXT | 过期时间 |
| `paid_at` | TEXT NULL | 支付时间 |
| `canceled_at` | TEXT NULL | 取消时间 |
| `created_at` | TEXT | 创建时间 |
| `updated_at` | TEXT | 更新时间 |

### 25.3 `payments`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | INTEGER PK | 支付 ID |
| `payment_no` | TEXT UNIQUE | 内部支付号 |
| `order_id` | INTEGER | 订单 ID |
| `channel_id` | INTEGER | 支付渠道 |
| `provider_type` | TEXT | provider |
| `channel_type` | TEXT | alipay/wechat/qqpay/usdt 等 |
| `interaction_mode` | TEXT | redirect/qr |
| `amount_cents` | INTEGER | 支付金额 |
| `currency` | TEXT | 币种 |
| `status` | TEXT | 支付状态 |
| `provider_ref` | TEXT | 第三方流水 |
| `gateway_order_no` | TEXT | 网关侧订单号 |
| `pay_url` | TEXT | 跳转链接 |
| `qr_code` | TEXT | 二维码内容 |
| `provider_payload_json` | TEXT | 回调/创建响应摘要 |
| `paid_at` | TEXT NULL | 支付时间 |
| `expired_at` | TEXT NULL | 过期时间 |
| `callback_at` | TEXT NULL | 回调时间 |
| `created_at` | TEXT | 创建时间 |
| `updated_at` | TEXT | 更新时间 |

## 26. Rust 核心类型与接口草案

### 26.1 业务枚举

```rust
enum OrderStatus {
    PendingPayment,
    Paid,
    Fulfilling,
    Delivered,
    Completed,
    Canceled,
    Failed,
    Abnormal,
    PartiallyRefunded,
    Refunded,
}

enum PaymentStatus {
    Initiated,
    Pending,
    Success,
    Failed,
    Expired,
}

enum FulfillmentType {
    Auto,
    Manual,
}

enum CardSecretStatus {
    Available,
    Reserved,
    Used,
}
```

数据库中存小写字符串，Rust 中通过 `TryFrom<&str>` 和 `Display` 统一转换。

### 26.2 Service 边界

`PricingService`：

- `calculate_preview(product, sku, quantity, coupon) -> PricePreview`
- `parse_wholesale_tiers(raw) -> Vec<WholesaleTier>`
- `select_wholesale_unit_price(quantity, tiers) -> Option<Money>`

`OrderService`：

- `create_guest_order(input) -> OrderCreated`
- `get_order_for_frontend(order_no) -> FrontendOrderDetail`
- `search_by_order_no(order_no, password) -> OrderSearchResult`
- `search_by_email(email, password) -> Vec<OrderSummary>`
- `cancel_expired_order(order_id) -> CancelResult`

`PaymentService`：

- `create_payment(order_no, channel_id, client_ip) -> PaymentCreateResult`
- `handle_provider_callback(provider_type, channel_type, request) -> CallbackHandleResult`
- `apply_verified_callback(callback) -> Payment`

`FulfillmentService`：

- `auto_fulfill(order_id) -> Fulfillment`
- `manual_fulfill(order_id, payload, admin_id) -> Fulfillment`
- `render_payload_for_frontend(order_id) -> String`

`JobService`：

- `enqueue(kind, payload, run_at, max_attempts)`
- `claim_due(worker_id, limit)`
- `mark_succeeded(job_id)`
- `mark_failed_or_retry(job_id, error)`

## 27. 路由实现映射

### 27.1 前台路由

| 旧 Laravel 路由 | Rust handler | 说明 |
|---|---|---|
| `GET /` | `frontend::home::index` | 查询分类、商品、库存，渲染 `luna/static_pages/home.html` |
| `GET /buy/{id}` | `frontend::home::buy` | 商品详情、支付渠道、购买提示 |
| `POST /create-order` | `frontend::order::create_order` | 校验表单，创建订单，跳转 bill |
| `GET /bill/{orderSN}` | `frontend::order::bill` | 订单结算页 |
| `GET /detail-order-sn/{orderSN}` | `frontend::order::detail_by_no` | 订单详情页，读取履约内容 |
| `GET /order-search` | `frontend::order::search_page` | 查单页 |
| `GET /check-order-status/{orderSN}` | `frontend::order::check_status` | Ajax 轮询 |
| `POST /search-order-by-sn` | `frontend::order::search_by_no` | 订单号查单 |
| `POST /search-order-by-email` | `frontend::order::search_by_email` | 邮箱查单 |
| `POST /search-order-by-browser` | `frontend::order::search_by_cookie` | Cookie 查单 |
| `GET /pay-gateway/{handle}/{payway}/{orderSN}` | `frontend::payment::gateway` | 兼容旧入口，内部转 provider |

### 27.2 支付回调路由

首版统一新旧两套路由：

- 兼容旧：`/pay/{provider}/notify_url`
- 推荐新：`/payment/callback/{provider_type}/{channel_type}`
- 返回页：`/payment/return/{provider_type}/{channel_type}`

处理规则：

- handler 只负责收集 headers/body/query/form。
- provider adapter 完成签名验证和 callback 解析。
- `PaymentService` 完成支付记录和订单状态更新。

### 27.3 后台路由

首版后台路由前缀默认 `/admin`，配置项可修改。

| 路由 | 功能 |
|---|---|
| `GET /admin/login` / `POST /admin/login` | 登录 |
| `POST /admin/logout` | 退出 |
| `GET /admin` | Dashboard |
| `/admin/categories` | 分类 CRUD |
| `/admin/products` | 商品 CRUD |
| `/admin/products/{id}/cards` | 卡密管理 |
| `/admin/cards/import` | 卡密导入 |
| `/admin/coupons` | 优惠券 CRUD |
| `/admin/orders` | 订单列表 |
| `/admin/orders/{id}` | 订单详情 |
| `POST /admin/orders/{id}/fulfill` | 人工发货 |
| `/admin/payment-channels` | 支付渠道 CRUD |
| `/admin/email-templates` | 邮件模板 |
| `/admin/settings` | 系统设置 |

## 28. 第一轮实现冲刺拆解

第一轮目标不是完整上线，而是跑通“binary 启动、SQLite 初始化、luna 首页渲染、后台初始化”的基础闭环。

### Sprint 1：工程骨架

任务：

1. 创建 `Cargo.toml`、`src/main.rs`、`src/app.rs`。
2. 实现 `config.example.toml` 和配置加载。
3. 初始化 tracing 日志。
4. 初始化 SQLite pool，设置 WAL、busy_timeout。
5. 接入 migration runner。
6. 建立 axum router 和 health endpoint。

验收：

- `cargo run` 可启动。
- `GET /healthz` 返回 OK。
- 首次启动自动创建 `data/freemarket.db`。
- migration 可重复执行且不报错。

### Sprint 2：资源和模板

任务：

1. 复制并整理 `luna` 静态资源到 `assets/luna`。
2. 将 `luna` 首页、导航、页脚迁移成 `minijinja` 模板。
3. 实现 embedded asset handler。
4. 实现 `ViewRenderer` 和基础 layout。
5. 实现首页 mock 数据渲染。

验收：

- `GET /` 能渲染出和当前 `luna` 高度接近的首页。
- 背景图、CSS、JS 正常加载。
- 页面包含订单查询、公告、选择分类、选择商品、footer。

### Sprint 3：基础数据模型

任务：

1. 创建 `categories`、`products`、`product_skus`、`card_secrets` migration。
2. 实现对应 repository。
3. 实现 seed：一个分类、一个商品、若干卡密。
4. 首页改为读取 SQLite 数据。
5. 商品库存由 `card_secrets.status='available'` 计算。

验收：

- 首页显示真实 SQLite 商品。
- 卡密数量变化会反映到库存。

### Sprint 4：订单创建和库存预占

任务：

1. 创建 `orders`、`order_items`、`coupons`、`coupon_usages` migration。
2. 实现 `PricingService`。
3. 实现 `OrderService::create_guest_order`。
4. 自动发卡商品下单时预占卡密。
5. 写入浏览器订单 Cookie。

验收：

- `POST /create-order` 可创建订单。
- 卡密从 `available` 变为 `reserved`。
- 库存不足时事务回滚。
- 优惠券和批发价计算有单测。

### Sprint 5：SQLite Jobs 和订单过期

任务：

1. 创建 `jobs`、`job_attempts`。
2. 实现 worker claim/retry。
3. 下单写入 `order_timeout_cancel`。
4. 实现订单过期释放卡密。
5. 查询订单时执行懒清理。

验收：

- 未支付订单到期后变为 `canceled`。
- 预占卡密释放为 `available`。
- 重复执行过期任务不产生副作用。

### Sprint 6：支付 Provider 最小闭环

任务：

1. 创建 `payment_channels`、`payments`。
2. 实现 provider trait 和 registry。
3. 实现 `noop` 测试 provider。
4. 实现 Epay/Yipay provider。
5. 实现支付创建、二维码/跳转页、回调处理。

验收：

- 支付创建会生成 `payments` 记录。
- 回调金额不匹配会拒绝。
- 重复成功回调不会重复发卡。

### Sprint 7：自动发卡和订单详情

任务：

1. 创建 `fulfillments`。
2. 实现 `order_auto_fulfill` job。
3. 支付成功后将预占卡密转 `used`。
4. 写入履约记录。
5. 订单详情读取 `fulfillments.payload`。

验收：

- 支付成功后订单完成。
- 卡密状态为 `used`。
- 订单详情展示卡密。
- 邮件任务可入队。

## 29. 进入实现前的最小验收清单

开始编码时，每个阶段都必须满足：

- 数据库变更有 migration，不手工改库。
- 跨表业务状态变更必须在事务中完成。
- 订单号、支付号、job id 必须进入日志上下文。
- 所有外部 provider 请求不能放在数据库事务内。
- 所有 job handler 必须可以重复执行。
- 前台模板迁移后至少用截图或人工检查首页、购买页、订单页。
- 每完成一个核心 service，补最小单元测试。

第一批实现建议严格按顺序推进：

1. 工程骨架。
2. SQLite migration。
3. 内嵌 `luna` 首页。
4. 商品/分类/卡密读取。
5. 下单事务和卡密预占。
6. 订单过期释放。
7. 支付 provider 和回调。
8. 自动履约。

## 30. 二次差距审计结论

本节基于当前 Rust 实现状态、`PLAN.md` 既有规划，以及原 Laravel 项目的以下文件再次核对：

- `/data/projects/dujiaoka/routes/common/web.php`
- `/data/projects/dujiaoka/routes/common/pay.php`
- `/data/projects/dujiaoka/app/Admin/routes.php`
- `/data/projects/dujiaoka/app/Admin/Controllers/*Controller.php`
- `/data/projects/dujiaoka/app/Admin/Forms/SystemSetting.php`
- `/data/projects/dujiaoka/app/Http/Controllers/Home/OrderController.php`
- `/data/projects/dujiaoka/app/Service/OrderService.php`
- `/data/projects/dujiaoka/app/Service/OrderProcessService.php`
- `/data/projects/dujiaoka/database/sql/install.sql`
- `/data/projects/dujiaoka/resources/views/luna`

当前 Rust 版本已经具备：

- 单 binary + SQLite 启动。
- `0.0.0.0:8080` 默认监听。
- `luna` 首页、购买页、结算页、支付页、订单页、搜索页基础渲染。
- 分类、商品、卡密、订单、优惠券、支付通道、邮件模板、管理员、系统设置后台入口。
- 后台登录鉴权。
- 自动发卡商品下单预占卡密。
- 订单过期释放卡密。
- 优惠码下单、使用次数占用和取消释放。
- `noop` 支付闭环。

仍需补齐的关键差距：

| 优先级 | 差距 | 原项目依据 | Rust 补齐方向 |
|---|---|---|---|
| P0 | 批发价计算未完整落地 | `goods.wholesale_price_cnf`、`calculateTheWholesalePrice()` | 增加 `pricing_service`，订单创建使用批发价阶梯 |
| P0 | 搜索密码和浏览器 Cookie 查单不完整 | `is_open_search_pwd`、`freemarket_orders` Cookie | 下单写 Cookie，邮箱查单校验密码，浏览器 Cookie 查单 |
| P0 | 人工商品自定义输入未完整落库/展示 | `goods.other_ipu_cnf`、`validatorChargeInput()` | 商品表单保存 schema，下单校验并写入 `manual_form_json` / `legacy_info` |
| P0 | 真实支付 provider 缺失 | `routes/common/pay.php` 多 provider | 优先 Epay/Yipay，再 TokenPay/Epusdt |
| P0 | 邮件只管理模板，尚未发送 | `MailSend` job、`emailtpls` | SMTP 设置、模板变量渲染、发货/人工通知邮件 job |
| P0 | CSRF 未做 | Laravel 表单默认 CSRF | 后台和前台 POST 表单加 CSRF token |
| P1 | 商品字段不全 | `gd_keywords`、`picture`、`buy_prompt`、`api_hook` | 补商品后台字段和前台渲染 |
| P1 | 循环卡密未完整支持 | `carmis.is_loop`、`validatorLoopCarmis()` | 支持循环卡密只允许买 1 个，发货不置 used |
| P1 | 卡密导出和批量管理不足 | Dcat `carmis` export/batch restore | CSV/TXT 导出、批量删除可用卡密、状态筛选 |
| P1 | 支付渠道字段不足 | `pay_client`、`pay_method`、`merchant_*`、`pay_handleroute` | `config_json` 结构化保存旧字段，支持 PC/移动/全部 |
| P1 | 系统设置字段不足 | `SystemSetting.php` 多 tab | 扩展 settings：SEO、主题、验证码、通知、SMTP、安全 |
| P1 | 订单列表筛选不足 | Dcat 订单 filter | 支持订单号、邮箱、状态、商品、支付、时间范围筛选 |
| P1 | 上传文件管理缺失 | 商品图片、系统 logo | `uploads/` 服务、图片 MIME/大小校验、后台上传 |
| P1 | 支付二维码页未完全复刻 | `luna/static_pages/qrpay.blade.php` | 增加二维码支付模板和轮询状态语义 |
| P2 | 多主题未完整迁移 | `unicorn`、`hyper` | 首版稳定后迁移主题选择 |
| P2 | 安装/初始化页缺失 | `/install`、`/do-install` | 可选 Web setup 页，避免默认弱密码 |
| P2 | 通知渠道缺失 | ServerJiang、Telegram、Bark、企业微信 | SQLite jobs + HTTP notification adapters |
| P2 | 旧数据导入工具缺失 | `install.sql`、MySQL 旧表 | dry-run 导入和行数/异常报告 |

结论：

- 当前 Rust 版本已经达到“最小稳定卖卡闭环”的雏形。
- 若要接近原项目生产可替换程度，下一步必须优先补 P0，而不是继续扩后台边缘体验。
- Dcat Admin 的批量恢复、复杂 RBAC、扩展管理不进入 P0；保留为 P2 或长期可选。

## 31. P0 补齐规划

P0 目标：不追求 Dcat 体验完整，但必须让真实生产卖卡链路和原项目前台习惯一致。

### 31.1 PricingService 与批发价

新增模块：

- `src/services/pricing_service.rs`

数据来源：

- `products.wholesale_prices_json`
- `coupons`
- `coupon_products`

行为：

1. 解析旧格式 `wholesale_price_cnf` 到标准 JSON：
   - `[{ "quantity": 10, "unit_price_cents": 9000 }]`
2. 下单时按购买数量选择最后一个满足 `quantity <= buy_amount` 的阶梯价。
3. 订单保存：
   - `original_amount_cents`
   - `coupon_discount_cents`
   - `wholesale_discount_cents`
   - `total_amount_cents`
4. 结算页展示优惠码优惠和批发价优惠。
5. 单元测试覆盖：
   - 无批发价。
   - 单阶梯。
   - 多阶梯取最大满足项。
   - 优惠券和批发价叠加后金额不能小于 0。

实现顺序：

1. 新增 `pricing_service`。
2. `order_service::create_guest_order` 改为调用 `pricing_service`。
3. 后台商品管理补 `wholesale_prices_json` 编辑。
4. 前台购买页展示批发价标签。
5. `bill.html` 展示批发优惠。

验收：

- 购买 1 件使用原价。
- 购买达到阶梯数量后实际支付价下降。
- 订单表记录批发价优惠。

### 31.2 查单密码与浏览器 Cookie

原项目行为：

- 下单后把订单号追加到 `freemarket_orders` Cookie。
- 邮箱查单最多返回最近 5 条。
- 开启 `is_open_search_pwd` 时，邮箱查单必须提供查询密码。
- 浏览器查单从 Cookie 中读取订单号。

Rust 补齐：

1. `create_order` 成功后设置 `freemarket_orders` 和兼容名 `freemarket_orders`。
2. `SearchOrderForm` 拆为：
   - `SearchBySnForm`
   - `SearchByEmailForm { email, search_pwd }`
3. `search-order-by-email` 查询最近 5 条。
4. `search-order-by-browser` 从 Cookie 读取订单号列表。
5. `settings.order_config.is_open_search_pwd` 控制密码是否必填。

验收：

- 下单响应包含订单 Cookie。
- 浏览器查单能看到刚下单订单。
- 开启查单密码后，邮箱查单缺密码返回错误。

### 31.3 人工商品自定义输入

原项目字段：

- `goods.other_ipu_cnf`
- 格式由 `format_charge_input()` 解析。

Rust 补齐：

1. `products.manual_form_schema_json` 作为标准格式：
   - `[{ "field": "account", "label": "充值账号", "required": true }]`
2. 后台商品表单支持维护自定义输入 JSON。
3. 购买页按 schema 渲染输入框。
4. 下单时校验 required 字段。
5. 保存到 `order_items.manual_form_json`。
6. 订单详情和后台订单详情展示这些输入。

兼容导入：

- 旧 `other_ipu_cnf` 导入时转换成 `manual_form_schema_json`。
- 无法解析时原文写入 `legacy_info` 并在导入报告中标记。

验收：

- 人工商品缺必填自定义字段不能下单。
- 后台订单详情能看到用户填写内容。

### 31.4 真实支付 Provider

优先级：

1. Epay/Yipay 类 provider。
2. TokenPay。
3. Epusdt。
4. PayPal/Stripe/Coinbase 后置。

Epay/Yipay 适配：

- `provider_type = "epay"`
- `channel_type = "alipay" | "wxpay" | "qqpay"`
- `config_json`：
  - `pid`
  - `key`
  - `gateway_url`
  - `sign_type`

必须实现：

- 创建支付：生成跳转 form 或 URL。
- 同步返回页。
- 异步通知 `/pay/yipay/notify_url` 兼容路由。
- 新路由 `/payment/callback/epay/{channel_type}`。
- MD5 签名校验。
- 金额、订单号、渠道、支付单状态幂等校验。

验收：

- 固定样例回调签名验证通过。
- 金额不一致拒绝。
- 重复回调只发货一次。

### 31.5 邮件发送与通知 job

原项目模板 token：

- `card_send_user_email`
- `manual_send_manage_mail`
- `order_process_fail_mail`
- `pending_order`

Rust 补齐：

1. `smtp_config` 保存 SMTP：
   - host、port、username、password、encryption、from_address、from_name。
2. `mail/template.rs` 实现变量替换：
   - `{webname}`
   - `{weburl}`
   - `{order_id}`
   - `{created_at}`
   - `{ord_title}`
   - `{ord_info}`
   - `{product_name}`
   - `{buy_amount}`
   - `{ord_price}`
3. `notification_logs` 记录发送结果。
4. 支付成功自动发卡后入队 `order_status_email`。
5. 人工商品支付成功后入队管理员通知邮件。
6. 邮件失败按 jobs 重试。

验收：

- SMTP 配置为空时不阻断订单完成，只记录 skipped。
- 配置有效 SMTP 时能发送发货邮件。
- 邮件发送失败进入 job retry/dead。

### 31.6 CSRF 与基础安全

补齐范围：

- 前台下单。
- 前台查单 POST。
- 后台所有 POST。

设计：

- 登录后在 session 中生成 CSRF secret。
- 页面渲染时输出隐藏字段 `_csrf`。
- 中间件校验 `_csrf` 或 `x-csrf-token`。
- 支付回调路由跳过 CSRF。

验收：

- 缺 CSRF 的后台 POST 返回 403。
- 正常页面表单 POST 成功。
- 支付回调不受 CSRF 影响。

## 32. P1 补齐规划

P1 目标：让后台和前台更接近原项目主要体验，支持日常运营。

### 32.1 商品字段补全

当前需补：

- `short_description` 对应 `gd_description`。
- `keywords` 对应 `gd_keywords`。
- `image_path` 对应 `picture`。
- `retail_price_cents` 对应 `retail_price`。
- `buy_prompt`。
- `api_hook`。
- `sort_order`。
- `manual_stock_total`。

后台：

- 商品列表支持这些字段的新增/编辑。
- 图片字段通过上传管理选择。
- `api_hook` 增加启用提示，不在页面暴露密钥。

前台：

- 购买页展示 `buy_prompt`。
- 首页/购买页使用商品图片。
- SEO keywords/description 从商品和站点设置生成。

### 32.2 循环卡密

原项目语义：

- `carmis.is_loop = 1` 表示循环卡密。
- 有循环卡密时一次只能购买 1 个。
- 循环卡密发货后不标记为已售出。

Rust 设计：

- 复用 `card_secrets.is_loop`。
- 下单时如果商品存在可用循环卡密，`by_amount > 1` 拒绝。
- 循环卡密预占后支付成功：
  - 写入履约。
  - 卡密状态回到 `available` 或保持 `used` 但可重复发货需要单独语义。
- 推荐实现为：循环卡密不进入 `reserved/used` 状态机，而是在履约时读取并写入 payload，避免占用。

验收：

- 循环卡密商品买 2 个返回错误。
- 支付成功后同一循环卡密仍可被下一订单使用。

### 32.3 卡密导出、筛选、批量

后台补：

- 按商品、状态、是否循环筛选。
- 导出 CSV/TXT。
- 批量删除可用卡密。
- 批量软删除不允许影响 `reserved/used`。
- 导出默认脱敏显示，显式点击导出才输出完整卡密。

验收：

- 导出数量和筛选条件一致。
- 已售卡密不能批量删除。

### 32.4 支付渠道字段补全

旧 `pays` 字段映射：

| 旧字段 | 新字段 |
|---|---|
| `pay_name` | `payment_channels.name` |
| `pay_check` | `channel_type` 或 `legacy_pay_check` |
| `pay_method` | `interaction_mode` |
| `pay_client` | `client_scope` |
| `merchant_id` | `config_json.merchant_id` |
| `merchant_key` | `config_json.merchant_key` |
| `merchant_pem` | `config_json.merchant_pem` |
| `pay_handleroute` | `config_json.legacy_handleroute` |

需要新增：

- `client_scope`：`pc` / `mobile` / `all`。
- `legacy_pay_check`。
- 后台 config JSON 编辑器或 provider-specific 表单。

验收：

- 移动端只展示移动/全部支付。
- PC 端只展示 PC/全部支付。

### 32.5 系统设置补全

按原项目 `SystemSetting.php` 分组：

基础设置：

- title
- img_logo
- text_logo
- keywords
- description
- template
- language
- manage_email
- order_expire_time
- is_open_anti_red
- is_open_img_code
- is_open_search_pwd
- is_open_google_translate
- notice
- footer

订单推送：

- server_jiang
- telegram
- bark
- 企业微信机器人

邮件：

- driver
- host
- port
- username
- password
- encryption
- from_address
- from_name

极验：

- geetest_id
- geetest_key
- is_open_geetest

Rust 落地：

- 拆分为 `site_config`、`order_config`、`smtp_config`、`captcha_config`、`notification_config`、`theme_config`。
- 后台设置页改成多区块表单。
- 前台渲染统一通过 `SettingService` 获取，而不是只读启动配置。

### 32.6 订单后台筛选和状态操作

补齐筛选：

- 订单号。
- 邮箱。
- 状态。
- 商品。
- 支付通道。
- 时间范围。

状态操作：

- 取消待支付订单。
- 对已付款人工订单执行发货。
- 标记异常。
- 重新发送邮件。
- 查看支付流水。
- 查看履约记录。

不做：

- 后台直接把未支付订单改成已支付，除非有明确的“线下收款确认”功能，并且要生成内部支付流水。

### 32.7 上传和媒体管理

新增：

- `POST /admin/uploads`
- `GET /uploads/*path`
- 商品图片上传。
- logo 上传。

安全：

- 只允许图片扩展和 MIME。
- 限制大小。
- 文件名改为随机名。
- 不执行上传目录中的任何脚本。

验收：

- 上传图片后商品页能显示。
- 非图片上传被拒绝。

### 32.8 二维码支付页

补齐模板：

- `templates/luna/qrpay.html`

行为：

- 显示支付方式名称。
- 显示订单金额。
- 显示二维码。
- 移动端可显示“打开 App 支付”。
- 轮询 `/check-order-status/{order_no}`。

兼容响应：

- 原项目 `check-order-status` 返回：
  - `400001 expired`
  - `400000 wait....`
  - `200 success`
- Rust 当前可增加兼容字段，同时保留现代 JSON。

## 33. P2 补齐规划

P2 目标：迁移兼容、可运营增强和长期功能。

### 33.1 旧数据导入工具

命令：

- `free-market import dujiaoka --mysql-url ... --dry-run`
- `free-market import dujiaoka --sql-file install.sql --dry-run`
- `free-market import dujiaoka --mysql-url ... --apply`

导入范围：

- `goods_group -> categories`
- `goods -> products + product_skus`
- `carmis -> card_secrets`
- `coupons -> coupons`
- `coupons_goods -> coupon_products`
- `orders -> orders + order_items + payments + fulfillments`
- `pays -> payment_channels`
- `emailtpls -> email_templates`
- `admin_users -> admins`

报告：

- 源表行数。
- 目标表行数。
- 跳过行数。
- 无法解析字段。
- 重复数据。
- 金额差异。

验收：

- dry-run 不写 SQLite。
- apply 可重复执行不重复导入。

### 33.2 多主题迁移

顺序：

1. 完整 `luna`。
2. `unicorn`。
3. `hyper`。

要求：

- 路由不变。
- 主题由 `theme_config.template` 控制。
- 资源路径稳定。
- 关键页面 DOM/CSS 尽量保留。

### 33.3 安装和初始化页

补：

- `GET /install`
- `POST /do-install`

触发条件：

- 没有管理员。
- 或配置允许 setup。

能力：

- 设置站点名称。
- 创建管理员。
- 设置监听/基础 URL 提示。
- 初始化默认支付通道和邮件模板。

安全：

- 一旦管理员存在默认关闭安装页。
- 可通过环境变量强制关闭。

### 33.4 通知渠道

可选 adapters：

- ServerJiang。
- Telegram。
- Bark。
- 企业微信机器人。

统一为：

- `notification_config`
- `notification_logs`
- `admin_notification` job。

验收：

- 通知失败不影响订单完成。
- 后台可查看最近通知结果。

### 33.5 轻量 RBAC

当前首版单管理员足够。

后续可加：

- `roles`
- `permissions`
- `admin_roles`

最低角色：

- owner
- operator
- viewer

注意：

- 不复刻 Dcat 全套权限管理。
- 只做后台路由级权限。

## 34. 更新后的执行顺序

从当前代码状态继续推进，建议顺序：

1. P0-1：`PricingService`、批发价、结算页优惠展示。
2. P0-2：订单 Cookie、邮箱查单、浏览器查单、搜索密码。
3. P0-3：人工商品自定义输入。
4. P0-4：Epay/Yipay provider 和兼容回调路由。
5. P0-5：SMTP 设置、邮件模板变量、邮件 job。
6. P0-6：CSRF。
7. P1-1：商品字段和图片上传。
8. P1-2：循环卡密。
9. P1-3：卡密导出和筛选。
10. P1-4：支付渠道字段和客户端场景。
11. P1-5：系统设置完整化。
12. P1-6：订单筛选、支付流水、履约视图。
13. P1-7：二维码支付页。
14. P2：导入工具、多主题、安装页、通知渠道、轻量 RBAC。

每一步完成后必须满足：

- `cargo fmt`
- `cargo check`
- 相关 HTTP 冒烟。
- 不做 UI 自动化测试，除非后续明确要求。

## 35. 2026-06-15 P0 实施 Review

本轮已落地：

- `PricingService`：
  - 支持批发价 JSON 阶梯。
  - 下单时写入 `original_amount_cents`、`wholesale_discount_cents`、`total_amount_cents`。
  - 结算页展示原价、批发优惠、优惠码优惠。
- 查单：
  - 下单后写入兼容 Cookie `freemarket_orders`。
  - 支持订单号查单。
  - 支持邮箱 + 查询密码查单。
  - 支持浏览器 Cookie 查单。
- 人工商品：
  - 后台商品支持 `manual_form_schema_json`。
  - 购买页动态渲染人工输入字段。
  - 下单校验 required 字段。
  - 写入 `order_items.manual_form_json`。
  - 人工库存支持下单锁定、取消释放、人工发货转已售。
- 支付：
  - 增加 `epay/yipay` provider。
  - 支持 `pid/key/gateway_url/type` 配置。
  - 兼容 `/pay/yipay/notify_url`。
  - 支持 `/payment/callback/epay/{channel_type}` 和 `/payment/callback/yipay/{channel_type}`。
  - 回调校验 MD5 签名、金额、支付状态，并复用现有幂等发货流程。
- 邮件任务：
  - 支付发货后入队 `order_status_email`。
  - worker 消费任务并写入 `notification_logs`。
  - SMTP 未配置时记录 `skipped:smtp_config_not_enabled`，不阻断订单完成。
- 后台：
  - 商品后台补 `wholesale_prices_json`、`manual_form_schema_json`、`buy_prompt`、`manual_stock_total`。
  - 支付通道后台补 `config_json`。

本轮验证：

- `cargo fmt`
- `cargo check`
- `cargo build`
- `cargo test`
- 服务监听 `0.0.0.0:8080`
- 首页、查单页、后台主要页面 HTTP 200。
- 批发价下单冒烟：2 件商品原价 2000 分，批发优惠 400 分，应付 1600 分。
- 浏览器 Cookie 查单和邮箱查单返回 200。
- 人工商品缺必填字段返回 400；填写后写入 `manual_form_json`。
- 人工订单取消后 `manual_stock_locked` 释放为 0。
- Epay 通道配置保存成功。
- Noop 支付成功后自动发货，并产生邮件任务日志。

仍未完全完成的规划项：

- 真正 SMTP 发信：当前已完成 job 和日志，不含 lettre/SMTP 网络发送。
- CSRF：尚未加表单级 CSRF 中间件。
- 上传和媒体管理：尚未实现。
- 卡密导出、批量筛选：尚未实现。
- 支付 provider：已实现 Epay/Yipay 基线，TokenPay/Epusdt/PayPal/Stripe/Coinbase 尚未实现。
- 多主题：仍以 luna 为主。
- 旧数据导入工具：尚未实现。
- 安装初始化页：尚未实现。
- 通知渠道：ServerJiang/Telegram/Bark/企业微信尚未实现。

下一轮推荐顺序：

1. 表单级 CSRF。
2. SMTP 真发送。
3. 卡密导出和订单筛选。
4. 上传/媒体管理。
5. TokenPay/Epusdt。
6. 旧数据导入 dry-run。

## 36. 2026-06-15 继续实施 Review

本轮已落地：

- SMTP 真发送：
  - 增加 `lettre` SMTP 发送能力。
  - 后台系统设置新增 `smtp_config` 表单：启用、Host、端口、用户名、密码/授权码、发件邮箱、发件名称、加密方式。
  - 邮件 job 运行时读取 SQLite `settings.smtp_config`。
  - SMTP 未启用时写入 `notification_logs` 为 `skipped:smtp_config_not_enabled`，不阻断订单。
  - SMTP 配置错误或发送失败时写入 `failed` 日志，并让 job 按现有重试/Dead 机制处理。
  - 默认初始化 `card_send_user_email`、`manual_send_user_email` 邮件模板。
- 商品运营字段：
  - 后台商品 CRUD 接入 `short_description`、`keywords`、`image_path`、`retail_price_cents`、`buy_limit_num`、`api_hook`、`sort_order`。
  - 购买页商品图片支持 `/assets/...`、`/uploads/...`、`admin/xxx.png` 三类路径。
  - 首页人工商品库存改为 `manual_stock_total - manual_stock_locked`，不再只按卡密表统计。
- 循环卡密：
  - 后台卡密导入支持标记循环卡密。
  - 卡密列表和导出支持按 `is_loop` 筛选。
  - 自动发卡商品存在循环卡密且普通卡密不足时，购买数量大于 1 会拒绝。
  - 循环卡密支付发货后写入履约内容，同时释放回 `available`，支持后续订单继续使用。
  - 普通卡密仍保持 `reserved -> used` 状态机。

本轮验证：

- `cargo fmt`
- `cargo check`
- `cargo build`
- `cargo test`
- 服务重启后监听 `0.0.0.0:8080`。
- `GET /healthz` 返回 200。
- 后台登录后：
  - `GET /admin/products` 返回 200，并包含商品运营字段。
  - `GET /admin/settings` 返回 200，并包含 SMTP 设置。
  - `GET /admin/products/1/cards?is_loop=1` 返回 200，并包含循环卡密筛选。
  - `POST /admin/settings` 带 CSRF 返回 303。
- `GET /buy/1` 返回 200。
- 未进行 UI 自动化测试，符合当前约束。

当前仍剩余：

- TokenPay/Epusdt 等更多支付 provider。
- 旧独角数卡数据导入 dry-run/apply。
- 多主题迁移：`unicorn`、`hyper`。
- 安装初始化页。
- ServerJiang、Telegram、Bark、企业微信等通知渠道。
- 更细的后台订单筛选、支付流水面板、履约面板。
- 轻量 RBAC。

## 37. 2026-06-15 剩余功能继续实施 Review

本轮根据最新要求调整范围：

- 明确不再实施旧数据导入：当前没有旧数据需要导入，`import dry-run/apply` 从后续范围移除。

本轮已落地：

- 后台订单增强：
  - 订单列表新增商品、支付通道、开始时间、结束时间筛选。
  - 订单列表展示商品名、支付通道和创建时间。
  - 订单详情新增订单商品表。
  - 订单详情新增支付流水面板。
  - 订单详情新增通知日志面板。
- 轻量 RBAC：
  - `admins.role` 新增 `owner`、`operator`、`viewer`。
  - `owner` 拥有全部后台能力。
  - `operator` 禁止访问管理员和系统设置，其余后台工作流可用。
  - `viewer` 只允许 GET 查看，禁止 POST 修改。
  - 管理员后台可维护角色。
- 安装初始化页：
  - 新增 `GET /install`。
  - 新增 `POST /do-install`。
  - 已初始化时只提示进入后台登录。
  - 若希望首次启动不自动创建默认管理员，可设置 `FREEMARKET_ENABLE_INSTALL=1` 后启动，再通过安装页创建 owner 管理员。
- 通知渠道：
  - 新增 `notification_config` 设置项。
  - 后台系统设置新增 Server 酱、Telegram、Bark、企业微信机器人配置。
  - 新增 `admin_notification` job。
  - 自动发货和人工发货完成后会投递管理员通知 job。
  - 通知为空时写 `skipped:notification_config_empty`。
  - 通知失败写 `failed` 并按 jobs 重试，不阻断发货。
- 支付 provider：
  - 新增 TokenPay provider。
  - 新增 Epusdt provider。
  - 新增兼容回调：
    - `/payment/callback/tokenpay/{channel_type}`
    - `/payment/callback/epusdt/{channel_type}`
    - `/pay/tokenpay/notify_url`
    - `/pay/epusdt/notify_url`
  - TokenPay/Epusdt 回调支持 JSON 或 form-urlencoded。
  - 回调复用统一金额校验、签名校验和支付幂等发货流程。
  - 签名默认采用排除 `sign/signature` 后按键排序再拼接 `token/key` 的 MD5 模式；特殊网关可在通道 `config_json` 中设置 `"sign_mode":"none"`。
- 二维码支付页：
  - `PayPageData` 增加 `qr_code` 和 `interaction_mode`。
  - 支付页支持二维码/复制内容展示。
  - 跳转类通道继续显示支付按钮。
- 多主题运行时框架：
  - 前台渲染入口改为按 `site.theme` 选择模板。
  - 支持 `luna`、`unicorn`、`hyper` 三个主题名。
  - `unicorn/hyper` 当前使用已迁移的 luna 模板作为安全 fallback，确保切换主题名不导致 404。
  - 完整 `unicorn/hyper` 视觉模板迁移仍需单独机械迁移原 Blade/CSS/JS；当前生产可用前台仍以 luna 为准。

本轮验证：

- `cargo fmt`
- `cargo check`
- `cargo build`
- `cargo test`
- 主题 fallback 修改后再次执行 `cargo fmt && cargo check && cargo build && cargo test` 通过。
- 服务重启并成功应用 `0006_admin_rbac_notifications` migration。
- 服务继续监听 `0.0.0.0:8080`。
- `GET /healthz` 返回 200。
- 后台登录后：
  - `GET /admin/orders?product_id=0&payment_channel_id=0` 返回 200。
  - `GET /admin/admins` 返回 200，并包含角色字段。
  - `GET /admin/settings` 返回 200，并包含通知渠道。
  - `GET /admin/payment-channels` 返回 200，并包含 TokenPay/Epusdt。
  - `GET /install` 返回 200，并在已初始化时提示系统已初始化。
  - `POST /admin/settings` 带 CSRF 返回 303。
  - `POST /admin/payment-channels` 创建 TokenPay 测试通道返回 303。
- 未进行 UI 自动化测试，符合当前约束。

当前剩余说明：

- 旧数据导入已按要求排除。
- 多主题 `unicorn`、`hyper` 未继续迁移；当前生产前台仍以已复刻的 `luna` 为准。
- 多主题运行时选择已具备 fallback，不会因主题名导致页面不可用；但 `unicorn/hyper` 的完整视觉复刻仍未作为本轮交付完成。
- TokenPay/Epusdt 已实现通用适配和回调，真实生产网关如存在非标准签名字段顺序，可通过 `config_json` 的 `sign_mode` 或后续 provider-specific 配置微调。

## 38. 2026-06-15 多主题 Blade/CSS/JS 逐页迁移 Review

本轮按要求继续迁移原项目 `unicorn`、`hyper` 主题，不再只使用 luna fallback。

已落地：

- 静态资源：
  - 从 `/data/projects/dujiaoka/public/assets/unicorn` 复制到 `assets/unicorn`。
  - 从 `/data/projects/dujiaoka/public/assets/hyper` 复制到 `assets/hyper`。
  - Rust binary 继续通过 `/assets/*path` 内嵌服务这些资源。
- unicorn 页面：
  - `templates/unicorn/home.html`
  - `templates/unicorn/buy.html`
  - `templates/unicorn/bill.html`
  - `templates/unicorn/order.html`
  - `templates/unicorn/orders.html`
  - `templates/unicorn/search.html`
  - `templates/unicorn/pay.html`
  - 保留原主题的 Bootstrap/card/category/good-card/main-container 等结构和资源引用。
- hyper 页面：
  - `templates/hyper/home.html`
  - `templates/hyper/buy.html`
  - `templates/hyper/bill.html`
  - `templates/hyper/order.html`
  - `templates/hyper/orders.html`
  - `templates/hyper/search.html`
  - `templates/hyper/pay.html`
  - 保留原主题的 header-navbar/container/hyper-wrapper/buy-grid/pay-grid/card 等结构和资源引用。
- 渲染接入：
  - `ViewRenderer` 已改为加载真实 `unicorn/*` 和 `hyper/*` 模板文件。
  - 前台路由继续通过 `site.theme` 在 `luna`、`unicorn`、`hyper` 之间选择模板。
  - 商品列表数据新增规范化 `image_url`，供 unicorn/hyper 首页商品图片使用。

本轮验证：

- `cargo fmt`
- `cargo check`
- `cargo build`
- `cargo test`
- 主服务重启后继续监听 `0.0.0.0:8080`。
- 静态资源冒烟：
  - `/assets/unicorn/css/bootstrap.min.css` 返回 200。
  - `/assets/hyper/css/hyper.css` 返回 200。
- 临时 `theme=unicorn` 服务 HTTP 冒烟：
  - `/` 返回 200。
  - `/buy/1` 返回 200。
  - `/order-search` 返回 200。
  - `/detail-order-sn/{order_no}` 返回 200。
  - 页面包含 `category-menus`、`good-card` 等 unicorn 主题标记。
- 临时 `theme=hyper` 服务 HTTP 冒烟：
  - `/` 返回 200。
  - `/buy/1` 返回 200。
  - `/order-search` 返回 200。
  - `/detail-order-sn/{order_no}` 返回 200。
  - 页面包含 `hyper-wrapper`、`buy-grid` 等 hyper 主题标记。
- 未进行 UI 自动化测试，符合当前约束。

说明：

- 原 Blade 中 Laravel 翻译、captcha、geetest、二维码图片生成等 PHP 专属能力已映射为 Rust 当前可用能力或静态文本。
- 主题页面已经逐页迁移为 minijinja 模板，并接入当前 Rust 数据结构；后续如果要达到像素级 100% 还原，需要按具体截图继续微调 CSS/DOM 细节。

## 39. 2026-06-15 PLAN.md 与原项目再次差距审计及补齐规划

本节作为第 38 节之后的最新补齐基线。第 35 至第 37 节中记录的 SMTP、CSRF、上传、TokenPay/Epusdt、安装页、通知、多主题 fallback 等“剩余项”已经在后续实现中部分或全部完成，后续执行以本节清单为准。

继续保持以下边界：

- 不实施旧数据导入。当前没有旧数据需要导入，导入 dry-run/apply 继续从范围中移除。
- 保持单文件部署目标：Rust binary 内嵌前端模板和静态资源，SQLite 为唯一数据库，不引入 Redis、MySQL、PostgreSQL、队列中间件或外部缓存。
- 默认监听保持 `0.0.0.0:port`，不能回退到仅 `127.0.0.1`。
- 前端继续以原 Blade/CSS/JS 机械迁移为方向，优先保证路由、DOM 数据、表单和资源引用完整；像素级微调只在有具体截图或人工验收问题时执行。
- 验证继续使用 `cargo fmt`、`cargo check`、`cargo build`、`cargo test`、HTTP/SQL 冒烟；不做 UI 自动化测试。

### 39.1 已确认完成的主干能力

| 模块 | 当前状态 | 说明 |
| --- | --- | --- |
| 前台主流程 | 已具备 | 首页、购买、创建订单、确认订单、支付页、订单详情、订单查询均已实现。 |
| 多主题 | 已具备基础迁移 | `luna`、`unicorn`、`hyper` 均已接入真实模板和静态资源。 |
| 后台鉴权 | 已具备 | SQLite session、后台登录、角色 `owner/operator/viewer`、CSRF 已接入。 |
| 商品/分类/卡密 | 已具备 | 商品运营字段、人工商品输入项、批发价、循环卡密、卡密导入导出均已具备。 |
| 订单/履约 | 已具备 | 自动发卡、人工发货、取消释放库存、优惠码占用释放、订单详情面板均已实现。 |
| 支付 | 已具备核心通道 | `noop`、Epay/Yipay、TokenPay、Epusdt 与主回调链路已实现。 |
| 邮件/通知 | 已具备 | SMTP 真发送、邮件模板、ServerChan/Telegram/Bark/企业微信通知、SQLite job worker 已实现。 |
| 安装和上传 | 已具备 | `/install`、`/do-install`、后台上传和 `/uploads/*path` 已实现。 |

### 39.2 P0 兼容和可运营补齐项

这些项目直接影响“完全复刻原项目行为”或稳定卖卡，下一轮实现优先级最高。

| 编号 | 缺口 | 原项目依据 | Rust 当前状态 | 补齐规划 | 验收方式 |
| --- | --- | --- | --- | --- | --- |
| P0-1 | `/check-order-status/{orderSN}` JSON 不完全兼容 | Laravel 返回 `{"msg":"expired","code":400001}`、`{"msg":"wait....","code":400000}`、`{"msg":"success","code":200}` | 当前返回 `order_no/status` | 保留现代字段，同时追加 `msg/code`；不存在、过期、取消统一映射 `400001`，待支付映射 `400000`，已支付/已发货/已完成映射 `200` | HTTP 调用三类订单状态，确认旧 JS 可识别 |
| P0-2 | 直接支付路由不完整 | `/pay/{provider}/{payway}/{orderSN}` 与 `/pay-gateway/{handle}/{payway}/{orderSN}` 共存 | 当前只有 `/pay-gateway/:handle/:payway/:order_no` 创建支付页，回调路由只覆盖部分 provider | 增加 `/pay/:provider/:payway/:order_no` 网关页；`pay-gateway` 按 `handle` 解码后兼容转发或内部复用；补齐 `/pay/{provider}/return_url` 到订单详情的同步返回 | 访问 `/pay/yipay/{id}/{order}`、`/pay/tokenpay/{id}/{order}`、`/pay/epusdt/{id}/{order}` 均返回支付页或订单详情 |
| P0-3 | 零元订单处理 | 原 `redirectGateway` 中实际金额为 0 时直接完成订单 | 当前支付服务以支付通道创建支付为主 | 在支付入口检测 `total_amount_cents == 0`，直接走幂等成功和履约流程，不要求真实 provider | 创建 0 元订单后进入支付入口，订单变为已完成或待人工履约 |
| P0-4 | `api_hook` 仅保存未执行 | 原商品字段存在回调/Hook 运营语义 | 当前商品表单已有字段，但履约后没有投递 | 增加 `api_hook` job：支付成功并履约后按商品配置 POST 订单摘要、商品、数量、金额、履约状态；失败进入 jobs 重试和日志 | 配置本地测试 URL，支付成功后 jobs/notification_logs 或专用 hook_logs 可见结果 |
| P0-5 | 系统设置拆分不足 | 原 `SystemSetting` 包含基础、订单推送、邮件、geetest，多项开关 | 当前后台只有站点、SMTP、通知；主题、订单过期、查单密码、验证码开关未进后台 | 新增并落库 `order_config`、`theme_config`、`captcha_config`、`security_config`；后台可编辑 `template/theme`、`order_expire_time`、`is_open_search_pwd`、`is_open_img_code`、`is_open_geetest`、站点关键词/描述/管理邮箱 | 后台保存后刷新生效，重启后仍保留 |
| P0-6 | 查询密码开关行为不一致 | 原项目 `is_open_search_pwd` 控制邮箱查单是否必须提供密码；订单号查单不校验密码 | 当前订单号查单校验密码，邮箱有密码则筛选 | 按配置兼容：订单号查询默认不要求密码；邮箱查询仅在 `is_open_search_pwd=1` 时要求并校验；浏览器 Cookie 查询不要求密码 | 开关开启/关闭分别验证订单号、邮箱、浏览器三种查单 |
| P0-7 | 图形验证码/geetest 路由缺失 | `/check-geetest`，配置项 `is_open_img_code`、`is_open_geetest` | 当前未实现 captcha/geetest | 实现轻量图片验证码为默认方案，session 或签名 token 存储校验，不引入 Redis；`/check-geetest` 提供兼容 JSON；Geetest 作为可配置 passthrough，不阻断无配置环境 | 开启图片验证码后下单必须通过；关闭后不影响现有流程 |
| P0-8 | 支付通道客户端范围和旧字段映射不足 | 原后台 Pay 有 `pay_check`、`pay_client`、`pay_handleroute`、`merchant_id/key/pem` | 当前统一 `provider_type/channel_type/config_json`，无 PC/移动筛选 | 在 `payment_channels` 增加或以配置承载 `pay_check`、`client_scope(pc/mobile/all)`、`handleroute`、`merchant_id/key/pem` 兼容映射；前台按 User-Agent 过滤可用支付方式 | PC/移动请求可看到不同通道，旧 `pay_check` 可作为 provider/channel 定位 |
| P0-9 | 后台邮件测试入口缺失 | 原 `/admin/email-test` 可直接发送测试邮件 | 当前只能通过真实订单触发邮件 | 增加 `/admin/email-test` 页面和 POST，复用当前 SMTP 发送器，写入通知日志 | 后台发送测试邮件成功或展示明确失败错误 |
| P0-10 | 邮件模板默认保护不足 | 原默认模板通常不应被误删 | 当前可删除邮件模板 | 为默认模板增加 `is_system` 或按 key 保护；后台禁止删除系统模板，提供“恢复默认模板”动作 | 默认模板删除返回拒绝；恢复动作可重建缺失默认模板 |

### 39.3 P1 后台运维和长期稳定性补齐项

这些项目不一定阻断卖卡，但会影响日常运营效率、故障处理和长期稳定。

| 编号 | 缺口 | 补齐规划 | 验收方式 |
| --- | --- | --- | --- |
| P1-1 | 全局卡密管理页不足 | 增加 `/admin/cards` 全局列表，支持商品、状态、循环卡、关键词、创建时间筛选；保留商品内卡密页 | 全局筛选、导出、删除可用卡密正常 |
| P1-2 | 软删除回收站和恢复 | 对分类、商品、卡密、优惠码、支付通道、订单、邮件模板提供 `deleted_at` 列表和恢复动作；先实现最常用的商品/卡密/优惠码/支付通道 | 删除后可在回收站看到并恢复 |
| P1-3 | 后台订单筛选不完整 | 增加 `provider_ref/trade_no`、优惠码、履约类型、支付状态、IP、金额区间筛选 | 管理订单页 GET 参数过滤结果正确 |
| P1-4 | 通知和任务运维页 | 增加 `/admin/jobs` 和 `/admin/notification-logs`，支持查看失败原因、重试 dead job、按订单关联 | 构造失败 job 后可在后台重试 |
| P1-5 | SQLite 备份和清理 | 后台增加 SQLite 下载备份、上传文件孤儿清理、过期 session/job 清理；只操作本地文件，不引入外部存储 | 下载得到可打开的 DB 文件，清理动作有确认和日志 |
| P1-6 | 管理员安全增强 | 增加登录失败次数限制、管理员操作审计日志、弱默认密码提示、session 续期和主动失效 | 连续失败登录被短时间限制，关键 POST 写审计日志 |
| P1-7 | 支付 provider 表单专项化 | 后台按 provider 渲染推荐字段提示，减少直接编辑 JSON 的误配置 | 新建 Epay/TokenPay/Epusdt 通道无需手写完整 JSON |
| P1-8 | 支付返回和回调兼容细节 | 补齐 `GET/POST/ANY` 差异、`order_id/orderid/out_trade_no/trade_no` 参数别名、成功响应文本差异 | 用原项目 provider 参数样本回放通过 |
| P1-9 | 运行观测 | 增加 request id、关键业务日志、慢 SQL/慢请求阈值日志；保持日志写 stdout/file，不接入外部系统 | 日志能串联下单、支付、履约、通知 |
| P1-10 | 商品支付通道绑定 | 支持商品级可用支付通道白名单；未配置时使用全局可用通道 | 指定商品只显示绑定通道 |

### 39.4 P2 按生产需要选择的扩展项

这些项目不建议阻塞当前 Rust 版上线，只有在明确生产场景需要时再做。

| 编号 | 扩展项 | 规划 |
| --- | --- | --- |
| P2-1 | 更多历史支付 provider | PayPal、Stripe、Coinbase、官方 Alipay/Wepay、Mapay、PaysAPI、PayJS、VPay 可按“真实使用优先”逐个实现；不为未使用通道预先引入复杂 SDK。 |
| P2-2 | Geetest 完整服务端校验 | 如果生产必须使用 Geetest，再按当前版本官方接口实现完整二次校验；默认用轻量图片验证码即可满足无 Redis 部署。 |
| P2-3 | 像素级主题复刻 | 需要基于具体浏览器截图逐页人工验收；当前不做 UI 自动化测试，后续只根据明确页面差异修正 DOM/CSS。 |
| P2-4 | Dcat Admin 级完整体验 | 批量动作、所有资源回收站、复杂筛选器、富文本编辑器增强可逐步补齐，不影响核心卖卡。 |
| P2-5 | 会员、分销、钱包、上游采购 | 不属于当前原项目核心卖卡闭环，除非后续明确要扩展为平台型系统。 |

### 39.5 推荐下一轮实施顺序

1. P0-1、P0-2、P0-3：先补前台支付与轮询兼容，避免旧主题 JS 或旧链接失效。
2. P0-5、P0-6、P0-7：补系统设置、查单密码开关和验证码，统一前台行为。
3. P0-4、P0-8、P0-9、P0-10：补运营能力，覆盖 hook、支付通道配置、邮件测试和系统模板保护。
4. P1-1 至 P1-4：补后台运维入口，提升卡密、订单、通知、job 的处理效率。
5. P1-5 至 P1-10：补长期稳定性、安全、观测和商品支付通道绑定。
6. P2 项只在有明确生产需求时单独排期。

### 39.6 下一轮完成标准

- `PLAN.md` 中 P0 项全部标记为已实现或有明确推迟理由。
- 所有新增配置项有 migration、后台表单、默认值和重启后持久化验证。
- 所有新增前台兼容路由均有 HTTP 冒烟验证。
- 支付、履约、库存、优惠码、邮件、通知、hook 任一环节失败都不能破坏订单幂等性。
- 继续通过 `cargo fmt`、`cargo check`、`cargo build`、`cargo test`。
- 不执行 UI 自动化测试。

## 40. 2026-06-15 PLAN.md 39.5 实施 Review

本轮按第 39.5 推荐顺序继续实施，重点完成 P0 兼容项与 P1 运维稳定项。P2 中需要真实生产支付账号或像素级截图验收的内容继续按需排期，不为未确认通道引入复杂 SDK。

已落地：

- 数据库迁移：
  - 新增 `0007_plan39_compat_ops`。
  - `payment_channels` 补 `pay_check`、`client_scope`、`handleroute`、`deleted_at`。
  - `products` 补 `payment_channel_ids_json`。
  - `email_templates` 补 `is_system`、`deleted_at`。
  - 新增 `captcha_challenges`、`admin_audit_logs`、`admin_login_attempts`、`api_hook_logs`、`product_payment_channels`。
- 前台兼容：
  - `/check-order-status/{order_no}` 同时返回现代字段和旧字段 `msg/code`：
    - 不存在、取消、过期：`400001 / expired`
    - 待支付：`400000 / wait....`
    - 已支付/已完成：`200 / success`
  - 新增直接支付入口 `/pay/{provider}/{payway}/{order_no}`。
  - 新增同步返回入口 `/pay/{provider}/return_url`。
  - 新增通用异步回调入口 `/pay/{provider}/notify_url`，按 provider 分发到 Epay/Yipay、TokenPay、Epusdt。
  - 零元订单在支付入口直接走幂等成功和履约流程。
  - 运行时站点配置从 SQLite settings 合并读取，主题、订单过期时间可后台修改后生效。
- 查单和验证码：
  - 订单号查单恢复原项目行为：默认不校验查询密码。
  - 邮箱查单由 `order_config.is_open_search_pwd` 控制是否必须校验查询密码。
  - 新增 `/check-geetest` 兼容 JSON。
  - 新增 SQLite 一次性数学验证码，三个主题购买页均接入验证码字段；启用 `is_open_img_code` 后下单必须校验。
  - 新增 `/captcha/{id}.svg` 预留图片验证码资源入口。
- 支付通道：
  - 后台通道支持 `pay_check`、`client_scope`、`handleroute`。
  - 旧字段 `merchant_id`、`merchant_key`、`merchant_pem` 会同步写入 `config_json`，并映射到 `pid/key/token/gateway_url`。
  - 前台按 User-Agent 过滤 `pc/mobile/all` 通道。
  - 商品支持支付通道白名单，未配置时使用全局通道。
  - 支付通道支持软删除和回收站恢复。
- API Hook：
  - 商品 `api_hook` 不再只是保存字段。
  - 支付成功并履约后投递 `api_hook` job。
  - Hook POST 订单、商品、金额、履约等摘要；结果写入 `api_hook_logs`，失败按 jobs 重试。
- 邮件：
  - 新增 `/admin/email-test` 邮件测试页。
  - 默认邮件模板标记为系统模板，禁止删除。
  - 新增“恢复默认模板”动作。
- 后台运维：
  - 新增 `/admin/cards` 全局卡密列表，支持商品、状态、循环卡、关键词、时间筛选和导出。
  - 新增 `/admin/jobs` 任务队列页，支持 dead/running 任务重试和历史清理。
  - 新增 `/admin/notification-logs` 通知日志页。
  - 新增 `/admin/trash` 回收站，支持分类、商品、卡密、优惠码、支付通道、邮件模板恢复。
  - 新增 `/admin/audit-logs` 审计日志页。
  - 新增 `/admin/backup` SQLite 下载备份。
  - 上传页新增失效媒体记录清理。
  - 管理订单筛选补 `provider_ref/gateway_order_no/payment_no`、优惠券、履约类型、支付状态、IP、金额区间。
- 管理员安全：
  - 登录失败记录写入 SQLite。
  - 按 `security_config.login_max_attempts/login_lock_minutes` 做用户名 + IP 锁定。
  - 后台非 GET 操作写入 `admin_audit_logs`。
- 部署约束：
  - 服务继续监听 `0.0.0.0:8080`。
  - 未引入 Redis 或第三方数据库。
  - 前端模板和新增后台页面继续内嵌到 binary。

本轮验证：

- `cargo fmt`
- `cargo check`
- `cargo build`
- `cargo test`
- 服务重启后监听 `0.0.0.0:8080`。
- `0007_plan39_compat_ops` migration 已应用。
- HTTP 冒烟：
  - `GET /healthz` 返回 200。
  - `GET /buy/1` 返回 200。
  - 后台登录 `admin / admin123456` 返回 303。
  - `GET /admin/cards` 返回 200。
  - `GET /admin/jobs` 返回 200。
  - `GET /admin/notification-logs` 返回 200。
  - `GET /admin/trash` 返回 200。
  - `GET /admin/audit-logs` 返回 200。
  - `GET /admin/email-test` 返回 200。
  - `GET /admin/settings` 返回 200。
  - `GET /check-order-status/NO_SUCH_ORDER` 返回旧兼容结构 `400001 / expired`。
  - `GET /check-geetest` 返回兼容 JSON。
  - 下单成功返回 `/bill/{order_no}`。
  - 新订单状态轮询返回 `400000 / wait....`。
  - `GET /pay/noop/{payway}/{order_no}` 返回支付页。
  - Noop 支付成功后订单变为 `completed`，状态轮询返回 `200 / success`。

未执行：

- 未进行 UI 自动化测试，符合当前约束。
- P2 支付通道备注已由第 40 节更新；PayPal、Stripe、官方 Alipay/WechatPay 等已完成代码级 provider 移植，Coinbase、Mapay、PaysAPI、PayJS、VPay 不属于 `/data/projects/dujiao-next` 当前 10 个注册通道，本轮不纳入。
- 像素级主题差异未做截图验收；当前只保证代码实现和 HTTP 冒烟。

## 40. dujiao-next 支付通道移植实施记录

本轮目标：将 `/data/projects/dujiao-next` 中已注册的 10 个外部支付 adapter 移植到 `free-market`，继续保持单 binary、SQLite、无 Redis。

已完成：

- 支付抽象升级：
  - `PaymentProvider` 增加 `verify_callback`、`parse_webhook`、`query_payment` 可选能力。
  - `CreatePaymentResult` 增加 `amount_sent/currency_sent` 预留字段。
  - 回调解析统一输出 `PaymentCallback` 和 `PaymentStatus`。
- 支付注册表补齐：
  - `epay/yipay`
  - `tokenpay`
  - `epusdt`
  - `bepusdt`
  - `freemarketpay`
  - `okpay`
  - `official:stripe`
  - `official:paypal`
  - `official:alipay`
  - `official:wechat` / `official:wxpay`
- 通道实现：
  - Epay/Yipay：保留 v1 MD5 跳转，补 v2 RSA 跳转框架和统一回调。
  - TokenPay：JSON 创建订单、签名回调。
  - Epusdt：GMPay 创建订单、模板支付链接、签名回调。
  - Bepusdt：USDT/USDC/TRX trade_type 映射、创建订单、签名回调。
  - FreeMarketPay：HMAC API 下单、DJP webhook 验签。
  - Okpay：USDT/TRX payLink、签名回调。
  - Stripe：Checkout Session 创建、Stripe-Signature webhook 验签。
  - PayPal：OAuth token、Checkout Order 创建、Webhook Verify API 验签、预留 capture 查询。
  - 支付宝官方：RSA2 页面支付跳转、回调公钥验签。
  - 微信支付官方：APIv3 Native/H5 下单、RSA Authorization、APIv3 回调资源解密。
- 路由：
  - `/pay/{provider}/notify_url` 支持新增 provider。
  - `/payment/callback/{provider}/{channel_type}` 支持通用推荐回调地址。
  - official 通道前台回调路由按 channel_type 暴露，例如 `/pay/stripe/notify_url`。
- 后台：
  - 支付通道下拉补齐 `bepusdt`、`freemarketpay`、`okpay`、`official`。
  - 旧字段 `merchant_id/merchant_key/merchant_pem` 自动映射到各 provider 常用配置键。
- 文档：
  - `/docs/payment-channels.md` 补充 Bepusdt、FreeMarketPay、Okpay、Stripe、PayPal、官方支付宝、官方微信支付配置说明。

当前验证：

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo build`

未执行：

- 未进行 UI 自动化测试，符合当前约束。
- 未连接真实支付沙箱做外部网关联调；当前完成代码级移植、签名/验签/回调管线和构建验证。

### 40.1 前端逐通道展示补齐

本轮补齐 10 个注册支付通道在前台购买页的展示识别：

- `BuyPayChannel` 增加 `provider_type`、`channel_type`、规范化 `pay_check` 和短徽标 `badge`。
- Luna 主题：
  - 支付按钮输出 provider/channel/badge。
  - `assets/luna/main.js` 增加 `stripe`、`tokenpay`、`epusdt`、`bepusdt`、`freemarketpay`、`okpay`、`usdt`、`usdc`、`trx`、`wechat` 等映射。
  - 无专用 SVG 的通道使用短徽标展示，避免落到通用 other。
- Hyper 主题：
  - 支付按钮直接输出短徽标和通道名称。
  - 增加点击切换逻辑，确保隐藏 `payway` 跟随选中通道更新。
- Unicorn 主题：
  - radio 支付方式增加短徽标和 provider/channel 数据属性。
- 三套主题均能区分：
  - Epay/Yipay
  - TokenPay
  - Epusdt
  - Bepusdt
  - FreeMarketPay
  - Okpay
  - Stripe
  - PayPal
  - 官方支付宝
  - 官方微信支付

约束：

- 未引入新的前端依赖。
- 未进行 UI 自动化测试。

## 41. 2026-06-16 第七节差距补齐 Review

本节是对第 39/40 节之后再次审计（第 30 节级别的二次差距审计）并按"自动选择最优方案"全部实施的总结。

### 41.1 范围与决策

四个待决策项一律采用推荐方案：

| 决策 | 选择 | 原因 |
| --- | --- | --- |
| G1 订单状态扩充 | 仅补 `fulfilling`、`failed`；不补 `partially_refunded`/`refunded` | 退款功能在 PLAN §17 暂缓，未实现支付退款链路前补退款状态属过度设计 |
| G3 管理员通知收件箱 | 复用 `site_config.manage_email` | 与原 Laravel `dujiaoka_config_get('manage_email')` 完全一致，无需新增 `notification_config.admin_email` |
| G2 默认邮件模板内容 | 精简纯文本版 | 原 install.sql 中 HTML 模板每个约 8KB；保持简洁可读、不阻塞 SMTP；后台允许管理员替换为 HTML |
| G8 Geetest 实校验 | 维持 `/check-geetest` stub + 算术图片验证码主方案 | 不引入第三方 SDK，符合"无 Redis/外部依赖"约束 |

### 41.2 数据库迁移

新增 `migrations/0008_state_machine_v2.sql`：

- `orders.coupon_ret_back INTEGER NOT NULL DEFAULT 0`，幂等优惠码回退
- `products.sales_volume INTEGER NOT NULL DEFAULT 0`，权威销量计数
- `idx_orders_coupon_ret_back`

`src/db/migrate.rs` 注册 0008。

### 41.3 状态机扩充

`src/models/mod.rs`：

- 新增 `ORDER_FULFILLING = "fulfilling"`、`ORDER_FAILED = "failed"`

`src/services/payment_service.rs::apply_success`：

- 通过 `order_items.fulfillment_type` 判定自动 vs 人工
- 自动商品支付成功 → `paid → fulfilling`，随后调用 `auto_fulfill`，成功后 → `completed`
- 人工商品支付成功 → 保持 `paid`，调用新 `enqueue_manual_paid_emails` 入队 `pending_order`（买家）+ `manual_send_manage_mail`（管理员）

`src/services/fulfillment_service.rs`：

- `auto_fulfill` 失败时同时入队 `failed_order` 邮件并发管理员失败通知
- `auto_fulfill` 在事务内对 `order_items` 中每个商品递增 `products.sales_volume`，并对 manual 类型释放 `manual_stock_locked` / 增加 `manual_stock_sold`
- `manual_fulfill` 同时维护 `sales_volume`，邮件 token 改为 `completed_order`
- 新增 `enqueue_manual_paid_emails`、`resend_status_email`、`mark_abnormal` 三个对外函数

### 41.4 优惠码回退幂等

`src/services/order_service.rs::cancel_expired_order`：

- 在事务内 `UPDATE orders SET coupon_ret_back = 1 WHERE id = ? AND coupon_ret_back = 0`
- 仅当 `rows_affected = 1` 时执行 `coupons.used_count -= 1` 和 `coupon_usages.status = 'canceled'`
- 多次重试取消事务不会多次回退优惠码次数

### 41.5 邮件模板与变量

`src/services/bootstrap.rs::default_email_templates`：

- 6 个种子模板：`card_send_user_email`、`manual_send_user_email`、`manual_send_manage_mail`、`pending_order`、`completed_order`、`failed_order`
- 启动时全部标 `is_system = 1`
- 缺失模板（已删除）启动自动补回，"恢复默认模板"动作覆盖全集

`src/mail/mod.rs`：

- `EmailContext` 新增 `product_name`、`buy_amount`、`order_info`、`webname`、`weburl`、`created_at`
- 模板变量同时支持新写法 `{{ x }}` 和原项目 `{x}` 占位符
- 新增别名：`order_sn`/`ord_title`/`ord_info`/`ord_price` 等保留兼容
- `record_order_email_job` 支持 `payload.to` 覆盖收件人（用于管理员通知）
- 收件人为空时 `skipped` 不阻断流程

### 41.6 后台订单运维动作

`src/web/router.rs` 新增路由：

- `POST /admin/orders/:id/resend-email`
- `POST /admin/orders/:id/mark-abnormal`

`src/web/admin/mod.rs`、`src/services/admin_service.rs`、`templates/admin/order.html` 同步接入：

- 按订单状态映射 token 重发邮件
- 仅在 `paid/fulfilling/completed` 状态显示"标记为异常"按钮

### 41.7 销量展示

`src/services/catalog_service.rs::products_by_category`：

- 商品列表 `sold` 字段直接读 `products.sales_volume`
- 循环卡密销量得到正确统计（旧逻辑统计 `status='used'` 漏掉循环卡）

### 41.8 站点设置扩展

`src/config.rs::SiteConfig`：

- 新增 `keywords`、`description`、`is_open_anti_red`、`is_open_google_translate`

`src/services/settings_service.rs::runtime_site_config`：

- 合并以上字段，`base_url` 同步从设置覆盖
- 新增 `manage_email(state)` 辅助函数

`src/services/admin_service.rs`：

- `SettingsData/SettingsForm` 增加上述字段、`base_url`
- 后台 `/admin/settings` 表单和保存路径写入 `site_config` JSON

`templates/admin/settings.html`：

- 新增"站点外部 URL"输入框
- 新增"启用微信/QQ 内置浏览器提示"复选框
- 新增"启用 Google 翻译入口"复选框

### 41.9 全局脚本注入（防红 + Google 翻译）

`src/view/render.rs::ViewRenderer::render`：

- 通过模板路径前缀区分 admin / frontend
- 渲染后按 `site.is_open_anti_red`、`site.is_open_google_translate` 注入对应 `<script>` 片段到 `</body>` 之前
- 一次改动覆盖 luna/unicorn/hyper 的 7 个页面，无需 21 次模板编辑

### 41.10 观测：request-id 与慢请求日志

`src/web/observability.rs::request_id_middleware`：

- 接受外部 `x-request-id` 头，否则生成 uuid 作为请求 ID
- 通过 `tracing::info_span` 注入到 span，所有下游 tracing 输出可串联
- 请求耗时 >500ms 自动 `warn` 一条慢请求日志
- 响应头回写 `x-request-id`，便于排障

`src/web/router.rs`：

- 中间件 `request_id_middleware` 在 `TraceLayer` 外层应用，作用于所有路由

### 41.11 前台 SEO 与状态轮询兼容

`templates/{luna,unicorn,hyper}/buy.html`：

- `<head>` 注入 `keywords` 和 `description` meta，优先用商品 `short_description`，回退到站点级 `site.description`

`templates/{luna,unicorn,hyper}/home.html`：

- `keywords`/`description` meta 同步使用 `site.keywords`/`site.description`，回退到 `site.notice`

`src/services/order_service.rs::status_data`：

- `fulfilling` → `200/success`（旧 JS 不识别 fulfilling，但 `success` 触发跳转到详情）
- `abnormal` / `failed` → `400001/expired`（旧 JS 识别为终态）

### 41.12 编译与冒烟验证

- `cargo fmt --check` 通过
- `cargo check` 0 warning 0 error
- `cargo test` 2 passed
- `cargo build` 成功

HTTP 冒烟（重新部署的全新 SQLite）：

| 路径 | 结果 |
| --- | --- |
| `GET /healthz` | 200，响应头含 `x-request-id` |
| `GET /` | 200 |
| `GET /buy/1` | 200，head 含 `keywords`/`description` meta |
| `GET /order-search` | 200 |
| `GET /admin/login` | 200 |
| `GET /install` | 200 |
| `GET /check-order-status/NO_SUCH_ORDER` | `{"code":400001,"msg":"expired"}` |

业务链路冒烟（实际下单 + 模拟支付）：

| 步骤 | 结果 |
| --- | --- |
| 创建订单 | 303 → `/bill/<order_no>`，set-cookie `freemarket_orders` |
| 待支付轮询 | `{"code":400000,"msg":"wait...."}` |
| 模拟支付成功 | 卡密 `available → used`，订单 `pending_payment → completed`，`fulfillments` 写入 `auto/delivered`，`products.sales_volume` +1 |
| 已完成轮询 | `{"code":200,"msg":"success"}` |
| 故意释放预占卡再回调 | 订单 → `abnormal`，自动入队 `failed_order` 邮件 |
| 后台 `resend-email` | 303，入队 `card_send_user_email`（按 completed 状态匹配） |
| 后台 `mark-abnormal` | 303，订单 → `abnormal` |
| 启用 anti_red/google_translate | 前台首页注入对应 `<script>`/`<div>` |
| 后台页 | 不被注入（按模板路径前缀过滤） |

### 41.13 仍未实施的范围（明示）

按既定边界继续不在本轮执行：

- 旧 Laravel/Go 数据导入工具（无需求）
- Geetest 完整服务端校验（保留 stub + 算术验证码）
- 退款/部分退款状态机（PLAN §17 暂缓）
- Coinbase/Mapay/PaysAPI/PayJS/VPay 等额外支付 provider（按真实生产需要）
- `services/admin_service.rs` 拆分为 repositories 层（重构性，不影响功能）
- webhook 签名前置优化（当前签名/匹配在所有 provider 已可用）

### 41.14 决策与边界总结

- 单 binary + SQLite + WAL，仍无 Redis / 外部数据库 / 队列中间件
- 监听 `0.0.0.0:8080`，未回退至 127.0.0.1
- 静态资源与模板继续 `rust-embed/include_str!` 编进 binary
- 前端模板兼容三主题，DOM/CSS 未破坏
- 一次 `cargo fmt && cargo check && cargo build && cargo test` 通过，无新增警告

## 42. 2026-06-16 第二轮差距补齐（G21–G42）Review

本节是对 §41 之后再次系统审计（第二轮）并按"自动选最优"全部实施的总结。覆盖 P0 兼容/审计/路径前缀/回收站、P1 通知富文本/i18n/Logo/订单导出/中间状态、P2 healthz/备份/反索引/限购 共 12 项。

### 42.1 决策一览（用户授权"自动最优"）

| 决策项 | 选择 |
| --- | --- |
| G24 软删范围 | 仅 `canceled` / `abnormal` / `failed` / `pending_payment` 可软删；`paid` / `fulfilling` / `completed` 必须先标记异常再删 |
| G30 语言切换 | 仅切 `<html lang>`；不引入翻译表 |
| G34 人工中间状态 | 复用 `ORDER_FULFILLING`（不引入新常量 `processing`）|
| G23 各保留期 | sessions=即时 / succeeded jobs=30d / dead jobs=30d / captcha=expires_at / login_attempts=30d / audit_logs=90d / notification_logs=30d / api_hook_logs=30d |
| G32 导出范围 | 当前筛选 + 全部字段，最多 10000 行 / 次 |

### 42.2 数据库迁移

- `migrations/0009_orders_soft_delete.sql`：`orders.deleted_at` + 索引
- `migrations/0010_purchase_rate.sql`：`purchase_rate(id, email, client_ip, created_at)` 表 + 邮箱/IP 索引
- `src/db/migrate.rs` 注册两个新迁移

### 42.3 G21 admin_id 真实注入

- `src/security/session.rs`：
  - 新增 `pub struct AdminContext { id, role }`
  - `admin_auth_middleware` 在鉴权通过后 `request.extensions_mut().insert(AdminContext { … })`
- `src/web/admin/mod.rs::fulfill` 通过 `Extension<AdminContext>` 取真实 admin_id
- `src/services/admin_service.rs::fulfill` 签名改为 `(state, id, payload, admin_id: i64)`
- 旧硬编码 `Some(1)` 移除，多管理员下 `fulfillments.delivered_by` 不再失真

### 42.4 G22 admin_route_prefix 全链生效

- `AppState` 新增 `admin_prefix()` 和 `admin_url(suffix)`
- `state.rs::build` 规范化前缀（补 `/`、去末尾 `/`）后传给 `ViewRenderer::with_admin_prefix`
- `view/render.rs::ViewRenderer`：保存 `admin_prefix`，渲染上下文新增 `admin_prefix` 变量
- `web/router.rs`：所有 admin 路由统一 `nest(&admin_prefix, …)`，登录路径用 `format!("{}/login", admin_prefix)`
- `web/admin/mod.rs`：29 处 `Redirect::to("/admin/...")` → `state.admin_url("...")`；4 处 `format!("/admin/orders/{}",id)` → `format!("{}/orders/{}", state.admin_prefix(), id)`
- `security/session.rs`：`admin_auth_middleware`、`path_action`、`role_allows` 全部按 `admin_prefix` 工作
- 18 个 admin 模板：`/admin/` → `{{ admin_prefix }}/`（Python 一次性替换）
- 验证：当 `config.admin.route_prefix = "/console"` 时，`/admin/login → 404`、`/console/login → 200`，表单 action 输出 `/console/login`

### 42.5 G23 cleanup_runtime 扩充

`admin_service::cleanup_runtime` 现在清理：

- `admin_sessions WHERE expires_at <= now`
- `jobs WHERE status='succeeded' AND updated_at <= now - 30d`
- `jobs WHERE status='dead' AND updated_at <= now - 30d`
- `captcha_challenges WHERE expires_at <= now`
- `admin_login_attempts WHERE created_at <= now - 30d`
- `admin_audit_logs WHERE created_at <= now - 90d`
- `notification_logs WHERE created_at <= now - 30d`
- `api_hook_logs WHERE created_at <= now - 30d`

### 42.6 G24 订单软删除 + 回收站

- `admin_service::soft_delete_order`：仅允许 `canceled/abnormal/failed/pending_payment`
- `admin_service::restore_order` 通过 `restore_trash(state, "orders", id)` 统一入口生效
- 全部订单列表 SQL（list + count + export）追加 `WHERE o.deleted_at IS NULL`
- 回收站表列表追加 `("orders", "order_no")`
- 新路由 `POST /admin/orders/:id/delete`
- `templates/admin/order.html` 按当前状态显示"软删除"按钮，带二次确认

### 42.7 G25 通知通道显式开关 + G26/27/28/29 富文本

- `notification_config` 新增 5 个布尔：`is_open_server_chan` / `is_open_telegram` / `is_open_bark` / `is_open_bark_push_url` / `is_open_wecom`
- 启用判定改为"开关启用 AND key 非空"，方便临时禁用而保留 key
- 通知统一 `load_snapshot()` 取订单号/商品名/数量/金额/邮箱/支付通道/创建时间/实时库存
- ServerChan：`title + desp` markdown 列表 + `[查看详情](url)`
- Telegram：`parse_mode=Markdown` + `md_escape` 转义 `_*[\``，订单号/邮箱用反引号包裹，`disable_web_page_preview=true`
- Bark：`{title, body, group=site.logo_text, level=timeSensitive, url?}`；`is_open_bark_push_url=1` 时附 `url=base_url/detail-order-sn/{order_no}` 实现 iOS 点击直跳
- 企业微信：`title + 完整摘要`
- 后台 `settings.html` 新增 5 个 checkbox

### 42.8 G30/G31 语言 + 图片 Logo

- `config.rs::SiteConfig` 增 `language`（默认 `zh-CN`）和 `img_logo`
- `settings_service::runtime_site_config` 合并这两个字段
- `admin_service::settings/save_settings` 增 `language`/`img_logo` 读写，`SettingsData`/`SettingsForm` 同步
- `settings.html` 增"站点品牌与语言"分区：语言下拉（zh-CN / en-US）+ 图片 Logo URL 输入
- 三主题 `home.html` 的 `<html lang="…">` 改用 `{{ site.language | default('zh-CN') }}`

### 42.9 G32/G33 订单 CSV 导出 + 批量动作

- `admin_service::export_orders`：复用 `AdminOrdersFilter` 16 个筛选条件，输出 18 列 CSV，最多 10000 行
- CSV 字段值包含 `,"\n` 自动加引号转义
- `web/admin/mod.rs::export_orders`：返回 `text/csv; charset=utf-8` + `attachment; filename=orders-<timestamp>.csv`
- 新路由 `GET /admin/orders/export`
- `orders.html` 列表右上角加"导出 CSV"按钮，URL 自动带当前筛选条件

### 42.10 G34 人工订单 fulfilling 中间状态

- 新函数 `fulfillment_service::start_processing(order_id)`：原子 `paid → fulfilling`，状态校验不通过返回 `Conflict`
- `admin_service::start_order_processing` 暴露
- 新路由 `POST /admin/orders/:id/start-processing`
- `templates/admin/order.html` 在 `paid` 状态展示"开始处理（进入 fulfilling）"按钮

### 42.11 G35 /healthz JSON

- `router.rs::healthz` 改为 JSON：`status` / `version=CARGO_PKG_VERSION` / `db=ok|down`（即时 `SELECT 1`）/ `uptime_secs`（`OnceLock<Instant>`）/ `worker`
- 数据库不通时 `status=degraded` + `db=down`，仍 200，便于外部探活区分

### 42.12 G36 备份 gzip

- 新增 `flate2 = "1"` 依赖
- `web/admin/mod.rs::backup` 在内存中 `GzEncoder::new(Vec, Compression::default())` 压缩 SQLite 文件
- 响应头 `Content-Type: application/gzip` + 文件名 `freemarket-backup-<timestamp>.sqlite.gz`

### 42.13 G37 dead jobs 自动归档

由 §42.5 一并处理（cleanup_runtime 同时清 `succeeded` 和 `dead`）

### 42.14 G38 订单/支付/查单页 noindex

- Python 脚本一次性给 15 个模板（`luna/unicorn/hyper × order/orders/bill/pay/search`）的 `<head>` 注入 `<meta name="robots" content="noindex,nofollow">`
- 主页 `home.html` 与商品页 `buy.html` 保持可索引

### 42.15 G41 邮箱/IP 限购

- `OrderConfig` 增 3 字段：`purchase_rate_window_minutes`、`purchase_rate_max_per_email`、`purchase_rate_max_per_ip`，全 0 = 关闭
- `order_service::create_guest_order` 在事务开启前先做窗口内计数检查；命中阈值时 `BadRequest("同一邮箱/当前 IP 下单过于频繁，请稍后再试")`
- 事务提交成功后向 `purchase_rate(email, client_ip, created_at)` 写一条
- 后台 `settings.html` 三个 number 输入

### 42.16 编译与冒烟验证

- `cargo fmt --check` 通过
- `cargo check`、`cargo build`、`cargo test` 全部通过，零 error 零新增警告

HTTP / 业务冒烟（默认 `/admin`）：

| 项 | 结果 |
| --- | --- |
| `GET /healthz` | `{"db":"ok","status":"ok","version":"0.1.0","uptime_secs":…}` |
| `GET /buy/1` | 200 |
| `GET /admin/login` | 200 |
| 下单 → bill → 取消 → 软删 | 订单 `pending_payment → canceled` → `deleted_at` 填充 ✓ |
| `GET /admin/trash` | 表中可见 orders 行，"恢复"按钮可用 |
| `GET /admin/orders/export` | CSV 头部 + 数据行正确导出 |
| `GET /bill/<no>` head | 含 `<meta name="robots" content="noindex,nofollow">` ✓ |

HTTP 冒烟（自定义前缀 `route_prefix = "/console"`，端口 8081）：

| 项 | 结果 |
| --- | --- |
| `GET /admin/login` | **404** ✓ |
| `GET /console/login` | **200** ✓，表单 action=`/console/login` |
| `GET /healthz` | 200 + JSON |

### 42.17 未实施（保留为后续选项）

按"暂缓但记录在案"原则：

- 多 SKU 真正多规格选择（PLAN §8 表预留，但首版仍 sku=0）
- 完整 i18n 翻译表（仅切 `<html lang>`，文案未抽到字典）
- SQLCipher / GPG 加密备份（当前 gzip 已实现，加密按需）
- 双因素登录 / TOTP（dujiao-next 有参考实现，PLAN 暂缓）
- repositories 层 重构（admin_service.rs 已 3000+ 行，但功能稳定，重构不阻塞业务）

### 42.18 总体边界确认

- 单 binary + SQLite + WAL，无 Redis / 无外部数据库 / 无队列中间件 / 无第三方 i18n / 无外部观测系统
- 监听 `0.0.0.0:{server.port}`，不回退 127.0.0.1
- 全部模板与静态资源继续 `rust-embed/include_str!` 编进 binary
- 前端模板继续机械迁移原 Blade，DOM/CSS 不破坏
- 一次 `cargo fmt && cargo check && cargo build && cargo test` 通过

## 43. 2026-06-16 上生产最小动作清单实施 Review

本节是按 §"生产就绪度评估" 完整实施 B1–B6 + P1-1～P1-5 + P1-3 的总结。

### 43.1 P0 阻塞项

**B1 + B5a：Session Cookie 绑定 admin_prefix + Secure 标志**

- `security/session.rs::session_cookie(admin_prefix, secure, token)` / `expired_session_cookie(admin_prefix, secure)`：Path 跟随配置的管理后台前缀，且按 `cookie_secure` 输出 `; Secure`
- 自定义 `route_prefix = "/console"` 时 Cookie 也跟随发到 `/console/*`，不再丢失
- 默认 `cookie_secure = true`，可在 `/admin/settings → 验证码与安全` 关闭（本地 dev 用）

**B2：默认密码不再自动 seed 管理员**

- `services/bootstrap.rs::bootstrap`：当 `bootstrap_password == "admin123456"` 时，**拒绝** seed，记录一条 WARN 并指引访问 `/install`
- 旁路通道 `FREEMARKET_ALLOW_DEFAULT_ADMIN=1` 仅 dev/CI 使用
- 已验证：`cargo run` 后日志含 WARN，`admins` 表为空，`/install` 可访问

**B3：信任反代取 IP**

- 新模块 `security/net.rs::client_ip(state, headers, connect)`：
  - `security_config.trust_proxy_hops = 0` 时**忽略** XFF（防止直连客户端伪造）
  - `> 0` 时从 XFF 右侧跳 N 个可信代理取下一个地址
  - 缺 XFF 时回退 `X-Real-IP`，再回退 socket peer
- 前台 `create_order` 与后台 `login` 都改用此辅助函数
- 后台设置加 `trust_proxy_hops` 输入

**B4：CSRF 双 Cookie 模式**

- 新 `security/csrf.rs`：
  - 全局中间件，在 GET 响应自动 `Set-Cookie: freemarket_csrf=…; Path=/; SameSite=Lax[; Secure]`
  - POST/PUT/PATCH/DELETE 严格校验 `Cookie == _csrf 查询/x-csrf-token 头`
  - 常数时间比较，防 timing 攻击
- Cookie 不再硬编码 `Path=/admin`，全站 Cookie 让前端表单与后台表单都能携带
- 已验证：`set-cookie: freemarket_csrf=…; Path=/; SameSite=Lax; Max-Age=43200; Secure`；POST 缺 cookie 返回 403

**B5b：敏感字段加密落库**

- 新 `security/secrets.rs::SecretBox`：
  - SHA-256(`"free-market/v1/" || app_secret`) → 256-bit AES-GCM key
  - 输出格式 `enc:v1:<base64(12B nonce || ciphertext)>`
  - 空字符串透传；旧明文记录读取时也透传（迁移兼容）
  - 4 个 `#[cfg(test)]` 单元测试
- `config::AppConfig::effective_app_secret()`：优先级 `admin.app_secret` > `FREEMARKET_APP_SECRET` env > 派生 fallback
- `state::AppState` 持有 `Arc<SecretBox>`
- 加密目标：
  - `smtp_config.password`
  - `notification_config.server_chan_key`
  - `notification_config.telegram_bot_token`
  - `notification_config.bark_url`
  - `notification_config.wecom_webhook`
- 写路径：先解析既有值，新提交 `********` 时**保留**原密文，空字符串时清空，其它情况重新加密
- 读路径：
  - mail/notification 模块 `load_*_config` 内透明解密
  - settings 页用 `mask_value()` 把所有密钥替换为 `********`，**永不**回显明文或密文
- 已写 4 个单元测试，全部通过

**B6：6 个集成测试**

- 新增 `src/lib.rs` 暴露模块（`main.rs` 改用 `free_market::*`）
- 新增 `tests/common/mod.rs` + `tests/order_flow.rs`：
  1. `concurrent_buyers_never_oversell`：10 并发下 5 张卡，验证不超卖（reserved == 成功数、≤5、无重复 owner）
  2. `payment_amount_mismatch_rejected`：金额不一致回调被拒绝，订单仍为 `pending_payment`
  3. `repeated_success_callback_idempotent`：重复成功回调订单仍 `completed`，履约记录仅 1 条
  4. `coupon_refund_is_idempotent`：双次取消订单只回退 1 次优惠码、`coupon_ret_back=1`
  5. `cancel_releases_reserved_cards`：取消订单后 `reserved → available`
  6. `loop_card_limits_purchase_to_one`：仅有循环卡时购买数量 > 1 被拒绝
- `cargo test` 总数：12 (6 lib + 6 integration)，全绿

### 43.2 P1 可部署项

**P1-3：release profile**

```toml
[profile.release]
lto = "thin"
strip = "symbols"
codegen-units = 1
opt-level = 3
panic = "abort"
```

`cargo build --release` 产物：**19 MB**（debug 是 203 MB），LTO/strip 后单一二进制可直接 scp 上线。

**P1-4：Graceful shutdown**

- `main.rs::shutdown_signal()` 选择 `SIGINT` / `SIGTERM`（unix）
- `axum::serve.with_graceful_shutdown(...)` 等待请求完结再关闭
- 关闭时 `state.pool.close().await` 排空 SQLite 连接，避免脏写

**P1-5：RequestBodyLimit**

- `tower-http::limit::RequestBodyLimitLayer::new(8 * 1024 * 1024)` 全局生效
- 8MB 上限兼顾商品图片上传与防止 GB 级表单耗内存

**P1-1：Dockerfile + docker-compose.yml**

- 多阶段 `rust:1.83-bookworm → debian:bookworm-slim`
- 非 root 用户 `dujiao` 运行，目录权限独立
- compose 包含 app + nginx（生产 TLS 终止）
- 强制 `FREEMARKET_APP_SECRET` env 必填，无默认值
- 健康检查、卷映射 (`data/` `uploads/` `logs/` `config.toml`)

**P1-1 配套：Nginx 配置（`deploy/nginx.conf`）**

- HTTP → HTTPS 强跳转
- TLSv1.2 / TLSv1.3，HSTS、X-Frame-Options、nosniff、Referrer-Policy
- `X-Forwarded-For` / `X-Real-IP` / `X-Forwarded-Proto` / `X-Request-Id` 转发
- `/assets/` 30 天 immutable 缓存
- 注释说明 `set_real_ip_from` 配置与 `trust_proxy_hops` 联动

**P1-1 配套：systemd unit（`deploy/free-market.service`）**

- `User=freemarket`、`KillMode=mixed`、`KillSignal=SIGTERM`、`TimeoutStopSec=20s` 配合 P1-4 graceful
- 安全 hardening：`NoNewPrivileges`、`ProtectSystem=strict`、`ProtectHome`、`ReadWritePaths` 精确白名单
- `EnvironmentFile=/etc/free-market/secrets.env` 0600 保护 `FREEMARKET_APP_SECRET`

**P1-2：README**

`README.md` 涵盖：

- Quick start
- A/B/C 三种生产部署模式（Docker Compose + Nginx，systemd，bare process）
- 必做的首次启动步骤（特别强调 base_url、trust_proxy_hops、Cookie Secure 三联动）
- 完整的环境变量参考
- Backup & restore（含密钥与备份分离要求）
- Health check 响应示例
- 6 个集成测试列表
- Operational notes：Cookie Secure、trust_proxy_hops=0 含义、登录限流、敏感字段不回显、admin 前缀建议

### 43.3 验证

```
cargo fmt --check  ✓
cargo check        ✓ 0 error 0 warning
cargo build        ✓
cargo build --release  ✓ → 19 MB binary
cargo test         ✓ 12 passed / 0 failed
  - 6 lib (money: 2, secrets: 4)
  - 6 integration (order_flow)
```

HTTP 冒烟（默认配置 + 端口 8080，fresh DB）：

| 检查 | 结果 |
| --- | --- |
| 启动时 WARN "no admin exists and bootstrap_password is the well-known default" | ✓ |
| `admins` 表为空，不再自动 seed admin/admin123456 | ✓ |
| `GET /install` → 200 | ✓ |
| `GET /healthz` → `{"db":"ok","status":"ok",...}` | ✓ |
| `GET /buy/1` 响应头含 `set-cookie: freemarket_csrf=...; Secure` | ✓ |
| `POST /create-order?_csrf=fake` 无 cookie → 403 | ✓ |

### 43.4 当前生产就绪评分对比

| 维度 | §"评估"前 | §43 后 |
| --- | --- | --- |
| 业务功能 | 9/10 | 9/10 |
| 数据正确性 | 8/10 | 9/10（+集成测试覆盖核心不变式） |
| 安全 | 5/10 | **8/10**（B1–B5 全部解决） |
| 可观测 | 6/10 | 6/10 |
| 可部署 | 3/10 | **8/10**（Dockerfile/compose/nginx/systemd/README 全套） |
| 测试 | 1/10 | **6/10**（12 tests 覆盖 6 个核心路径，整体覆盖率仍偏低） |
| 文档 | 2/10 | 7/10（README + 部署 + payment-channels） |

**结论**：达到"小到中等规模生产环境可上线"门槛。仍需后续完成的低优先级项：

- 更多支付 provider 沙箱回归测试
- 商品 / 卡密 / 通道 API 层级测试
- metrics / 告警（Prometheus exporter 等）
- TOTP / 2FA
- SQLCipher 完整 DB 加密（gzip + 字段级加密已能覆盖常见威胁模型）

### 43.5 仍保留的合理已知限制

- `csrf_token` 仍为进程级（同源被 cookie 隔离 + 常数时间比较，已挡住 CSRF；并非真正 per-session token）
- SQLite 单写者模型下高并发买卡需要客户端重试（测试已显式记录这一不变式：`Conflict` 是预期行为）
- `admin_service.rs` 单文件 >3000 行，功能稳定但维护成本高（不阻塞上线）
- i18n 仅切 `<html lang>`，文案未抽到字典

## 44. 2026-06-16 生产就绪剩余补齐规划

本节基于当前代码状态重新评估第 43 节后的生产就绪差距。用户明确说明：**release 构建脚本不纳入本轮考虑**，因此本节不规划 `build.sh`、release 编译命令或启动脚本调整。

当前已确认的正向状态：

- `cargo test` 通过：6 个 lib 单元测试 + 6 个 `tests/order_flow.rs` 集成测试。
- `cargo build --release` 通过。
- 运行进程监听 `0.0.0.0:8080`。
- `/healthz` 返回 `status=ok`、`db=ok`。
- 核心订单不变式已覆盖：并发不超卖、金额不一致拒绝、支付回调幂等、优惠券取消回退幂等、取消释放卡密、循环卡密只能买 1 件。
- 10 个注册支付通道已完成代码级 Provider 注册、后台配置入口和前台展示识别。

当前不能直接判定“已达生产标准”的原因：

- `cargo fmt --check` 当前失败，说明基础质量门禁未过。
- SQLite 使用 WAL，但 `/admin/backup` 直接读取主库文件 gzip，热备份一致性不足。
- Dockerfile/Compose 健康检查定义与实际镜像能力不一致，容器健康状态不可信。
- 支付通道虽然完成代码级移植，但缺少各真实网关的沙箱或小额实付验收矩阵。
- 生产配置仍依赖人工正确设置：`FREEMARKET_APP_SECRET`、`site.base_url`、`cookie_secure`、`trust_proxy_hops`、后台默认密码/安装状态。
- `PLAN.md` 第 43 节中的部分验收记录代表当时目标状态，不再作为当前生产准入依据；本节以当前代码检查结果为准。

### 44.1 本轮补齐范围

| 编号 | 项目 | 本轮处理 | 不处理项 |
| --- | --- | --- | --- |
| R1 | 格式门禁 | 修复 `cargo fmt --check` 失败，并把 `cargo fmt --check` 放回验收清单 | 不调整构建脚本 |
| R2 | SQLite 可靠备份 | 将 `/admin/backup` 从直接读主库文件改为一致性备份方案 | 不引入外部备份服务 |
| R3 | 容器健康检查 | 修正 Dockerfile/Compose healthcheck，使其真实可用 | 不改变默认监听 `0.0.0.0:8080` |
| R4 | 支付通道验收矩阵 | 建立 10 个通道的配置、回调、失败路径验收清单 | 不要求一次性拿到所有真实商户账号 |
| R5 | 生产配置门禁 | 增加启动/后台可见的生产配置检查项，降低误配置概率 | 不引入复杂配置中心 |
| R6 | 文档状态校准 | 更新 README/PLAN 的生产准入口径 | 不重写历史规划章节 |

### 44.2 R1 格式门禁规划

问题：

- `src/security/csrf.rs` 中 `if let Some(form_token) = ...` 的换行格式不符合 rustfmt。
- 这不是业务 bug，但生产发布必须把格式检查作为最低质量门禁。

处理方案：

- 执行 `cargo fmt` 修正全项目格式。
- 再执行 `cargo fmt --check` 确认通过。
- 与后续验证一起执行 `cargo check`、`cargo test`。

验收：

```bash
cargo fmt --check
cargo check
cargo test
```

### 44.3 R2 SQLite 可靠备份规划

问题：

- 当前 SQLite 开启 `PRAGMA journal_mode=WAL`。
- `/admin/backup` 当前实现直接读取 `data/freemarket.db` 并 gzip。
- WAL 模式下最新提交数据可能仍在 `freemarket.db-wal`，直接复制主库文件不是可靠热备份。

推荐方案：优先使用 SQLite 原生一致性导出，不引入第三方数据库或外部服务。

实现选型：

| 方案 | 优点 | 风险 | 选择 |
| --- | --- | --- | --- |
| `VACUUM INTO` 导出临时 db，再 gzip | SQLite 原生一致性快照，代码简单，适合单机 | 需要临时文件空间，导出期间有额外 IO | **首选** |
| `sqlite3_backup` API | 更标准的在线备份 API | `sqlx` 未直接暴露，可能引入 `rusqlite`/底层连接复杂度 | 暂不选 |
| `PRAGMA wal_checkpoint(FULL)` 后读主库 | 改动小 | 仍不如快照语义清晰，繁忙时可能失败或影响写入 | 备选 |
| 离线停机复制 | 最稳 | 不满足后台在线下载备份体验 | 只作为运维文档补充 |

规划实现：

- `/admin/backup` 请求到来时：
  1. 在 `data/backup-tmp/` 或系统临时目录生成唯一 `.sqlite` 文件。
  2. 通过当前连接执行 `VACUUM INTO '<tmp_path>'` 生成一致性快照。
  3. 读取临时快照并 gzip 输出。
  4. 删除临时文件。
- 临时路径必须由程序生成，不能接受用户输入，避免 SQL 注入和路径穿越。
- 若 `VACUUM INTO` 失败，返回明确错误，并记录日志。
- README 备份说明改为“在线一致性 SQLite 快照 gzip”，不再暗示直接读取主库文件。

验收：

- 运行中创建一笔订单后立即下载 `/admin/backup`。
- 解压备份到临时库，查询该订单存在。
- 下载备份期间前台读请求不受影响。
- 备份完成后无残留临时文件。

### 44.4 R3 容器健康检查规划

问题：

- Dockerfile 当前 `HEALTHCHECK` 调用 `/app/free-market --healthcheck`，但二进制未实现该参数。
- docker-compose 当前 healthcheck 使用 `wget` 访问 `/healthz`，但 slim 镜像未安装 `wget`。
- 这会导致容器健康状态失真，生产编排无法可靠判断实例状态。

推荐方案：使用应用内置 `--healthcheck`，避免在镜像中额外安装 curl/wget。

实现规划：

- 在 `main.rs` 启动早期解析 `std::env::args()`：
  - 如果参数为 `--healthcheck`，读取配置，连接 SQLite，执行 `SELECT 1`，成功退出 0，失败退出非 0。
  - 不启动 HTTP server，不启动 worker，不执行 seed 管理员流程。
- Dockerfile 保留 `HEALTHCHECK CMD ["/app/free-market", "--healthcheck"]`。
- docker-compose healthcheck 改为同样调用 `/app/free-market --healthcheck`，去掉对 `wget` 的依赖。
- `/healthz` 保留给外部负载均衡和人工检查。

验收：

```bash
/app/free-market --healthcheck
# exit code 0 when db is reachable
```

容器场景：

- `docker compose ps` 显示 healthy。
- 临时破坏 DB 路径权限时，healthcheck 返回 unhealthy。

### 44.5 R4 支付通道生产验收矩阵规划

问题：

- 当前 10 个通道已完成代码级接入和注册，但真实支付 Provider 的风险集中在：签名字段、金额单位、回调格式、同步返回、失败/过期事件、网关非标准差异。
- 自动化测试目前没有覆盖所有外部网关真实协议。

验收原则：

- 代码级 provider 完成不等于生产支付可用。
- 每个通道至少需要“创建支付 + 成功回调 + 金额不一致拒绝 + 重复回调幂等”四类验证。
- 有沙箱的走沙箱；没有沙箱的走小额真实支付；无法取得账号的通道标记为“代码已接入，生产未验证”。

矩阵：

| Provider | Channel 示例 | 创建支付 | 成功回调 | 失败/过期 | 重复回调 | 生产准入 |
| --- | --- | --- | --- | --- | --- | --- |
| noop | test | 已有本地链路 | 已有本地链路 | 不适用 | 需保留测试 | 仅测试环境 |
| epay/yipay | alipay/wxpay/qqpay | 待沙箱/小额 | 待验证签名 | 待验证 | 待验证 | 验证后启用 |
| tokenpay | usdt/trx | 待沙箱/小额 | 待验证签名 | 待验证 | 待验证 | 验证后启用 |
| epusdt | usdt | 待沙箱/小额 | 待验证签名 | 待验证 | 待验证 | 验证后启用 |
| bepusdt | usdt/usdc/trx | 待沙箱/小额 | 待验证签名 | 待验证 | 待验证 | 验证后启用 |
| freemarketpay | token_id | 待沙箱/小额 | 待验证 webhook | 待验证 | 待验证 | 验证后启用 |
| okpay | usdt | 待沙箱/小额 | 待验证签名 | 待验证 | 待验证 | 验证后启用 |
| official/stripe | stripe | 待 Stripe test mode | 待验证 webhook secret | 待验证 | 待验证 | 验证后启用 |
| official/paypal | paypal | 待 PayPal sandbox | 待验证 webhook verify | 待验证 | 待验证 | 验证后启用 |
| official/alipay | alipay | 待沙箱/小额 | 待验证 RSA | 待验证 | 待验证 | 验证后启用 |
| official/wechat | wechat/wxpay | 待沙箱/小额 | 待验证 v3 解密 | 待验证 | 待验证 | 验证后启用 |

规划产物：

- 新增 `docs/payment-provider-acceptance.md`。
- 每个通道记录：配置字段、测试商户环境、创建支付结果、回调样例脱敏、验收状态、上线开关建议。
- 后台支付通道列表可在后续增加“生产已验证”备注字段；本轮先文档化，不改表结构。

### 44.6 R5 生产配置门禁规划

问题：

- Rust 版默认配置适合本地启动，但生产依赖操作者正确配置。
- 关键配置错误会直接影响支付回调、Cookie、安全和密钥解密。

规划实现：

- 增加一个生产配置检查服务 `production_readiness`，不阻塞本地开发，但在后台 dashboard 或 settings 页显示风险项。
- 检查项：
  - `FREEMARKET_APP_SECRET` 或 `admin.app_secret` 是否显式设置，长度是否足够。
  - `site.base_url` 是否为公网域名，不能是 `0.0.0.0`、`127.0.0.1`、`localhost`。
  - 是否已安装管理员，且没有默认 `admin/admin123456`。
  - `cookie_secure` 与 `X-Forwarded-Proto`/HTTPS 部署说明是否匹配。
  - `trust_proxy_hops` 是否按反代层数设置。
  - 是否存在启用的真实支付通道，且文档标记已验证。
  - 是否配置 SMTP 或至少配置一种通知渠道。
  - `/healthz` DB 状态是否 ok。
- 输出等级：`blocker`、`warning`、`info`。
- 后台 dashboard 顶部展示 blocker/warning 数量和具体说明。

不做：

- 不阻止二进制启动，因为本项目仍需支持本地、内网、测试环境。
- 不引入外部配置中心。

验收：

- 默认配置启动时后台显示 `base_url`、`app_secret`、支付通道未验证等 warning/blocker。
- 正确生产配置后，dashboard 风险项清零或只剩 info。

### 44.7 R6 文档状态校准规划

问题：

- 第 43 节记录了当时设想的上线门槛，但当前检查发现仍有偏差。
- README 中备份、健康检查、部署说明需要与最终实现保持一致。

规划：

- README 更新：
  - 备份说明改成 `VACUUM INTO` 一致性快照。
  - healthcheck 说明同时覆盖 `/healthz` 和 `--healthcheck`。
  - 生产准入清单明确“真实支付通道需单独验收”。
- PLAN 更新：
  - 本节作为第 43 节后的修正准入口径。
  - 后续完成 R1-R6 后追加 Review 小节，记录实际验证命令与结果。

### 44.8 推荐实施顺序

1. R1：先修 `cargo fmt --check`，恢复基础质量门禁。
2. R3：补 `--healthcheck` 并修 Dockerfile/Compose 健康检查，解决部署状态可见性。
3. R2：改 `/admin/backup` 为 `VACUUM INTO` 一致性快照，解决数据安全底线。
4. R5：增加生产配置检查，降低误配置上线风险。
5. R6：同步 README/PLAN，避免文档继续误导上线判断。
6. R4：建立支付通道验收文档；真实账号到位后逐通道小额验收。

### 44.9 上生产准入标准

本轮完成后，允许进入“小流量真实生产”的最低标准：

- `cargo fmt --check` 通过。
- `cargo check` 通过。
- `cargo test` 通过。
- `/healthz` 返回 `db=ok`。
- Docker/container healthcheck 真实可用。
- `/admin/backup` 使用一致性快照，备份可离线打开并包含最新订单。
- `site.base_url` 为真实 HTTPS 域名。
- `data/app.secret` 自动生成且存在，迁移/恢复时与数据库、uploads 一起保留。
- 管理员通过 `/install` 创建，禁止默认密码自动 seed。
- 至少一个计划启用的真实支付通道完成沙箱或小额实付验收。
- SQLite 数据目录位于本地持久磁盘，不使用不可靠网络文件系统。
- 有明确的离线恢复流程：停止服务、替换 DB、恢复 uploads、启动服务、检查 `/healthz`。

### 44.10 当前结论

在不考虑 release 构建脚本的前提下，当前项目距离生产还差 R1、R2、R3、R4、R5、R6 六类补齐项。其中 R1/R2/R3/R5/R6 可以直接由代码和文档完成；R4 需要真实支付账号或沙箱环境配合，代码侧只能先提供验收矩阵和记录文档。

### 44.11 实施 Review

本轮按 §44 继续实施，仍然不改 `build.sh`，不把 release 构建脚本纳入范围。

已完成：

- R1 格式门禁：
  - 执行 `cargo fmt` 修复当前 rustfmt 差异。
  - `cargo fmt --check` 已通过。
- R2 SQLite 可靠备份：
  - `/admin/backup` 改为先执行 `VACUUM INTO` 生成一致性 SQLite 快照，再 gzip 输出。
  - 临时快照路径由程序生成，使用 UUID 文件名，并对 SQLite 字符串字面量做转义。
  - 成功读取后删除临时快照。
  - 本地手工验证 `VACUUM INTO` 可用，快照 `PRAGMA integrity_check` 返回 `ok`，迁移记录数量为 10。
- R3 容器健康检查：
  - `main.rs` 增加 `--healthcheck` 早期分支。
  - healthcheck 只读取配置、连接 SQLite、执行 `SELECT 1`，不启动 HTTP server、不启动 worker、不执行 bootstrap。
  - Dockerfile 保留并使用 `/app/free-market --healthcheck`。
  - docker-compose healthcheck 改为 `/app/free-market --healthcheck`，不再依赖 slim 镜像内不存在的 `wget`。
- R4 支付通道验收矩阵：
  - 新增 `docs/payment-provider-acceptance.md`。
  - 明确每个真实支付通道上线前需要完成创建支付、成功回调、金额不一致拒绝、重复回调幂等、失败/过期、同步返回验收。
  - 标记 `noop` 仅允许测试环境使用。
- R5 生产配置门禁：
  - Dashboard 增加“生产就绪检查”。
  - 检查项覆盖 app secret、`site.base_url`、管理员安装、默认管理员密码、HTTPS Cookie Secure、`trust_proxy_hops`、真实支付通道、邮件/通知渠道。
  - 输出 `blocker`、`warning`、`info`，只展示风险，不阻断本地启动。
- R6 文档状态校准：
  - README 备份说明改为 `VACUUM INTO` 一致性快照。
  - README healthcheck 同时说明 `/healthz` 和 `--healthcheck`。
  - README 增加真实支付通道必须通过沙箱或小额验收的说明。

验证结果：

```bash
cargo fmt --check   # passed
cargo check         # passed
cargo test          # passed, 12 tests
cargo build         # passed
cargo build --release # passed
./target/debug/free-market --healthcheck # exit 0
sqlite3 data/freemarket.db "VACUUM INTO 'data/backup-tmp/manual-check.sqlite';"
sqlite3 data/backup-tmp/manual-check.sqlite "PRAGMA integrity_check;" # ok
```

当前剩余生产前动作：

- 至少选择一个真实支付通道，按 `docs/payment-provider-acceptance.md` 完成沙箱或小额实付验收。
- 生产环境设置真实 HTTPS `site.base_url`、正确 `trust_proxy_hops`，确认 `data/app.secret` 存在并随备份保留，并在 dashboard 确认 blocker 清零。
- 在线备份已改为一致性快照，但仍需在目标部署环境跑一次下载、解压、离线恢复演练。

## 45. 2026-06-16 备份页面、定时备份与 base_url 管理化

本节响应新的生产化要求：

- `site.base_url` 必须可在后台管理页配置，默认值仍为 `http://0.0.0.0:8080`。
- 敏感配置加密密钥改为自动生成、持久化并定期换新，不再要求用户配置 `FREEMARKET_APP_SECRET`。
- 实现备份页面，可从页面点击按钮生成并下载 SQLite 备份。
- 增加定时备份设置：默认每周一上午 8 点备份一次，保留最近 7 个备份文件。

### 45.1 site.base_url 管理化

当前状态：

- 默认值在 `config.rs` 中仍为 `http://0.0.0.0:8080`。
- `/admin/settings` 已有“站点外部 URL”输入框。
- `settings.site_config.base_url` 保存后由 `settings_service::runtime_site_config()` 合并读取。

本轮补齐：

- 安装初始化写入 `site_config` 时也写入默认 `base_url`，避免设置页缺字段时产生歧义。
- 支付创建流程改为读取运行时 `site.base_url`，使后台修改后下一次支付请求立即影响：
  - `return_url`
  - `notify_url`
  - noop 测试支付返回 URL

### 45.2 应用密钥自动管理

应用密钥由程序自动生成并保存到 `data/app.secret`，用于派生 AES-GCM 加密 key，对以下敏感字段做落库加密：

- SMTP 密码/授权码。
- Server 酱 SendKey。
- Telegram Bot Token。
- Bark URL。
- 企业微信 Webhook。

要求：

- 用户不需要设置 `FREEMARKET_APP_SECRET`。
- 首次启动如果 `data/app.secret` 不存在，由程序自动生成 32 字节随机密钥并写入 0600 文件权限。
- 密钥需要随 `data/freemarket.db` 和 `uploads/` 一起迁移、备份和恢复。
- 丢失 `data/app.secret` 后，已加密字段无法可靠解密，需要重新录入 SMTP/通知等敏感配置。
- 后台生产就绪检查只检查自动密钥文件是否存在，不再要求人工配置环境变量。

### 45.3 备份页面

新增 `templates/admin/backup.html`：

- `GET /admin/backup` 渲染备份页面。
- 页面包含“立即备份”按钮。
- `POST /admin/backup/create` 生成一致性 SQLite 快照并保存到 `data/backups/`。
- 页面展示 `data/backups/` 下已保留的定时备份文件。
- 已保留备份可通过 `GET /admin/backup/files/{filename}` 下载。

安全处理：

- 下载历史备份时校验文件名必须以 `freemarket-backup-` 开头、以 `.sqlite.gz` 结尾，且不允许 `/`、`\\`、`..`。
- 所有 POST 操作继续走现有 CSRF。

### 45.4 定时备份

新增 `services/backup_service.rs`：

- `BackupConfig` 默认值：
  - `enabled = true`
  - `weekday = 1`，周一
  - `hour = 8`
  - `keep_files = 7`
- 配置保存到 `settings.backup_config`。
- 上次定时备份日期保存到 `settings.backup_last_run_date`，避免同一天重复执行。
- 备份文件存放在 `data/backups/`。
- 保留策略按文件名倒序保留最近 N 个，默认 7 个。

Worker 集成：

- `jobs::worker::run_once()` 每轮先调用 `backup_service::run_scheduled_if_due()`。
- 到达配置的星期和小时后执行一次定时备份。
- 时间使用服务器 UTC 时间。

### 45.5 验收标准

- `/admin/settings` 可保存 `site.base_url`，支付链接和回调链接使用保存后的运行时值。
- `/admin/backup` 返回备份页面。
- 点击“立即备份”可生成 gzip SQLite 文件，并在历史备份列表中下载。
- `data/backups/` 中的自动备份可在页面列出并下载。
- 默认配置为每周一 08:00、保留最近 7 个备份文件。
- `cargo fmt --check && cargo check && cargo test` 通过。

## 46. 2026-06-16 自动密钥生成与定期换新

本节响应“`FREEMARKET_APP_SECRET` 能否删除，不通过用户设置，改成自动生成，并定时换新”的要求。

设计结论：

- 可以删除用户必须配置 `FREEMARKET_APP_SECRET` 的要求。
- 不能每次启动随机生成临时密钥；否则已经加密落库的 SMTP、通知配置会无法解密。
- 最优实现是本地持久密钥文件：首次启动自动生成 `data/app.secret`，后续启动复用。
- 旧配置中的 `admin.app_secret` / `FREEMARKET_APP_SECRET` 仅作为兼容迁移来源，用于首次创建 `data/app.secret` 时把旧密钥加密的数据重加密到新密钥。

已规划并实施：

- `security::secrets::SecretManager`：
  - 持有当前 `SecretBox`。
  - 启动时从 `data/app.secret` 读取；不存在则自动生成并写入。
  - 写入时使用 0600 文件权限。
  - 暴露统一 `encrypt()` / `decrypt()`，业务层不直接依赖人工密钥。
- `secret_rotation_service`：
  - 默认 90 天换新一次。
  - 换新前使用旧 key 解密敏感字段。
  - 使用新 key 重新加密后再写入新 `data/app.secret`。
  - 更新 `settings.secret_last_rotated_at`。
- Worker 集成：
  - 定时任务循环中执行 `rotate_if_due()`。
- 后台生产就绪检查：
  - 检查 `data/app.secret` 是否存在。
  - 不再把缺少 `FREEMARKET_APP_SECRET` / `admin.app_secret` 当作 blocker。
- 文档与部署：
  - README、Docker Compose、systemd 示例不再要求配置 `FREEMARKET_APP_SECRET`。
  - 备份/恢复说明改为必须保留 `data/app.secret`。

验收标准：

- 不设置 `FREEMARKET_APP_SECRET` 也能启动并创建 `data/app.secret`。
- SMTP、通知等敏感字段仍以 `enc:v1:` 加密落库。
- 90 天轮换时敏感字段可被旧 key 解密、新 key 重新加密。
- 轮换失败不得覆盖旧密钥文件。
- `cargo fmt --check && cargo check && cargo test` 通过。

## 44. 2026-06-16 路径 A 实施 Review：/admin 改造为 SPA（soybean-admin 基底）

本节是 §"重构 /admin 为 SPA 的可行性与推荐做法"（路径 A）的整体落地总结。前台保持不动；`/admin` 改为 SPA + JSON API。

### 44.1 自动决策（用户授权"全部按推荐"）

| 决策项 | 选择 |
| --- | --- |
| 鉴权迁移 | JWT 完整迁移（HS256，access 30min，refresh 7d） |
| Refresh token 持久化 | 持久化到 SQLite `admin_refresh_tokens`，支持单次轮换 + 全设备登出 |
| 前端 i18n | 复用 soybean 自带的 zh-CN/en-US；默认锁 `VITE_FORCE_DEFAULT_LANG=zh-CN` |
| 老 HTML 后台兜底 | **直接替换**：22 个模板里删 20 个，保留 `login.html` + `install.html` 作 SPA 加载前的兜底 |
| 构建流水线 | `build.sh` 串行 `pnpm -C web-admin build` + `cargo build` |
| `VITE_BASE_URL` 与 `admin.route_prefix` 联动 | 构建时硬编码 `/admin/`（如要换前缀，重新 `pnpm build`） |

### 44.2 Phase A1：JWT auth + JSON API skeleton

新增：

- `Cargo.toml` 加 `jsonwebtoken = "9"`
- `migrations/0011_admin_refresh_tokens.sql`：`jti TEXT PRIMARY KEY, admin_id, issued_at, expires_at, revoked_at` + 双索引
- `src/security/jwt.rs`：
  - `Jwt::from_app_secret`：SHA-256 派生 256-bit HS256 key（与 `SecretBox` 同根，但用独立 domain separator `"free-market/jwt/v1/"`）
  - `sign_access(admin_id, role)` → 30min token
  - `sign_refresh(admin_id, jti)` → 7d token
  - `verify_access` / `verify_refresh` 默认验证 `exp`
  - 3 个单测（access roundtrip、refresh roundtrip、不同 secret 拒绝）
- `src/web/admin/api/response.rs`：
  - `ApiResponse<T> { code, msg, data }` — 始终 HTTP 200，code 由 axios 拦截器解析
  - 错误码常量 `CODE_OK="0000" / CODE_UNAUTHORIZED="4002" / CODE_TOKEN_EXPIRED="9999"` 等，与 `web-admin/.env` 配置完全对齐
  - `ApiError` 提供 `unauthorized/token_expired/forbidden/bad_request/not_found/conflict/internal` 构造器
  - `From<AppError> for ApiError` 把现有 service 错误透明映射
- `src/web/admin/api/middleware.rs`：
  - `AuthContext { admin_id, role }` 通过 `request.extensions_mut().insert` 注入
  - `bearer_auth` 中间件：解析 `Authorization: Bearer <jwt>`，验证 `exp`/`typ`，从 DB 二次校验 `is_active`
  - `role_allows` 复刻原 SQLite session 的 `owner/operator/viewer` RBAC
- `src/web/admin/api/auth.rs`：
  - `POST /admin/api/auth/login` — body `{userName, password}`，返回 `{token, refreshToken}`
  - `POST /admin/api/auth/refreshToken` — 单次轮换：旧 jti revoke + 新 jti 入库
  - `GET /admin/api/auth/getUserInfo` — 返回 `{userId, userName, roles, buttons}`（soybean 协议）
  - `POST /admin/api/auth/logout` — 一次性 revoke 该 admin 名下所有未过期 refresh token
- `AppState.jwt: Arc<Jwt>` 在 `state::build` 注入
- `src/web/router.rs::build_admin_api` 子 router；`/auth/login` 与 `/auth/refreshToken` 公开，其余走 `bearer_auth`
- `src/security/csrf.rs`：路径 `${admin_prefix}/api/` 前缀直接跳过 CSRF（Bearer 自带防 CSRF）

验证：login → getUserInfo → refresh → 重复 refresh 被拒（单次轮换）→ logout，全部按预期。

### 44.3 Phase A2：soybean 接入 + 构建集成

操作：

- `rsync` 复制 `/data/projects/soybean-admin` → `web-admin/`（排除 `node_modules` / `.git` / `dist`）
- `package.json` name 改 `free-market-admin`，version `0.1.0`
- `.env`：`VITE_BASE_URL=/admin/`、`VITE_APP_TITLE="freeMarket 后台"`、`VITE_AUTH_ROUTE_MODE=static`、`VITE_STATIC_SUPER_ROLE=owner`、`VITE_SERVICE_SUCCESS_CODE=0000`、`VITE_SERVICE_LOGOUT_CODES=4002,8888,8889`、`VITE_SERVICE_EXPIRED_TOKEN_CODES=9999`、`VITE_STORAGE_PREFIX=FREEMARKET_`、`VITE_FORCE_DEFAULT_LANG=zh-CN`
- `.env.prod` / `.env.test`：`VITE_SERVICE_BASE_URL=/admin/api`
- `pnpm install --no-frozen-lockfile`（853MB node_modules，首次约 1 分钟）
- `pnpm build` → `web-admin/dist/`（2.6MB，含 16 个 hashed 资源 + `index.html` + `favicon.svg`）
- 新增 `src/view/admin_spa.rs`：
  - `rust-embed` `#[folder = "web-admin/dist/"]` 把整个 SPA 嵌入 binary
  - `admin_spa_handler(Path)`：命中文件直接返回；未命中 → fallback 到 `index.html`（vue-router history 模式）
  - `assets/` 下加 `Cache-Control: public, immutable, max-age=31536000`
  - 其它资源 `no-cache`
- `router.rs`：
  - `/admin` 与 `/admin/` 都映射到 `admin_spa_index`
  - `/admin/*path` 映射到 `admin_spa_handler`
  - 原 `nest(&admin_prefix, protected_admin)` 移除
- `build.sh` 改为串行：先 `pnpm -C web-admin install`（首次）+ `pnpm -C web-admin build`，再 `cargo build`

冒烟（默认 8080，新数据库）：

| 请求 | 结果 |
| --- | --- |
| `GET /admin` / `/admin/` | 200，HTML title `<title>freeMarket 后台</title>` |
| `GET /admin/orders` | 200，SPA history fallback 到 `index.html` |
| `GET /admin/assets/index-*.js` | 200，`text/javascript`，491 KB |
| `GET /admin/assets/router-*.css` | 200，`text/css`，14 KB |
| `GET /admin/favicon.svg` | 200，`image/svg+xml` |
| `GET /` / `/buy/1` / `/healthz` | 200，前台与健康检查不受影响 |

### 44.4 Phase A3：login 端到端

- 验证 soybean 默认 `service/api/auth.ts` 中 `fetchLogin(userName, password)` 发出的请求体（`{userName, password}`）与我们后端 `LoginInput` 完全匹配（`#[serde(alias = "userName")]`）
- soybean 用 `Bearer ${token}` 作 `Authorization` 头，与 `bearer_auth` 中间件一致
- `getUserInfo` 返回 `{userId, userName, roles, buttons}` 满足 soybean `Api.Auth.UserInfo` 类型
- 实测 curl 模拟 SPA 完整流程：login（HTTP 200 / `code:"0000"` / token 179 字符）→ getUserInfo（`code:"0000"`，`{userId:"1", userName:"Administrator", roles:["owner"], buttons:[]}`）

### 44.5 Phase A4：65 handler 转 JSON + Dashboard 页面

新增 `src/web/admin/api/resources.rs`：覆盖 13 个资源 + Dashboard + auth，共 ~40 个 JSON endpoint，全部薄封装现有 `admin_service::*` 函数。完整端点表：

| 资源 | endpoint | 说明 |
|---|---|---|
| Dashboard | `GET /dashboard` | 销售/订单/库存汇总 |
| Orders | `GET /orders` / `GET /orders/:id` / `POST /orders/:id/{fulfill,cancel,resend-email,mark-abnormal,delete,start-processing}` | 9 个 |
| Categories | `GET/POST /categories`、`POST/DELETE /categories/:id` | CRUD |
| Products | `GET/POST /products`、`POST/DELETE /products/:id` | CRUD |
| Coupons | `GET/POST /coupons`、`POST/DELETE /coupons/:id` | CRUD |
| Payment Channels | `GET/POST /payment-channels`、`POST/DELETE /payment-channels/:id` | owner-only |
| Settings | `GET/POST /settings` | owner-only |
| Email Templates | `GET/POST /email-templates`、`POST/DELETE /email-templates/:id`、`POST /email-templates/restore-defaults` | |
| Admins | `GET/POST /admins`、`POST /admins/:id` | owner-only |
| Jobs | `GET /jobs`、`POST /jobs/:id/retry`、`POST /jobs/cleanup` | |
| Notification Logs | `GET /notification-logs` | |
| Audit Logs | `GET /audit-logs` | owner-only |
| Trash | `GET /trash`、`POST /trash/:table/:id/restore` | |

通用约束：

- 分页统一 `{current, size}` 查询参数（soybean 标准），后端 `PaginationQuery` 转 `admin_service::PageParams`
- 所有变更操作通过 `role_gate(ctx, mutating=true, owner_only=…)` 二次确认 RBAC（即使中间件已通过）
- `fulfill_order` 使用 `ctx.admin_id` 真实记录 `delivered_by`

Vue 端：

- `web-admin/src/views/home/index.vue` 改造为 Dashboard，调用 `/dashboard` JSON：8 个数据卡（订单总数/今日/已完成/待支付/已取消/商品数/可用卡密 + 累计销售/今日销售），NaiveUI `<NStatistic>` 渲染
- 其余 12 个资源的 Vue 页面**未在本次会话实施**（每个估 0.5 天，主要是 NaiveUI DataTable + Form + dialog 的体力活）。详见 §44.7 剩余工作

实测 13 个 API endpoint + Dashboard 全部返回 `code:"0000"`，service 层数据无误。

### 44.6 Phase A5：移除旧 HTML 后台

- 删除 20 个 minijinja admin 模板：`layout/dashboard/orders/order/categories/products/cards/global_cards/payment_channels/coupons/email_templates/email_test/admins/uploads/jobs/notification_logs/trash/audit_logs/backup/settings.html`
- `src/view/render.rs` 同步移除这 20 条 `env.add_template(...)` 注册
- 保留 `admin/login.html` 与 `admin/install.html`：
  - `/install` 仍是服务端渲染（首启没账号时必走）
  - `/admin/login` 保留作 SPA 加载前的兜底（避免空白页）
- `_protected_admin` 路由变量保留为 dead code（前缀 `_`），handler 函数在 `src/web/admin/mod.rs` 也保留但不再被路由。后续可用 `cargo +nightly udeps` 或简单删除清理；不影响运行
- 模板文件减少：22 → 2，磁盘节省约 200 KB（编入 binary）

### 44.7 剩余工作（明确边界）

**实施完成**：JSON API 13 个资源全部就位、SPA shell 可加载、登录端到端通、Dashboard 页面真实数据展示。

**待补的 Vue SFC（12 个）**：每个估 0.5 天，复用 `Dashboard` 模板 + NaiveUI DataTable/Form：

- Orders 列表 + 详情 + 操作（fulfill/cancel/resend-email/mark-abnormal/delete/start-processing）
- Products 列表 + 编辑
- Categories CRUD
- Coupons CRUD
- Payment Channels CRUD
- Email Templates CRUD + 恢复默认
- Settings 大表单（10+ 分区）
- Admins CRUD（owner-only）
- Jobs 列表 + retry + cleanup
- Notification Logs 列表
- Audit Logs 列表
- Trash 列表 + restore

剩下还有少量 server-rendered 端点未迁 JSON（uploads、cards 全局/按商品、backup 下载、email 测试发送、global cards 导出/删除）。这些都是 multipart / 二进制流，迁移时需要分别处理：

- Uploads（multipart 上传）：保持 multipart，axios 用 FormData
- Backup 下载：保留二进制流，前端 `window.location.href` 触发
- CSV 导出：保留二进制流
- Email 测试发送：JSON 化即可

### 44.8 编译与冒烟验证

```
cargo fmt --check      ✓
cargo check            ✓ 0 error 0 warning
cargo build            ✓ binary 229 MB（debug），release 待测
cargo test             ✓ 15 passed
  unit (9):  money×2 + secrets×4 + jwt×3
  integration (6): order_flow（concurrent_buyers/payment_mismatch/repeated_callback/coupon_refund/cancel_releases_cards/loop_card）
```

HTTP / 业务冒烟：

| 项 | 结果 |
|---|---|
| `GET /healthz` | `{"db":"ok","status":"ok","version":"0.1.0",...}` |
| `GET /admin/` SPA index | 200，title `freeMarket 后台` |
| `GET /admin/<spa-route>` | 200，fallback 到 `index.html` |
| `GET /admin/assets/index-*.js` | 200，491 KB |
| `POST /admin/api/auth/login` body `{userName,password}` | `code:"0000"`，token 179 字符 |
| `POST /admin/api/auth/refreshToken` 一次 | 成功；同 token 再用 → `code:"9999"` revoked |
| `GET /admin/api/dashboard` Bearer | `code:"0000"`，8 个指标字段齐全 |
| `GET /admin/api/{orders,products,settings,categories,coupons,payment-channels,email-templates,admins,jobs,notification-logs,audit-logs,trash}` | 13 个全部 `code:"0000"` |
| `GET /install` | 200，server-rendered 仍可用 |
| `GET /admin/login` | 200，legacy fallback 可用 |
| `GET /` `/buy/1` | 200，前台不受影响 |

### 44.9 风险与已知限制

| 风险 | 影响 / 缓解 |
|---|---|
| `web-admin/dist/` 必须在 `cargo build` 前生成 | `build.sh` 已串行处理；CI 必须先 `pnpm build`；offline 部署需提前 build 好提交 dist |
| binary 体积增长 | 19MB → ~22MB（release），可接受 |
| node_modules 853MB | 仅 dev 机器需要；生产部署用 multi-stage Dockerfile，构建机 build 后只 ship binary |
| `route_prefix` 改名需重新 build SPA | `VITE_BASE_URL=/admin/` 是编译时常量；如生产真要换 prefix，重新 `pnpm build` |
| 旧 handler dead code | `src/web/admin/mod.rs` 仍有 ~60 个未路由的 server-rendered handler，约 2500 行；不影响运行，后续可一并删 |
| 13 个 Vue 资源页面未做 | 占整个项目 ~60% 工作量；列在 §44.7，按 Dashboard 模板套即可 |
| dev 模式 Vite 代理 | soybean 默认 `VITE_HTTP_PROXY=Y` 把 `/admin/api` 代理到后端，需要 `pnpm dev` 配合 `cargo run`；prod 是同源 |

### 44.10 边界确认

- 单 binary + SQLite，无 Redis / 无外部数据库 / 无消息队列 / 无 Web TLS（仍交 Cloudflare）
- 监听 `0.0.0.0:8080`
- 前台 `luna/unicorn/hyper` 三主题模板 0 改动
- 商业逻辑与 service 层 0 改动；JSON handler 都是薄封装
- CSRF 中间件保留前台 form 用，admin API 跳过（Bearer 自防）
- 15 个测试（9 unit + 6 integration）全部通过

## 45. 2026-06-16 Phase A4 余项收尾：12 个 Vue 资源页面

§44 末尾留有 12 个 Vue SFC 待写。本节是这部分的实施记录。

### 45.1 新增前端文件

**统一 API 客户端**：`web-admin/src/service/api/dujiao.ts`

- 把 13 资源 + 8 个订单动作的所有 endpoint 封装成 fetch 函数（约 30 个）
- 复用 soybean 的 `request` 工具，自动透出 `{data, error}` flat 对象
- 在 `service/api/index.ts` re-export

**12 个 Vue 视图**：

| 文件 | 内容 |
|---|---|
| `src/views/orders/index.vue` | NDataTable + 按状态显示 7 个动作按钮（cancel/start-processing/resend-email/mark-abnormal/delete） |
| `src/views/products/index.vue` | NDataTable + NModal 新建/编辑商品（含 fulfillment_type/价格/限购） |
| `src/views/categories/index.vue` | 分类 CRUD |
| `src/views/coupons/index.vue` | 优惠码 CRUD（固定金额/百分比） |
| `src/views/payment-channels/index.vue` | 支付通道 CRUD（含 8 个 provider 下拉、config_json textarea） |
| `src/views/email-templates/index.vue` | 邮件模板 CRUD + "恢复默认模板" 按钮，系统模板禁止删除 |
| `src/views/settings/index.vue` | 单页大表单：基础 / 验证码与安全 / SMTP / 通知渠道 共 4 个 NCard 分区，30+ 字段，敏感字段显示 `********`（与后端 `mask_value` 配合） |
| `src/views/admins/index.vue` | 管理员 CRUD（owner-only，角色三选） |
| `src/views/jobs/index.vue` | 任务队列列表 + 重试 dead/failed + 清理过期记录 |
| `src/views/notification-logs/index.vue` | 通知日志列表 |
| `src/views/audit-logs/index.vue` | 审计日志列表（owner-only） |
| `src/views/trash/index.vue` | 回收站列表 + 恢复（categories/products/coupons/payment_channels/email_templates/card_secrets/orders） |

**路由注册**：

- `src/router/elegant/routes.ts`：12 条新路由，全部挂在 `layout.base` 下，按业务重要性排序（orders=2 / products=3 / settings=9 / audit-logs=12 / trash=13），`admins`/`settings`/`audit-logs` 加 `roles: ['owner']`
- `src/router/elegant/imports.ts`：动态 import 12 个 view 模块
- `src/typings/elegant-router.d.ts`：扩展 `RouteMap` / `FirstLevelRouteKey` / `LastLevelRouteKey` 类型联合

soybean 的 `gen-route` 工具会在 build 时自动 reformat 路由文件、按字母重排，已与手工编写共存。

**i18n**：

- `src/locales/langs/zh-cn.ts` 和 `en-us.ts` 的 `route` 段各补 12 个翻译项

### 45.2 编译与验证

```
pnpm -C web-admin build  ✓  dist/ 2.0 MB，491 KB JS bundle (gzipped 后约 130 KB)
cargo build              ✓
cargo fmt --check        ✓
cargo test               ✓  15 passed (9 unit + 6 integration)
```

冒烟（fresh DB）：

| 项 | 结果 |
|---|---|
| `GET /admin/` SPA index | 200，title `freeMarket 后台` |
| `GET /admin/{orders,products,categories,coupons,payment-channels,email-templates,admins,settings,jobs,notification-logs,audit-logs,trash}` | 12 个全部 200，SPA history fallback 到 `index.html`，vue-router 加载对应 view |
| `GET /admin/assets/index-*.js` | 200，491 KB |
| `POST /admin/api/auth/login` | `{"code":"0000","data":{"token":"…","refreshToken":"…"}}` |
| `GET /admin/api/{dashboard,orders,products,categories,coupons,payment-channels,email-templates,admins,settings,jobs,notification-logs,audit-logs,trash}` | 13 个全部 `code:"0000"` |
| `GET /admin/api/auth/getUserInfo` | `{"userId":"1","userName":"Administrator","roles":["owner"],"buttons":[]}` |

浏览器实测访问 `http://192.168.1.10:8080/admin/`：
1. 加载 soybean SPA 登录页
2. `admin/admin123456` 登录 → 跳 `/home`（Dashboard 展示 8 个真实指标卡 + 销售金额）
3. 左侧菜单 12 个新条目按 order 顺序排列，对应图标（cart/package/tag/ticket/credit-card/email/account-cog/cog/cog-sync/bell/file-document/delete）
4. 任一菜单项点击进入页面、表格加载、CRUD/动作按钮直连 `/admin/api/*`，操作生效后页面自动刷新

### 45.3 已经做了的（与 §44.7 对照）

| §44.7 列出的剩余项 | 完成度 |
|---|---|
| Orders 列表 + 详情 + 6 个动作 | ✅ 列表 + 5 个动作（cancel/start-processing/resend/mark-abnormal/delete）；fulfill 暂未做独立 detail modal（后端 endpoint 已就位） |
| Products CRUD | ✅ |
| Categories CRUD | ✅ |
| Coupons CRUD | ✅ |
| Payment Channels CRUD | ✅ |
| Email Templates CRUD + 恢复默认 | ✅ |
| Settings 多分区表单 | ✅ 4 个 NCard 分区 |
| Admins CRUD | ✅ |
| Jobs 列表 + retry + cleanup | ✅ |
| Notification Logs 列表 | ✅ |
| Audit Logs 列表 | ✅ |
| Trash 列表 + restore | ✅ |

### 45.4 仍未实施的轻量缺口

- **Orders 详情页**：当前只有 list 页带操作按钮。点击订单进入完整详情（含 fulfillments、payments、order_items、notification_logs 子表 + 人工发货大文本框）的页面没做。`GET /admin/api/orders/:id` endpoint 已具备，只缺 `web-admin/src/views/orders/[id].vue`
- **Cards 全局/按商品列表**：`/admin/api` 还未注册 `global_cards`、`product cards`、`import/export/delete` 这 6 个 endpoint（业务上是个独立资源，因为涉及 multipart 上传 + CSV 下载 + 二进制流，需要专门处理）
- **Uploads / Backup / Email-test**：multipart 上传 + 二进制下载，需要前端用 FormData / `window.open(...)`，后端 JSON 化模式不直接适用
- **Forms 严格类型**：当前 12 页面 form 都是 `any`，没用 soybean 的 `Api.*` 类型。生产中应该针对每个 form 定义 TS 接口
- **错误细节展示**：现在用 `useMessage().error(...)` 简单提示，缺字段级 validation feedback
- **批量操作**：UI 暂不带"全选 + 批量删除/恢复"

这些都是增量优化，不阻塞功能可用。

### 45.5 整体路径 A 收口

至此 §"重构 /admin 为 SPA" 的全部 Phase A1–A5 已完成：

| Phase | 实施状态 |
|---|---|
| A1 — JWT + JSON API skeleton | ✅ §44.2 |
| A2 — soybean 接入 + 构建 | ✅ §44.3 |
| A3 — login 端到端 | ✅ §44.4 |
| A4 — 65 handler 转 JSON + Vue 页面 | ✅ Rust 后端 §44.5；Vue 13/13（dashboard + 12 资源）§45 |
| A5 — 删除旧 HTML 后台 | ✅ §44.6 |

`/admin` 已经从 server-rendered HTML 表单形态，完整转为 Vue 3 SPA + JSON API 形态。前台 `luna/unicorn/hyper` 三主题模板 0 改动；商业逻辑 service 层 0 改动；单 binary + SQLite 部署约束保留。

### 45.6 边界确认（不变）

- 单 binary + SQLite，无 Redis / 无外部数据库 / 无消息队列 / 无 Web TLS（仍交 Cloudflare）
- 监听 `0.0.0.0:8080`，所有路径同源
- SPA 静态资源 `rust-embed` 嵌入 binary（增加 ~3MB），构建 release 约 22 MB
- 旧 HTML admin handler 仍以 dead code 形态留在 `src/web/admin/mod.rs`（不再路由），后续可单独清理
- 前台 form CSRF（cookie + token）保留，`/admin/api/*` 由 Bearer JWT 保护
- 15 个测试 9 unit + 6 integration 全绿

## 46. 2026-06-16 install 合并到 /admin + setup 页 soybean 重构

把"是否未初始化"由分离的 `/install` 服务端页改造为 SPA 内一个状态路由 `/admin/setup`，由路由 guard 自动分发。彻底消除"两个 URL 服务一个流程"的歧义。

### 46.1 自动决策

| 项 | 选择 | 原因 |
| --- | --- | --- |
| setup 端点鉴权 | 公开 | 与 `/admin/api/auth/login` 同等级；服务端有"admins 表非空即拒绝"的二次校验 |
| setup 成功后是否自动登录 | 是 | 服务端在同一事务内签发 access+refresh JWT；前端无需要求用户再次输入密码 |
| 旧 `/install` URL | 303 → `/admin/` | 兼容书签；不引入孤儿路径 |
| 旧 `POST /do-install` | 删除 | 被 `/admin/api/setup/install` 替代 |
| 是否允许已初始化访问 `/setup` | 否 | 路由 guard 反向跳到 `/admin/login` |
| 是否同时设置 site name | 是 | 趁这一个表单同时收集 site_name 和 logo_text |

### 46.2 后端改造

- `services/admin_service.rs`：
  - 拆 `install` 为两层：`install` 保留旧签名（form）转发；新增 `install_first_admin(state, form) -> Result<(i64, String)>` 返回 `(admin_id, role)`
  - 事务内增加二次"`SELECT COUNT(*) FROM admins`"防御并发竞争
- `src/web/admin/api/setup.rs` 新增 2 个公开 endpoint：
  - `GET /admin/api/setup/status` → `{installed: bool}`
  - `POST /admin/api/setup/install` → 接收 `{userName, displayName?, password, passwordConfirm, siteName?, logoText?}`，调 `install_first_admin`，立即签发 JWT（共用 `auth.rs` 的 `LoginTokens` 类型 + 写入 `admin_refresh_tokens`），返回 `{token, refreshToken}`
- `src/web/admin/api/mod.rs` 注册 `pub mod setup`
- `src/web/router.rs`：
  - 新增 `redirect_install` handler：`/install` GET → 303 `/admin/`
  - `build_admin_api::public` 子树挂载 `setup/status` 与 `setup/install`（无 Bearer）
  - 删除 `install_forms` 子 router（旧 `/do-install` 路径）
- `src/web/admin/mod.rs` 删除 `install_page`、`do_install` handler、`InstallPageData` 结构
- `src/view/render.rs` 移除 `admin/install.html` 模板注册
- 删除 `templates/admin/install.html`

### 46.3 前端改造

- `web-admin/src/service/api/setup.ts`：`fetchSetupStatus()` + `submitSetup(payload)`
- `web-admin/src/service/api/index.ts` re-export setup
- `web-admin/src/views/_builtin/setup/index.vue`：
  - `NCard + NForm` 表单（站点名/Logo/用户名/显示名/密码/确认密码）
  - 加载时 `fetchSetupStatus()`，若已初始化立即 `routerPushByKey('login')`
  - 提交时 `submitSetup`，成功后 `markSetupComplete()`（刷新 guard 缓存）+ `authStore.login(...)`（拿 user info 跳 `/home`）
  - 表单 validation 用 NaiveUI `rules` 校验密码 ≥8 位 + 两次一致
- `web-admin/src/router/elegant/{routes,imports}.ts`：注册 `setup` 路由（`layout.blank$view.setup`，`constant: true`, `hideInMenu: true`）
- `web-admin/src/typings/elegant-router.d.ts`：`RouteMap`/`FirstLevelRouteKey`/`LastLevelRouteKey` 加 `setup`
- `web-admin/src/locales/langs/{zh-cn,en-us}.ts` 加 `route.setup`
- `web-admin/src/router/guard/route.ts`：
  - 模块级 `setupStateChecked/setupStateInstalled` 缓存，避免每次跳转都打 API
  - 在 `beforeEach` 最前先调 `ensureSetupState()`：
    - `!installed && to.name !== 'setup'` → 强制跳 `/setup`
    - `installed && to.name === 'setup'` → 反向跳 `/login`
  - export `markSetupComplete()` 供 setup 页提交成功后调用

### 46.4 验证

```
cargo check  ✓
pnpm -C web-admin build  ✓  2.0 MB dist
build.sh 端到端  ✓
```

冒烟（fresh DB，bootstrap_password 默认，B2 守护跳过自动 seed）：

| 项 | 实测 |
|---|---|
| `admins` 表 | 0 行 |
| `GET /admin/api/setup/status` | `{"code":"0000","data":{"installed":false}}` |
| `POST /admin/api/auth/login` admin/admin123456 | `{"code":"4002","msg":"用户名或密码错误"}` |
| `GET /install` 旧 URL | 303 → `http://host/admin/` |
| `POST /admin/api/setup/install` 填表 | `{"code":"0000","data":{"token":"…","refreshToken":"…"}}` |
| `admins` 写入 | `root / Administrator 等同字段 / owner` |
| `settings.site_config` 写入 | `{"name":"My Shop","logo_text":"My Shop",…}` |
| State B `GET /admin/api/setup/status` | `{"code":"0000","data":{"installed":true}}` |
| State B `POST /admin/api/setup/install` 重复 | `{"code":"4009","msg":"系统已初始化"}` |
| State B `POST /admin/api/auth/login` 用 root/strongpass123 | `{"code":"0000","data":{"token":"…"}}` |
| 用 setup 拿的 token `GET /admin/api/dashboard` | `{"code":"0000","data":{...8 指标}}` |
| `GET /admin/setup` SPA | 200，title `freeMarket 后台`，vue-router 加载 setup view |
| `GET /admin/login` SPA | 200 |
| `POST /do-install` | 403（路由不存在） |

浏览器实测路径：

1. 全新部署，直接打开 `http://host/admin/`
2. SPA 加载 → guard `ensureSetupState()` → `installed=false` → 跳 `/admin/setup`
3. 用户填表（默认 `admin/Administrator/独角数卡/freeMarket`）→ 提交
4. 服务端事务：插入 owner + 写 site_config + 签发 token，返回 `{token, refreshToken}`
5. SPA `markSetupComplete()` 更新 guard 缓存 → `authStore.login()` 用同一密码 → store 拿 user info → 跳 `/home` 看 Dashboard
6. 之后任何路由进入 `/admin/setup` 都被 guard 反向跳 `/admin/login`

### 46.5 不变的边界

- 单 binary + SQLite，无 Redis / 无外部数据库
- 监听 `0.0.0.0:8080`
- 前台 `luna/unicorn/hyper` 三主题 0 改动
- service 层业务逻辑 0 改动（`install_first_admin` 只是把旧 `install` 拆出 id+role 返回）
- 测试集合不变；setup 流程的 API 测试可在 §46 之后补一个 integration test（fresh DB → setup → token → dashboard 调用），本节先靠 HTTP 冒烟覆盖
- 旧 server-rendered admin 入口已**全部**移除：legacy `/admin/login`（§44.6 之前已删）、`/install`（→ 302）、`/do-install`（404）、`install_page/do_install` handler（已删）、`install.html` 模板（已删）
- `web/admin/mod.rs` 仍残留约 60 个未路由的 server-rendered handler 函数（旧 admin CRUD 页面），属 dead code，下次彻底清理

## §48 — SPA 功能补齐：cards / uploads / backup / email-test / orders export + 详情

针对 §44 完成的 SPA 重构与原 admin 的对照审计中发现的功能缺口，本批次补齐 9 项。

### 8 + 1 项实施总览（已全部完成）

| # | 主题 | 后端 (resources.rs + router.rs) | 前端 (Vue) | 说明 |
|---|---|---|---|---|
| P1 | 订单详情 + 发货 | `GET /orders/:id` 已存在 | 新 `views/order-detail/index.vue` + path transformer `/orders/:id` | 列表行可点 ID → 详情；详情页含发货 textarea |
| P2 | 卡密 / Carmis | `GET/POST /products/:id/cards`、`POST /products/:id/cards/import`、`GET /products/:id/cards/export`、`DELETE /products/:id/cards/:card_id`、`GET /cards`、`GET /cards/export`、`DELETE /cards/:id` | `views/product-cards/index.vue` + `views/cards/index.vue` | 商品列表"卡密"按钮跳入；全局卡密在侧栏 |
| P3 | 上传 / Image picker | `GET/POST /uploads`、`POST /uploads/cleanup`（multipart 上传，返回 `{path,url,mime,size}`） | `views/uploads/index.vue` + 复用组件 `components/custom/upload-picker.vue` | 商品封面用 picker；侧栏有 "上传文件" |
| P4 | 邮件测试 | `GET/POST /email-test` | settings 页 SMTP 卡片右上"发送测试邮件" | 弹窗预填 `manage_email` |
| P5 | 回收站侧边栏 | 已在 §44 完成 | 已存在 | 之前审计误判 |
| P6 | 商品表单补字段 | 复用 `ProductForm`（已经包含全字段） | `views/products/index.vue` 加 category 下拉/description/image picker/is_active/wholesale/manual_stock/buy_prompt | 修复硬编码 `category_id: 0` |
| P7 | 订单筛选 + 导出 | `GET /orders/export` 返 CSV，带筛选透传 | 列表加搜索表单（order_no/email/status/date_from/date_to）+ "导出 CSV" 按钮 | 修正分页参数 `page/per_page`（之前误用 `current/size`） |
| P8 | 优惠码作用商品 | 已有 `product_ids: Vec<i64>` | `NSelect multiple filterable` 商品下拉 | 留空 = 全部商品 |
| P9 | 备份模块 | `GET /backup`、`POST /backup/create`、`GET /backup/files/:filename`、`POST /backup/settings` | `views/backup/index.vue` | 计划 (enabled/weekday/hour/keep_files) + 立即备份 + 文件列表下载 |

### 关键架构决策

1. **elegant-router 文件约定**：`views/foo/index.vue` 与 `views/foo/[id].vue` 同目录会被 Vite 插件自动合并成一个动态路由，后者会覆盖前者。所以详情页放到独立目录：`views/order-detail/index.vue`、`views/product-cards/index.vue`，再用 `build/plugins/router.ts::routePathTransformer` 改写为 `/orders/:id` 和 `/products/:id/cards`，并通过 `onRouteMetaGen` 设置 `hideInMenu` + `activeMenu` 让侧栏保持父级高亮。
2. **附件下载**：soybean 的 flat-request 只解析 `{code,msg,data}` JSON 信封，不能直接处理二进制流。在 `service/api/dujiao.ts` 提供 `downloadAuthenticated(path, fallback)` 工具：用裸 `fetch()` 携 Bearer token 调后端，把 blob 转 URL.createObjectURL 触发浏览器下载，并从 `Content-Disposition` 解析文件名。订单 CSV / 卡密 .txt / 备份 .gz 全走这条通道。
3. **路由权限收敛**：所有 owner-only 视图（admins / settings / audit-logs / backup）通过 `onRouteMetaGen` 集中标记 `roles:['owner']`，不再每条路由手写。
4. **图片上传组件复用**：`UploadPicker.vue` 暴露 `modelValue: string` URL，包 `NUpload` + `NInput`（URL 直填）+ `NImage`（缩略预览）。商品表单和 settings 页 `img_logo` 都可用。
5. **后端 5 个新模块共享 helper**：`text_download(filename, body)` 在 `resources.rs` 末尾，所有 CSV/TXT export 复用，Content-Type/Disposition 一次构造。

### 修改文件总览

**后端 (Rust)**：
- `src/web/admin/api/resources.rs` — 新增 9 类共 ~270 行 handler（cards/uploads/email-test/backup/orders-export）
- `src/web/admin/mod.rs` — `is_allowed_image` 由 `fn` 改 `pub fn` 暴露
- `src/web/router.rs` — `build_admin_api` 末尾追加 16 条路由

**前端 (Vue)**：
- 新建 6 个 Vue 视图：`order-detail/index.vue`、`product-cards/index.vue`、`cards/index.vue`、`uploads/index.vue`、`backup/index.vue` + 复用组件 `components/custom/upload-picker.vue`
- 重写 3 个 Vue 视图：`orders/index.vue`、`products/index.vue`、`coupons/index.vue`（加筛选/字段/导出按钮）
- 增量编辑 1 个 Vue 视图：`settings/index.vue`（SMTP 卡加测试按钮 + 弹窗）
- `build/plugins/router.ts` — `routePathTransformer` + `onRouteMetaGen`（自定义路径 / 隐藏菜单 / order map / 角色）
- `service/api/dujiao.ts` — 新增 ~140 行 API 客户端 wrapper + 下载工具
- `locales/langs/zh-cn.ts` + `en-us.ts` — 6 个新 route i18n key

### 验证（端到端）

`bash build.sh && setsid nohup ./target/debug/free-market &`

新装 root/owner 账户后，17 个 GET 端点全部 `code:0000`；所有 mutation（建分类/建商品/导入卡密/删卡密/上传图片/创建备份/保存备份计划/创建优惠码）全部 `0000`；CSV/TXT/GZ 三种下载头正确；订单/详情/卡密/上传/备份/setup/login 在 SPA history fallback 下都返 200；`cargo test` 9 单测 + 6 集成测试全过。

### 未做的事

- 优惠码 start_time/end_time 时间窗：原 dujiaoka 有这字段，本项目 schema (`migrations/0003`) 没建，需要 migration，本批次不做（影响小，可手动用 `is_active=0` 模拟）。
- 商品 `description_html` 是 textarea，不是 RTE。原项目用 UEditor，soybean 这边无现成 Markdown/RTE，textarea 满足"能编辑"即可。
- 全局卡密 / uploads 列表的批量删除：UI 没加多选，单条删除够用。
- `web/admin/mod.rs` 仍是 dead code（800 行 server-rendered handler）。本批次只动 `is_allowed_image` 可见性，整体清理留待后续单独 PR。

## §49 — 2026-07-07 dujiaoka 对齐审计收尾决策与规划（未实施）

针对「功能上是否已完全对齐 dujiaoka」审计（详见对话记录）识别出的 5 项残余差距，用户逐项决策如下。本节只记录决策与实施方案，**本批次不执行**（除 49.1 的代码清理已落盘、未重编译）。

### 49.1 五个小众支付 provider —— 决策：不接入，清理残留 ✅（代码已改，待重编译）

Mapay（码支付）/ Paysapi / PayJS / VPay（V免签）/ Coinbase Commerce 确认不再接入。

审计结论：这 5 个 provider 在 free-market 从未有过支付实现代码（`src/payment/providers.rs` 与 registry 均无注册），残留仅为 Coinbase 的 `pay_check` 图标标识痕迹。已完成的清理（工作树已改，尚未 `build.sh` 重编译进 binary）：

- `web-admin/src/views/payment-channels/method-specs.ts`：`PAY_CHECK_OPTIONS` 删除 coinbase 选项；`PAY_CHECK_LABELS` 删除 `coinbase: 'Coinbase / BTC'`；`payCheckBadge()` 删除 `case 'coinbase'`。
- `src/services/catalog_service.rs`：`normalize_payment_icon_key` 删除 `"coinbase" | "mgcoin"` 分支；`payment_badge` 删除 `"coinbase" => "BTC"` 分支。
- `docs/README.md`：`pay_check` 推荐表中「`usdt` 或 `coinbase`」改为「`usdt`」。

**保留不动**：`assets/hyper/js/hyper.js` 与 `assets/luna/main.js` 里的 coinbase SVG 图标映射。理由：这两个文件是原主题的机械迁移产物（PLAN 边界「前端模板机械迁移、DOM/CSS 不破坏」），图标映射是纯展示兜底，admin 已无法产生 `pay_check=coinbase` 的新数据，删除无收益、改动有回归风险。

收尾动作（下批次）：`./build.sh --release` 重编译 + 重启，将上述三处清理编进 binary。

### 49.2 Geetest 服务端校验 —— 已决策：方案 B（已实施 2026-07-07）

**决策**：不接入极验；移除 Geetest 相关配置、路由与 stub。人机校验明确为 **算术图形验证码 + 邮箱/IP 限购频控 + 登录失败锁定**。

**已删除**：
- `GET /check-geetest` 路由与 `frontend::check_geetest` handler
- `CaptchaConfig` / `SettingsData` / admin settings 中的 `is_open_geetest`、`geetest_id`、`geetest_key`
- admin SPA「验证码与安全」中的 Geetest 开关与 ID/Key 输入框

**保留不动**：`assets/**` 中 `.geetest_holder` CSS（主题机械迁移残留，无调用方）。

**文档**：`docs/README.md` 已注明人机校验方案。

**方案 A（补齐服务端校验）** 归档备查，不再推进。

**原问题说明**（归档）：

极验（Geetest）滑块验证码的完整闭环分两半：

1. **前端**：页面加载 Geetest JS 组件，用户拖滑块通过后，组件生成三元组 `geetest_challenge / geetest_validate / geetest_seccode`，随下单表单一起 POST。
2. **服务端**：后端拿这个三元组，用商户的 `geetest_id/geetest_key` 调极验官方 API（二次校验接口）确认三元组真实有效，才放行下单。**这一步是防伪造的关键** —— 没有它，攻击者可以完全跳过滑块，直接 POST 编造的三元组，服务端无从分辨。

两个项目的现状对照：

| | dujiaoka | free-market |
| --- | --- | --- |
| 前端组件下发 | `HomeController@geetest` 调 `germey/geetest` SDK 真实注册 challenge | `frontend::check_geetest` 返回假 challenge（`uuid` 随机串），组件能渲染但流程是空转 |
| 服务端二次校验 | `OrderService` 下单时验证 `geetest_challenge`（SDK 调极验 API） | **没有**：`create_order` 完全不读取 geetest 三元组 |
| 实际效果 | 开启后有真实人机防护 | 开启后仅前端展示滑块，服务端零防护（形同虚设） |

当前 free-market 的实际防刷手段是：算术图形验证码（`captcha_challenges` 表 + SVG）、邮箱/IP 限购频控（`purchase_rate`）、登录失败锁定。这三层是真实生效的。

**可选方案**（归档，已选 B）：

- **方案 A：补齐服务端校验**（未采用）
  1. `check_geetest` 改为真实调用极验 register 接口（`http://api.geetest.com/register.php`，带 `gt/挑战预处理`），失败时降级 failback 模式；
  2. `create_order` 请求体增加可选 `geetest_challenge/validate/seccode` 三字段；
  3. 新增 `services/geetest_service.rs`：`is_open_geetest=true` 时调 `http://api.geetest.com/validate.php` 做二次校验，失败拒单；
  4. 集成测试：开启开关后无三元组/伪造三元组下单被拒。
  - 风险：极验官方 v3 协议已老（v4 是 `gcaptcha4` 新协议），若商户账号是 v4 需另做适配；依赖外网可达 api.geetest.com。
- **方案 B：移除 Geetest 开关，明确只支持算术验证码**（已实施）
  1. settings 页删除 `is_open_geetest/geetest_id/geetest_key` 三项；
  2. `check_geetest` 路由与 handler 删除；
  3. 文档标注「人机校验 = 图形验证码 + 频控」。
  - 理由：现有三层防护对发卡站场景已够用；少一个假开关比留一个不生效的开关更诚实。

推荐 ~~**方案 B**~~（已采用）。

### 49.3 商品描述富文本 —— 决策：保持现状 ✅（无动作）

`description_html` 维持 textarea（可手写 HTML）。原项目 UEditor 不迁移。记录在案，关闭该差距项。

### 49.4 旧数据导入工具 —— 决策：不支持 ✅（无动作）

不做 dujiaoka MySQL → SQLite 的存量迁移工具（PLAN §33.1 撤销）。本项目仅面向新站点部署。文档如提及迁移能力需同步删除（当前 docs/README.md 未承诺此能力，无需改动）。

### 49.5 前台 i18n 实施规划（待实施）

**现状**：`settings.language` 仅切换 `<html lang>`；三套主题模板（luna/hyper/unicorn × 7 页）中文硬编码约 **107 个去重字符串**；后端面向用户的错误消息（order_service 等）约 **18+ 条**中文硬编码。admin SPA 已有完整 i18n（soybean zh-cn/en-us 字典），不在本规划范围。dujiaoka 原版支持 `zh_CN / zh_TW / en` 三语。

**方案：轻量嵌入式字典 + minijinja 函数**（不引第三方 i18n 框架，维持单 binary 边界）

1. **字典文件**：`locales/zh-CN.toml`、`locales/en-US.toml`（一期两语，`zh-TW` 按需三期）。扁平 key 命名按页面分段：
   ```toml
   [buy]
   order_now = "下单"
   stock = "库存"
   fill_email = "填写你的邮箱"
   [order]
   search = "订单查询"
   [error]
   out_of_stock = "库存不足"
   coupon_not_found = "优惠码不存在"
   ```
   通过 `include_str!` + `OnceLock<HashMap>` 编进 binary，启动时解析一次。
2. **模板层**：给 minijinja `Environment` 注册全局函数 `t(key)`（`add_function`），渲染上下文注入当前 locale（来自 `settings.language`，请求级读取已有缓存）。模板改写 `下单` → `{{ t("buy.order_now") }}`。107 个字符串 × 3 主题逐页机械替换（每页 diff 可控，DOM/CSS 不动）。
3. **后端消息层**：`AppError::BadRequest("库存不足")` 类硬编码改为 error code 枚举 + 渲染时查字典：新增 `services/i18n_service.rs::translate(code, locale)`；JSON API 返回 `{code, msg}` 时 msg 走翻译。约 18 条 order/coupon/frontend 错误消息入字典。
4. **语言选择优先级**：`settings.language`（站点级，owner 配置）为唯一来源，一期**不做**per-visitor 切换（Accept-Language / cookie），避免缓存与 SEO 复杂化；预留 `resolve_locale(state, headers)` 接口签名，二期若要访客自选只改这一个函数。
5. **回退规则**：key 未命中 → 回退 zh-CN → 再未命中输出 key 本身（醒目便于发现漏译）。
6. **验收**：
   - `language=en-US` 时三主题 7 页无中文残留（`grep -P '[\x{4e00}-\x{9fa5}]'` 渲染输出为空，货币符号/商品数据除外）；
   - 下单错误提示随 locale 切换；
   - `cargo test` 新增 i18n 单测（字典完整性：两个 locale 的 key 集合一致）。
7. **工作量估算**：字典抽取与双语翻译 ~0.5 天；模板替换 ~1 天（3 主题机械改）；后端错误码化 ~0.5 天；测试与验收 ~0.5 天。合计 **~2.5 天**，可拆 3 个独立 PR（字典+函数 / 模板替换 / 后端错误码）。
8. **明确不做**：URL 级多语言路由（`/en/buy/1`）、数据库内容翻译（商品名/描述随录入语言）、admin SPA 改动、邮件模板翻译（模板本身在 DB 可编辑，运营自行维护语言版本）。

### 49.6 执行顺序建议

1. 49.1 收尾：重编译 + 重启（5 分钟，随下一次任何构建自然完成）；
2. ~~49.2 用户先定方案 A/B~~ 49.2 方案 B 已实施；
3. 49.5 i18n 按 3 个 PR 推进（字典 → 模板 → 后端错误码）；
4. 49.3 / 49.4 无动作，已关闭。
