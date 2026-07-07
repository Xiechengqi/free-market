<script setup lang="ts">
import { h, onMounted, ref } from 'vue';
import {
  NButton,
  NCard,
  NDataTable,
  NImage,
  NPagination,
  NSpace,
  NUpload,
  useDialog,
  useMessage
} from 'naive-ui';
import type { UploadFileInfo } from 'naive-ui';
import { cleanupUploads, fetchUploads, uploadFile } from '@/service/api';

const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const uploading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 30, itemCount: 0 });

async function load() {
  loading.value = true;
  const { data, error } = await fetchUploads({
    current: pagination.value.page,
    size: pagination.value.pageSize
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.media || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

async function customRequest({ file, onFinish, onError }: { file: UploadFileInfo; onFinish: () => void; onError: () => void }) {
  if (!file.file) return onError();
  uploading.value = true;
  const { error } = await uploadFile(file.file);
  uploading.value = false;
  if (error) {
    message.error(error.message || '上传失败');
    onError();
    return;
  }
  message.success('上传成功');
  onFinish();
  load();
}

function doCleanup() {
  dialog.warning({
    title: '清理未引用上传',
    content: '将扫描 uploads 目录并清理数据库中不再引用的文件，确定继续？',
    positiveText: '清理',
    negativeText: '取消',
    onPositiveClick: async () => {
      const { data, error } = await cleanupUploads();
      if (error) message.error(error.message || '清理失败');
      else {
        message.success(`已清理 ${data?.removed ?? 0} 个文件`);
        load();
      }
    }
  });
}

function copyUrl(r: any) {
  const u = `/uploads/${r.path}`;
  navigator.clipboard?.writeText(u).then(
    () => message.success('已复制 URL'),
    () => message.warning(u)
  );
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  {
    title: '预览',
    key: 'preview',
    width: 100,
    render: (r: any) =>
      h(NImage, {
        src: `/uploads/${r.path}`,
        width: 60,
        height: 60,
        objectFit: 'cover',
        style: 'border-radius:4px'
      })
  },
  { title: '路径', key: 'path' },
  { title: 'MIME', key: 'mime', width: 140 },
  {
    title: '大小',
    key: 'size_bytes',
    width: 100,
    render: (r: any) => `${(r.size_bytes / 1024).toFixed(1)} KB`
  },
  { title: '创建时间', key: 'created_at', width: 170 },
  {
    title: '操作',
    key: 'actions',
    width: 120,
    render: (r: any) =>
      h(NButton, { size: 'tiny', onClick: () => copyUrl(r) }, { default: () => '复制 URL' })
  }
];

function onPageChange(p: number) { pagination.value.page = p; load(); }

onMounted(load);
</script>

<template>
  <NCard title="上传文件" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NUpload :show-file-list="false" accept="image/*" :custom-request="customRequest as any">
          <NButton type="primary" :loading="uploading">上传图片</NButton>
        </NUpload>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton @click="doCleanup">清理未引用</NButton>
      </NSpace>
    </template>
    <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    <div style="margin-top:16px;display:flex;justify-content:flex-end">
      <NPagination v-model:page="pagination.page" :item-count="pagination.itemCount" @update:page="onPageChange" />
    </div>
  </NCard>
</template>
