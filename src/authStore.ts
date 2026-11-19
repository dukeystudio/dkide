import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { api } from './api';
import type { AuthSession } from './types';

export const useAuthStore = defineStore('auth', () => {
  const session = ref<AuthSession | null>(null);
  const verifying = ref(true);
  const loginDialogOpen = ref(false);
  const pendingReason = ref('');
  const error = ref('');

  const isLoggedIn = computed(() => session.value !== null);
  const account = computed(() => session.value?.account ?? '');

  /** 账号脱敏：邮箱保留本地部分首尾，中间打码（如 388*****972@qq.com） */
  const maskedAccount = computed(() => {
    const raw = account.value;
    if (!raw) return '';
    if (raw.includes('@')) {
      const [local, domain] = raw.split('@');
      const head = local.slice(0, 3);
      const tail = local.length > 5 ? local.slice(-3) : '';
      const stars = Math.max(1, local.length - head.length - tail.length);
      return `${head}${'*'.repeat(stars)}${tail}@${domain}`;
    }
    if (raw.length <= 4) return raw;
    return `${raw.slice(0, 2)}${'*'.repeat(raw.length - 4)}${raw.slice(-2)}`;
  });

  async function init() {
    verifying.value = true;
    try {
      session.value = await api.authVerify();
    } catch {
      session.value = await api.authSession().catch(() => null);
    } finally {
      verifying.value = false;
    }
  }

  async function login(username: string, password: string) {
    error.value = '';
    try {
      session.value = await api.authLogin(username.trim(), password);
      loginDialogOpen.value = false;
      pendingReason.value = '';
      return true;
    } catch (e: any) {
      error.value = typeof e === 'string' ? e : e?.message ?? String(e);
      return false;
    }
  }

  async function register(email: string, password: string) {
    error.value = '';
    try {
      const msg = await api.authRegister(email.trim(), password);
      return { ok: true, message: msg };
    } catch (e: any) {
      error.value = typeof e === 'string' ? e : e?.message ?? String(e);
      return { ok: false, message: error.value };
    }
  }

  async function logout() {
    await api.authLogout();
    session.value = null;
  }

  async function refreshQuota() {
    if (!session.value) return;
    try {
      const quota = await api.authQuota();
      session.value = {
        ...session.value,
        coin: quota.coin,
        expire_display: quota.expire_display,
      };
    } catch { /* ignore */ }
  }

  function requireLogin(reason = '') {
    if (isLoggedIn.value) return true;
    pendingReason.value = reason;
    loginDialogOpen.value = true;
    return false;
  }

  return {
    session, verifying, loginDialogOpen, pendingReason, error,
    isLoggedIn, account, maskedAccount,
    init, login, register, logout, refreshQuota, requireLogin,
  };
});
