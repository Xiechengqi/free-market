<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { NButton, NCard, NDataTable, NPagination, useMessage } from 'naive-ui';
import { fetchAuditLogs } from '@/service/api';

const message = useMessage();
const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 50, itemCount: 0 });

async function load() {
  loading.value = true;
  const { data, error } = await fetchAuditLogs({ current: pagination.value.page, size: pagination.value.pageSize });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.logs || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: 'Admin', key: 'admin_id', width: 80 },
  { title: 'Method', key: 'method', width: 100 },
  { title: 'Path', key: 'path' },
  { title: 'Action', key: 'action', width: 200 },
  { title: '时间', key: 'created_at', width: 200 }
];

onMounted(load);
</script>

<template>
  <NCard title="操作审计日志" :bordered="false">
    <template #header-extra><NButton @click="load" :loading="loading">刷新</NButton></template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
  </NCard>
</template>
