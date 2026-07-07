import type { SelectBaseOption } from 'naive-ui/es/select/src/interface';

// Payment-method specs — single source of truth for the channel form.
//
// Each entry maps a user-facing payment method to:
//   - backend provider_type (what gets saved to db.provider_type)
//   - channel_type widget (locked / select / free)
//   - the ordered list of config fields that compose config_json
//
// When provider code in `src/payment/providers.rs` gains/removes a field,
// update the corresponding entry here. Phase-3 backend validate endpoint
// (`POST /admin/api/payment-channels/validate`) catches mismatches at save time.

export type FieldGroup = 'basic' | 'merchant' | 'advanced';

export type FieldType =
  | 'text'
  | 'password'
  | 'textarea'
  | 'pem'
  | 'select'
  | 'multiselect'
  | 'switch';

export type SelectOption = SelectBaseOption<string, string>;

export interface FieldSpec {
  /** JSON key inside config_json */
  key: string;
  label: string;
  type: FieldType;
  required?: boolean;
  /** Initial value used when starting create */
  default?: string | string[] | boolean;
  /** select / multiselect options */
  options?: SelectOption[];
  placeholder?: string;
  help?: string;
  group?: FieldGroup;
  /** conditional visibility based on current field values */
  showIf?: (form: Record<string, any>) => boolean;
}

export type ChannelTypeWidget =
  | { kind: 'locked'; value: string; label?: string }
  | { kind: 'select'; options: SelectOption[]; default?: string }
  | { kind: 'free'; placeholder?: string; default?: string };

export interface MethodSpec {
  /** Unique UI key — does not hit the wire */
  id: string;
  label: string;
  /** Backend provider_type column value */
  providerType: string;
  /** How the channel_type field is rendered */
  channelType: ChannelTypeWidget;
  /** Default value for the pay_check legacy alias column */
  payCheckDefault?: string;
  /** Default interaction_mode (redirect / qr) */
  interactionDefault?: string;
  /** Anchor in /admin/docs */
  docsAnchor: string;
  /** Tagline shown under the method dropdown */
  hint?: string;
  fields: FieldSpec[];
}

const COMMON_GATEWAY_PLACEHOLDER = 'https://example.com';

export const PAY_CHECK_OPTIONS: SelectOption[] = [
  { label: '支付宝 alipay', value: 'alipay' },
  { label: '微信支付 wxpay', value: 'wxpay' },
  { label: 'QQ 钱包 qqpay', value: 'qqpay' },
  { label: 'USDT usdt', value: 'usdt' },
  { label: 'USDC usdc', value: 'usdc' },
  { label: 'PayPal paypal', value: 'paypal' },
  { label: 'Stripe stripe', value: 'stripe' },
  { label: 'Epusdt epusdt', value: 'epusdt' },
  { label: 'BEpusdt bepusdt', value: 'bepusdt' },
  { label: '其他 other', value: 'other' }
];

const PAY_CHECK_LABELS: Record<string, string> = {
  alipay: '支付宝',
  wxpay: '微信支付',
  qqpay: 'QQ 钱包',
  usdt: '虚拟币 / USDT',
  usdc: '虚拟币 / USDC',
  paypal: 'PayPal',
  stripe: 'Stripe',
  epusdt: 'Epusdt',
  bepusdt: 'BEpusdt',
  other: '其他支付'
};

const CHANNEL_TYPE_LABELS: Record<string, string> = {
  test: '模拟测试',
  alipay: '支付宝',
  wxpay: '微信支付',
  wechat: '微信支付',
  qqpay: 'QQ 钱包',
  usdt: 'USDT',
  trx: 'TRX',
  stripe: 'Stripe',
  paypal: 'PayPal',
  'bsc-usdt': 'BNB Smart Chain USDT',
  'bsc-usdc': 'BNB Smart Chain USDC',
  'base-usdc': 'Base USDC',
  'polygon-usdt': 'Polygon USDT',
  'polygon-usdc': 'Polygon USDC',
  'arbitrum-usdc': 'Arbitrum USDC',
  'optimism-usdc': 'OP Mainnet USDC',
  'eth-sepolia-usdc': 'Ethereum Sepolia USDC',
  'base-sepolia-usdc': 'Base Sepolia USDC',
  'polygon-amoy-usdc': 'Polygon Amoy USDC',
  'arbitrum-sepolia-usdc': 'Arbitrum Sepolia USDC',
  'optimism-sepolia-usdc': 'OP Sepolia USDC',
  'bnb-testnet-erc20': 'BNB Testnet 自定义 ERC20',
  'evm-erc20': '自定义 EVM ERC20'
};

export function payCheckDisplay(value?: string) {
  const raw = (value || '').trim().toLowerCase();
  if (!raw) return '-';
  return `${PAY_CHECK_LABELS[raw] ?? '自定义图标'} (${raw})`;
}

export function payCheckBadge(value?: string) {
  const raw = (value || '').trim().toLowerCase();
  switch (raw) {
    case 'alipay':
      return 'Ali';
    case 'wxpay':
    case 'wechat':
      return 'Wx';
    case 'qqpay':
      return 'QQ';
    case 'paypal':
      return 'PP';
    case 'stripe':
      return 'S';
    case 'tokenpay':
      return 'TP';
    case 'epusdt':
      return 'EUS';
    case 'bepusdt':
      return 'BUS';
    case 'dujiaopay':
      return 'DJP';
    case 'okpay':
      return 'OK';
    case 'usdt':
      return 'USDT';
    case 'usdc':
      return 'USDC';
    case 'trx':
      return 'TRX';
    default:
      return 'Pay';
  }
}

export function channelTypeDisplay(providerType?: string, channelType?: string) {
  const provider = (providerType || '').trim().toLowerCase();
  const raw = (channelType || '').trim().toLowerCase();
  if (!raw) return '-';
  const fromSpec = findMethodByRow(provider, raw)?.channelType;
  if (fromSpec?.kind === 'select') {
    const option = fromSpec.options.find(item => item.value === raw);
    if (option?.label) return `${option.label} (${raw})`;
  }
  if (fromSpec?.kind === 'locked' && fromSpec.label) {
    return `${fromSpec.label.replace(/\s*\(固定\)\s*$/, '')} (${raw})`;
  }
  return `${CHANNEL_TYPE_LABELS[raw] ?? '自定义渠道'} (${raw})`;
}

export const METHOD_SPECS: MethodSpec[] = [
  {
    id: 'noop',
    label: '模拟支付 (Noop)',
    providerType: 'noop',
    channelType: { kind: 'locked', value: 'test', label: '模拟测试' },
    payCheckDefault: 'other',
    interactionDefault: 'redirect',
    docsAnchor: '模拟支付 Noop',
    hint: '本地联调用，不接入真实支付平台。生产请禁用。',
    fields: []
  },
  {
    id: 'epay',
    label: '易支付 / 彩虹易支付 (Epay)',
    providerType: 'epay',
    channelType: {
      kind: 'select',
      default: 'alipay',
      options: [
        { label: '支付宝', value: 'alipay' },
        { label: '微信支付', value: 'wxpay' },
        { label: 'QQ 钱包', value: 'qqpay' }
      ]
    },
    interactionDefault: 'redirect',
    docsAnchor: 'Epay / Yipay 通道',
    hint: '兼容易支付协议（彩虹/码支付/各类二开）。',
    fields: [
      {
        key: 'epay_version',
        label: '协议版本',
        type: 'select',
        default: 'v1',
        group: 'basic',
        options: [
          { label: 'v1 (MD5 签名)', value: 'v1' },
          { label: 'v2 (RSA 签名)', value: 'v2' }
        ]
      },
      {
        key: 'gateway_url',
        label: '网关地址',
        type: 'text',
        required: true,
        group: 'basic',
        placeholder: 'https://pay.example.com',
        help: '易支付站点根地址，不要带 /submit.php'
      },
      {
        key: 'pid',
        label: '商户 ID (PID)',
        type: 'text',
        required: true,
        group: 'merchant',
        placeholder: '10001'
      },
      {
        key: 'merchant_key',
        label: '商户密钥 (KEY)',
        type: 'password',
        required: true,
        group: 'merchant',
        help: 'v1 用作 MD5 签名；v2 作为回调验签兜底',
        placeholder: 'xxxx-xxxx-xxxx'
      },
      {
        key: 'private_key',
        label: '商户私钥 (RSA)',
        type: 'pem',
        group: 'merchant',
        required: true,
        help: 'PKCS8 或 PKCS1 PEM 格式',
        showIf: form => form.epay_version === 'v2'
      },
      {
        key: 'submit_path',
        label: '创建订单路径',
        type: 'text',
        group: 'advanced',
        placeholder: 'v1=/submit.php，v2=/api/pay/submit'
      },
      {
        key: 'device',
        label: '设备类型 (v1)',
        type: 'select',
        group: 'advanced',
        default: 'pc',
        options: [
          { label: 'PC', value: 'pc' },
          { label: 'Mobile', value: 'mobile' }
        ],
        showIf: form => form.epay_version !== 'v2'
      },
      {
        key: 'method',
        label: '提交方式 (v2)',
        type: 'text',
        group: 'advanced',
        placeholder: 'web',
        showIf: form => form.epay_version === 'v2'
      }
    ]
  },
  {
    id: 'yipay',
    label: '码支付 (Yipay)',
    providerType: 'yipay',
    channelType: {
      kind: 'select',
      default: 'alipay',
      options: [
        { label: '支付宝', value: 'alipay' },
        { label: '微信支付', value: 'wxpay' },
        { label: 'QQ 钱包', value: 'qqpay' }
      ]
    },
    interactionDefault: 'redirect',
    docsAnchor: 'Epay / Yipay 通道',
    hint: '与 Epay 共用同一适配器；该项目历史命名为 yipay。',
    fields: [
      {
        key: 'gateway_url',
        label: '网关地址',
        type: 'text',
        required: true,
        group: 'basic',
        placeholder: 'https://pay.example.com'
      },
      {
        key: 'pid',
        label: '商户 ID (PID)',
        type: 'text',
        required: true,
        group: 'merchant',
        placeholder: '10001'
      },
      {
        key: 'merchant_key',
        label: '商户密钥 (KEY)',
        type: 'password',
        required: true,
        group: 'merchant'
      }
    ]
  },
  {
    id: 'tokenpay',
    label: 'TokenPay',
    providerType: 'tokenpay',
    channelType: {
      kind: 'free',
      default: 'usdt',
      placeholder: 'usdt / trx / 平台约定值'
    },
    payCheckDefault: 'usdt',
    interactionDefault: 'redirect',
    docsAnchor: 'TokenPay 通道',
    hint: 'TokenPay 系网关，常用于 USDT / TRX 收款。',
    fields: [
      {
        key: 'gateway_url',
        label: '网关地址',
        type: 'text',
        required: true,
        group: 'basic',
        placeholder: 'https://tokenpay.example.com'
      },
      {
        key: 'notify_secret',
        label: '回调验签密钥 (Token)',
        type: 'password',
        required: true,
        group: 'merchant'
      },
      {
        key: 'currency',
        label: 'TokenPay 币种',
        type: 'text',
        default: 'USDT',
        group: 'advanced',
        placeholder: 'USDT'
      },
      {
        key: 'create_path',
        label: '创建订单路径',
        type: 'text',
        default: '/CreateOrder',
        group: 'advanced'
      },
      {
        key: 'base_currency',
        label: '回调记账法币',
        type: 'text',
        default: 'CNY',
        group: 'advanced'
      }
    ]
  },
  {
    id: 'epusdt',
    label: 'Epusdt (GMPay)',
    providerType: 'epusdt',
    channelType: {
      kind: 'locked',
      value: 'usdt',
      label: 'usdt (固定)'
    },
    payCheckDefault: 'usdt',
    interactionDefault: 'qrcode',
    docsAnchor: 'Epusdt 通道',
    hint: 'Epusdt 原版及 GMPay 兼容协议。',
    fields: [
      {
        key: 'gateway_url',
        label: '网关地址',
        type: 'text',
        required: true,
        group: 'basic',
        placeholder: 'https://epusdt.example.com'
      },
      {
        key: 'secret_key',
        label: '验签密钥',
        type: 'password',
        required: true,
        group: 'merchant'
      },
      {
        key: 'pid',
        label: '合作伙伴 ID (可选)',
        type: 'text',
        group: 'merchant',
        help: '仅部分版本需要'
      },
      {
        key: 'token',
        label: '币种 (Token)',
        type: 'text',
        default: 'usdt',
        group: 'advanced',
        help: '上游字段名 token，币种枚举'
      },
      {
        key: 'network',
        label: '链网络',
        type: 'select',
        default: 'trc20',
        group: 'advanced',
        options: [
          { label: 'TRC20 (TRON)', value: 'trc20' },
          { label: 'ERC20 (Ethereum)', value: 'erc20' },
          { label: 'BEP20 (BSC)', value: 'bep20' },
          { label: 'Polygon', value: 'polygon' }
        ]
      },
      {
        key: 'currency',
        label: '法币',
        type: 'text',
        default: 'cny',
        group: 'advanced'
      },
      {
        key: 'create_path',
        label: '创建订单路径',
        type: 'text',
        default: '/payments/gmpay/v1/order/create-transaction',
        group: 'advanced'
      },
      {
        key: 'pay_url_template',
        label: '跳转模板 URL (旧版兼容)',
        type: 'text',
        group: 'advanced',
        placeholder: 'https://epusdt.example.com/pay/{payment_no}?amount={amount}',
        help: '填了则走模板跳转模式，不走 API'
      }
    ]
  },
  {
    id: 'evm-local',
    label: '基于 Alchemy ERC20 Token',
    providerType: 'evm-local',
    channelType: {
      kind: 'select',
      default: 'bsc-usdt',
      options: [
        { label: 'BSC USDT', value: 'bsc-usdt' },
        { label: 'BSC USDC', value: 'bsc-usdc' },
        { label: 'Base USDC', value: 'base-usdc' },
        { label: 'Polygon USDT', value: 'polygon-usdt' },
        { label: 'Polygon USDC', value: 'polygon-usdc' },
        { label: 'Arbitrum USDC', value: 'arbitrum-usdc' },
        { label: 'Optimism USDC', value: 'optimism-usdc' },
        { label: 'Ethereum Sepolia USDC (测试网)', value: 'eth-sepolia-usdc' },
        { label: 'Base Sepolia USDC (测试网)', value: 'base-sepolia-usdc' },
        { label: 'Polygon Amoy USDC (测试网)', value: 'polygon-amoy-usdc' },
        { label: 'Arbitrum Sepolia USDC (测试网)', value: 'arbitrum-sepolia-usdc' },
        { label: 'OP Sepolia USDC (测试网)', value: 'optimism-sepolia-usdc' },
        { label: 'BNB Testnet 自定义 ERC20 (测试网)', value: 'bnb-testnet-erc20' },
        { label: '自定义 EVM ERC20', value: 'evm-erc20' }
      ]
    },
    payCheckDefault: 'usdt',
    interactionDefault: 'qrcode',
    docsAnchor: 'EVM 本地收款',
    hint: '使用 Alchemy JSON-RPC 监听 ERC20 入账，资金直接进入自有地址。',
    fields: [
      {
        key: 'network_env',
        label: '网络环境',
        type: 'select',
        required: true,
        default: 'mainnet',
        group: 'basic',
        options: [
          { label: '主网 (mainnet)', value: 'mainnet' },
          { label: '测试网 (testnet)', value: 'testnet' }
        ],
        help: '测试网 Token 没有真实价值，前台支付页会显示测试网标识。'
      },
      {
        key: 'evm_chain_preset',
        label: '链预设',
        type: 'select',
        default: 'bsc-mainnet',
        group: 'basic',
        options: [],
        help: '选择后自动填充 chain_id、Alchemy Network、链名称和区块浏览器。'
      },
      {
        key: 'evm_token_preset',
        label: 'Token 预设',
        type: 'select',
        default: 'USDT:0x55d398326f99059ff775485246999027b3197955',
        group: 'basic',
        options: [],
        help: '测试网只内置 Circle 官方 USDC；USDT 测试币请选自定义 ERC20 后填写合约。'
      },
      {
        key: 'alchemy_api_key',
        label: 'Alchemy API Key',
        type: 'password',
        group: 'merchant',
        help: '用于生成 https://{network}.g.alchemy.com/v2/{key}；也可在高级配置里直接填写 rpc_url。'
      },
      {
        key: 'addresses',
        label: '收款地址池',
        type: 'textarea',
        required: true,
        group: 'basic',
        placeholder: '0x...\n0x...',
        help: '一行一个 EVM 地址；首期仅监听收款，不保存私钥、不自动归集。'
      },
      {
        key: 'fiat_per_token',
        label: '单 Token 法币价格',
        type: 'text',
        required: true,
        default: '1',
        group: 'basic',
        help: '订单金额除以此值得到链上应付数量。例如 CNY 订单可填 7.25。'
      },
      {
        key: 'chain_id',
        label: '链 ID',
        type: 'text',
        required: true,
        default: '56',
        group: 'advanced',
        placeholder: '56'
      },
      {
        key: 'alchemy_network',
        label: 'Alchemy Network',
        type: 'text',
        required: true,
        default: 'bnb-mainnet',
        group: 'advanced',
        placeholder: 'bnb-mainnet'
      },
      {
        key: 'chain_slug',
        label: '链标识',
        type: 'text',
        default: 'bnb-mainnet',
        group: 'advanced'
      },
      {
        key: 'chain_name',
        label: '链名称',
        type: 'text',
        default: 'BNB Smart Chain',
        group: 'advanced'
      },
      {
        key: 'scan_host',
        label: '区块浏览器',
        type: 'text',
        default: 'https://bscscan.com',
        group: 'advanced'
      },
      {
        key: 'token_symbol',
        label: 'Token 符号',
        type: 'text',
        required: true,
        default: 'USDT',
        group: 'advanced'
      },
      {
        key: 'token_contract',
        label: 'Token 合约地址',
        type: 'text',
        required: true,
        default: '0x55d398326f99059ff775485246999027b3197955',
        group: 'advanced',
        help: 'BSC USDT 默认值；BSC USDC 为 0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d。'
      },
      {
        key: 'token_decimals',
        label: 'Token 精度',
        type: 'text',
        default: '18',
        group: 'advanced'
      },
      {
        key: 'confirmations',
        label: '确认数',
        type: 'text',
        default: '12',
        group: 'advanced'
      },
      {
        key: 'amount_precision',
        label: '支付识别小数位',
        type: 'text',
        default: '6',
        group: 'advanced'
      },
      {
        key: 'allow_overpay',
        label: '允许过付',
        type: 'switch',
        default: false,
        group: 'advanced',
        help: '默认关闭；开启后只接受不低于应付金额且不超过容差的到账。'
      },
      {
        key: 'overpay_tolerance',
        label: '过付容差',
        type: 'text',
        default: '0',
        group: 'advanced',
        help: '以 Token 数量填写，例如 0.01；仅允许过付开启时生效。'
      },
      {
        key: 'expire_minutes',
        label: '支付有效分钟',
        type: 'text',
        default: '30',
        group: 'advanced'
      },
      {
        key: 'log_scan_block_range',
        label: '单次扫描区块数',
        type: 'text',
        default: '10',
        group: 'advanced',
        help: 'Alchemy BNB 免费层建议保持 10。'
      },
      {
        key: 'max_scan_chunks_per_tick',
        label: '每轮扫描分片数',
        type: 'text',
        default: '12',
        group: 'advanced'
      },
      {
        key: 'rpc_url',
        label: '自定义 RPC URL',
        type: 'text',
        placeholder: 'https://bnb-mainnet.g.alchemy.com/v2/your-key',
        group: 'advanced'
      }
    ]
  },
  {
    id: 'bepusdt',
    label: 'Bepusdt',
    providerType: 'bepusdt',
    channelType: {
      kind: 'select',
      default: 'usdt',
      options: [
        { label: 'USDT TRC20', value: 'usdt' },
        { label: 'USDT TRC20 (显式)', value: 'usdt-trc20' },
        { label: 'USDC TRC20', value: 'usdc-trc20' },
        { label: 'TRX', value: 'trx' }
      ]
    },
    payCheckDefault: 'usdt',
    interactionDefault: 'qrcode',
    docsAnchor: 'Bepusdt 通道',
    hint: 'BEpusdt 协议，多链 USDT / USDC / TRX 收款。',
    fields: [
      {
        key: 'gateway_url',
        label: '网关地址',
        type: 'text',
        required: true,
        group: 'basic',
        placeholder: 'https://bepusdt.example.com'
      },
      {
        key: 'auth_token',
        label: 'API Token',
        type: 'password',
        required: true,
        group: 'merchant'
      },
      {
        key: 'fiat',
        label: '法币',
        type: 'text',
        default: 'CNY',
        group: 'advanced'
      },
      {
        key: 'trade_type',
        label: '强制 trade_type (可选)',
        type: 'text',
        group: 'advanced',
        placeholder: 'usdt.trc20 / usdc.trc20 / tron.trx',
        help: '留空则按通道类型自动映射'
      }
    ]
  },
  {
    id: 'okpay',
    label: 'OkPay',
    providerType: 'okpay',
    channelType: {
      kind: 'select',
      default: 'usdt',
      options: [
        { label: 'USDT', value: 'usdt' },
        { label: 'TRX', value: 'trx' }
      ]
    },
    payCheckDefault: 'usdt',
    interactionDefault: 'redirect',
    docsAnchor: 'Okpay 通道',
    hint: 'OkPay 商户系统，USDT / TRX 收款。',
    fields: [
      {
        key: 'merchant_id',
        label: '商户 ID',
        type: 'text',
        required: true,
        group: 'merchant'
      },
      {
        key: 'merchant_token',
        label: '商户 Token',
        type: 'password',
        required: true,
        group: 'merchant'
      },
      {
        key: 'gateway_url',
        label: '网关地址',
        type: 'text',
        default: 'https://api.okaypay.me/shop',
        group: 'advanced',
        help: '自建网关可改为对应地址'
      }
    ]
  },
  {
    id: 'dujiaopay',
    label: 'DujiaoPay',
    providerType: 'dujiaopay',
    channelType: {
      kind: 'select',
      default: 'tron-usdt',
      options: [
        { label: 'TRON - USDT', value: 'tron-usdt' },
        { label: 'TRON - TRX', value: 'tron-trx' },
        { label: 'Ethereum - USDT', value: 'ethereum-usdt' },
        { label: 'Ethereum - USDC', value: 'ethereum-usdc' },
        { label: 'Ethereum - ETH', value: 'ethereum-eth' },
        { label: 'BSC - USDT', value: 'bsc-usdt' },
        { label: 'BSC - BNB', value: 'bsc-bnb' },
        { label: 'Polygon - USDC', value: 'polygon-usdc' },
        { label: 'Polygon - USDT0', value: 'polygon-usdt0' },
        { label: 'Base - USDC', value: 'base-usdc' },
        { label: 'Arbitrum - USDC', value: 'arbitrum-usdc' },
        { label: 'Arbitrum - USDT0', value: 'arbitrum-usdt0' },
        { label: 'Plasma - USDT0', value: 'plasma-usdt0' },
        { label: 'X Layer - USDT0', value: 'x-layer-usdt0' },
        { label: 'Solana - USDC', value: 'solana-usdc' },
        { label: 'Solana - USDT', value: 'solana-usdt' },
        { label: 'Aptos - USDC', value: 'aptos-usdc' },
        { label: 'Aptos - USDT', value: 'aptos-usdt' }
      ]
    },
    payCheckDefault: 'usdt',
    interactionDefault: 'redirect',
    docsAnchor: 'DujiaoPay 通道',
    hint: '多链虚拟币收款 SaaS。channel_type 即 token_id。',
    fields: [
      {
        key: 'api_base_url',
        label: 'API 根地址',
        type: 'text',
        required: true,
        group: 'basic',
        default: 'https://www.dujiaopay.com',
        placeholder: 'https://www.dujiaopay.com',
        help: 'DujiaoPay 官方网关；自托管时改成自己的 HTTPS 入口'
      },
      {
        key: 'api_key_id',
        label: 'API Key ID',
        type: 'text',
        required: true,
        group: 'merchant'
      },
      {
        key: 'api_secret',
        label: 'API Secret',
        type: 'password',
        required: true,
        group: 'merchant',
        help: '请求签名用，HMAC-SHA256'
      },
      {
        key: 'webhook_secret',
        label: 'Webhook 密钥',
        type: 'password',
        required: true,
        group: 'merchant',
        help: '独立于 API Secret，仅用于回调验签'
      },
      {
        key: 'fiat_currency',
        label: '法币币种',
        type: 'text',
        default: 'CNY',
        group: 'advanced'
      },
      {
        key: 'chain',
        label: '强制 chain (可选)',
        type: 'text',
        group: 'advanced',
        placeholder: '留空按 token_id 自动映射'
      }
    ]
  },
  {
    id: 'official-stripe',
    label: '官方 Stripe',
    providerType: 'official',
    channelType: { kind: 'locked', value: 'stripe', label: 'Stripe' },
    payCheckDefault: 'stripe',
    interactionDefault: 'redirect',
    docsAnchor: '官方 Stripe 通道',
    hint: '基于 Stripe Checkout Session。',
    fields: [
      {
        key: 'secret_key',
        label: 'Secret Key',
        type: 'password',
        required: true,
        group: 'merchant',
        placeholder: 'sk_live_xxx',
        help: 'Stripe Dashboard → Developers → API keys'
      },
      {
        key: 'webhook_secret',
        label: 'Webhook Signing Secret',
        type: 'password',
        required: true,
        group: 'merchant',
        placeholder: 'whsec_xxx',
        help: 'Dashboard → Webhooks → endpoint 详情'
      },
      {
        key: 'payment_method_types',
        label: '允许的支付方式',
        type: 'multiselect',
        default: ['card'],
        group: 'basic',
        options: [
          { label: 'card', value: 'card' },
          { label: 'alipay', value: 'alipay' },
          { label: 'wechat_pay', value: 'wechat_pay' },
          { label: 'link', value: 'link' },
          { label: 'ideal', value: 'ideal' },
          { label: 'klarna', value: 'klarna' }
        ]
      },
      {
        key: 'target_currency',
        label: '提交币种 (Stripe 端)',
        type: 'text',
        group: 'basic',
        placeholder: 'usd / eur，留空使用订单币种',
        help: '影响金额换算；零小数币种 (JPY/KRW/VND) 自动除以 100'
      },
      {
        key: 'api_base_url',
        label: 'API 根地址',
        type: 'text',
        default: 'https://api.stripe.com',
        group: 'advanced'
      },
      {
        key: 'success_url',
        label: '支付成功跳转 URL',
        type: 'text',
        group: 'advanced',
        placeholder: '留空使用订单 return_url'
      },
      {
        key: 'cancel_url',
        label: '取消跳转 URL',
        type: 'text',
        group: 'advanced',
        placeholder: '留空使用订单 return_url'
      }
    ]
  },
  {
    id: 'official-paypal',
    label: '官方 PayPal',
    providerType: 'official',
    channelType: { kind: 'locked', value: 'paypal', label: 'PayPal' },
    payCheckDefault: 'paypal',
    interactionDefault: 'redirect',
    docsAnchor: '官方 PayPal 通道',
    hint: '基于 PayPal Orders v2 API。PayPal 不支持 CNY 入账。',
    fields: [
      {
        key: 'client_id',
        label: 'Client ID',
        type: 'text',
        required: true,
        group: 'merchant'
      },
      {
        key: 'client_secret',
        label: 'Client Secret',
        type: 'password',
        required: true,
        group: 'merchant'
      },
      {
        key: 'base_url',
        label: '运行环境',
        type: 'select',
        required: true,
        default: 'https://api-m.sandbox.paypal.com',
        group: 'basic',
        options: [
          {
            label: '沙箱 (sandbox)',
            value: 'https://api-m.sandbox.paypal.com'
          },
          { label: '生产 (production)', value: 'https://api-m.paypal.com' }
        ]
      },
      {
        key: 'webhook_id',
        label: 'Webhook ID',
        type: 'text',
        group: 'merchant',
        help: '建议填写。留空则跳过 Webhook 签名校验。'
      },
      {
        key: 'target_currency',
        label: '收款币种',
        type: 'text',
        default: 'USD',
        group: 'basic',
        help: 'PayPal 不支持 CNY；建议 USD / EUR / HKD'
      },
      {
        key: 'user_action',
        label: 'user_action',
        type: 'select',
        default: 'PAY_NOW',
        group: 'advanced',
        options: [
          { label: 'PAY_NOW', value: 'PAY_NOW' },
          { label: 'CONTINUE', value: 'CONTINUE' }
        ]
      },
      {
        key: 'shipping_preference',
        label: 'shipping_preference',
        type: 'select',
        default: 'NO_SHIPPING',
        group: 'advanced',
        options: [
          { label: 'NO_SHIPPING', value: 'NO_SHIPPING' },
          { label: 'SET_PROVIDED_ADDRESS', value: 'SET_PROVIDED_ADDRESS' },
          { label: 'GET_FROM_FILE', value: 'GET_FROM_FILE' }
        ]
      }
    ]
  },
  {
    id: 'official-alipay',
    label: '官方支付宝',
    providerType: 'official',
    channelType: { kind: 'locked', value: 'alipay', label: '支付宝' },
    payCheckDefault: 'alipay',
    interactionDefault: 'redirect',
    docsAnchor: '官方支付宝通道',
    hint: '支付宝开放平台 OpenAPI（非易支付）。',
    fields: [
      {
        key: 'app_id',
        label: 'APPID',
        type: 'text',
        required: true,
        group: 'merchant',
        placeholder: '2021000123456789'
      },
      {
        key: 'private_key',
        label: '商户私钥 (RSA2)',
        type: 'pem',
        required: true,
        group: 'merchant',
        help: 'PKCS8 / PKCS1 均可'
      },
      {
        key: 'alipay_public_key',
        label: '支付宝公钥',
        type: 'pem',
        group: 'merchant',
        help: '强烈建议填写，否则回调不验签'
      },
      {
        key: 'sign_type',
        label: '签名类型',
        type: 'select',
        default: 'RSA2',
        group: 'advanced',
        options: [
          { label: 'RSA2 (推荐)', value: 'RSA2' },
          { label: 'RSA', value: 'RSA' }
        ]
      },
      {
        key: 'interaction_mode',
        label: '交互模式',
        type: 'select',
        default: '',
        group: 'basic',
        options: [
          { label: 'PC 网页跳转 (默认)', value: '' },
          { label: 'WAP 手机网页', value: 'wap' },
          { label: '扫码 (precreate)', value: 'qr' }
        ]
      },
      {
        key: 'gateway_url',
        label: '网关地址',
        type: 'text',
        default: 'https://openapi.alipay.com/gateway.do',
        group: 'advanced',
        help: '沙箱用 https://openapi.alipaydev.com/gateway.do'
      }
    ]
  },
  {
    id: 'official-wechat',
    label: '官方微信支付',
    providerType: 'official',
    channelType: {
      kind: 'select',
      default: 'wechat',
      options: [
        { label: 'wechat (Native 扫码)', value: 'wechat' },
        { label: 'wxpay (同上别名)', value: 'wxpay' },
        { label: 'h5 (H5 跳转)', value: 'h5' },
        { label: 'wap (同上别名)', value: 'wap' }
      ]
    },
    payCheckDefault: 'wxpay',
    interactionDefault: 'qrcode',
    docsAnchor: '官方微信支付通道',
    hint: '微信支付 APIv3 (Native / H5)。',
    fields: [
      {
        key: 'appid',
        label: '公众号 / 小程序 AppID',
        type: 'text',
        required: true,
        group: 'merchant',
        placeholder: 'wx1234567890abcdef'
      },
      {
        key: 'mchid',
        label: '商户号 (mchid)',
        type: 'text',
        required: true,
        group: 'merchant'
      },
      {
        key: 'merchant_serial_no',
        label: '商户证书序列号',
        type: 'text',
        required: true,
        group: 'merchant'
      },
      {
        key: 'merchant_private_key',
        label: '商户私钥 (apiclient_key.pem)',
        type: 'pem',
        required: true,
        group: 'merchant'
      },
      {
        key: 'api_v3_key',
        label: 'APIv3 密钥',
        type: 'password',
        required: true,
        group: 'merchant',
        help: '32 字节字符串，用于解密回调'
      },
      {
        key: 'trade_type',
        label: '交易类型',
        type: 'select',
        default: '',
        group: 'basic',
        options: [
          { label: 'Native (扫码，默认)', value: '' },
          { label: 'H5', value: 'h5' }
        ]
      },
      {
        key: 'h5_type',
        label: 'H5 类型',
        type: 'select',
        default: 'Wap',
        group: 'advanced',
        options: [
          { label: 'Wap (移动浏览器)', value: 'Wap' },
          { label: 'iOS', value: 'iOS' },
          { label: 'Android', value: 'Android' }
        ],
        showIf: form => form.trade_type === 'h5'
      },
      {
        key: 'base_url',
        label: 'API 根地址',
        type: 'text',
        default: 'https://api.mch.weixin.qq.com',
        group: 'advanced'
      }
    ]
  }
];

const SPEC_BY_ID: Record<string, MethodSpec> = Object.fromEntries(
  METHOD_SPECS.map(s => [s.id, s])
);

export function getMethodById(id: string): MethodSpec | undefined {
  return SPEC_BY_ID[id];
}

/**
 * Reverse-lookup for edit hydration. Matches (provider_type, channel_type) to
 * a method id. Multiple specs may share provider_type=official; we prefer the
 * one whose locked/selected channel matches first.
 */
export function findMethodByRow(
  providerType: string,
  channelType: string
): MethodSpec | undefined {
  const pt = (providerType || '').trim().toLowerCase();
  const ct = (channelType || '').trim().toLowerCase();
  // 1. Try exact match on locked channel
  for (const spec of METHOD_SPECS) {
    if (spec.providerType !== pt) continue;
    if (spec.channelType.kind === 'locked' && spec.channelType.value === ct) {
      return spec;
    }
  }
  // 2. Try select channel containing the value
  for (const spec of METHOD_SPECS) {
    if (spec.providerType !== pt) continue;
    if (
      spec.channelType.kind === 'select' &&
      spec.channelType.options.some(o => o.value === ct)
    ) {
      return spec;
    }
  }
  // 3. Free-text channel — first matching provider_type wins
  for (const spec of METHOD_SPECS) {
    if (spec.providerType !== pt) continue;
    if (spec.channelType.kind === 'free') {
      return spec;
    }
  }
  // 4. Provider_type match without channel constraint as last resort
  return METHOD_SPECS.find(s => s.providerType === pt);
}

/**
 * Compose a JSON object from the per-method form values. Honors showIf and
 * skips empty/blank values so the resulting JSON is compact and the backend
 * defaults take over.
 */
export function buildConfigJson(
  spec: MethodSpec,
  values: Record<string, any>,
  touched: Record<string, boolean>
): Record<string, any> {
  const out: Record<string, any> = {};
  for (const field of spec.fields) {
    if (field.showIf && !field.showIf(values)) continue;
    const raw = values[field.key];
    // Password / pem fields with mask placeholder: skip when untouched on edit
    if (
      (field.type === 'password' || field.type === 'pem') &&
      typeof raw === 'string' &&
      raw === SECRET_MASK &&
      !touched[field.key]
    ) {
      continue;
    }
    if (field.type === 'multiselect') {
      if (Array.isArray(raw) && raw.length > 0) out[field.key] = raw;
      continue;
    }
    if (field.type === 'switch') {
      if (typeof raw === 'boolean') out[field.key] = raw;
      continue;
    }
    if (typeof raw === 'string') {
      const trimmed = raw.trim();
      if (trimmed.length > 0) out[field.key] = trimmed;
    } else if (raw != null) {
      out[field.key] = raw;
    }
  }
  return out;
}

export const SECRET_MASK = '********';

/**
 * Hydrate form values from an existing config_json. Sensitive fields show the
 * mask string and `touched` stays false until the user types.
 */
export function hydrateValues(
  spec: MethodSpec,
  config: Record<string, any>
): Record<string, any> {
  const values: Record<string, any> = {};
  for (const field of spec.fields) {
    let value = config[field.key];
    // PID alias compatibility — backend stores both `pid` and `merchant_id`.
    if (value == null && field.key === 'pid' && config.merchant_id != null) {
      value = config.merchant_id;
    }
    if (
      value == null &&
      field.key === 'merchant_key' &&
      (config.key != null || config.token != null)
    ) {
      value = config.key ?? config.token;
    }
    if (value == null) {
      if (field.default != null) {
        values[field.key] = Array.isArray(field.default)
          ? [...field.default]
          : field.default;
      } else if (field.type === 'multiselect') {
        values[field.key] = [];
      } else if (field.type === 'switch') {
        values[field.key] = false;
      } else {
        values[field.key] = '';
      }
      continue;
    }
    if (
      (field.type === 'password' || field.type === 'pem') &&
      typeof value === 'string' &&
      value.length > 0
    ) {
      values[field.key] = SECRET_MASK;
      continue;
    }
    if (field.type === 'multiselect') {
      values[field.key] = Array.isArray(value) ? value : [String(value)];
      continue;
    }
    if (field.type === 'switch') {
      values[field.key] = Boolean(value);
      continue;
    }
    values[field.key] = String(value);
  }
  return values;
}

export function initialValues(spec: MethodSpec): Record<string, any> {
  const values: Record<string, any> = {};
  for (const field of spec.fields) {
    if (field.default != null) {
      values[field.key] = Array.isArray(field.default)
        ? [...field.default]
        : field.default;
    } else if (field.type === 'multiselect') {
      values[field.key] = [];
    } else if (field.type === 'switch') {
      values[field.key] = false;
    } else {
      values[field.key] = '';
    }
  }
  return values;
}
