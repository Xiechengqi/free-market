<script setup lang="ts">
import { computed } from 'vue';
import { payCheckBadge, payCheckDisplay } from '../method-specs';

const props = withDefaults(
  defineProps<{
    value?: string;
    showDescription?: boolean;
  }>(),
  {
    showDescription: false
  }
);

const raw = computed(() => (props.value || '').trim().toLowerCase());
const badge = computed(() => payCheckBadge(raw.value));
</script>

<template>
  <span class="pay-check-preview">
    <span
      :class="[
        'payment-badge-preview',
        `payment-badge-preview-${raw || 'other'}`
      ]"
    >
      {{ badge }}
    </span>
    <span class="pay-check-text">
      {{ showDescription ? payCheckDisplay(raw) : raw || '-' }}
    </span>
  </span>
</template>

<style scoped>
.pay-check-preview {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  vertical-align: middle;
}

.payment-badge-preview {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 32px;
  height: 22px;
  padding: 0 6px;
  border-radius: 4px;
  background: #22b6f2;
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  line-height: 22px;
  vertical-align: middle;
  flex: 0 0 auto;
}

.payment-badge-preview-stripe {
  background: #635bff;
}

.payment-badge-preview-tokenpay,
.payment-badge-preview-epusdt,
.payment-badge-preview-bepusdt,
.payment-badge-preview-freemarketpay,
.payment-badge-preview-okpay,
.payment-badge-preview-usdt,
.payment-badge-preview-usdc,
.payment-badge-preview-trx {
  background: #16a34a;
}

.pay-check-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
