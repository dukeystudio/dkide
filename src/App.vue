<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useChatStore } from './store';
import Sidebar from './components/Sidebar.vue';
import ChatView from './components/ChatView.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import SkillsPanel from './components/SkillsPanel.vue';
import AboutPanel from './components/AboutPanel.vue';
import Onboarding from './components/Onboarding.vue';
import ApprovalDialog from './components/ApprovalDialog.vue';

type Page = 'chat' | 'settings' | 'skills' | 'about';
const page = ref<Page>('chat');
const store = useChatStore();
const collapsed = ref(false);
const ready = ref(false);

onMounted(async () => {
  await store.init();
  ready.value = true;
});

const activeProvider = computed(() => {
  const c = store.config;
  if (!c) return null;
  return c.providers.find((p) => p.id === c.active_provider_id) ?? c.providers[0];
});

function onNavigate(p: Page) { page.value = p; }
</script>

<template>
  <Onboarding v-if="ready && store.config && !store.config.providers.some((p) => p.api_key)" @done="store.init()" />

  <div v-else class="app-shell">
    <Sidebar :page="page" :collapsed="collapsed" @navigate="onNavigate" @toggle-collapse="collapsed = !collapsed" />
    <main class="main">
      <ChatView v-if="page === 'chat'" :provider="activeProvider" />
      <SettingsPanel v-else-if="page === 'settings'" />
      <SkillsPanel v-else-if="page === 'skills'" />
      <AboutPanel v-else-if="page === 'about'" />
    </main>
  </div>

  <ApprovalDialog :approvals="store.pendingApprovals" @respond="store.respondApproval($event.id, $event.approved)" />
</template>
