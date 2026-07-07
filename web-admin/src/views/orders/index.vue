<script setup lang="ts">
import { h, onMounted, reactive, ref } from 'vue';
import { useRouter } from 'vue-router';
import {
  NButton,
  NDataTable,
  NForm,
  NFormItem,
  NInput,
  NPagination,
  NSelect,
  NSpace,
  NTag,
  useDialog,
  useMessage
} from 'naive-ui';
import {
  cancelOrder,
  deleteOrder,
  downloadOrdersCsv,
  fetchOrders,
  markOrderAbnormal,
  resendOrderEmail,
  startOrderProcessing
} from '@/service/api';

const router = useRouter();
const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 20, itemCount: 0 });

const filters = reactive({
  order_no: '',
  email: '',
  status: '' as string,
  date_from: '',
  date_to: ''
});

const statusOptions = [
  { label: '全部', value: '' },
  { label: '待支付', value: 'pending_payment' },
  { label: '已支付', value: 'paid' },
  { label: '发货中', value: 'fulfilling' },
  { label: '已完成', value: 'completed' },
  { label: '已取消', value: 'canceled' },
  { label: '异常', value: 'abnormal' },
  { label: '失败', value: 'failed' }
];

const statusColor: Record<string, string> = {
  pending_payment: 'warning',
  paid: 'info',
  fulfilling: 'info',
  completed: 'success',
  canceled: 'default',
  abnormal: 'error',
  failed: 'error'
};

function filterParams(): Record<string, any> {
  const p: Record<string, any> = {
    page: pagination.value.page,
    per_page: pagination.value.pageSize
  };
  if (filters.order_no.trim()) p.order_no = filters.order_no.trim();
  if (filters.email.trim()) p.email = filters.email.trim();
  if (filters.status) p.status = filters.status;
  if (filters.date_from) p.date_from = filters.date_from;
  if (filters.date_to) p.date_to = filters.date_to;
  return p;
}

async function load() {
  loading.value = true;
  const { data, error } = await fetchOrders(filterParams());
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.orders || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

function resetFilters() {
  filters.order_no = '';
  filters.email = '';
  filters.status = '';
  filters.date_from = '';
  filters.date_to = '';
  pagination.value.page = 1;
  load();
}

function applyFilters() {
  pagination.value.page = 1;
  load();
}

async function doExport() {
  try {
    await downloadOrdersCsv(filterParams());
  } catch (e: any) {
    message.error(e.message || '导出失败');
  }
}

function doAction(label: string, fn: () => Promise<any>) {
  dialog.warning({
    title: label,
    content: `确认对该订单执行：${label}？`,
    positiveText: '确认',
    negativeText: '取消',
    onPositiveClick: async () => {
      const { error } = await fn();
      if (error) message.error(error.message || `${label} 失败`);
      else {
        message.success(`${label} 成功`);
        load();
      }
    }
  });
}

function gotoDetail(r: any) {
  const id = r.order?.id || r.id;
  router.push({ name: 'order-detail', params: { id } });
}

const columns = [
  { title: 'ID', key: 'id', width: 60, render: (r: any) => r.order?.id || r.id },
  {
    title: '订单号',
    key: 'order.order_no',
    width: 260,
    render: (r: any) => {
      const id = r.order?.id || r.id;
      const no = r.order?.order_no || r.order_no;
      return h(
        NButton,
        { text: true, type: 'primary', onClick: () => router.push({ name: 'order-detail', params: { id } }) },
        { default: () => no }
      );
    }
  },
  {
    title: '状态',
    key: 'order.status',
    width: 100,
    render: (r: any) => {
      const s = r.order?.status || r.status;
      return h(NTag, { type: (statusColor[s] as any) || 'default', size: 'small' }, { default: () => s });
    }
  },
  { title: '金额', key: 'amount_display', width: 90, render: (r: any) => `￥${r.amount_display || '-'}` },
  { title: '邮箱', key: 'order.guest_email', render: (r: any) => r.order?.guest_email || r.guest_email },
  { title: '商品', key: 'product_name', render: (r: any) => r.product_name },
  { title: '支付通道', key: 'payment_channel_name', width: 140, render: (r: any) => r.payment_channel_name || '-' },
  { title: '创建时间', key: 'order.created_at', width: 170, render: (r: any) => r.order?.created_at },
  {
    title: '操作',
    key: 'actions',
    width: 360,
    render: (r: any) => {
      const id = r.order?.id || r.id;
      const status = r.order?.status || r.status;
      const btns: any[] = [
        h(NButton, { size: 'tiny', onClick: () => gotoDetail(r) }, { default: () => '详情' })
      ];
      if (status === 'pending_payment') {
        btns.push(h(NButton, { size: 'tiny', type: 'warning', onClick: () => doAction('取消订单', () => cancelOrder(id)) }, { default: () => '取消' }));
      }
      if (status === 'paid') {
        btns.push(h(NButton, { size: 'tiny', type: 'primary', onClick: () => doAction('开始处理', () => startOrderProcessing(id)) }, { default: () => '开始处理' }));
      }
      btns.push(h(NButton, { size: 'tiny', onClick: () => doAction('重发邮件', () => resendOrderEmail(id)) }, { default: () => '重发邮件' }));
      if (['paid', 'fulfilling', 'completed'].includes(status)) {
        btns.push(h(NButton, { size: 'tiny', type: 'error', onClick: () => doAction('标记异常', () => markOrderAbnormal(id)) }, { default: () => '异常' }));
      }
      if (['canceled', 'abnormal', 'failed', 'pending_payment'].includes(status)) {
        btns.push(h(NButton, { size: 'tiny', type: 'error', onClick: () => doAction('软删除', () => deleteOrder(id)) }, { default: () => '删除' }));
      }
      return h(NSpace, { size: 4 }, { default: () => btns });
    }
  }
];

function onPageChange(p: number) {
  pagination.value.page = p;
  load();
}
function onPageSizeChange(s: number) {
  pagination.value.pageSize = s;
  pagination.value.page = 1;
  load();
}

onMounted(load);
</script>

<template>
  <NCard title="订单列表" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton @click="doExport">导出 CSV</NButton>
      </NSpace>
    </template>
    <NForm inline label-placement="left" :show-feedback="false" style="margin-bottom:12px">
      <NFormItem label="订单号">
        <NInput v-model:value="filters.order_no" placeholder="订单号关键字" clearable />
      </NFormItem>
      <NFormItem label="邮箱">
        <NInput v-model:value="filters.email" placeholder="客户邮箱关键字" clearable />
      </NFormItem>
      <NFormItem label="状态">
        <NSelect v-model:value="filters.status" :options="statusOptions" style="width:140px" />
      </NFormItem>
      <NFormItem label="起始">
        <NInput v-model:value="filters.date_from" placeholder="2025-01-01" />
      </NFormItem>
      <NFormItem label="截止">
        <NInput v-model:value="filters.date_to" placeholder="2025-12-31" />
      </NFormItem>
      <NFormItem :show-label="false">
        <NSpace>
          <NButton type="primary" @click="applyFilters">查询</NButton>
          <NButton @click="resetFilters">重置</NButton>
        </NSpace>
      </NFormItem>
    </NForm>
    <NDataTable
      :columns="columns"
      :data="rows"
      :loading="loading"
      :pagination="false"
      :scroll-x="1500"
      striped
    />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination
        v-model:page="pagination.page"
        v-model:page-size="pagination.pageSize"
        :item-count="pagination.itemCount"
        :show-size-picker="true"
        :page-sizes="[10, 20, 50, 100]"
        @update:page="onPageChange"
        @update:page-size="onPageSizeChange"
      />
    </div>
  </NCard>
</template>
