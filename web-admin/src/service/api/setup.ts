import { request } from '@/service/request';

export function fetchSetupStatus() {
  return request<{ installed: boolean }>({ url: '/setup/status' });
}

export interface SetupPayload {
  userName: string;
  displayName?: string;
  password: string;
  passwordConfirm: string;
  siteName?: string;
  logoText?: string;
}

export function submitSetup(payload: SetupPayload) {
  return request<{ token: string; refreshToken: string }>({
    url: '/setup/install',
    method: 'post',
    data: payload
  });
}
