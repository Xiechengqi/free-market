<script setup lang="ts">
import { computed } from 'vue';
import { useSiteInfoStore } from '@/store/modules/site-info';
import defaultLogo from '@/assets/imgs/logo.jpg';

defineOptions({
  name: 'SystemLogo'
});

const siteStore = useSiteInfoStore();

const src = computed(() => siteStore.imgLogo?.trim() || defaultLogo);
</script>

<template>
  <div class="app-logo">
    <img :src="src" alt="logo" />
  </div>
</template>

<style scoped>
.app-logo {
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  border-radius: 8px;
}
.app-logo img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
</style>
