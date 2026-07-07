<script setup lang="ts">
import { computed, h, reactive, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import {
  NButton,
  NCollapse,
  NCollapseItem,
  NDivider,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NModal,
  NSelect,
  NSpace,
  NSwitch,
  NTag,
  NText,
  useMessage
} from 'naive-ui';
import {
  createPaymentChannel,
  fetchEvmPaymentPresets,
  updatePaymentChannel,
  validatePaymentChannel
} from '@/service/api';
import FieldRenderer from './field-renderer.vue';
import PayCheckPreview from './pay-check-preview.vue';
import {
  METHOD_SPECS,
  PAY_CHECK_OPTIONS,
  buildConfigJson,
  channelTypeDisplay,
  findMethodByRow,
  getMethodById,
  hydrateValues,
  initialValues,
  type MethodSpec
} from '../method-specs';

defineOptions({ name: 'ChannelForm' });

interface Props {
  show: boolean;
  editing: any | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: 'update:show', value: boolean): void;
  (e: 'saved'): void;
}>();

const message = useMessage();
const router = useRouter();

type MethodGroup = 'simulated' | 'builtin' | 'external';

const METHOD_GROUP_OPTIONS = [
  { label: '模拟', value: 'simulated' },
  { label: '内置', value: 'builtin' },
  { label: '外部', value: 'external' }
];
const SCOPE_OPTIONS = [
  { label: '全部', value: 'all' },
  { label: 'PC', value: 'pc' },
  { label: '移动端', value: 'mobile' }
];
const INTERACTION_OPTIONS = [
  { label: '跳转 (redirect)', value: 'redirect' },
  { label: '二维码 (qrcode)', value: 'qrcode' }
];

function renderPayCheckOption(option: any) {
  return h(PayCheckPreview, { value: String(option.value || '') });
}

interface MetaForm {
  name: string;
  methodGroup: MethodGroup;
  methodId: string;
  channelType: string;
  interaction_mode: string;
  client_scope: string;
  pay_check: string;
  sort_order: number;
  is_active: boolean;
}

const meta = reactive<MetaForm>({
  name: '',
  methodGroup: 'simulated',
  methodId: 'noop',
  channelType: 'test',
  interaction_mode: 'redirect',
  client_scope: 'all',
  pay_check: 'other',
  sort_order: 100,
  is_active: true
});

const fieldValues = ref<Record<string, any>>({});
const touched = ref<Record<string, boolean>>({});
const rawMode = ref(false);
const rawJson = ref('{}');
const submitting = ref(false);
const validating = ref(false);
const evmPresets = ref<EvmChainPreset[]>([]);
let syncingEvm = false;

interface EvmTokenPreset {
  symbol: string;
  contract: string;
  decimals: number;
  official: boolean;
  note: string;
}

interface EvmChainPreset {
  id: string;
  env: 'mainnet' | 'testnet';
  label: string;
  alchemy_network: string;
  chain_id: number;
  chain_slug: string;
  chain_name: string;
  scan_host: string;
  default_confirmations: number;
  default_log_scan_block_range: number;
  tokens: EvmTokenPreset[];
}

const baseSpec = computed<MethodSpec>(() => {
  return getMethodById(meta.methodId) ?? METHOD_SPECS[0];
});

const methodOptions = computed(() =>
  METHOD_SPECS.filter(spec => methodGroupForSpec(spec) === meta.methodGroup).map(spec => ({
    label: spec.label,
    value: spec.id
  }))
);

const currentSpec = computed<MethodSpec>(() => {
  const spec = baseSpec.value;
  if (spec.id !== 'evm-local') return spec;
  const env = fieldValues.value.network_env === 'testnet' ? 'testnet' : 'mainnet';
  const chains = evmPresets.value.filter(chain => chain.env === env);
  const selectedChain =
    chains.find(chain => chain.id === fieldValues.value.evm_chain_preset) ?? chains[0];
  const tokenOptions =
    selectedChain?.tokens.map(token => ({
      label: `${token.symbol}${token.official ? '' : ' (非官方)'} - ${shortAddress(token.contract)}`,
      value: tokenPresetValue(token)
    })) ?? [];
  tokenOptions.push({ label: '自定义 ERC20 / 测试 USDT', value: '__custom__' });
  return {
    ...spec,
    channelType: {
      kind: 'select',
      default: evmChannelTypeFor(selectedChain, selectedChain?.tokens[0]),
      options: buildEvmChannelTypeOptions(chains)
    },
    fields: spec.fields.map(field => {
      if (field.key === 'evm_chain_preset') {
        return {
          ...field,
          options: chains.map(chain => ({
            label: `${chain.label} (${chain.alchemy_network})`,
            value: chain.id
          }))
        };
      }
      if (field.key === 'evm_token_preset') {
        return { ...field, options: tokenOptions };
      }
      return field;
    })
  };
});

const groupedFields = computed(() => {
  const fields = currentSpec.value.fields.filter(
    f => !f.showIf || f.showIf(fieldValues.value)
  );
  return {
    basic: fields.filter(f => (f.group ?? 'basic') === 'basic'),
    merchant: fields.filter(f => f.group === 'merchant'),
    advanced: fields.filter(f => f.group === 'advanced')
  };
});

const callbackPreview = computed(() => {
  const spec = currentSpec.value;
  const ch = (meta.channelType || '').trim() || '{channel_type}';
  return {
    primary: `/payment/callback/${spec.providerType}/${ch}`,
    legacy: `/pay/${spec.providerType}/notify_url`,
    syncReturn: `/pay/${spec.providerType}/return_url`
  };
});

const channelTypeWidget = computed(() => currentSpec.value.channelType);

function openDocs() {
  router.push({ path: '/docs', query: { h: currentSpec.value.docsAnchor } });
  emit('update:show', false);
}

watch(
  () => props.show,
  show => {
    if (!show) return;
    loadEvmPresets();
    resetForm();
    if (props.editing) hydrateFromRow(props.editing);
    else applyMethodDefaults(currentSpec.value, true);
  }
);

watch(
  () => fieldValues.value.network_env,
  () => {
    if (syncingEvm || meta.methodId !== 'evm-local' || rawMode.value) return;
    const env = fieldValues.value.network_env === 'testnet' ? 'testnet' : 'mainnet';
    const first = evmPresets.value.find(chain => chain.env === env);
    if (first) applyEvmChainPreset(first.id, true);
  }
);

watch(
  () => fieldValues.value.evm_chain_preset,
  value => {
    if (syncingEvm || meta.methodId !== 'evm-local' || rawMode.value || !value) return;
    applyEvmChainPreset(String(value), true);
  }
);

watch(
  () => fieldValues.value.evm_token_preset,
  value => {
    if (syncingEvm || meta.methodId !== 'evm-local' || rawMode.value || !value) return;
    applyEvmTokenPreset(String(value));
  }
);

watch(
  () => meta.channelType,
  value => {
    if (syncingEvm || meta.methodId !== 'evm-local' || rawMode.value || !value) return;
    applyEvmChannelType(String(value));
  }
);

async function loadEvmPresets() {
  if (evmPresets.value.length > 0) return;
  const { data, error } = await fetchEvmPaymentPresets();
  if (error) {
    message.warning(error.message || 'EVM 预设加载失败，可继续手动填写');
    return;
  }
  evmPresets.value = Array.isArray(data?.chains) ? data.chains : [];
  if (meta.methodId === 'evm-local' && !rawMode.value) ensureEvmPresetApplied();
}

function resetForm() {
  meta.name = '';
  meta.methodGroup = 'simulated';
  meta.methodId = 'noop';
  meta.channelType = 'test';
  meta.interaction_mode = 'redirect';
  meta.client_scope = 'all';
  meta.pay_check = 'other';
  meta.sort_order = 100;
  meta.is_active = true;
  fieldValues.value = {};
  touched.value = {};
  rawMode.value = false;
  rawJson.value = '{}';
}

function applyMethodDefaults(spec: MethodSpec, alsoChannel: boolean) {
  if (alsoChannel) {
    const w = spec.channelType;
    if (w.kind === 'locked') meta.channelType = w.value;
    else if (w.kind === 'select') {
      const first = w.options[0]?.value;
      meta.channelType = w.default ?? (typeof first === 'string' ? first : '');
    }
    else meta.channelType = w.default ?? '';
  }
  if (spec.payCheckDefault) meta.pay_check = spec.payCheckDefault;
  if (spec.interactionDefault) meta.interaction_mode = spec.interactionDefault;
  fieldValues.value = initialValues(spec);
  touched.value = {};
  rawJson.value = JSON.stringify(
    buildConfigJson(spec, fieldValues.value, touched.value),
    null,
    2
  );
  if (spec.id === 'evm-local') ensureEvmPresetApplied();
}

function hydrateFromRow(row: any) {
  meta.name = row.name ?? '';
  meta.interaction_mode = row.interaction_mode ?? 'redirect';
  meta.client_scope = row.client_scope ?? 'all';
  meta.pay_check = row.pay_check ?? '';
  meta.sort_order = row.sort_order ?? 100;
  meta.is_active = !!row.is_active;

  const matched = findMethodByRow(row.provider_type, row.channel_type);
  if (matched) {
    meta.methodGroup = methodGroupForSpec(matched);
    meta.methodId = matched.id;
    meta.channelType = row.channel_type ?? '';
    let config: Record<string, any> = {};
    try {
      config = row.config_json ? JSON.parse(row.config_json) : {};
    } catch {
      // fall through to raw mode if json broken
      rawMode.value = true;
      rawJson.value = row.config_json ?? '{}';
      return;
    }
    fieldValues.value = hydrateValues(matched, config);
    touched.value = {};
    rawJson.value = JSON.stringify(config, null, 2);
    if (matched.id === 'evm-local') ensureEvmPresetApplied();
  } else {
    // No matching spec — show JSON mode so the row isn't broken.
    meta.methodId = METHOD_SPECS[0].id;
    meta.channelType = row.channel_type ?? '';
    rawMode.value = true;
    rawJson.value = row.config_json ?? '{}';
  }
}

function onMethodChange(id: string) {
  meta.methodId = id;
  const spec = getMethodById(id);
  if (!spec) return;
  meta.methodGroup = methodGroupForSpec(spec);
  applyMethodDefaults(spec, true);
}

function onMethodGroupChange(group: MethodGroup) {
  meta.methodGroup = group;
  const next = METHOD_SPECS.find(spec => methodGroupForSpec(spec) === group) ?? METHOD_SPECS[0];
  onMethodChange(next.id);
}

function markTouched(key: string) {
  touched.value[key] = true;
}

function tokenPresetValue(token: EvmTokenPreset) {
  return `${token.symbol}:${token.contract.toLowerCase()}`;
}

function methodGroupForSpec(spec: MethodSpec): MethodGroup {
  if (spec.id === 'noop') return 'simulated';
  if (spec.id === 'evm-local') return 'builtin';
  return 'external';
}

function shortAddress(value: string) {
  if (!value) return '';
  return value.length > 16 ? `${value.slice(0, 8)}...${value.slice(-6)}` : value;
}

function findEvmChain(id: string) {
  return evmPresets.value.find(chain => chain.id === id);
}

function evmChannelTypeFor(chain?: EvmChainPreset, token?: EvmTokenPreset) {
  if (!chain) return '';
  const tokenPart = token?.symbol?.toLowerCase() || 'erc20';
  const base = chain.env === 'mainnet' ? chain.id.replace('-mainnet', '') : chain.id;
  return `${base}-${tokenPart}`;
}

function buildEvmChannelTypeOptions(chains: EvmChainPreset[]) {
  const options: { label: string; value: string }[] = [];
  for (const chain of chains) {
    for (const token of chain.tokens) {
      options.push({
        label: `${chain.label} ${token.symbol}${chain.env === 'testnet' ? ' (测试网)' : ''}`,
        value: evmChannelTypeFor(chain, token)
      });
    }
    options.push({
      label: `${chain.label} 自定义 ERC20${chain.env === 'testnet' ? ' (测试网)' : ''}`,
      value: evmChannelTypeFor(chain)
    });
  }
  return options;
}

function ensureEvmPresetApplied() {
  if (meta.methodId !== 'evm-local' || rawMode.value || evmPresets.value.length === 0) return;
  normalizeEvmPresetSelectionFromValues();
  const env = fieldValues.value.network_env === 'testnet' ? 'testnet' : 'mainnet';
  const chainId =
    fieldValues.value.evm_chain_preset ||
    evmPresets.value.find(chain => chain.env === env)?.id ||
    evmPresets.value[0]?.id;
  if (chainId) applyEvmChainPreset(chainId, !fieldValues.value.evm_token_preset);
}

function normalizeEvmPresetSelectionFromValues() {
  if (evmPresets.value.length === 0) return;
  const rawChainId = Number(fieldValues.value.chain_id);
  const rawNetwork = String(fieldValues.value.alchemy_network || '').trim();
  const rawSlug = String(fieldValues.value.chain_slug || '').trim();
  const chain = evmPresets.value.find(item => {
    if (Number.isFinite(rawChainId) && rawChainId > 0 && item.chain_id === rawChainId) return true;
    if (rawNetwork && item.alchemy_network === rawNetwork) return true;
    if (rawSlug && item.chain_slug === rawSlug) return true;
    return false;
  });
  if (!chain) return;
  const rawContract = String(fieldValues.value.token_contract || '').trim().toLowerCase();
  const token = rawContract
    ? chain.tokens.find(item => item.contract.toLowerCase() === rawContract)
    : chain.tokens[0];
  const tokenPreset = token ? tokenPresetValue(token) : '__custom__';
  if (
    fieldValues.value.network_env === chain.env &&
    fieldValues.value.evm_chain_preset === chain.id &&
    fieldValues.value.evm_token_preset === tokenPreset
  ) {
    return;
  }
  syncingEvm = true;
  fieldValues.value = {
    ...fieldValues.value,
    network_env: chain.env,
    evm_chain_preset: chain.id,
    evm_token_preset: tokenPreset
  };
  syncingEvm = false;
}

function applyEvmChainPreset(chainId: string, resetToken: boolean) {
  const chain = findEvmChain(chainId);
  if (!chain) return;
  syncingEvm = true;
  const next: Record<string, any> = {
    ...fieldValues.value,
    network_env: chain.env,
    evm_chain_preset: chain.id,
    alchemy_network: chain.alchemy_network,
    chain_id: String(chain.chain_id),
    chain_slug: chain.chain_slug,
    chain_name: chain.chain_name,
    scan_host: chain.scan_host,
    confirmations: String(chain.default_confirmations),
    log_scan_block_range: String(chain.default_log_scan_block_range)
  };
  if (resetToken || !next.evm_token_preset) {
    const firstToken = chain.tokens[0];
    next.evm_token_preset = firstToken ? tokenPresetValue(firstToken) : '__custom__';
  }
  fieldValues.value = next;
  applyEvmTokenPreset(String(fieldValues.value.evm_token_preset || '__custom__'));
  syncingEvm = false;
}

function applyEvmTokenPreset(value: string) {
  if (value === '__custom__') {
    const chain = findEvmChain(String(fieldValues.value.evm_chain_preset || ''));
    if (chain) meta.channelType = evmChannelTypeFor(chain);
    return;
  }
  const chain = findEvmChain(String(fieldValues.value.evm_chain_preset || ''));
  const token = chain?.tokens.find(item => tokenPresetValue(item) === value);
  if (!chain || !token) return;
  fieldValues.value = {
    ...fieldValues.value,
    token_symbol: token.symbol,
    token_contract: token.contract,
    token_decimals: String(token.decimals)
  };
  meta.channelType = evmChannelTypeFor(chain, token);
  meta.pay_check = token.symbol.toUpperCase() === 'USDC' ? 'usdc' : token.symbol.toLowerCase();
}

function applyEvmChannelType(channelType: string) {
  const match = findEvmChannelType(channelType);
  if (!match) return;
  syncingEvm = true;
  fieldValues.value = {
    ...fieldValues.value,
    network_env: match.chain.env,
    evm_chain_preset: match.chain.id,
    evm_token_preset: match.token ? tokenPresetValue(match.token) : '__custom__',
    alchemy_network: match.chain.alchemy_network,
    chain_id: String(match.chain.chain_id),
    chain_slug: match.chain.chain_slug,
    chain_name: match.chain.chain_name,
    scan_host: match.chain.scan_host,
    confirmations: String(match.chain.default_confirmations),
    log_scan_block_range: String(match.chain.default_log_scan_block_range),
    ...(match.token
      ? {
          token_symbol: match.token.symbol,
          token_contract: match.token.contract,
          token_decimals: String(match.token.decimals)
        }
      : {})
  };
  meta.pay_check = match.token
    ? match.token.symbol.toUpperCase() === 'USDC'
      ? 'usdc'
      : match.token.symbol.toLowerCase()
    : meta.pay_check || 'usdt';
  syncingEvm = false;
}

function findEvmChannelType(channelType: string) {
  for (const chain of evmPresets.value) {
    if (evmChannelTypeFor(chain) === channelType) return { chain, token: null as EvmTokenPreset | null };
    for (const token of chain.tokens) {
      if (evmChannelTypeFor(chain, token) === channelType) return { chain, token };
    }
  }
  return null;
}

function buildPayload(): { ok: true; payload: any } | { ok: false; error: string } {
  if (!meta.name.trim()) return { ok: false, error: '请填写通道名称' };
  if (!meta.channelType.trim() && currentSpec.value.channelType.kind !== 'locked')
    return { ok: false, error: '请选择 / 填写 channel_type' };

  let configJson = '{}';
  if (rawMode.value) {
    try {
      const parsed = JSON.parse(rawJson.value || '{}');
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed))
        return { ok: false, error: '原始 JSON 必须是对象' };
      configJson = JSON.stringify(parsed);
    } catch (e: any) {
      return { ok: false, error: `JSON 解析失败: ${e.message}` };
    }
  } else {
    const spec = currentSpec.value;
    for (const f of spec.fields) {
      if (f.showIf && !f.showIf(fieldValues.value)) continue;
      if (!f.required) continue;
      const v = fieldValues.value[f.key];
      const empty =
        v == null ||
        (typeof v === 'string' && v.trim() === '') ||
        (Array.isArray(v) && v.length === 0);
      if (empty) return { ok: false, error: `请填写「${f.label}」` };
    }
    const obj = buildConfigJson(spec, fieldValues.value, touched.value);
    configJson = JSON.stringify(obj);
  }

  return {
    ok: true,
    payload: {
      name: meta.name.trim(),
      provider_type: currentSpec.value.providerType,
      channel_type: meta.channelType.trim(),
      interaction_mode: meta.interaction_mode,
      client_scope: meta.client_scope,
      pay_check: meta.pay_check || '',
      sort_order: meta.sort_order,
      is_active: meta.is_active ? 'on' : '',
      config_json: configJson
    }
  };
}

async function submit() {
  const built = buildPayload();
  if (!built.ok) {
    message.error(built.error);
    return;
  }
  submitting.value = true;
  const action = props.editing
    ? updatePaymentChannel(props.editing.id, built.payload)
    : createPaymentChannel(built.payload);
  const { error } = await action;
  submitting.value = false;
  if (error) {
    message.error(error.message || '保存失败');
    return;
  }
  message.success('保存成功');
  emit('update:show', false);
  emit('saved');
}

async function runValidate() {
  const built = buildPayload();
  if (!built.ok) {
    message.error(built.error);
    return;
  }
  validating.value = true;
  const { error } = await validatePaymentChannel(built.payload);
  validating.value = false;
  if (error) {
    message.error(error.message || '配置不合法');
    return;
  }
  message.success('配置合法 ✓ 可以保存');
}

function toggleRawMode() {
  if (!rawMode.value) {
    // Switching into raw: serialize current field values
    rawJson.value = JSON.stringify(
      buildConfigJson(currentSpec.value, fieldValues.value, touched.value),
      null,
      2
    );
    rawMode.value = true;
  } else {
    // Switching out of raw: try parse and rehydrate
    try {
      const parsed = JSON.parse(rawJson.value || '{}');
      if (typeof parsed !== 'object' || parsed === null) {
        message.error('JSON 不是对象，保留原始 JSON 模式');
        return;
      }
      fieldValues.value = hydrateValues(currentSpec.value, parsed);
      touched.value = {};
      rawMode.value = false;
    } catch (e: any) {
      message.error(`JSON 解析失败: ${e.message}`);
    }
  }
}
</script>

<template>
  <NModal
    :show="show"
    preset="card"
    :title="editing ? '编辑支付通道' : '新建支付通道'"
    style="max-width: 760px"
    @update:show="(v: boolean) => emit('update:show', v)"
  >
    <NForm label-placement="left" label-width="120">
      <NFormItem label="支付类型" required>
        <NSelect
          :value="meta.methodGroup"
          :options="METHOD_GROUP_OPTIONS"
          @update:value="onMethodGroupChange"
        />
      </NFormItem>

      <NFormItem label="支付方式" required>
        <NSelect
          :value="meta.methodId"
          :options="methodOptions"
          @update:value="onMethodChange"
        />
        <template v-if="currentSpec.hint" #feedback>
          <span>{{ currentSpec.hint }}</span>
          <a
            href="javascript:void(0)"
            style="margin-left: 8px; color: #2266dd"
            @click="openDocs"
          >查看配置文档 →</a>
        </template>
      </NFormItem>

      <NFormItem label="通道名称" required>
        <NInput
          v-model:value="meta.name"
          placeholder="前台展示的支付方式名称，例如「支付宝」「USDT-TRC20」"
        />
      </NFormItem>

      <NFormItem label="渠道类型" required>
        <template v-if="channelTypeWidget.kind === 'locked'">
          <NTag size="medium" round>
            {{ channelTypeDisplay(currentSpec.providerType, channelTypeWidget.value) }}
          </NTag>
        </template>
        <NSelect
          v-else-if="channelTypeWidget.kind === 'select'"
          v-model:value="meta.channelType"
          :options="channelTypeWidget.options"
        />
        <NInput
          v-else
          v-model:value="meta.channelType"
          :placeholder="channelTypeWidget.placeholder || ''"
        />
      </NFormItem>

      <NFormItem label="交互方式">
        <NSelect v-model:value="meta.interaction_mode" :options="INTERACTION_OPTIONS" />
      </NFormItem>
      <NFormItem label="客户端">
        <NSelect v-model:value="meta.client_scope" :options="SCOPE_OPTIONS" />
      </NFormItem>
      <NFormItem label="图标标识">
        <NSelect
          v-model:value="meta.pay_check"
          :options="PAY_CHECK_OPTIONS"
          :render-label="renderPayCheckOption"
          filterable
          tag
          placeholder="选择前台图标标识"
        />
      </NFormItem>
      <NFormItem label="排序">
        <NInputNumber v-model:value="meta.sort_order" :min="0" />
      </NFormItem>
      <NFormItem label="启用">
        <NSwitch v-model:value="meta.is_active" />
      </NFormItem>

      <NDivider title-placement="left" style="margin: 20px 0 16px">
        <span style="font-size: 13px; color: #666">配置参数</span>
      </NDivider>

      <template v-if="!rawMode">
        <NText v-if="currentSpec.fields.length === 0" depth="3" style="margin-left: 120px">
          该支付方式无需额外配置。
        </NText>

        <NFormItem
          v-for="field in groupedFields.basic"
          :key="field.key"
          :label="field.label"
          :required="field.required"
        >
          <FieldRenderer
            :field="field"
            :model-value="fieldValues[field.key]"
            :touched="!!touched[field.key]"
            @update:model-value="(v: any) => (fieldValues[field.key] = v)"
            @touch="markTouched(field.key)"
          />
          <template v-if="field.help" #feedback>{{ field.help }}</template>
        </NFormItem>

        <template v-if="groupedFields.merchant.length">
          <NDivider title-placement="left" style="margin: 12px 0 16px">
            <span style="font-size: 12px; color: #999">商户参数</span>
          </NDivider>
          <NFormItem
            v-for="field in groupedFields.merchant"
            :key="field.key"
            :label="field.label"
            :required="field.required"
          >
            <FieldRenderer
              :field="field"
              :model-value="fieldValues[field.key]"
              :touched="!!touched[field.key]"
              @update:model-value="(v: any) => (fieldValues[field.key] = v)"
              @touch="markTouched(field.key)"
            />
            <template v-if="field.help" #feedback>{{ field.help }}</template>
          </NFormItem>
        </template>

        <NCollapse
          v-if="groupedFields.advanced.length"
          arrow-placement="right"
          style="margin: 16px 0 8px"
        >
          <NCollapseItem title="高级设置" name="advanced">
            <NFormItem
              v-for="field in groupedFields.advanced"
              :key="field.key"
              :label="field.label"
              :required="field.required"
            >
              <FieldRenderer
                :field="field"
                :model-value="fieldValues[field.key]"
                :touched="!!touched[field.key]"
                @update:model-value="(v: any) => (fieldValues[field.key] = v)"
                @touch="markTouched(field.key)"
              />
              <template v-if="field.help" #feedback>{{ field.help }}</template>
            </NFormItem>
          </NCollapseItem>
        </NCollapse>
      </template>

      <NFormItem v-else label="config_json">
        <NInput
          v-model:value="rawJson"
          type="textarea"
          :rows="10"
          placeholder='{"pid":"...","key":"..."}'
          style="font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px"
        />
      </NFormItem>

      <NDivider title-placement="left" style="margin: 20px 0 16px">
        <span style="font-size: 12px; color: #999">回调地址（仅供参考，复制到上游平台）</span>
      </NDivider>
      <NFormItem label=" " :show-feedback="false">
        <div style="font-size: 12px; color: #555; line-height: 1.9">
          <div>异步通知（推荐）：<code>{{ callbackPreview.primary }}</code></div>
          <div>异步通知（兼容旧路由）：<code>{{ callbackPreview.legacy }}</code></div>
          <div>同步返回：<code>{{ callbackPreview.syncReturn }}</code></div>
        </div>
      </NFormItem>
    </NForm>

    <template #footer>
      <NSpace justify="space-between" align="center">
        <NButton size="small" tertiary @click="toggleRawMode">
          {{ rawMode ? '← 切回字段模式' : '切换到原始 JSON 模式 →' }}
        </NButton>
        <NSpace>
          <NButton @click="emit('update:show', false)">取消</NButton>
          <NButton :loading="validating" @click="runValidate">测试配置</NButton>
          <NButton type="primary" :loading="submitting" @click="submit">保存</NButton>
        </NSpace>
      </NSpace>
    </template>
  </NModal>
</template>
