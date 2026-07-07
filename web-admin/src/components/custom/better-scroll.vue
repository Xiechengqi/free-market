<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import BScroll from '@better-scroll/core';
import type { BScrollConstructor, Options } from '@better-scroll/core';

defineOptions({ name: 'BetterScroll' });

interface Props {
  /**
   * Forwarded as the second argument to `new BScroll(el, options)`.
   * See https://better-scroll.github.io/docs/en-US/guide/base-scroll-options.html
   */
  options?: Options;
}

const props = withDefaults(defineProps<Props>(), {
  options: () => ({}) as Options
});

const wrapperRef = ref<HTMLElement | null>(null);
const instance = ref<BScrollConstructor | null>(null);

async function init() {
  if (!wrapperRef.value) return;
  await nextTick();
  instance.value = new BScroll(wrapperRef.value, props.options) as BScrollConstructor;
}

async function refresh() {
  await nextTick();
  instance.value?.refresh();
}

function scrollTo(x: number, y = 0, time = 300) {
  instance.value?.scrollTo(x, y, time);
}

function scrollBy(deltaX: number, deltaY = 0, time = 300) {
  if (!instance.value) return;
  const { x, y } = instance.value;
  scrollTo(x + deltaX, y + deltaY, time);
}

onMounted(init);

onBeforeUnmount(() => {
  instance.value?.destroy();
  instance.value = null;
});

watch(
  () => props.options,
  () => refresh(),
  { deep: true }
);

defineExpose({
  instance,
  refresh,
  scrollTo,
  scrollBy
});
</script>

<template>
  <div ref="wrapperRef" class="overflow-hidden">
    <div class="inline-flex">
      <slot />
    </div>
  </div>
</template>
