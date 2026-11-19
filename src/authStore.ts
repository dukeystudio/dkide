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
    isLoggedIn, account,
    init, login, register, logout, refreshQuota, requireLogin,
  };
});
