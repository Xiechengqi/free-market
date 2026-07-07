<script setup lang="ts">
import { h, onMounted, ref } from 'vue';
import {
  NButton,
  NDataTable,
  NForm,
  NFormItem,
  NInput,
  NModal,
  NPagination,
  NSpace,
  useDialog,
  useMessage
} from 'naive-ui';
import {
  createEmailTemplate,
  deleteEmailTemplate,
  fetchEmailTemplates,
  restoreDefaultEmailTemplates,
  updateEmailTemplate
} from '@/service/api';

const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 20, itemCount: 0 });
const showModal = ref(false);
const editing = ref<any>(null);
const form = ref<any>({});

async function load() {
  loading.value = true;
  const { data, error } = await fetchEmailTemplates({
    current: pagination.value.page,
    size: pagination.value.pageSize
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.templates || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}
function startCreate() {
  editing.value = null;
  form.value = { token: '', subject: '', content: '' };
  showModal.value = true;
}
function startEdit(r: any) {
  editing.value = r;
  form.value = { ...r };
  showModal.value = true;
}
async function submit() {
  const action = editing.value ? updateEmailTemplate(editing.value.id, form.value) : createEmailTemplate(form.value);
  const { error } = await action;
  if (error) message.error(error.message || '保存失败');
  else { message.success('保存成功'); showModal.value = false; load(); }
}
function doDelete(r: any) {
  if (r.is_system) {
    message.warning('系统模板不可删除');
    return;
  }
  dialog.warning({
    title: '删除模板',
    content: `确认删除 token = ${r.token}？`,
    onPositiveClick: async () => {
      const { error } = await deleteEmailTemplate(r.id);
      if (error) message.error(error.message || '删除失败');
      else { message.success('已删除'); load(); }
    }
  });
}
async function restoreDefaults() {
  const { error } = await restoreDefaultEmailTemplates();
  if (error) message.error(error.message || '恢复失败');
  else { message.success('已恢复默认模板'); load(); }
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: 'Token', key: 'token', width: 220 },
  { title: '标题', key: 'subject' },
  { title: '系统', key: 'is_system', width: 80, render: (r: any) => (r.is_system ? '是' : '否') },
  {
    title: '操作',
    key: 'actions',
    width: 160,
    render: (r: any) => h(NSpace, { size: 4 }, {
      default: () => [
        h(NButton, { size: 'tiny', onClick: () => startEdit(r) }, { default: () => '编辑' }),
        h(NButton, { size: 'tiny', type: 'error', disabled: r.is_system, onClick: () => doDelete(r) }, { default: () => '删除' })
      ]
    })
  }
];

onMounted(load);
</script>

<template>
  <NCard title="邮件模板" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton type="primary" @click="startCreate">新建模板</NButton>
        <NButton type="warning" @click="restoreDefaults">恢复默认模板</NButton>
      </NSpace>
    </template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
    <NModal v-model:show="showModal" preset="card" :title="editing ? '编辑模板' : '新建模板'" style="max-width:720px">
      <NForm label-placement="left" label-width="80">
        <NFormItem label="Token"><NInput v-model:value="form.token" /></NFormItem>
        <NFormItem label="标题"><NInput v-model:value="form.subject" /></NFormItem>
        <NFormItem label="内容"><NInput v-model:value="form.content" type="textarea" :rows="14" /></NFormItem>
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
