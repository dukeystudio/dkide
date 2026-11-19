<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
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

type Page = 'chat' | 'settings' | 'advanced' | 'skills' | 'about';
const page = ref<Page>('chat');
const store = useChatStore();
const auth = useAuthStore();
const collapsed = ref(false);
const sidebarWidth = ref(248);
const resizingSidebar = ref(false);
const ready = ref(false);
const initError = ref('');
let stopSidebarResize: (() => void) | null = null;

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function startSidebarResize(event: MouseEvent) {
  if (collapsed.value) return;
  event.preventDefault();
  stopSidebarResize?.();
  const startX = event.clientX;
  const startWidth = sidebarWidth.value;
  resizingSidebar.value = true;
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
  const onMove = (moveEvent: MouseEvent) => {
    sidebarWidth.value = Math.round(clamp(startWidth + moveEvent.clientX - startX, 200, 400));
  };
  const onUp = () => stopSidebarResize?.();
  window.addEventListener('mousemove', onMove);
  window.addEventListener('mouseup', onUp, { once: true });
  stopSidebarResize = () => {
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
    resizingSidebar.value = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    stopSidebarResize = null;
  };
}

onUnmounted(() => stopSidebarResize?.());

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

function onNavigate(p: Page) {
  // "模型设置"改为打开服务商配置弹窗，不再切整页
  if (p === 'settings') { openProviderDialog(); return; }
  page.value = p;
}
function openProviderDialog(focusAdd = false, focusProviderId: string | null = null) {
  store.providerRequired = false;
  store.focusAddProvider = focusAdd;
  store.providerDialogFocusId = focusProviderId;
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

  <div
    v-else-if="ready"
    class="app-shell"
    :style="{ '--sidebar-width': `${sidebarWidth}px` }"
  >
    <Sidebar :page="page" :collapsed="collapsed" @navigate="onNavigate" @toggle-collapse="collapsed = !collapsed" />
    <div
      v-if="!collapsed"
      class="panel-resizer sidebar-resizer"
      :class="{ dragging: resizingSidebar }"
      title="拖动调整侧边栏宽度"
      @mousedown="startSidebarResize"
    ></div>
    <main class="main">
      <ChatView
        v-show="page === 'chat'"
        @open-settings="openProviderDialog(($event?.focusAdd) ?? false, ($event?.focusProviderId) ?? null)"
        @open-skills="page = 'skills'"
      />
      <SettingsPanel v-if="page === 'advanced'" />
      <SkillsPanel v-if="page === 'skills'" />
      <AboutPanel v-if="page === 'about'" />
    </main>
  </div>

  <ProviderDialog />
  <ApprovalDialog :approvals="store.pendingApprovals" @respond="store.respondApproval($event.id, $event.approved)" />
  <LoginDialog />

  <!-- 点空白不关闭：mask 不加 @click.self，只有按钮 -->
  <div v-if="store.providerRequired" class="modal-mask">
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
