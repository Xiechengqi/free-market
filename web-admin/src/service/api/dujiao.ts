/**
 * Thin axios wrappers for the dujiao-rust admin JSON API.
 *
 * Every call goes through `@/service/request`, which already strips the
 * `{code, msg, data}` envelope (data is returned via `.data`). The shared
 * interceptor handles 4002/9999 logout/refresh automatically.
 *
 * Each call returns the flat `{ data, error, response }` object —
 * components prefer `error` over try/catch.
 */
import { request } from '@/service/request';

export interface Pagination {
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
  prev_page: number;
  next_page: number;
}

// ---- Site info (public, before login) ----
export interface SiteInfo {
  name: string;
  logoText: string;
  imgLogo: string;
  language: string;
  footer: string;
}
export function fetchSiteInfo() {
  return request<SiteInfo>({ url: '/site-info' });
}

// ---- Docs (public, server-rendered markdown HTML) ----
export interface DocsResponse {
  html: string;
}
export function fetchDocs() {
  return request<DocsResponse>({ url: '/docs' });
}

// ---- Dashboard ----
export function fetchDashboard() {
  return request<any>({ url: '/dashboard' });
}

// ---- Orders ----
export function fetchOrders(params?: Record<string, any>) {
  return request<any>({ url: '/orders', params });
}
export function fetchOrder(id: number) {
  return request<any>({ url: `/orders/${id}` });
}
export function fulfillOrder(id: number, payload: string) {
  return request<any>({ url: `/orders/${id}/fulfill`, method: 'post', data: { payload } });
}
export function cancelOrder(id: number) {
  return request<any>({ url: `/orders/${id}/cancel`, method: 'post' });
}
export function resendOrderEmail(id: number) {
  return request<any>({ url: `/orders/${id}/resend-email`, method: 'post' });
}
export function markOrderAbnormal(id: number) {
  return request<any>({ url: `/orders/${id}/mark-abnormal`, method: 'post' });
}
export function deleteOrder(id: number) {
  return request<any>({ url: `/orders/${id}/delete`, method: 'post' });
}
export function startOrderProcessing(id: number) {
  return request<any>({ url: `/orders/${id}/start-processing`, method: 'post' });
}
export function confirmEvmIntent(orderId: number, intentId: number, txHash: string) {
  return request<any>({
    url: `/orders/${orderId}/evm-intents/${intentId}/confirm`,
    method: 'post',
    data: { tx_hash: txHash }
  });
}

// ---- Categories ----
export function fetchCategories(params?: Record<string, any>) {
  return request<any>({ url: '/categories', params });
}
export function createCategory(data: any) {
  return request<any>({ url: '/categories', method: 'post', data });
}
export function updateCategory(id: number, data: any) {
  return request<any>({ url: `/categories/${id}`, method: 'post', data });
}
export function deleteCategory(id: number) {
  return request<any>({ url: `/categories/${id}`, method: 'delete' });
}

// ---- Products ----
export function fetchProducts(params?: Record<string, any>) {
  return request<any>({ url: '/products', params });
}
export function createProduct(data: any) {
  return request<any>({ url: '/products', method: 'post', data });
}
export function updateProduct(id: number, data: any) {
  return request<any>({ url: `/products/${id}`, method: 'post', data });
}
export function deleteProduct(id: number) {
  return request<any>({ url: `/products/${id}`, method: 'delete' });
}

// ---- Coupons ----
export function fetchCoupons(params?: Record<string, any>) {
  return request<any>({ url: '/coupons', params });
}
export function createCoupon(data: any) {
  return request<any>({ url: '/coupons', method: 'post', data });
}
export function updateCoupon(id: number, data: any) {
  return request<any>({ url: `/coupons/${id}`, method: 'post', data });
}
export function deleteCoupon(id: number) {
  return request<any>({ url: `/coupons/${id}`, method: 'delete' });
}

// ---- Payment channels ----
export function fetchPaymentChannels(params?: Record<string, any>) {
  return request<any>({ url: '/payment-channels', params });
}
export function createPaymentChannel(data: any) {
  return request<any>({ url: '/payment-channels', method: 'post', data });
}
export function updatePaymentChannel(id: number, data: any) {
  return request<any>({ url: `/payment-channels/${id}`, method: 'post', data });
}
export function deletePaymentChannel(id: number) {
  return request<any>({ url: `/payment-channels/${id}`, method: 'delete' });
}
export function validatePaymentChannel(data: any) {
  return request<{ ok: boolean }>({
    url: '/payment-channels/validate',
    method: 'post',
    data
  });
}
export function fetchEvmPaymentPresets() {
  return request<any>({ url: '/payment-channels/evm-presets' });
}

// ---- Settings ----
export function fetchSettings() {
  return request<any>({ url: '/settings' });
}
export function saveSettings(data: any) {
  return request<any>({ url: '/settings', method: 'post', data });
}

// ---- Email templates ----
export function fetchEmailTemplates(params?: Record<string, any>) {
  return request<any>({ url: '/email-templates', params });
}
export function createEmailTemplate(data: any) {
  return request<any>({ url: '/email-templates', method: 'post', data });
}
export function updateEmailTemplate(id: number, data: any) {
  return request<any>({ url: `/email-templates/${id}`, method: 'post', data });
}
export function deleteEmailTemplate(id: number) {
  return request<any>({ url: `/email-templates/${id}`, method: 'delete' });
}
export function restoreDefaultEmailTemplates() {
  return request<any>({ url: '/email-templates/restore-defaults', method: 'post' });
}

// ---- Admins ----
export function fetchAdmins(params?: Record<string, any>) {
  return request<any>({ url: '/admins', params });
}
export function createAdmin(data: any) {
  return request<any>({ url: '/admins', method: 'post', data });
}
export function updateAdmin(id: number, data: any) {
  return request<any>({ url: `/admins/${id}`, method: 'post', data });
}

// ---- Jobs ----
export function fetchJobs(params?: Record<string, any>) {
  return request<any>({ url: '/jobs', params });
}
export function retryJob(id: number) {
  return request<any>({ url: `/jobs/${id}/retry`, method: 'post' });
}
export function cleanupRuntime() {
  return request<any>({ url: '/jobs/cleanup', method: 'post' });
}

// ---- Logs ----
export function fetchNotificationLogs(params?: Record<string, any>) {
  return request<any>({ url: '/notification-logs', params });
}
export function fetchAuditLogs(params?: Record<string, any>) {
  return request<any>({ url: '/audit-logs', params });
}

// ---- Trash ----
export function fetchTrash(params?: Record<string, any>) {
  return request<any>({ url: '/trash', params });
}
export function restoreTrash(table: string, id: number) {
  return request<any>({ url: `/trash/${table}/${id}/restore`, method: 'post' });
}

// ---- Cards / Carmis ----
export function fetchProductCards(productId: number, params?: Record<string, any>) {
  return request<any>({ url: `/products/${productId}/cards`, params });
}
export function importProductCards(productId: number, data: { secrets: string; is_loop?: string; remove_duplication?: string }) {
  return request<any>({ url: `/products/${productId}/cards/import`, method: 'post', data });
}
export function deleteProductCard(productId: number, cardId: number) {
  return request<any>({ url: `/products/${productId}/cards/${cardId}`, method: 'delete' });
}
export function fetchGlobalCards(params?: Record<string, any>) {
  return request<any>({ url: '/cards', params });
}
export function deleteGlobalCard(id: number) {
  return request<any>({ url: `/cards/${id}`, method: 'delete' });
}

// ---- Uploads ----
export function fetchUploads(params?: Record<string, any>) {
  return request<any>({ url: '/uploads', params });
}
export function uploadFile(file: File) {
  const data = new FormData();
  data.append('file', file);
  return request<{ path: string; url: string; mime: string; size_bytes: number }>({
    url: '/uploads',
    method: 'post',
    data,
    headers: { 'Content-Type': 'multipart/form-data' }
  });
}
export function cleanupUploads() {
  return request<any>({ url: '/uploads/cleanup', method: 'post' });
}

// ---- Email test ----
export function fetchEmailTest() {
  return request<any>({ url: '/email-test' });
}
export function sendEmailTest(data: { to: string; title: string; body: string }) {
  return request<any>({ url: '/email-test', method: 'post', data });
}

// ---- Backup ----
export function fetchBackup() {
  return request<any>({ url: '/backup' });
}
export function createBackup() {
  return request<{ filename: string }>({ url: '/backup/create', method: 'post' });
}
export function saveBackupSettings(data: { enabled: boolean; weekday: number; hour: number; keep_files: number }) {
  return request<any>({ url: '/backup/settings', method: 'post', data });
}

// ---- Authenticated downloads ----
// The base request helper only handles JSON envelopes. For attachment downloads
// we hit the same backend with a hand-rolled fetch() carrying the Bearer token.
import { localStg } from '@/utils/storage';
function backendBase(): string {
  const env = (import.meta as any).env || {};
  const isProxy = env.DEV && env.VITE_HTTP_PROXY === 'Y';
  const base =
    (isProxy ? env.VITE_SERVICE_BASE_URL : env.VITE_OTHER_SERVICE_BASE_URL?.split(',')?.[0] || env.VITE_SERVICE_BASE_URL) || '';
  return (base as string) || '/admin/api';
}
async function downloadAuthenticated(path: string, fallbackName: string) {
  const token = localStg.get('token');
  const res = await fetch(`${backendBase()}${path}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : undefined
  });
  if (!res.ok) {
    throw new Error(`下载失败 (HTTP ${res.status})`);
  }
  const blob = await res.blob();
  let filename = fallbackName;
  const dispo = res.headers.get('content-disposition') || '';
  const match = dispo.match(/filename="?([^";]+)"?/i);
  if (match) filename = match[1];
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
export function downloadProductCardsCsv(productId: number, params?: Record<string, any>) {
  const q = params ? new URLSearchParams(params as any).toString() : '';
  return downloadAuthenticated(
    `/products/${productId}/cards/export${q ? `?${q}` : ''}`,
    `cards-${productId}.txt`
  );
}
export function downloadGlobalCardsCsv(params?: Record<string, any>) {
  const q = params ? new URLSearchParams(params as any).toString() : '';
  return downloadAuthenticated(`/cards/export${q ? `?${q}` : ''}`, 'cards.txt');
}
export function downloadOrdersCsv(params?: Record<string, any>) {
  const q = params ? new URLSearchParams(params as any).toString() : '';
  return downloadAuthenticated(`/orders/export${q ? `?${q}` : ''}`, 'orders.csv');
}
export function downloadBackupFile(filename: string) {
  return downloadAuthenticated(`/backup/files/${encodeURIComponent(filename)}`, filename);
}
