import { ref } from 'vue';
import { defineStore } from 'pinia';
import { SetupStoreId } from '@/enum';
import { fetchSiteInfo } from '@/service/api/freemarket';

/**
 * Centralized brand info. Reads from the backend on app startup so the
 * login splash, header title, browser tab, watermark, logo, and footer all
 * read the same value the operator typed into /admin → 系统设置.
 *
 * imgLogo: if the operator uploaded a brand image in 系统设置 → 图片 Logo URL
 * (settings.img_logo), system-logo.vue renders it; otherwise the SPA falls
 * back to the bundled default (assets/imgs/logo.jpg).
 */
export const useSiteInfoStore = defineStore(SetupStoreId.SiteInfo, () => {
  const fallback = (import.meta.env.VITE_APP_TITLE as string) || 'freeMarket';
  const name = ref(fallback);
  const logoText = ref(fallback);
  const imgLogo = ref('');
  const footer = ref('');
  const language = ref('zh-CN');
  let initialized = false;

  async function init() {
    if (initialized) return;
    initialized = true;
    try {
      const { data } = await fetchSiteInfo();
      if (data?.name) {
        name.value = data.name;
        document.title = data.name;
      }
      if (data?.logoText) logoText.value = data.logoText;
      if (data?.imgLogo !== undefined) imgLogo.value = data.imgLogo;
      if (data?.footer !== undefined) footer.value = data.footer;
      if (data?.language) language.value = data.language;
    } catch (error) {
      // Swallow — falls back to env-defined title; setup screen still loads.
      console.warn('fetch site info failed, using fallback title', error);
    }
  }

  return { name, logoText, imgLogo, footer, language, init };
});
