<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import { NCard, NSpin, useMessage } from 'naive-ui';
import { fetchDocs } from '@/service/api/dujiao';

defineOptions({ name: 'DocsPage' });

const message = useMessage();
const route = useRoute();
const html = ref('');
const loading = ref(true);
const articleRef = ref<HTMLElement | null>(null);

async function load() {
  loading.value = true;
  const { data, error } = await fetchDocs();
  loading.value = false;
  if (error) {
    message.error(error.message || '文档加载失败');
    return;
  }
  html.value = data?.html || '';
  await nextTick();
  assignHeadingIds();
  scrollToQueryAnchor();
}

/** Walk h1/h2/h3 and assign slug ids derived from text content. */
function assignHeadingIds() {
  const root = articleRef.value;
  if (!root) return;
  const headings = root.querySelectorAll('h1, h2, h3, h4');
  const seen = new Map<string, number>();
  headings.forEach(h => {
    const slug = slugify(h.textContent || '');
    if (!slug) return;
    const n = seen.get(slug) ?? 0;
    seen.set(slug, n + 1);
    h.id = n === 0 ? slug : `${slug}-${n}`;
  });
}

function slugify(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^\p{L}\p{N}-]+/gu, '')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '');
}

/** Find the first heading whose textContent contains `needle` (case-insensitive). */
function scrollToQueryAnchor() {
  const root = articleRef.value;
  if (!root) return;
  const needle = String(route.query.h || route.query.anchor || '').trim();
  if (!needle) return;
  const needleLower = needle.toLowerCase();
  const headings = root.querySelectorAll('h1, h2, h3, h4');
  for (const h of Array.from(headings)) {
    if ((h.textContent || '').toLowerCase().includes(needleLower)) {
      // Briefly highlight the target so the user notices where they landed.
      (h as HTMLElement).style.transition = 'background-color 1.2s ease-out';
      (h as HTMLElement).style.backgroundColor = '#fff4c2';
      h.scrollIntoView({ behavior: 'smooth', block: 'start' });
      setTimeout(() => {
        (h as HTMLElement).style.backgroundColor = '';
      }, 1500);
      return;
    }
  }
}

watch(
  () => route.query.h,
  () => scrollToQueryAnchor()
);

onMounted(load);
</script>

<template>
  <NSpin :show="loading" class="docs-spin">
    <NCard :bordered="false" class="docs-card">
      <!--
        Server pre-renders the markdown via pulldown-cmark; we trust the source
        (it's `docs/README.md` baked into the binary), so v-html is safe here.
      -->
      <article ref="articleRef" class="docs-article" v-html="html" />
    </NCard>
  </NSpin>
</template>

<style scoped>
/*
 * Style intentionally mirrors ruanyifeng.com — white background, narrow
 * column, generous line-height, headings with subtle border, serif accents
 * on body copy. Background is forced white regardless of dark mode because
 * the customer-facing source content was written for a light reading
 * surface.
 */
.docs-spin {
  background: #fff;
  min-height: 100%;
}
.docs-card :deep(.n-card__content) {
  background: #fff;
  padding: 40px 24px;
  max-width: 820px;
  margin: 0 auto;
}
.docs-article {
  font-family:
    -apple-system,
    BlinkMacSystemFont,
    'PingFang SC',
    'Hiragino Sans GB',
    'Microsoft YaHei',
    'WenQuanYi Micro Hei',
    Helvetica,
    Arial,
    sans-serif;
  color: #222;
  font-size: 16px;
  line-height: 1.75;
  text-align: justify;
}
.docs-article :deep(h1) {
  font-size: 30px;
  font-weight: 600;
  line-height: 1.3;
  margin: 36px 0 20px;
  padding-bottom: 12px;
  border-bottom: 1px solid #e0e0e0;
  color: #111;
}
.docs-article :deep(h2) {
  font-size: 24px;
  font-weight: 600;
  line-height: 1.35;
  margin: 32px 0 16px;
  padding-bottom: 6px;
  border-bottom: 1px dashed #e0e0e0;
  color: #111;
}
.docs-article :deep(h3) {
  font-size: 20px;
  font-weight: 600;
  margin: 24px 0 12px;
  color: #222;
}
.docs-article :deep(h4),
.docs-article :deep(h5),
.docs-article :deep(h6) {
  font-size: 17px;
  font-weight: 600;
  margin: 20px 0 10px;
  color: #333;
}
.docs-article :deep(p) {
  margin: 0 0 14px;
}
.docs-article :deep(a) {
  color: #0066cc;
  text-decoration: none;
  border-bottom: 1px solid transparent;
  transition: border-color 0.15s ease;
}
.docs-article :deep(a:hover) {
  border-bottom-color: #0066cc;
}
.docs-article :deep(ul),
.docs-article :deep(ol) {
  margin: 0 0 14px;
  padding-left: 28px;
}
.docs-article :deep(li) {
  margin: 4px 0;
}
.docs-article :deep(li > p) {
  margin: 4px 0;
}
.docs-article :deep(blockquote) {
  margin: 16px 0;
  padding: 8px 16px;
  border-left: 4px solid #ddd;
  color: #555;
  background: #fafafa;
}
.docs-article :deep(code) {
  font-family: 'SFMono-Regular', Menlo, Consolas, 'Liberation Mono', monospace;
  font-size: 0.92em;
  padding: 1px 6px;
  border-radius: 3px;
  background: #f5f5f5;
  color: #c7254e;
}
.docs-article :deep(pre) {
  background: #f6f8fa;
  border: 1px solid #eaecef;
  border-radius: 4px;
  padding: 14px 16px;
  margin: 16px 0;
  overflow-x: auto;
  line-height: 1.55;
}
.docs-article :deep(pre code) {
  background: transparent;
  color: #24292e;
  padding: 0;
  font-size: 13.5px;
}
.docs-article :deep(table) {
  border-collapse: collapse;
  margin: 16px 0;
  width: 100%;
}
.docs-article :deep(th),
.docs-article :deep(td) {
  border: 1px solid #e0e0e0;
  padding: 8px 12px;
  text-align: left;
}
.docs-article :deep(th) {
  background: #f8f8f8;
  font-weight: 600;
}
.docs-article :deep(hr) {
  border: none;
  border-top: 1px solid #e0e0e0;
  margin: 28px 0;
}
.docs-article :deep(strong) {
  font-weight: 600;
  color: #111;
}
</style>
