<script setup lang="ts">
import { h, onMounted, reactive, ref } from 'vue';
import {
  NButton,
  NCard,
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
  deleteGlobalCard,
  downloadGlobalCardsCsv,
  fetchGlobalCards,
  fetchProducts
} from '@/service/api';

const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const rows = ref<any[]>([]);
const products = ref<Array<{ label: string; value: number | '' }>>([{ label: '全部商品', value: '' as any }]);
const pagination = ref({ page: 1, pageSize: 50, itemCount: 0 });
const filters = reactive({
  product_id: '' as number | '',
  status: '' as string,
  is_loop: '' as string,
  keyword: ''
});

const statusOptions = [
  { label: '全部', value: '' },
  { label: '可用', value: 'available' },
  { label: '已售出', value: 'sold' }
];
const loopOptions = [
  { label: '全部', value: '' },
  { label: '是', value: '1' },
  { label: '否', value: '0' }
];

async function loadProducts() {
  const { data } = await fetchProducts({ current: 1, size: 200 });
  const list: Array<{ label: string; value: number | '' }> = [{ label: '全部商品', value: '' as any }];
  (data?.products || []).forEach((p: any) => list.push({ label: p.name, value: p.id }));
  products.value = list;
}

async function load() {
  loading.value = true;
  const { data, error } = await fetchGlobalCards({
    current: pagination.value.page,
    size: pagination.value.pageSize,
    product_id: filters.product_id || undefined,
    status: filters.status || undefined,
    is_loop: filters.is_loop || undefined,
    keyword: filters.keyword || undefined
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.cards || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

function doDelete(r: any) {
  dialog.warning({
    title: '删除卡密',
    content: `确认删除该卡密？`,
    positiveText: '删除',
    onPositiveClick: async () => {
      const { error } = await deleteGlobalCard(r.id);
      if (error) message.error(error.message || '删除失败');
      else { message.success('已删除'); load(); }
    }
  });
}

async function doExport() {
  try {
    await downloadGlobalCardsCsv({
      product_id: filters.product_id || undefined,
      status: filters.status || undefined,
      is_loop: filters.is_loop || undefined,
      keyword: filters.keyword || undefined
    });
  } catch (e: any) {
    message.error(e.message || '导出失败');
  }
}

const statusType: Record<string, string> = {
  available: 'success',
  sold: 'default',
  reserved: 'info'
};

const columns = [
  { title: 'ID', key: 'id', width: 70 },
  { title: '商品', key: 'product_name', width: 200 },
  {
    title: '卡密',
    key: 'secret',
    render: (r: any) => h('code', { style: 'word-break:break-all;font-size:12px' }, r.secret)
  },
  {
    title: '状态',
    key: 'status',
    width: 100,
    render: (r: any) =>
      h(NTag, { type: (statusType[r.status] as any) || 'default', size: 'small' }, { default: () => r.status })
  },
  { title: '循环', key: 'is_loop', width: 70, render: (r: any) => (r.is_loop ? '是' : '否') },
  { title: '订单 ID', key: 'order_id', width: 90, render: (r: any) => r.order_id ?? '-' },
  { title: '创建时间', key: 'created_at', width: 170 },
  {
    title: '操作',
    key: 'actions',
    width: 80,
    render: (r: any) => {
      if (r.status !== 'available') return '-';
      return h(NButton, { size: 'tiny', type: 'error', onClick: () => doDelete(r) }, { default: () => '删除' });
    }
  }
];

function applyFilters() { pagination.value.page = 1; load(); }
function resetFilters() {
  filters.product_id = '' as any;
  filters.status = '';
  filters.is_loop = '';
  filters.keyword = '';
  pagination.value.page = 1;
  load();
}
function onPageChange(p: number) { pagination.value.page = p; load(); }
function onPageSizeChange(s: number) { pagination.value.pageSize = s; pagination.value.page = 1; load(); }

onMounted(async () => {
  await loadProducts();
  await load();
});
</script>

<template>
  <NCard title="全局卡密" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton @click="doExport">导出 .txt</NButton>
      </NSpace>
    </template>
    <NForm inline label-placement="left" :show-feedback="false" style="margin-bottom:12px">
      <NFormItem label="商品">
        <NSelect v-model:value="filters.product_id" :options="products as any" style="min-width:180px" filterable />
      </NFormItem>
      <NFormItem label="状态">
        <NSelect v-model:value="filters.status" :options="statusOptions" style="width:120px" />
      </NFormItem>
      <NFormItem label="循环">
        <NSelect v-model:value="filters.is_loop" :options="loopOptions" style="width:100px" />
      </NFormItem>
      <NFormItem label="关键字">
        <NInput v-model:value="filters.keyword" placeholder="卡密内容" clearable />
      </NFormItem>
      <NFormItem :show-label="false">
        <NSpace>
          <NButton type="primary" @click="applyFilters">查询</NButton>
          <NButton @click="resetFilters">重置</NButton>
        </NSpace>
      </NFormItem>
    </NForm>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination
        v-model:page="pagination.page"
        v-model:page-size="pagination.pageSize"
        :item-count="pagination.itemCount"
        :show-size-picker="true"
        :page-sizes="[20, 50, 100, 200]"
        @update:page="onPageChange"
        @update:page-size="onPageSizeChange"
      />
    </div>
  </NCard>
</template>
