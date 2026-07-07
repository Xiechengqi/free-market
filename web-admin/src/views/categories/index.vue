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
  NSpace,
  NSwitch,
  useDialog,
  useMessage
} from 'naive-ui';
import { createCategory, deleteCategory, fetchCategories, updateCategory } from '@/service/api';

const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 50, itemCount: 0 });
const showModal = ref(false);
const editing = ref<any>(null);
const form = ref<any>({});

async function load() {
  loading.value = true;
  const { data, error } = await fetchCategories({
    current: pagination.value.page,
    size: pagination.value.pageSize
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.categories || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

function startCreate() {
  editing.value = null;
  form.value = { name: '', sort_order: 100, is_active: true };
  showModal.value = true;
}

function startEdit(r: any) {
  editing.value = r;
  form.value = { ...r };
  showModal.value = true;
}

async function submit() {
  const action = editing.value ? updateCategory(editing.value.id, form.value) : createCategory(form.value);
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
    title: '删除分类',
    content: `确认删除「${r.name}」？`,
    positiveText: '删除',
    onPositiveClick: async () => {
      const { error } = await deleteCategory(r.id);
      if (error) message.error(error.message || '删除失败');
      else {
        message.success('已删除');
        load();
      }
    }
  });
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '分类名', key: 'name' },
  { title: '排序', key: 'sort_order', width: 100 },
  {
    title: '状态',
    key: 'is_active',
    width: 80,
    render: (r: any) => (r.is_active ? '启用' : '禁用')
  },
  {
    title: '操作',
    key: 'actions',
    width: 160,
    render: (r: any) =>
      h(NSpace, { size: 4 }, {
        default: () => [
          h(NButton, { size: 'tiny', onClick: () => startEdit(r) }, { default: () => '编辑' }),
          h(NButton, { size: 'tiny', type: 'error', onClick: () => doDelete(r) }, { default: () => '删除' })
        ]
      })
  }
];

onMounted(load);
</script>

<template>
  <NCard title="商品分类" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton type="primary" @click="startCreate">新建分类</NButton>
      </NSpace>
    </template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
    <NModal v-model:show="showModal" preset="card" :title="editing ? '编辑分类' : '新建分类'" style="max-width:480px">
      <NForm label-placement="left" label-width="100">
        <NFormItem label="名称"><NInput v-model:value="form.name" /></NFormItem>
        <NFormItem label="排序"><NInputNumber v-model:value="form.sort_order" /></NFormItem>
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
