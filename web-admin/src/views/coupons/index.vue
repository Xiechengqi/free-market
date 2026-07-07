<script setup lang="ts">
import { h, onMounted, ref } from 'vue';
import {
  NButton,
  NDataTable,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NModal,
  NPagination,
  NSelect,
  NSpace,
  NSwitch,
  useDialog,
  useMessage
} from 'naive-ui';
import { createCoupon, deleteCoupon, fetchCoupons, fetchProducts, updateCoupon } from '@/service/api';

const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 20, itemCount: 0 });
const showModal = ref(false);
const editing = ref<any>(null);
const form = ref<any>({});
const productOptions = ref<Array<{ label: string; value: number }>>([]);

async function loadProducts() {
  const { data } = await fetchProducts({ current: 1, size: 200 });
  productOptions.value = (data?.products || []).map((p: any) => ({ label: p.name, value: p.id }));
}

async function load() {
  loading.value = true;
  const { data, error } = await fetchCoupons({
    current: pagination.value.page,
    size: pagination.value.pageSize
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.coupons || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

function startCreate() {
  editing.value = null;
  form.value = {
    code: '',
    value_cents: 0,
    min_amount_cents: 0,
    usage_limit: 1,
    is_active: true,
    product_ids: [] as number[]
  };
  showModal.value = true;
}
function startEdit(r: any) {
  editing.value = r;
  form.value = {
    ...r,
    is_active: !!r.is_active,
    product_ids: r.product_ids || []
  };
  showModal.value = true;
}
async function submit() {
  // backend's CouponForm.is_active is Option<String> — convert.
  const body: any = { ...form.value };
  body.is_active = form.value.is_active ? '1' : undefined;
  const action = editing.value ? updateCoupon(editing.value.id, body) : createCoupon(body);
  const { error } = await action;
  if (error) message.error(error.message || '保存失败');
  else {
    message.success('保存成功');
    showModal.value = false;
    load();
  }
}
function doDelete(r: any) {
  dialog.warning({
    title: '删除优惠码',
    content: `确认删除「${r.code}」？`,
    positiveText: '删除',
    onPositiveClick: async () => {
      const { error } = await deleteCoupon(r.id);
      if (error) message.error(error.message || '删除失败');
      else { message.success('已删除'); load(); }
    }
  });
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '优惠码', key: 'code' },
  { title: '类型', key: 'type', width: 80 },
  { title: '面值', key: 'value_display', width: 100, render: (r: any) => `￥${r.value_display || '-'}` },
  { title: '门槛', key: 'min_amount_display', width: 100, render: (r: any) => `￥${r.min_amount_display || '-'}` },
  { title: '剩余', key: 'usage_limit', width: 100, render: (r: any) => `${r.usage_limit - r.used_count}/${r.usage_limit}` },
  { title: '适用商品', key: 'product_scope', minWidth: 160 },
  { title: '状态', key: 'is_active', width: 80, render: (r: any) => (r.is_active ? '启用' : '禁用') },
  {
    title: '操作',
    key: 'actions',
    width: 160,
    render: (r: any) => h(NSpace, { size: 4 }, {
      default: () => [
        h(NButton, { size: 'tiny', onClick: () => startEdit(r) }, { default: () => '编辑' }),
        h(NButton, { size: 'tiny', type: 'error', onClick: () => doDelete(r) }, { default: () => '删除' })
      ]
    })
  }
];

onMounted(async () => {
  await loadProducts();
  await load();
});
</script>

<template>
  <NCard title="优惠码管理" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton type="primary" @click="startCreate">新建优惠码</NButton>
      </NSpace>
    </template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
    <NModal v-model:show="showModal" preset="card" :title="editing ? '编辑优惠码' : '新建优惠码'" style="max-width:560px">
      <NForm label-placement="left" label-width="100">
        <NFormItem label="优惠码"><NInput v-model:value="form.code" /></NFormItem>
        <NFormItem label="面值（分）"><NInputNumber v-model:value="form.value_cents" :min="0" /></NFormItem>
        <NFormItem label="门槛（分）"><NInputNumber v-model:value="form.min_amount_cents" :min="0" /></NFormItem>
        <NFormItem label="可用次数"><NInputNumber v-model:value="form.usage_limit" :min="0" /></NFormItem>
        <NFormItem label="适用商品">
          <NSelect
            v-model:value="form.product_ids"
            multiple
            filterable
            clearable
            :options="productOptions"
            placeholder="留空 = 适用全部商品"
          />
        </NFormItem>
        <NFormItem label="启用"><NSwitch v-model:value="form.is_active" /></NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton @click="showModal = false">取消</NButton>
          <NButton type="primary" @click="submit">保存</NButton>
        </NSpace>
      </template>
    </NModal>
  </NCard>
</template>
