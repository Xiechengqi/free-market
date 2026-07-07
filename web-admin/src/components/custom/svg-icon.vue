<script setup lang="ts">
import { computed, useAttrs } from 'vue';
import { Icon } from '@iconify/vue';

defineOptions({
  name: 'SvgIcon',
  inheritAttrs: false
});

interface Props {
  /** iconify name */
  icon?: string;
  /** local svg sprite id (registered via vite-plugin-svg-icons) */
  localIcon?: string;
}

const props = defineProps<Props>();
const attrs = useAttrs();

const bindAttrs = computed<{ class: string; style: string }>(() => ({
  class: (attrs.class as string) || '',
  style: (attrs.style as string) || ''
}));

const symbolId = computed(() => (props.localIcon ? `#icon-${props.localIcon}` : ''));
</script>

<template>
  <Icon v-if="icon" :icon="icon" width="1em" height="1em" v-bind="bindAttrs" />
  <svg v-else aria-hidden="true" width="1em" height="1em" v-bind="bindAttrs">
    <use :xlink:href="symbolId" fill="currentColor" />
  </svg>
</template>
