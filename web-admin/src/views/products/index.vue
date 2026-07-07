<script setup lang="ts">
import { h, onMounted, reactive, ref } from 'vue';
import { useRouter } from 'vue-router';
import {
  NButton,
  NCard,
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
import {
  createProduct,
  deleteProduct,
  fetchCategories,
  fetchProducts,
  updateProduct
} from '@/service/api';
import UploadPicker from '@/components/custom/upload-picker.vue';

const router = useRouter();
const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 20, itemCount: 0 });
const showModal = ref(false);
const editing = ref<any>(null);
const form = reactive<any>({});
const categoryOptions = ref<Array<{ label: string; value: number }>>([]);

async function loadCategories() {
  const { data } = await fetchCategories({ current: 1, size: 200 });
  categoryOptions.value = (data?.categories || []).map((c: any) => ({ label: c.name, value: c.id }));
}

async function load() {
  loading.value = true;
  const { data, error } = await fetchProducts({
    current: pagination.value.page,
    size: pagination.value.pageSize
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.products || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

function startCreate() {
  editing.value = null;
  Object.assign(form, {
    name: '',
    short_description: '',
    description_html: '',
    keywords: '',
    image_path: '',
    category_id: categoryOptions.value[0]?.value || 0,
    price_cents: 0,
    retail_price_cents: 0,
    fulfillment_type: 'auto',
    is_active: true,
    sort_order: 100,
    buy_limit_num: 0,
    manual_stock_total: 0,
    buy_prompt: '',
    api_hook: '',
    wholesale_prices_json: ''
  });
  showModal.value = true;
}

function startEdit(r: any) {
  editing.value = r;
  Object.assign(form, {
    name: r.name || '',
    short_description: r.short_description || '',
    description_html: r.description_html || '',
    keywords: r.keywords || '',
    image_path: r.image_path || '',
    category_id: r.category_id || 0,
    price_cents: r.price_cents || 0,
    retail_price_cents: r.retail_price_cents || 0,
    fulfillment_type: r.fulfillment_type || 'auto',
    is_active: !!r.is_active,
    sort_order: r.sort_order || 100,
    buy_limit_num: r.buy_limit_num || 0,
    manual_stock_total: r.manual_stock_total || 0,
    buy_prompt: r.buy_prompt || '',
    api_hook: r.api_hook || '',
    wholesale_prices_json: r.wholesale_prices_json || ''
  });
  showModal.value = true;
}

async function submit() {
  // backend expects is_active as Option<String> (presence -> active)
  const body: any = { ...form };
  body.is_active = form.is_active ? '1' : undefined;
  const action = editing.value ? updateProduct(editing.value.id, body) : createProduct(body);
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
    title: '删除商品',
    content: `确认删除「${r.name}」？`,
    positiveText: '删除',
    negativeText: '取消',
    onPositiveClick: async () => {
      const { error } = await deleteProduct(r.id);
      if (error) message.error(error.message || '删除失败');
      else { message.success('已删除'); load(); }
    }
  });
}

function gotoCards(r: any) {
  router.push({ name: 'product-cards', params: { id: r.id } });
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '名称', key: 'name', minWidth: 200 },
  { title: '分类', key: 'category_name', width: 120 },
  {
    title: '价格',
    key: 'price_cents',
    width: 100,
    render: (r: any) => `￥${(r.price_cents / 100).toFixed(2)}`
  },
  {
    title: '类型',
    key: 'fulfillment_type',
    width: 100,
    render: (r: any) => (r.fulfillment_type === 'auto' ? '自动发卡' : '人工发货')
  },
  { title: '库存', key: 'stock', width: 80 },
  { title: '销量', key: 'sales_volume', width: 80 },
  {
    title: '状态',
    key: 'is_active',
    width: 80,
    render: (r: any) => (r.is_active ? '上架' : '下架')
  },
  {
    title: '操作',
    key: 'actions',
    width: 220,
    render: (r: any) =>
      h(NSpace, { size: 4 }, {
        default: () => [
          h(NButton, { size: 'tiny', onClick: () => startEdit(r) }, { default: () => '编辑' }),
          h(NButton, { size: 'tiny', type: 'info', onClick: () => gotoCards(r) }, { default: () => '卡密' }),
          h(NButton, { size: 'tiny', type: 'error', onClick: () => doDelete(r) }, { default: () => '删除' })
        ]
      })
  }
];

function onPageChange(p: number) { pagination.value.page = p; load(); }

onMounted(async () => {
  await loadCategories();
  await load();
});
</script>

<template>
  <NCard title="商品管理" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton type="primary" @click="startCreate">新建商品</NButton>
      </NSpace>
    </template>
    <NDataTable
      :columns="columns"
      :data="rows"
      :loading="loading"
      :pagination="false"
      striped
      :scroll-x="1200"
    />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination
        v-model:page="pagination.page"
        :item-count="pagination.itemCount"
        @update:page="onPageChange"
      />
    </div>
    <NModal
      v-model:show="showModal"
      preset="card"
      :title="editing ? '编辑商品' : '新建商品'"
      style="max-width:760px"
    >
      <NForm label-placement="left" label-width="110">
        <NFormItem label="商品名">
          <NInput v-model:value="form.name" />
        </NFormItem>
        <NFormItem label="分类">
          <NSelect v-model:value="form.category_id" :options="categoryOptions" filterable placeholder="选择分类" />
        </NFormItem>
        <NFormItem label="简介">
          <NInput v-model:value="form.short_description" type="textarea" :rows="2" />
        </NFormItem>
        <NFormItem label="详情">
          <NInput v-model:value="form.description_html" type="textarea" :rows="4" placeholder="HTML 或 Markdown" />
        </NFormItem>
        <NFormItem label="商品封面">
          <UploadPicker v-model="form.image_path" />
        </NFormItem>
        <NFormItem label="关键字">
          <NInput v-model:value="form.keywords" placeholder="逗号分隔" />
        </NFormItem>
        <NFormItem label="单价（分）">
          <NInputNumber v-model:value="form.price_cents" :min="0" />
        </NFormItem>
        <NFormItem label="零售价（分）">
          <NInputNumber v-model:value="form.retail_price_cents" :min="0" />
        </NFormItem>
        <NFormItem label="批发价 JSON">
          <NInput v-model:value="form.wholesale_prices_json" type="textarea" :rows="2" placeholder='[{"min":10,"price_cents":900}]' />
        </NFormItem>
        <NFormItem label="发货类型">
          <NSelect
            v-model:value="form.fulfillment_type"
            :options="[
              { label: '自动发卡', value: 'auto' },
              { label: '人工发货', value: 'manual' }
            ]"
          />
        </NFormItem>
        <NFormItem label="人工库存上限">
          <NInputNumber v-model:value="form.manual_stock_total" :min="0" />
        </NFormItem>
        <NFormItem label="单次限购">
          <NInputNumber v-model:value="form.buy_limit_num" :min="0" />
        </NFormItem>
        <NFormItem label="排序">
          <NInputNumber v-model:value="form.sort_order" />
        </NFormItem>
        <NFormItem label="购买说明">
          <NInput v-model:value="form.buy_prompt" type="textarea" :rows="2" />
        </NFormItem>
        <NFormItem label="上架">
          <NSwitch v-model:value="form.is_active" />
        </NFormItem>
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
