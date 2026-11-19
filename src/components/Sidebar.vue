<script setup lang="ts">
import { computed } from 'vue';
import { useChatStore } from '../store';

const props = defineProps<{ page: string; collapsed: boolean }>();
const emit = defineEmits<{
  navigate: [page: 'chat' | 'settings' | 'skills' | 'about'];
  toggleCollapse: [];
}>();
const store = useChatStore();

const providerName = computed(() => {
  const c = store.config;
  if (!c) return '';
  const p = c.providers.find((x) => x.id === c.active_provider_id) ?? c.providers[0];
  return p?.name ?? '';
});
</script>

<template>
  <aside class="sidebar" :class="{ collapsed }">
    <div class="sidebar-header">
      <div class="brand"><span class="brand-dot"></span><span>都客AI助手</span></div>
      <button class="icon-btn" @click="emit('toggleCollapse')" :title="collapsed ? '展开' : '收起'">
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
      <button class="nav-btn" :class="{ active: page === 'chat' }" @click="emit('navigate', 'chat')">
        <span class="nav-icon">💬</span><span>对话</span>
      </button>
      <button class="nav-btn" :class="{ active: page === 'skills' }" @click="emit('navigate', 'skills')">
        <span class="nav-icon">🧩</span><span>技能管理</span>
      </button>
      <button class="nav-btn" :class="{ active: page === 'settings' }" @click="emit('navigate', 'settings')">
        <span class="nav-icon">⚙️</span><span>模型设置</span>
      </button>
      <button class="nav-btn" :class="{ active: page === 'about' }" @click="emit('navigate', 'about')">
        <span class="nav-icon">ℹ️</span><span>关于</span>
      </button>
      <div v-if="!collapsed" class="skill-meta" style="padding:8px 12px 0;">{{ providerName }}</div>
    </div>
  </aside>
</template>
