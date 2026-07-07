<script setup lang="ts">
import { h, onMounted, ref } from 'vue';
import { NButton, NCard, NDataTable, NPagination, NSpace, NTag, useDialog, useMessage } from 'naive-ui';
import { cleanupRuntime, fetchJobs, retryJob } from '@/service/api';

const message = useMessage();
const dialog = useDialog();
const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 20, itemCount: 0 });

const statusColor: Record<string, string> = {
  pending: 'warning',
  running: 'info',
  succeeded: 'success',
  dead: 'error',
  failed: 'error'
};

async function load() {
  loading.value = true;
  const { data, error } = await fetchJobs({ current: pagination.value.page, size: pagination.value.pageSize });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.jobs || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

async function doRetry(id: number) {
  const { error } = await retryJob(id);
  if (error) message.error(error.message || '重试失败');
  else { message.success('已重试'); load(); }
}

function doCleanup() {
  dialog.warning({
    title: '清理',
    content: '将删除：30 天前已完成/失败任务、过期 session/captcha、30/90 天前的登录/审计/通知日志。',
    positiveText: '执行',
    onPositiveClick: async () => {
      const { error } = await cleanupRuntime();
      if (error) message.error(error.message || '清理失败');
      else { message.success('清理完成'); load(); }
    }
  });
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: 'Kind', key: 'kind', width: 200 },
  { title: '状态', key: 'status', width: 100, render: (r: any) => h(NTag, { type: (statusColor[r.status] as any) || 'default', size: 'small' }, { default: () => r.status }) },
  { title: '尝试', key: 'attempts', width: 80, render: (r: any) => `${r.attempts}/${r.max_attempts}` },
  { title: '运行时间', key: 'run_at', width: 200 },
  { title: '最后错误', key: 'last_error', ellipsis: true },
  {
    title: '操作', key: 'actions', width: 100,
    render: (r: any) => r.status === 'dead' || r.status === 'failed'
      ? h(NButton, { size: 'tiny', onClick: () => doRetry(r.id) }, { default: () => '重试' })
      : null
  }
];

onMounted(load);
</script>

<template>
  <NCard title="任务队列" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton type="warning" @click="doCleanup">清理过期记录</NButton>
      </NSpace>
    </template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="load" />
    </div>
  </NCard>
</template>
