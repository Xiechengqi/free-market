<script setup lang="ts">
import { NButton, NTooltip, type PopoverPlacement } from 'naive-ui';

defineOptions({ name: 'ButtonIcon' });

interface Props {
  /** iconify name, e.g. "ph:sun-bold" */
  icon?: string;
  /** local svg sprite id */
  localIcon?: string;
  /** class applied to the fallback SvgIcon (typically a `text-icon*` font-size) */
  iconClass?: string;
  /** tooltip text. empty/undefined → no tooltip */
  tooltipContent?: string;
  tooltipPlacement?: PopoverPlacement;
  /** tooltip z-index */
  zIndex?: number;
}

withDefaults(defineProps<Props>(), {
  iconClass: 'text-icon',
  tooltipPlacement: 'bottom',
  zIndex: 98
});

// Explicit forward of click so callers wiring @click on <ButtonIcon> reach
// the inner NButton. Without this, Vue's attribute fall-through fails when
// the template has more than one root element, and clicks silently drop —
// which is exactly what broke ThemeSchemaSwitch / ClearTabsButton /
// ReloadButton / FullScreen earlier.
const emit = defineEmits<{
  (e: 'click', event: MouseEvent): void;
}>();

function handleClick(event: MouseEvent) {
  emit('click', event);
}
</script>

<template>
  <NTooltip
    v-if="tooltipContent"
    :placement="tooltipPlacement"
    :z-index="zIndex"
    trigger="hover"
  >
    <template #trigger>
      <NButton quaternary class="button-icon-wrapper" @click="handleClick">
        <slot>
          <SvgIcon :icon="icon" :local-icon="localIcon" :class="iconClass" />
        </slot>
      </NButton>
    </template>
    <span>{{ tooltipContent }}</span>
  </NTooltip>
  <NButton v-else quaternary class="button-icon-wrapper" @click="handleClick">
    <slot>
      <SvgIcon :icon="icon" :local-icon="localIcon" :class="iconClass" />
    </slot>
  </NButton>
</template>

<style scoped>
.button-icon-wrapper {
  height: 36px;
  min-width: 36px;
  padding: 0 8px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.button-icon-wrapper :deep(.n-button__content) {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
</style>
