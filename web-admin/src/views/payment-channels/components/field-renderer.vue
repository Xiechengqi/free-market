<script setup lang="ts">
import { computed } from 'vue';
import { NInput, NSelect, NSwitch } from 'naive-ui';
import type { FieldSpec } from '../method-specs';
import { SECRET_MASK } from '../method-specs';

defineOptions({ name: 'PaymentFieldRenderer' });

interface Props {
  field: FieldSpec;
  modelValue: any;
  touched?: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: 'update:modelValue', value: any): void;
  (e: 'touch'): void;
}>();

const placeholder = computed(() => {
  if (props.field.placeholder) return props.field.placeholder;
  if (
    (props.field.type === 'password' || props.field.type === 'pem') &&
    props.modelValue === SECRET_MASK
  ) {
    return '保留原密钥不变（直接保存）；如需修改请清空后重填';
  }
  return '';
});

function handleInput(value: any) {
  emit('update:modelValue', value);
  emit('touch');
}

function handleFocus() {
  if (
    (props.field.type === 'password' || props.field.type === 'pem') &&
    props.modelValue === SECRET_MASK
  ) {
    emit('update:modelValue', '');
    emit('touch');
  }
}
</script>

<template>
  <NInput
    v-if="field.type === 'text'"
    :value="modelValue"
    :placeholder="placeholder"
    @update:value="handleInput"
  />
  <NInput
    v-else-if="field.type === 'password'"
    type="password"
    show-password-on="click"
    :value="modelValue"
    :placeholder="placeholder"
    @focus="handleFocus"
    @update:value="handleInput"
  />
  <NInput
    v-else-if="field.type === 'textarea'"
    type="textarea"
    :rows="4"
    :value="modelValue"
    :placeholder="placeholder"
    @update:value="handleInput"
  />
  <NInput
    v-else-if="field.type === 'pem'"
    type="textarea"
    :rows="6"
    :value="modelValue"
    :placeholder="placeholder || '-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----'"
    style="font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px"
    @focus="handleFocus"
    @update:value="handleInput"
  />
  <NSelect
    v-else-if="field.type === 'select'"
    :value="modelValue"
    :options="field.options || []"
    @update:value="handleInput"
  />
  <NSelect
    v-else-if="field.type === 'multiselect'"
    multiple
    :value="modelValue"
    :options="field.options || []"
    @update:value="handleInput"
  />
  <NSwitch
    v-else-if="field.type === 'switch'"
    :value="modelValue"
    @update:value="handleInput"
  />
</template>
