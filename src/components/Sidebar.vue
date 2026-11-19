<script setup lang="ts">
import { computed, ref } from 'vue';
import { useChatStore } from '../store';
import { useAuthStore } from '../authStore';

const props = defineProps<{ page: string; collapsed: boolean }>();
const emit = defineEmits<{
  navigate: [page: 'chat' | 'settings' | 'skills' | 'about'];
  toggleCollapse: [];
}>();
const store = useChatStore();
const auth = useAuthStore();
const accountMenuOpen = ref(false);

const providerName = computed(() => {
  const c = store.config;
  if (!c) return '';
  const p = c.providers.find((x) => x.id === c.active_provider_id) ?? c.providers[0];
  return p?.name ?? '';
});

function onAccountClick() {
  if (!auth.isLoggedIn) {
    auth.requireLogin('登录后可开始对话');
    return;
  }
  accountMenuOpen.value = !accountMenuOpen.value;
}

async function doLogout() {
  accountMenuOpen.value = false;
  await auth.logout();
}
</script>

<template>
  <aside class="sidebar" :class="{ collapsed }">
    <div class="sidebar-header">
      <div class="brand"><span class="brand-dot"></span><span v-if="!collapsed">都客AI助手</span></div>
      <button class="icon-btn collapse-btn" @click="emit('toggleCollapse')" :title="collapsed ? '展开侧栏' : '收起侧栏'">
        {{ collapsed ? '»' : '«' }}
      </button>
    </div>

    <template v-if="page === 'chat'">
      <button class="new-chat-btn" @click="store.newSession()" title="新建会话">+ 新建会话</button>
      <div class="search-box">
        <input v-model="store.searchQuery" placeholder="搜索会话…" />
      </div>
      <div class="session-list">
        <div
          v-for="s in store.filteredSessions"
          :key="s.id"
          class="session-item"
          :class="{ active: s.id === store.currentId }"
          @click="store.selectSession(s.id)"
        >
          <span v-if="s.pinned" class="session-pin">📌</span>
          <span class="session-title">{{ s.title }}</span>
          <div class="session-actions">
            <button @click.stop="store.togglePin(s.id)" :title="s.pinned ? '取消置顶' : '置顶'">📌</button>
            <button @click.stop="store.deleteSession(s.id)" title="删除">✕</button>
          </div>
        </div>
      </div>
      <div class="usage-mini">
        本期用量 <b>{{ store.totalUsage.total_tokens.toLocaleString() }}</b> tokens ·
        <b>${{ store.totalUsage.estimated_cost.toFixed(4) }}</b>
      </div>
    </template>

    <div class="sidebar-footer">
      <div class="account-wrap">
        <button
          class="nav-btn account-entry"
          :class="{ active: accountMenuOpen }"
          @click="onAccountClick"
          :title="auth.isLoggedIn ? `账号 ${auth.maskedAccount} · 到期 ${auth.session?.expire_display || '未知'}` : '登录'"
        >
          <span class="nav-icon">👤</span>
          <span class="account-label">{{ auth.isLoggedIn ? auth.maskedAccount : '登录' }}</span>
        </button>
        <div v-if="accountMenuOpen && auth.isLoggedIn" class="account-menu">
          <div class="account-menu-title">{{ auth.maskedAccount }}</div>
          <div class="account-menu-sub">💰 余额 {{ auth.session?.coin ?? 0 }}</div>
          <div class="account-menu-sub" :title="`到期时间：${auth.session?.expire_display || '未知'}`">
            ⏳ 到期 {{ auth.session?.expire_display || '未知' }}
          </div>
          <button class="account-menu-item refresh" @click="auth.refreshQuota()">🔄 刷新余额</button>
          <button class="account-menu-item" @click="doLogout">退出登录</button>
        </div>
      </div>

      <button class="nav-btn" :class="{ active: page === 'chat' }" @click="emit('navigate', 'chat')" title="对话">
        <span class="nav-icon">💬</span><span class="nav-label">对话</span>
      </button>
      <button class="nav-btn" :class="{ active: page === 'settings' }" @click="emit('navigate', 'settings')" title="模型设置">
        <span class="nav-icon">⚙️</span><span class="nav-label">模型设置</span>
      </button>
      <button class="nav-btn" :class="{ active: page === 'about' }" @click="emit('navigate', 'about')" title="关于">
        <span class="nav-icon">ℹ️</span><span class="nav-label">关于</span>
      </button>
      <div v-if="!collapsed" class="skill-meta" style="padding:8px 12px 0;">{{ providerName }}</div>
    </div>
  </aside>
</template>
