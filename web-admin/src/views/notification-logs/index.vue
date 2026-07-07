<script setup lang="ts">
import { h, onMounted, ref } from 'vue';
import { NButton, NCard, NDataTable, NPagination, NSpace, NTag, useMessage } from 'naive-ui';
import { fetchNotificationLogs } from '@/service/api';

const message = useMessage();
const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 50, itemCount: 0 });

async function load() {
  loading.value = true;
  const { data, error } = await fetchNotificationLogs({ current: pagination.value.page, size: pagination.value.pageSize });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.logs || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: 'Kind', key: 'kind', width: 200 },
  { title: 'Target', key: 'target' },
  {
    title: '状态', key: 'status', width: 100,
    render: (r: any) => h(NTag, { type: r.status === 'sent' ? 'success' : (r.status === 'failed' ? 'error' : 'default'), size: 'small' }, { default: () => r.status })
  },
  { title: '错误', key: 'error', ellipsis: true },
  { title: '时间', key: 'created_at', width: 200 }
];

onMounted(load);
</script>

<template>
  <NCard title="通知日志" :bordered="false">
    <template #header-extra><NButton @click="load" :loading="loading">刷新</NButton></template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
  </NCard>
</template>
