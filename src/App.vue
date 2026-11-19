<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useChatStore } from './store';
import { useAuthStore } from './authStore';
import Sidebar from './components/Sidebar.vue';
import ChatView from './components/ChatView.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import SkillsPanel from './components/SkillsPanel.vue';
import AboutPanel from './components/AboutPanel.vue';
import LoginDialog from './components/LoginDialog.vue';
import ApprovalDialog from './components/ApprovalDialog.vue';
import ProviderDialog from './components/ProviderDialog.vue';

type Page = 'chat' | 'settings' | 'skills' | 'about';
const page = ref<Page>('chat');
const store = useChatStore();
const auth = useAuthStore();
const collapsed = ref(false);
const ready = ref(false);
const initError = ref('');

onMounted(async () => {
  try {
    await Promise.all([store.init(), auth.init()]);
    ready.value = true;
  } catch (e: any) {
    initError.value = typeof e === 'string' ? e : e?.message ?? String(e);
  }
});

async function retryInit() {
  initError.value = '';
  ready.value = false;
  try {
    await Promise.all([store.init(), auth.init()]);
    ready.value = true;
  } catch (e: any) {
    initError.value = typeof e === 'string' ? e : e?.message ?? String(e);
  }
}

const activeProvider = computed(() => {
  const c = store.config;
  if (!c) return null;
  return c.providers.find((p) => p.id === c.active_provider_id) ?? c.providers[0];
});

function onNavigate(p: Page) {
  // "模型设置"改为打开服务商配置弹窗，不再切整页
  if (p === 'settings') { openProviderDialog(); return; }
  page.value = p;
}
function openProviderDialog(focusAdd = false) {
  store.providerRequired = false;
  store.focusAddProvider = focusAdd;
  store.providerDialogOpen = true;
}
function goSettings() {
  openProviderDialog();
}
</script>

<template>
  <div v-if="initError" class="init-error">
    <div class="card" style="max-width:460px;width:90vw;">
      <h2>启动失败</h2>
      <p style="color:var(--text-muted);line-height:1.7;">应用初始化时出错：{{ initError }}</p>
      <p style="color:var(--text-faint);font-size:12px;">你的数据不会被删除，可重试启动。</p>
      <button class="btn btn-primary" @click="retryInit">重试</button>
    </div>
  </div>

  <div v-else-if="ready" class="app-shell">
    <Sidebar :page="page" :collapsed="collapsed" @navigate="onNavigate" @toggle-collapse="collapsed = !collapsed" />
    <main class="main">
      <ChatView
        v-show="page === 'chat'"
        :provider="activeProvider"
        @open-settings="openProviderDialog(($event?.focusAdd) ?? false)"
        @open-skills="page = 'skills'"
      />
      <SettingsPanel v-if="page === 'settings'" />
      <SkillsPanel v-if="page === 'skills'" />
      <AboutPanel v-if="page === 'about'" />
    </main>
  </div>

  <ProviderDialog />
  <ApprovalDialog :approvals="store.pendingApprovals" @respond="store.respondApproval($event.id, $event.approved)" />
  <LoginDialog />

  <div v-if="store.providerRequired" class="modal-mask" @click.self="store.providerRequired = false">
    <div class="modal">
      <h3>需要配置服务商</h3>
      <p>首次对话前请先配置一个模型中转站。API Key 只保存在你本机，不会由账号自动下发。</p>
      <div class="modal-actions">
        <button class="btn" @click="store.providerRequired = false">稍后再说</button>
        <button class="btn btn-primary" @click="goSettings">去配置</button>
      </div>
    </div>
  </div>
</template>
