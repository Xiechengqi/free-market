<script setup lang="ts">
/**
 * Settings page — a flat form covering site / order / security / SMTP / notification / branding.
 * Loads the merged data shape returned by GET /admin/api/settings, posts the same shape back.
 * Sensitive fields (smtp_password, notify_*_*key, etc) come back as "********"; backend's
 * resolve_secret() preserves the stored cipher when the user re-submits that mask.
 */
import { onMounted, ref } from 'vue';
import {
  NAlert,
  NButton,
  NCard,
  NDivider,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NInput,
  NInputNumber,
  NModal,
  NSelect,
  NSpace,
  NSpin,
  NSwitch,
  useMessage
} from 'naive-ui';
import { fetchEmailTest, fetchSettings, saveSettings, sendEmailTest } from '@/service/api';

const message = useMessage();
const loading = ref(false);
const saving = ref(false);
const form = ref<any>({});

const emailTestModal = ref(false);
const emailSending = ref(false);
const emailTestForm = ref({ to: '', title: '', body: '' });

async function load() {
  loading.value = true;
  const { data, error } = await fetchSettings();
  if (error) message.error(error.message || '加载失败');
  else if (data) form.value = data;
  loading.value = false;
}

async function submit() {
  saving.value = true;
  const { error } = await saveSettings(form.value);
  saving.value = false;
  if (error) message.error(error.message || '保存失败');
  else message.success('保存成功');
}

async function openEmailTest() {
  emailTestModal.value = true;
  const { data } = await fetchEmailTest();
  emailTestForm.value = {
    to: data?.default_to || form.value?.manage_email || '',
    title: data?.default_title || `${form.value?.name || ''} 邮件测试`,
    body: data?.default_body || '这是一封邮件测试。'
  };
}

async function doSendTest() {
  if (!emailTestForm.value.to.trim()) {
    message.warning('请输入接收邮箱');
    return;
  }
  emailSending.value = true;
  const { error } = await sendEmailTest(emailTestForm.value);
  emailSending.value = false;
  if (error) message.error(error.message || '发送失败');
  else {
    message.success('已发送，请检查收件箱');
    emailTestModal.value = false;
  }
}

onMounted(load);
</script>

<template>
  <NSpace vertical :size="16">
    <NAlert title="系统设置" type="info">
      所有变更立即写入 SQLite。SMTP 密码/通知密钥显示为 ********，保留此值再保存不会覆盖原密文；清空则清除。
    </NAlert>

    <NSpin :show="loading">
      <NSpace vertical :size="16">
        <NCard title="基础设置" :bordered="false">
          <NGrid :cols="2" :x-gap="16" :y-gap="12">
            <NGridItem><NFormItem label="站点名称"><NInput v-model:value="form.name" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="Logo 文案"><NInput v-model:value="form.logo_text" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="关键词"><NInput v-model:value="form.keywords" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="管理邮箱"><NInput v-model:value="form.manage_email" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="站点外部 URL"><NInput v-model:value="form.base_url" /></NFormItem></NGridItem>
            <NGridItem>
              <NFormItem label="前台主题">
                <NSelect v-model:value="form.template" :options="[
                  { label: 'luna', value: 'luna' },
                  { label: 'unicorn', value: 'unicorn' },
                  { label: 'hyper', value: 'hyper' }
                ]" />
              </NFormItem>
            </NGridItem>
            <NGridItem><NFormItem label="订单过期分钟"><NInputNumber v-model:value="form.order_expire_minutes" /></NFormItem></NGridItem>
            <NGridItem>
              <NFormItem label="语言">
                <NSelect v-model:value="form.language" :options="[
                  { label: '简体中文', value: 'zh-CN' },
                  { label: 'English', value: 'en-US' }
                ]" />
              </NFormItem>
            </NGridItem>
            <NGridItem><NFormItem label="图片 Logo URL"><NInput v-model:value="form.img_logo" placeholder="/uploads/logo.png" /></NFormItem></NGridItem>
          </NGrid>
          <NFormItem label="站点描述"><NInput v-model:value="form.description" type="textarea" :rows="2" /></NFormItem>
          <NFormItem label="公告"><NInput v-model:value="form.notice" type="textarea" :rows="2" /></NFormItem>
          <NFormItem label="页脚"><NInput v-model:value="form.footer" type="textarea" :rows="2" /></NFormItem>
          <NSpace>
            <NSwitch v-model:value="form.is_open_search_pwd" /> 邮箱查单需要密码
            <NSwitch v-model:value="form.is_open_anti_red" /> 微信/QQ 浏览器提示
            <NSwitch v-model:value="form.is_open_google_translate" /> Google 翻译入口
          </NSpace>
        </NCard>

        <NCard title="验证码与安全" :bordered="false">
          <NSpace>
            <NSwitch v-model:value="form.is_open_img_code" /> 图片验证码
          </NSpace>
          <p style="margin:8px 0 0;color:var(--n-text-color-3);font-size:13px">
            人机校验仅支持内置算术图形验证码及邮箱/IP 下单频控，不支持 Geetest 极验。
          </p>
          <NGrid :cols="2" :x-gap="16" :y-gap="12" style="margin-top:12px">
            <NGridItem><NFormItem label="登录失败次数限制"><NInputNumber v-model:value="form.login_max_attempts" :min="1" :max="20" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="锁定分钟"><NInputNumber v-model:value="form.login_lock_minutes" :min="1" :max="1440" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="信任反代层数"><NInputNumber v-model:value="form.trust_proxy_hops" :min="0" :max="10" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="Cookie Secure"><NSwitch v-model:value="form.cookie_secure" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="限购窗口分钟（0=关）"><NInputNumber v-model:value="form.purchase_rate_window_minutes" :min="0" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="窗口内同邮箱最多"><NInputNumber v-model:value="form.purchase_rate_max_per_email" :min="0" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="窗口内同 IP 最多"><NInputNumber v-model:value="form.purchase_rate_max_per_ip" :min="0" /></NFormItem></NGridItem>
          </NGrid>
        </NCard>

        <NCard title="SMTP 邮件" :bordered="false">
          <template #header-extra>
            <NButton size="small" @click="openEmailTest">发送测试邮件</NButton>
          </template>
          <NSpace style="margin-bottom:12px"><NSwitch v-model:value="form.smtp_enabled" /> 启用 SMTP</NSpace>
          <NGrid :cols="2" :x-gap="16" :y-gap="12">
            <NGridItem><NFormItem label="Host"><NInput v-model:value="form.smtp_host" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="Port"><NInputNumber v-model:value="form.smtp_port" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="用户名"><NInput v-model:value="form.smtp_username" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="密码"><NInput v-model:value="form.smtp_password" type="password" show-password-on="click" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="发件邮箱"><NInput v-model:value="form.smtp_from_email" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="发件名称"><NInput v-model:value="form.smtp_from_name" /></NFormItem></NGridItem>
            <NGridItem>
              <NFormItem label="加密">
                <NSelect v-model:value="form.smtp_encryption" :options="[
                  { label: 'STARTTLS', value: 'starttls' },
                  { label: 'TLS', value: 'tls' },
                  { label: 'None', value: 'none' }
                ]" />
              </NFormItem>
            </NGridItem>
          </NGrid>
        </NCard>

        <NCard title="通知渠道" :bordered="false">
          <NGrid :cols="2" :x-gap="16" :y-gap="12">
            <NGridItem><NFormItem label="Server 酱 SendKey"><NInput v-model:value="form.notify_server_chan_key" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="Telegram Bot Token"><NInput v-model:value="form.notify_telegram_bot_token" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="Telegram Chat ID"><NInput v-model:value="form.notify_telegram_chat_id" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="Bark URL"><NInput v-model:value="form.notify_bark_url" /></NFormItem></NGridItem>
            <NGridItem><NFormItem label="企业微信 Webhook"><NInput v-model:value="form.notify_wecom_webhook" /></NFormItem></NGridItem>
          </NGrid>
          <NDivider />
          <NSpace>
            <NSwitch v-model:value="form.is_open_server_chan" /> Server 酱
            <NSwitch v-model:value="form.is_open_telegram" /> Telegram
            <NSwitch v-model:value="form.is_open_bark" /> Bark
            <NSwitch v-model:value="form.is_open_bark_push_url" /> Bark 附详情链接
            <NSwitch v-model:value="form.is_open_wecom" /> 企业微信
          </NSpace>
        </NCard>

        <div style="display:flex;justify-content:flex-end">
          <NButton type="primary" :loading="saving" size="large" @click="submit">保存全部设置</NButton>
        </div>
      </NSpace>
    </NSpin>

    <NModal v-model:show="emailTestModal" preset="card" title="发送测试邮件" style="max-width:520px">
      <NForm label-placement="left" label-width="80">
        <NFormItem label="收件人"><NInput v-model:value="emailTestForm.to" placeholder="收件邮箱" /></NFormItem>
        <NFormItem label="标题"><NInput v-model:value="emailTestForm.title" /></NFormItem>
        <NFormItem label="正文"><NInput v-model:value="emailTestForm.body" type="textarea" :rows="6" /></NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton @click="emailTestModal = false">取消</NButton>
          <NButton type="primary" :loading="emailSending" @click="doSendTest">发送</NButton>
        </NSpace>
      </template>
    </NModal>
  </NSpace>
</template>
