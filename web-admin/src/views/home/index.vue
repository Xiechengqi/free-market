<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { request } from '@/service/request';

interface DashboardData {
  order_count: number;
  product_count: number;
  available_cards: number;
  completed_order_count: number;
  pending_order_count: number;
  canceled_order_count: number;
  total_sales_display: string;
  today_orders: number;
  today_sales_display: string;
}

const data = ref<DashboardData | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);

async function load() {
  loading.value = true;
  error.value = null;
  const res = await request<DashboardData>({ url: '/dashboard', method: 'get' });
  if (res.error) {
    error.value = res.error.message || '加载失败';
  } else if (res.data) {
    data.value = res.data;
  }
  loading.value = false;
}

onMounted(load);

const cards = [
  { key: 'order_count', label: '订单总数', color: '#2e7d32' },
  { key: 'today_orders', label: '今日订单', color: '#1565c0' },
  { key: 'completed_order_count', label: '已完成', color: '#5e35b1' },
  { key: 'pending_order_count', label: '待支付', color: '#ef6c00' },
  { key: 'canceled_order_count', label: '已取消', color: '#9e9e9e' },
  { key: 'product_count', label: '商品数', color: '#00695c' },
  { key: 'available_cards', label: '可用卡密', color: '#7b1fa2' }
];
</script>

<template>
  <NSpace vertical :size="16">
    <NAlert title="freeMarket 后台" type="info">
      欢迎使用，访问 <code>/admin/api/*</code> 查看完整 JSON API。
    </NAlert>

    <NSpin :show="loading">
      <NGrid x-gap="16" y-gap="16" responsive="screen" item-responsive>
        <NGi v-for="c in cards" :key="c.key" span="24 s:12 m:8 l:6">
          <NCard :bordered="false" class="card-wrapper">
            <NStatistic :label="c.label">
              <span :style="{ color: c.color, fontSize: '32px', fontWeight: 700 }">
                {{ data ? (data as any)[c.key] : '—' }}
              </span>
            </NStatistic>
          </NCard>
        </NGi>
      </NGrid>
    </NSpin>

    <NCard :bordered="false" class="card-wrapper" title="销售金额">
      <NSpace>
        <NStatistic label="累计销售">
          <span style="font-size:28px;font-weight:700;color:#2e7d32">
            ￥{{ data?.total_sales_display ?? '—' }}
          </span>
        </NStatistic>
        <NStatistic label="今日销售">
          <span style="font-size:28px;font-weight:700;color:#1565c0">
            ￥{{ data?.today_sales_display ?? '—' }}
          </span>
        </NStatistic>
      </NSpace>
    </NCard>

    <NAlert v-if="error" type="error" :title="error" closable />
  </NSpace>
</template>

<style scoped>
.card-wrapper {
  min-height: 120px;
}
</style>
