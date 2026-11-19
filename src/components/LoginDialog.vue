<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue';
import { useAuthStore } from '../authStore';

const auth = useAuthStore();
const mode = ref<'login' | 'register'>('login');
const username = ref('');
const password = ref('');
const confirmPassword = ref('');
const feedback = ref('');
const feedbackOk = ref(false);

watch(() => auth.loginDialogOpen, (open) => {
  if (open) {
    feedback.value = '';
    confirmPassword.value = '';
  }
});

function close() {
  auth.loginDialogOpen = false;
  auth.pendingReason = '';
  auth.error = '';
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && auth.loginDialogOpen) close();
}
onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));

function switchMode(next: 'login' | 'register') {
  mode.value = next;
  feedback.value = '';
  auth.error = '';
}

async function submit() {
  feedback.value = '';
  if (!username.value.trim() || !password.value) {
    feedback.value = '请输入账号和密码';
    feedbackOk.value = false;
    return;
  }
  if (mode.value === 'login') {
    const ok = await auth.login(username.value, password.value);
    if (ok) close();
    else { feedback.value = auth.error; feedbackOk.value = false; }
  } else {
    if (password.value !== confirmPassword.value) {
      feedback.value = '两次输入的密码不一致';
      feedbackOk.value = false;
      return;
    }
    const res = await auth.register(username.value, password.value);
    feedback.value = res.message;
    feedbackOk.value = res.ok;
    if (res.ok) mode.value = 'login';
  }
}
</script>

<template>
  <div v-if="auth.loginDialogOpen" class="modal-mask" role="dialog" aria-modal="true" aria-label="登录" @click.self="close">
    <div class="modal login-modal">
      <h3>{{ mode === 'login' ? '登录都客账号' : '注册都客账号' }}</h3>
      <p v-if="mode === 'login' && auth.pendingReason">{{ auth.pendingReason }}</p>
      <p v-else-if="mode === 'login'">账号与 moha 通用，仅用于登录与权限校验。</p>
      <p v-else>使用邮箱注册，注册成功后返回登录。</p>

      <div class="field">
        <label>{{ mode === 'login' ? '账号' : '邮箱' }}</label>
        <input
          v-model="username"
          :placeholder="mode === 'login' ? '账号 / 邮箱' : 'you@example.com'"
          @keyup.enter="submit"
        />
      </div>
      <div class="field">
        <label>密码</label>
        <input v-model="password" type="password" placeholder="密码" @keyup.enter="submit" />
      </div>
      <div v-if="mode === 'register'" class="field">
        <label>确认密码</label>
        <input v-model="confirmPassword" type="password" placeholder="再次输入密码" @keyup.enter="submit" />
      </div>

      <div v-if="feedback" :class="feedbackOk ? 'toast' : 'error-box'" style="margin-bottom:0;">{{ feedback }}</div>

      <div class="modal-actions">
        <button class="btn" @click="close">取消</button>
        <button class="btn btn-primary" @click="submit">{{ mode === 'login' ? '登录' : '注册' }}</button>
      </div>
      <div class="login-switch">
        {{ mode === 'login' ? '还没有账号？' : '已有账号？' }}
        <button class="link-btn" @click="switchMode(mode === 'login' ? 'register' : 'login')">
          {{ mode === 'login' ? '立即注册' : '返回登录' }}
        </button>
      </div>
    </div>
  </div>
</template>
