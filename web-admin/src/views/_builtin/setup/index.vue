<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import {
  NAlert,
  NButton,
  NCard,
  NForm,
  NFormItem,
  NH2,
  NInput,
  NSpace,
  NSpin,
  type FormInst,
  useMessage
} from 'naive-ui';
import { useAuthStore } from '@/store/modules/auth';
import { useRouterPush } from '@/hooks/common/router';
import { markSetupComplete } from '@/router/guard/route';
import { fetchSetupStatus, submitSetup } from '@/service/api';

defineOptions({ name: 'SetupPage' });

const authStore = useAuthStore();
const { routerPushByKey } = useRouterPush();
const message = useMessage();

const checking = ref(true);
const installed = ref(false);
const submitting = ref(false);
const formRef = ref<FormInst | null>(null);

interface SetupModel {
  userName: string;
  displayName: string;
  password: string;
  passwordConfirm: string;
  siteName: string;
  logoText: string;
}

const model: SetupModel = reactive({
  userName: 'admin',
  displayName: 'Administrator',
  password: '',
  passwordConfirm: '',
  siteName: '独角数卡',
  logoText: 'Dujiao Rust'
});

const rules = computed(() => ({
  userName: [
    { required: true, message: '请输入用户名', trigger: 'blur' },
    { min: 3, max: 32, message: '用户名长度 3-32', trigger: 'blur' }
  ],
  password: [
    { required: true, message: '请输入密码', trigger: 'blur' },
    { min: 8, message: '密码至少 8 位', trigger: 'blur' }
  ],
  passwordConfirm: [
    { required: true, message: '请再次输入密码', trigger: 'blur' },
    {
      validator: (_rule: any, value: string) => value === model.password,
      message: '两次输入的密码不一致',
      trigger: ['blur', 'input']
    }
  ],
  siteName: [{ required: true, message: '请输入站点名称', trigger: 'blur' }]
}));

async function refreshStatus() {
  checking.value = true;
  const { data, error } = await fetchSetupStatus();
  if (error) {
    message.error(error.message || '加载状态失败');
  } else if (data?.installed) {
    installed.value = true;
    // System already initialized; bounce to login.
    routerPushByKey('login');
    return;
  }
  checking.value = false;
}

async function handleSubmit() {
  await formRef.value?.validate();
  submitting.value = true;
  const { error } = await submitSetup({
    userName: model.userName.trim(),
    displayName: model.displayName.trim() || model.userName.trim(),
    password: model.password,
    passwordConfirm: model.passwordConfirm,
    siteName: model.siteName.trim(),
    logoText: model.logoText.trim()
  });
  submitting.value = false;
  if (error) {
    message.error(error.message || '初始化失败');
    return;
  }
  message.success('初始化成功，正在登录...');
  markSetupComplete();
  // Use the credentials we just set to get clean store state via login().
  await authStore.login(model.userName.trim(), model.password);
}

onMounted(refreshStatus);
</script>

<template>
  <div class="setup-wrapper">
    <NCard class="setup-card" :bordered="false">
      <NSpin :show="checking">
        <div class="setup-header">
          <NH2 style="margin:0">Dujiao Rust 初始化</NH2>
          <p style="color:#888;margin-top:4px">系统首次运行，创建第一个 owner 管理员</p>
        </div>

        <NAlert type="warning" style="margin:16px 0">
          此页面只在 <code>admins</code> 表为空时可用。设置完成后将自动登录进入后台。
        </NAlert>

        <NForm
          ref="formRef"
          :model="model"
          :rules="rules"
          label-placement="left"
          label-width="100"
          require-mark-placement="right-hanging"
        >
          <NFormItem label="站点名称" path="siteName">
            <NInput v-model:value="model.siteName" placeholder="独角数卡" />
          </NFormItem>
          <NFormItem label="Logo 文案" path="logoText">
            <NInput v-model:value="model.logoText" placeholder="Dujiao Rust" />
          </NFormItem>
          <NFormItem label="用户名" path="userName">
            <NInput v-model:value="model.userName" placeholder="admin" />
          </NFormItem>
          <NFormItem label="显示名" path="displayName">
            <NInput v-model:value="model.displayName" placeholder="Administrator" />
          </NFormItem>
          <NFormItem label="密码" path="password">
            <NInput
              v-model:value="model.password"
              type="password"
              show-password-on="click"
              placeholder="至少 8 位"
            />
          </NFormItem>
          <NFormItem label="确认密码" path="passwordConfirm">
            <NInput
              v-model:value="model.passwordConfirm"
              type="password"
              show-password-on="click"
              placeholder="再次输入密码"
            />
          </NFormItem>
        </NForm>

        <NSpace justify="end">
          <NButton
            type="primary"
            size="large"
            :loading="submitting"
            @click="handleSubmit"
          >
            创建管理员并登录
          </NButton>
        </NSpace>
      </NSpin>
    </NCard>
  </div>
</template>

<style scoped>
.setup-wrapper {
  display: grid;
  place-items: center;
  min-height: 100vh;
  background: linear-gradient(135deg, #f4f6f8 0%, #e8edf3 100%);
  padding: 24px;
}
.setup-card {
  width: min(560px, 100%);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.06);
}
.setup-header {
  margin-bottom: 8px;
}
</style>
