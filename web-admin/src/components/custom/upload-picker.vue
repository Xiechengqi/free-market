<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NSpin, NUpload, type UploadFileInfo, useMessage } from 'naive-ui';
import { uploadFile } from '@/service/api/freemarket';

defineOptions({ name: 'UploadPicker' });

interface Props {
  modelValue: string;
  width?: number;
  height?: number;
}

const props = withDefaults(defineProps<Props>(), {
  width: 120,
  height: 120
});

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void;
}>();

const message = useMessage();
const uploading = ref(false);
const preview = computed(() => props.modelValue);

async function handleUpload({ file }: { file: UploadFileInfo }) {
  if (!file.file) {
    message.error('请选择文件');
    return false;
  }
  uploading.value = true;
  const { data, error } = await uploadFile(file.file);
  uploading.value = false;
  if (error) {
    message.error(error.message || '上传失败');
    return false;
  }
  if (data?.path) {
    emit('update:modelValue', data.path);
    message.success('上传成功');
  }
  return false;
}

function clear() {
  emit('update:modelValue', '');
}
</script>

<template>
  <div class="upload-picker">
    <div
      v-if="preview"
      class="upload-preview"
      :style="{ width: `${width}px`, height: `${height}px` }"
    >
      <img :src="preview" alt="preview" />
      <NButton size="tiny" tertiary class="upload-clear" @click="clear">×</NButton>
    </div>
    <NUpload
      :show-file-list="false"
      :on-change="handleUpload as any"
      :default-upload="false"
      accept="image/*"
    >
      <NSpin :show="uploading">
        <NButton size="small" type="primary" ghost>
          {{ preview ? '替换图片' : '上传图片' }}
        </NButton>
      </NSpin>
    </NUpload>
    <NButton v-if="preview" size="small" quaternary @click="clear">清除</NButton>
  </div>
</template>

<style scoped>
.upload-picker {
  display: flex;
  align-items: center;
  gap: 12px;
}
.upload-preview {
  position: relative;
  overflow: hidden;
  border: 1px dashed #ddd;
  border-radius: 6px;
}
.upload-preview img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.upload-clear {
  position: absolute;
  top: 2px;
  right: 2px;
}
</style>
