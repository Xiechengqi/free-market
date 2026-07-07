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
  NSelect,
  NSpace,
  NSwitch,
  useMessage
} from 'naive-ui';
import { createAdmin, fetchAdmins, updateAdmin } from '@/service/api';

const message = useMessage();
const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 20, itemCount: 0 });
const showModal = ref(false);
const editing = ref<any>(null);
const form = ref<any>({});

async function load() {
  loading.value = true;
  const { data, error } = await fetchAdmins({ current: pagination.value.page, size: pagination.value.pageSize });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.admins || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}
function startCreate() {
  editing.value = null;
  form.value = { username: '', display_name: '', password: '', role: 'operator', is_active: true };
  showModal.value = true;
}
function startEdit(r: any) {
  editing.value = r;
  form.value = { ...r, password: '' };
  showModal.value = true;
}
async function submit() {
  const action = editing.value ? updateAdmin(editing.value.id, form.value) : createAdmin(form.value);
  const { error } = await action;
  if (error) message.error(error.message || '保存失败');
  else { message.success('保存成功'); showModal.value = false; load(); }
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '用户名', key: 'username' },
  { title: '显示名', key: 'display_name' },
  { title: '角色', key: 'role', width: 100 },
  { title: '启用', key: 'is_active', width: 80, render: (r: any) => (r.is_active ? '是' : '否') },
  { title: '创建时间', key: 'created_at', width: 180 },
  {
    title: '操作',
    key: 'actions',
    width: 100,
    render: (r: any) => h(NButton, { size: 'tiny', onClick: () => startEdit(r) }, { default: () => '编辑' })
  }
];

onMounted(load);
</script>

<template>
  <NCard title="管理员" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton type="primary" @click="startCreate">新建管理员</NButton>
      </NSpace>
    </template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
    <NModal v-model:show="showModal" preset="card" :title="editing ? '编辑管理员' : '新建管理员'" style="max-width:560px">
      <NForm label-placement="left" label-width="100">
        <NFormItem label="用户名"><NInput v-model:value="form.username" :disabled="!!editing" /></NFormItem>
        <NFormItem label="显示名"><NInput v-model:value="form.display_name" /></NFormItem>
        <NFormItem :label="editing ? '新密码（留空不变）' : '密码'">
          <NInput v-model:value="form.password" type="password" show-password-on="click" />
        </NFormItem>
        <NFormItem label="角色">
          <NSelect v-model:value="form.role" :options="[
            { label: 'owner（最高权限）', value: 'owner' },
            { label: 'operator（运营，无管理员/系统设置）', value: 'operator' },
            { label: 'viewer（只读）', value: 'viewer' }
          ]" />
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
