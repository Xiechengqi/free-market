<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import {
  NButton,
  NCard,
  NDataTable,
  NForm,
  NFormItem,
  NInputNumber,
  NSelect,
  NSpace,
  NSwitch,
  useDialog,
  useMessage
} from 'naive-ui';
import {
  createBackup,
  downloadBackupFile,
  fetchBackup,
  saveBackupSettings
} from '@/service/api';
import { h } from 'vue';

const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const creating = ref(false);
const saving = ref(false);
const rows = ref<any[]>([]);
const cfg = ref({ enabled: false, weekday: 1, hour: 3, keep_files: 7 });

const weekdayOptions = [
  { label: '周一', value: 1 },
  { label: '周二', value: 2 },
  { label: '周三', value: 3 },
  { label: '周四', value: 4 },
  { label: '周五', value: 5 },
  { label: '周六', value: 6 },
  { label: '周日', value: 7 }
];

async function load() {
  loading.value = true;
  const { data, error } = await fetchBackup();
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.files || [];
    cfg.value = {
      enabled: !!data.enabled,
      weekday: data.weekday,
      hour: data.hour,
      keep_files: data.keep_files
    };
  }
  loading.value = false;
}

async function doCreate() {
  creating.value = true;
  const { data, error } = await createBackup();
  creating.value = false;
  if (error) message.error(error.message || '创建失败');
  else {
    message.success(`已创建：${data?.filename || ''}`);
    load();
  }
}

async function doSave() {
  saving.value = true;
  const { error } = await saveBackupSettings({ ...cfg.value });
  saving.value = false;
  if (error) message.error(error.message || '保存失败');
  else message.success('已保存');
}

async function doDownload(r: any) {
  try {
    await downloadBackupFile(r.filename);
  } catch (e: any) {
    message.error(e.message || '下载失败');
  }
}

const columns = computed(() => [
  { title: '文件名', key: 'filename' },
  {
    title: '大小',
    key: 'size_bytes',
    width: 120,
    render: (r: any) => `${(r.size_bytes / 1024).toFixed(1)} KB`
  },
  { title: '创建时间', key: 'created_at', width: 200 },
  {
    title: '操作',
    key: 'actions',
    width: 120,
    render: (r: any) => h(NButton, { size: 'tiny', onClick: () => doDownload(r) }, { default: () => '下载' })
  }
]);

onMounted(load);
</script>

<template>
  <NSpace vertical :size="16">
    <NCard title="备份计划" :bordered="false">
      <NForm label-placement="left" label-width="120">
        <NFormItem label="启用定时备份">
          <NSwitch v-model:value="cfg.enabled" />
        </NFormItem>
        <NFormItem label="星期">
          <NSelect v-model:value="cfg.weekday" :options="weekdayOptions" style="width:160px" />
        </NFormItem>
        <NFormItem label="小时（0-23）">
          <NInputNumber v-model:value="cfg.hour" :min="0" :max="23" />
        </NFormItem>
        <NFormItem label="保留份数">
          <NInputNumber v-model:value="cfg.keep_files" :min="1" :max="30" />
        </NFormItem>
        <NFormItem :show-label="false">
          <NSpace>
            <NButton type="primary" :loading="saving" @click="doSave">保存</NButton>
          </NSpace>
        </NFormItem>
      </NForm>
    </NCard>

    <NCard title="备份文件" :bordered="false">
      <template #header-extra>
        <NSpace>
          <NButton @click="load" :loading="loading">刷新</NButton>
          <NButton type="primary" :loading="creating" @click="doCreate">立即备份</NButton>
        </NSpace>
      </template>
      <NDataTable :columns="columns" :data="rows" :loading="loading" :pagination="false" striped />
    </NCard>
  </NSpace>
</template>
