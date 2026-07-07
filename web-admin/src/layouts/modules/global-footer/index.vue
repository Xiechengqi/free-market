<script setup lang="ts">
import { computed } from 'vue';
import { useSiteInfoStore } from '@/store/modules/site-info';

defineOptions({
  name: 'GlobalFooter'
});

const siteStore = useSiteInfoStore();
const year = new Date().getFullYear();
const footer = computed(() => siteStore.footer || `Copyright © ${year} ${siteStore.name}`);
</script>

<template>
  <DarkModeContainer class="h-full flex-center">
    <span>{{ footer }}</span>
  </DarkModeContainer>
</template>

<style scoped></style>
