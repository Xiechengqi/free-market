<script setup lang="ts">
import { useTabStore } from '@/store/modules/tab';
import { $t } from '@/locales';

defineOptions({ name: 'ClearTabsButton' });

const tabStore = useTabStore();

function handleClick() {
  // tabStore.clearTabs() preserves the home tab + any pinned tabs.
  tabStore.clearTabs();
}
</script>

<template>
  <ButtonIcon :tooltip-content="$t('icon.clearTabs')" @click="handleClick">
    <icon-mdi-tab-remove />
  </ButtonIcon>
</template>

<style scoped></style>
