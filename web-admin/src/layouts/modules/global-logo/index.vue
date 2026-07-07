<script setup lang="ts">
import { useSiteInfoStore } from '@/store/modules/site-info';

defineOptions({
  name: 'GlobalLogo'
});

interface Props {
  /** Whether to show the title */
  showTitle?: boolean;
}

withDefaults(defineProps<Props>(), {
  showTitle: true
});

const siteStore = useSiteInfoStore();
</script>

<template>
  <RouterLink to="/" class="w-full flex-center nowrap-hidden">
    <SystemLogo class="size-32px" />
    <h2 v-show="showTitle" class="pl-8px text-16px text-primary font-bold transition duration-300 ease-in-out">
      {{ siteStore.name }}
    </h2>
  </RouterLink>
</template>

<style scoped></style>
