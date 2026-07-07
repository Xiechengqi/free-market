<script setup lang="ts">
import { h, onMounted, ref } from 'vue';
import {
  NButton,
  NDataTable,
  NPagination,
  NSpace,
  useDialog,
  useMessage
} from 'naive-ui';
import {
  deletePaymentChannel,
  fetchPaymentChannels
} from '@/service/api';
import ChannelForm from './components/channel-form.vue';
import PayCheckPreview from './components/pay-check-preview.vue';
import { channelTypeDisplay, findMethodByRow } from './method-specs';

defineOptions({ name: 'PaymentChannels' });

const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const rows = ref<any[]>([]);
const pagination = ref({ page: 1, pageSize: 20, itemCount: 0 });
const showModal = ref(false);
const editing = ref<any>(null);

async function load() {
  loading.value = true;
  const { data, error } = await fetchPaymentChannels({
    current: pagination.value.page,
    size: pagination.value.pageSize
  });
  if (error) message.error(error.message || '加载失败');
  else if (data) {
    rows.value = data.channels || [];
    pagination.value.itemCount = data.pagination?.total || 0;
  }
  loading.value = false;
}

function startCreate() {
  editing.value = null;
  showModal.value = true;
}
function startEdit(r: any) {
  editing.value = r;
  showModal.value = true;
}
function doDelete(r: any) {
  dialog.warning({
    title: '删除支付通道',
    content: `确认删除「${r.name}」？`,
    positiveText: '删除',
    onPositiveClick: async () => {
      const { error } = await deletePaymentChannel(r.id);
      if (error) message.error(error.message || '删除失败');
      else {
        message.success('已删除');
        load();
      }
    }
  });
}

function methodLabel(r: any) {
  const spec = findMethodByRow(r.provider_type, r.channel_type);
  return spec ? spec.label : `${r.provider_type}:${r.channel_type}`;
}

const columns = [
  { title: 'ID', key: 'id', width: 60 },
  { title: '名称', key: 'name' },
  {
    title: '支付方式',
    key: 'method',
    width: 220,
    render: (r: any) => methodLabel(r)
  },
  { title: 'Provider', key: 'provider_type', width: 100 },
  {
    title: '渠道类型',
    key: 'channel_type',
    width: 190,
    render: (r: any) => channelTypeDisplay(r.provider_type, r.channel_type)
  },
  {
    title: '图标标识',
    key: 'pay_check',
    width: 190,
    render: (r: any) => h(PayCheckPreview, { value: r.pay_check, showDescription: true })
  },
  { title: '交互', key: 'interaction_mode', width: 90 },
  { title: '设备', key: 'client_scope', width: 80 },
  {
    title: '状态',
    key: 'is_active',
    width: 70,
    render: (r: any) => (r.is_active ? '启用' : '禁用')
  },
  {
    title: '操作',
    key: 'actions',
    width: 160,
    render: (r: any) =>
      h(
        NSpace,
        { size: 4 },
        {
          default: () => [
            h(
              NButton,
              { size: 'tiny', onClick: () => startEdit(r) },
              { default: () => '编辑' }
            ),
            h(
              NButton,
              { size: 'tiny', type: 'error', onClick: () => doDelete(r) },
              { default: () => '删除' }
            )
          ]
        }
      )
  }
];

onMounted(load);
</script>

<template>
  <NCard title="支付通道" :bordered="false">
    <template #header-extra>
      <NSpace>
        <NButton @click="load" :loading="loading">刷新</NButton>
        <NButton type="primary" @click="startCreate">新建通道</NButton>
      </NSpace>
    </template>
    <NDataTable
      :columns="columns"
      :data="rows"
      :loading="loading"
      :pagination="false"
      striped
    />
    <div style="margin-top: 16px; display: flex; justify-content: flex-end">
      <NPagination
        v-model:page="pagination.page"
        :item-count="pagination.itemCount"
        @update:page="load"
      />
    </div>
    <ChannelForm
      v-model:show="showModal"
      :editing="editing"
      @saved="load"
    />
  </NCard>
</template>
