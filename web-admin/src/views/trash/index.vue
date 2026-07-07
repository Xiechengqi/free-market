<script setup lang="ts">
import { h, onMounted, ref } from 'vue';
import { NButton, NCard, NDataTable, NPagination, NTag, useDialog, useMessage } from 'naive-ui';
import { fetchTrash, restoreTrash } from '@/service/api';

const message = useMessage();
const dialog = useDialog();
const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 50, itemCount: 0 });

async function load() {
  loading.value = true;
  const { data, error } = await fetchTrash({ current: pagination.value.page, size: pagination.value.pageSize });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.rows || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

function doRestore(table: string, id: number) {
  dialog.warning({
    title: '恢复',
    content: `从 ${table} 表恢复 id=${id}？`,
    positiveText: '恢复',
    onPositiveClick: async () => {
      const { error } = await restoreTrash(table, id);
      if (error) message.error(error.message || '恢复失败');
      else { message.success('已恢复'); load(); }
    }
  });
}

const columns = [
  { title: '类型', key: 'table_name', width: 160, render: (r: any) => h(NTag, { size: 'small' }, { default: () => r.table_name }) },
  { title: 'ID', key: 'id', width: 80 },
  { title: '标题', key: 'title' },
  { title: '删除时间', key: 'deleted_at', width: 200 },
  {
    title: '操作', key: 'actions', width: 100,
    render: (r: any) => h(NButton, { size: 'tiny', type: 'primary', onClick: () => doRestore(r.table_name, r.id) }, { default: () => '恢复' })
  }
];

onMounted(load);
</script>

<template>
  <NCard title="回收站" :bordered="false">
    <template #header-extra><NButton @click="load" :loading="loading">刷新</NButton></template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
  </NCard>
</template>
