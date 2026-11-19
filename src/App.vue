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

type Page = 'chat' | 'settings' | 'skills' | 'about';
const page = ref<Page>('chat');
const store = useChatStore();
const auth = useAuthStore();
const collapsed = ref(false);
const ready = ref(false);

onMounted(async () => {
  await store.init();
  ready.value = true;
  auth.init();
});

const activeProvider = computed(() => {
  const c = store.config;
  if (!c) return null;
  return c.providers.find((p) => p.id === c.active_provider_id) ?? c.providers[0];
});

function onNavigate(p: Page) { page.value = p; }
function goSettings() {
  store.providerRequired = false;
  page.value = 'settings';
}
</script>

<template>
  <div v-if="ready" class="app-shell">
    <Sidebar :page="page" :collapsed="collapsed" @navigate="onNavigate" @toggle-collapse="collapsed = !collapsed" />
    <main class="main">
      <ChatView
        v-if="page === 'chat'"
        :provider="activeProvider"
        @open-settings="page = 'settings'"
        @open-skills="page = 'skills'"
      />
      <SettingsPanel v-else-if="page === 'settings'" />
      <SkillsPanel v-else-if="page === 'skills'" />
      <AboutPanel v-else-if="page === 'about'" />
    </main>
  </div>

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
