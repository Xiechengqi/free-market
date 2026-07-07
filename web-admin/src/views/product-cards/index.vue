<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import {
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NModal,
  NPagination,
  NSelect,
  NSpace,
  NTag,
  useDialog,
  useMessage
} from 'naive-ui';
import { h } from 'vue';
import {
  deleteProductCard,
  downloadProductCardsCsv,
  fetchProductCards,
  importProductCards
} from '@/service/api';

const route = useRoute();
const router = useRouter();
const message = useMessage();
const dialog = useDialog();

const productId = computed(() => Number(route.params.id));
const loading = ref(false);
const rows = ref<any[]>([]);
const productName = ref('');
const pagination = ref({ page: 1, pageSize: 50, itemCount: 0 });
const filters = reactive({ status: '' as string, is_loop: '' as string });

const importModal = ref(false);
const importing = ref(false);
const importForm = reactive({
  secrets: '',
  remove_duplication: true,
  is_loop: false
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

async function load() {
  if (!productId.value) return;
  loading.value = true;
  const { data, error } = await fetchProductCards(productId.value, {
    current: pagination.value.page,
    size: pagination.value.pageSize,
    status: filters.status,
    is_loop: filters.is_loop
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.cards || [];
    productName.value = data.product_name || '';
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

async function doDelete(r: any) {
  dialog.warning({
    title: '删除卡密',
    content: `确认删除该卡密？`,
    positiveText: '删除',
    negativeText: '取消',
    onPositiveClick: async () => {
      const { error } = await deleteProductCard(productId.value, r.id);
      if (error) message.error(error.message || '删除失败');
      else {
        message.success('已删除');
        load();
      }
    }
  });
}

async function doImport() {
  if (!importForm.secrets.trim()) {
    message.warning('请输入卡密内容');
    return;
  }
  importing.value = true;
  const { error } = await importProductCards(productId.value, {
    secrets: importForm.secrets,
    remove_duplication: importForm.remove_duplication ? '1' : undefined,
    is_loop: importForm.is_loop ? '1' : undefined
  });
  importing.value = false;
  if (error) {
    message.error(error.message || '导入失败');
  } else {
    message.success('导入成功');
    importModal.value = false;
    importForm.secrets = '';
    load();
  }
}

async function doExport() {
  try {
    await downloadProductCardsCsv(productId.value, {
      status: filters.status,
      is_loop: filters.is_loop
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
  {
    title: '循环使用',
    key: 'is_loop',
    width: 90,
    render: (r: any) => (r.is_loop ? '是' : '否')
  },
  { title: '订单 ID', key: 'order_id', width: 100, render: (r: any) => r.order_id ?? '-' },
  { title: '创建时间', key: 'created_at', width: 170 },
  {
    title: '操作',
    key: 'actions',
    width: 100,
    render: (r: any) => {
      if (r.status !== 'available') return '-';
      return h(NButton, { size: 'tiny', type: 'error', onClick: () => doDelete(r) }, { default: () => '删除' });
    }
  }
];

function applyFilters() {
  pagination.value.page = 1;
  load();
}
function resetFilters() {
  filters.status = '';
  filters.is_loop = '';
  pagination.value.page = 1;
  load();
}
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
  <NCard :bordered="false">
    <template #header>
      <NSpace align="center">
        <NButton size="small" @click="router.push({ name: 'products' })">‹ 返回商品列表</NButton>
        <span>{{ productName || '商品' }} · 卡密管理</span>
      </NSpace>
    </template>
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton @click="doExport">导出 .txt</NButton>
        <NButton type="primary" @click="importModal = true">导入卡密</NButton>
      </NSpace>
    </template>
    <NForm inline label-placement="left" :show-feedback="false" style="margin-bottom:12px">
      <NFormItem label="状态">
        <NSelect v-model:value="filters.status" :options="statusOptions" style="width:140px" />
      </NFormItem>
      <NFormItem label="循环">
        <NSelect v-model:value="filters.is_loop" :options="loopOptions" style="width:120px" />
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

    <NModal v-model:show="importModal" preset="card" title="批量导入卡密" style="max-width:640px">
      <NForm label-placement="top">
        <NFormItem label="卡密内容（每行一条）">
          <NInput
            v-model:value="importForm.secrets"
            type="textarea"
            :rows="12"
            placeholder="卡密1\n卡密2\n卡密3"
          />
        </NFormItem>
        <NFormItem :show-label="false">
          <NSpace>
            <NCheckbox v-model:checked="importForm.remove_duplication">导入时去重</NCheckbox>
            <NCheckbox v-model:checked="importForm.is_loop">循环使用</NCheckbox>
          </NSpace>
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton @click="importModal = false">取消</NButton>
          <NButton type="primary" :loading="importing" @click="doImport">导入</NButton>
        </NSpace>
      </template>
    </NModal>
  </NCard>
</template>
