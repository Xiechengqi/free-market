<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import {
  NAlert,
  NButton,
  NCard,
  NDescriptions,
  NDescriptionsItem,
  NEmpty,
  NInput,
  NSpace,
  NSpin,
  NTable,
  NTag,
  useMessage
} from 'naive-ui';
import { confirmEvmIntent, fetchOrder, fulfillOrder } from '@/service/api';
import { channelTypeDisplay } from '../payment-channels/method-specs';

const route = useRoute();
const router = useRouter();
const message = useMessage();

const orderId = computed(() => Number(route.params.id));
const loading = ref(false);
const submitting = ref(false);
const data = ref<any>(null);
const fulfillPayload = ref('');
const evmTxHashes = ref<Record<number, string>>({});
const confirmingIntentId = ref<number | null>(null);

async function load() {
  if (!orderId.value) return;
  loading.value = true;
  const { data: d, error } = await fetchOrder(orderId.value);
  if (error) message.error(error.message || '加载失败');
  else data.value = d;
  loading.value = false;
}

async function doFulfill() {
  if (!fulfillPayload.value.trim()) {
    message.warning('请输入发货内容');
    return;
  }
  submitting.value = true;
  const { error } = await fulfillOrder(orderId.value, fulfillPayload.value);
  submitting.value = false;
  if (error) {
    message.error(error.message || '发货失败');
  } else {
    message.success('发货成功');
    fulfillPayload.value = '';
    load();
  }
}

async function doConfirmEvmIntent(intentId: number) {
  const txHash = (evmTxHashes.value[intentId] || '').trim();
  if (!txHash) {
    message.warning('请输入 tx_hash');
    return;
  }
  confirmingIntentId.value = intentId;
  const { error } = await confirmEvmIntent(orderId.value, intentId, txHash);
  confirmingIntentId.value = null;
  if (error) {
    message.error(error.message || '补单失败');
  } else {
    message.success('补单成功');
    evmTxHashes.value[intentId] = '';
    load();
  }
}

function shortHash(value: string) {
  if (!value) return '-';
  return value.length > 18 ? `${value.slice(0, 10)}...${value.slice(-8)}` : value;
}

function back() {
  router.push({ name: 'orders' });
}

const statusColor: Record<string, string> = {
  pending_payment: 'warning',
  paid: 'info',
  fulfilling: 'info',
  completed: 'success',
  canceled: 'default',
  abnormal: 'error',
  failed: 'error'
};

onMounted(load);
</script>

<template>
  <NSpace vertical :size="16">
    <NCard :bordered="false">
      <template #header>
        <NSpace align="center">
          <NButton size="small" @click="back">‹ 返回订单列表</NButton>
          <span>订单详情 #{{ orderId }}</span>
        </NSpace>
      </template>
      <NSpin :show="loading">
        <div v-if="data">
          <NDescriptions label-placement="left" :column="2" bordered>
            <NDescriptionsItem label="订单号">{{ data.order?.order_no }}</NDescriptionsItem>
            <NDescriptionsItem label="状态">
              <NTag :type="(statusColor[data.order?.status] as any) || 'default'" size="small">
                {{ data.order?.status }}
              </NTag>
            </NDescriptionsItem>
            <NDescriptionsItem label="金额">￥{{ data.amount_display }}</NDescriptionsItem>
            <NDescriptionsItem label="客户邮箱">{{ data.order?.guest_email }}</NDescriptionsItem>
            <NDescriptionsItem label="客户 IP">{{ data.order?.client_ip || '-' }}</NDescriptionsItem>
            <NDescriptionsItem label="过期时间">{{ data.order?.expires_at }}</NDescriptionsItem>
            <NDescriptionsItem label="创建时间">{{ data.order?.created_at }}</NDescriptionsItem>
            <NDescriptionsItem label="支付时间">{{ data.order?.paid_at || '-' }}</NDescriptionsItem>
          </NDescriptions>
        </div>
        <NEmpty v-else description="加载中或未找到订单" />
      </NSpin>
    </NCard>

    <NCard v-if="data?.items?.length" title="订单商品" :bordered="false">
      <NTable striped size="small">
        <thead>
          <tr><th>商品</th><th>类型</th><th>单价</th><th>数量</th><th>小计</th></tr>
        </thead>
        <tbody>
          <tr v-for="(item, idx) in data.items" :key="idx">
            <td>{{ item.product_name }}</td>
            <td>{{ item.fulfillment_type === 'auto' ? '自动' : '人工' }}</td>
            <td>￥{{ item.unit_price_display }}</td>
            <td>{{ item.quantity }}</td>
            <td>￥{{ item.total_price_display }}</td>
          </tr>
        </tbody>
      </NTable>
    </NCard>

    <NCard v-if="data?.payments?.length" title="支付记录" :bordered="false">
      <NTable striped size="small">
        <thead>
          <tr><th>流水号</th><th>通道</th><th>状态</th><th>金额</th><th>第三方单号</th></tr>
        </thead>
        <tbody>
          <tr v-for="(p, idx) in data.payments" :key="idx">
            <td>{{ p.payment_no }}</td>
            <td>{{ channelTypeDisplay(p.provider_type, p.channel_type) }}</td>
            <td>{{ p.status }}</td>
            <td>￥{{ p.amount_display }}</td>
            <td>{{ p.provider_ref || '-' }}</td>
          </tr>
        </tbody>
      </NTable>
    </NCard>

    <NCard v-if="data?.evm_intents?.length" title="EVM 本地收款" :bordered="false">
      <NTable striped size="small">
        <thead>
          <tr>
            <th>ID</th>
            <th>链/币种</th>
            <th>收款地址</th>
            <th>金额</th>
            <th>状态</th>
            <th>扫描</th>
            <th>匹配交易</th>
            <th>补单</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="intent in data.evm_intents" :key="intent.id">
            <td>{{ intent.id }}</td>
            <td>
              <NSpace size="small" align="center">
                <span>{{ intent.chain_slug }} / {{ intent.token_symbol }}</span>
                <NTag v-if="intent.network_env === 'testnet'" size="small" type="warning">
                  测试网
                </NTag>
              </NSpace>
            </td>
            <td style="word-break:break-all">{{ intent.receive_address }}</td>
            <td>{{ intent.amount_text }}</td>
            <td>
              <NTag size="small" :type="intent.status === 'matched' ? 'success' : intent.status === 'pending' ? 'warning' : 'default'">
                {{ intent.status }}
              </NTag>
              <div v-if="intent.last_error" style="max-width:260px;white-space:pre-wrap;color:#c62828">
                {{ intent.last_error }}
              </div>
            </td>
            <td>
              <div>from: {{ intent.scan_from_block }}</div>
              <div>last: {{ intent.last_scanned_block }}</div>
              <div>{{ intent.last_checked_at || '-' }}</div>
            </td>
            <td>
              <span :title="intent.matched_tx_hash">{{ shortHash(intent.matched_tx_hash) }}</span>
              <div>{{ intent.matched_at || '-' }}</div>
            </td>
            <td style="min-width:280px">
              <NSpace v-if="intent.status === 'pending'" vertical size="small">
                <NInput
                  v-model:value="evmTxHashes[intent.id]"
                  size="small"
                  placeholder="0x..."
                />
                <NButton
                  size="small"
                  type="primary"
                  :loading="confirmingIntentId === intent.id"
                  @click="doConfirmEvmIntent(intent.id)"
                >
                  校验并补单
                </NButton>
              </NSpace>
              <span v-else>-</span>
            </td>
          </tr>
        </tbody>
      </NTable>
    </NCard>

    <NCard v-if="data?.notifications?.length" title="通知记录" :bordered="false">
      <NTable striped size="small">
        <thead>
          <tr><th>通道</th><th>状态</th><th>消息</th><th>时间</th></tr>
        </thead>
        <tbody>
          <tr v-for="(n, idx) in data.notifications" :key="idx">
            <td>{{ n.channel }}</td>
            <td>{{ n.status }}</td>
            <td style="white-space:pre-wrap">{{ n.message }}</td>
            <td>{{ n.created_at }}</td>
          </tr>
        </tbody>
      </NTable>
    </NCard>

    <NCard title="人工发货" :bordered="false">
      <NAlert v-if="data?.fulfillment?.status === 'delivered'" type="success" :show-icon="false" style="margin-bottom:12px">
        已发货：{{ data.fulfillment.delivered_at || '' }}<br>
        <pre style="margin:8px 0 0;white-space:pre-wrap;word-break:break-all">{{ data.fulfillment.payload }}</pre>
      </NAlert>
      <NSpace vertical>
        <NInput
          v-model:value="fulfillPayload"
          type="textarea"
          :rows="6"
          placeholder="粘贴发货内容（卡密/激活码/账号密码 等），会以邮件发送给客户"
        />
        <NSpace justify="end">
          <NButton type="primary" :loading="submitting" @click="doFulfill">提交发货</NButton>
        </NSpace>
      </NSpace>
    </NCard>
  </NSpace>
</template>
