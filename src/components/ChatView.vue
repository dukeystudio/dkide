<script setup lang="ts">
import { ref, nextTick, watch, computed, onMounted, onUnmounted } from 'vue';
import { useChatStore } from '../store';
import { useSkillsStore } from '../skillsStore';
import { api } from '../api';
import ThemeMenu from './ThemeMenu.vue';
import { renderMarkdown, copyText } from '../markdown';
import type { Provider, Message, ToolCallView } from '../types';

const props = defineProps<{ provider: Provider | null }>();
const emit = defineEmits<{ openSettings: []; openSkills: [] }>();
const store = useChatStore();
const skills = useSkillsStore();
const input = ref('');
const messagesEl = ref<HTMLElement | null>(null);
const expandedTools = ref<Set<string>>(new Set());
const editingId = ref<string | null>(null);
const editContent = ref('');
const copiedId = ref('');
const modelMenuOpen = ref(false);
const skillMenuOpen = ref(false);

const session = computed(() => store.currentSession);
const streaming = computed(() => session.value ? store.isStreaming(session.value.id) : false);
const streamText = computed(() => session.value ? store.textFor(session.value.id) : '');
const streamTools = computed(() => session.value ? store.toolsFor(session.value.id) : []);
const streamUsage = computed(() => session.value ? store.usageFor(session.value.id) : null);
const renderedProvider = computed(() =>
  props.provider ? `${props.provider.name} · ${props.provider.model}` : '未配置模型'
);
const enabledSkills = computed(() => {
  const ids = new Set(store.config?.enabled_skills ?? []);
  return skills.skills.filter((s) => ids.has(s.dir_name) || s.enabled);
});

function scrollToBottom() {
  nextTick(() => {
    if (messagesEl.value) messagesEl.value.scrollTop = messagesEl.value.scrollHeight;
  });
}

function onDocClick(e: MouseEvent) {
  const el = e.target as HTMLElement;
  if (!el.closest('.toolbar-menu')) {
    modelMenuOpen.value = false;
    skillMenuOpen.value = false;
  }
}
onMounted(() => document.addEventListener('click', onDocClick));
onUnmounted(() => document.removeEventListener('click', onDocClick));

watch(
  () => [streamText.value, streamTools.value.length, session.value?.messages.length, store.currentId],
  scrollToBottom
);

function md(content: string) { return renderMarkdown(content); }

async function send() {
  const text = input.value.trim();
  if (!text || streaming.value) return;
  const ok = await store.send(text);
  if (ok) input.value = '';
  scrollToBottom();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    send();
  }
}

async function selectProvider(p: Provider) {
  modelMenuOpen.value = false;
  const c = store.config;
  if (!c || c.active_provider_id === p.id) return;
  await store.saveConfig({ ...c, active_provider_id: p.id });
}

function onModelButtonClick() {
  if (!store.config?.providers.some((p) => p.api_key)) {
    emit('openSettings');
    return;
  }
  modelMenuOpen.value = !modelMenuOpen.value;
  skillMenuOpen.value = false;
}

function onSkillButtonClick() {
  skillMenuOpen.value = !skillMenuOpen.value;
  modelMenuOpen.value = false;
}

function toggleTool(idx: number) {
  const key = `${store.currentId}-${idx}`;
  const next = new Set(expandedTools.value);
  next.has(key) ? next.delete(key) : next.add(key);
  expandedTools.value = next;
}
function isToolExpanded(idx: number) { return expandedTools.value.has(`${store.currentId}-${idx}`); }
function toolIcon(name: string) {
  const map: Record<string, string> = {
    read_file: '📖', write_file: '✍️', list_files: '📂', run_command: '⌨️', activate_skill: '🧩',
  };
  return map[name] ?? '🔧';
}

async function copyMessage(m: Message) {
  await copyText(m.content);
  copiedId.value = m.id;
  setTimeout(() => { copiedId.value = ''; }, 1500);
}

function startEdit(m: Message) { editingId.value = m.id; editContent.value = m.content; }
async function saveEdit(m: Message) {
  await store.editMessage(m.id, editContent.value);
  editingId.value = null;
}

async function onBubbleClick(e: MouseEvent) {
  const target = e.target as HTMLElement;
  if (target.classList.contains('copy-code')) {
    const code = target.parentElement?.querySelector('code')?.textContent ?? '';
    await copyText(code);
    target.textContent = '已复制';
    setTimeout(() => { target.textContent = '复制'; }, 1500);
  }
}
</script>

<template>
  <div class="chat-view" style="display:flex;flex-direction:column;height:100%;">
    <div class="topbar">
      <div class="topbar-title">{{ session?.title ?? '对话' }}</div>
      <div class="topbar-provider">
        <button
          v-if="session?.messages.length"
          class="topbar-export"
          title="导出当前对话为 Markdown"
          @click="api.exportSession(session.id, session.title)"
        >⬇ 导出</button>
        <ThemeMenu />
        <span class="model-badge">{{ renderedProvider }}</span>
      </div>
    </div>

    <div class="messages" ref="messagesEl">
      <div class="messages-inner">
        <div v-if="!session?.messages.length" class="empty-state">
          <h2>开始对话</h2>
          <p>向 AI 助手提问，它会按需自动加载技能并调用工具。</p>
        </div>

        <div v-for="(m, mi) in session?.messages" :key="m.id" class="msg" :class="m.role">
          <div class="msg-avatar">{{ m.role === 'user' ? '我' : 'AI' }}</div>
          <div class="msg-body">
            <div class="msg-name">
              {{ m.role === 'user' ? '你' : '助手' }}
              <div class="msg-actions">
                <button @click="copyMessage(m)">{{ copiedId === m.id ? '已复制' : '复制' }}</button>
                <button v-if="m.role === 'assistant'" @click="store.regenerate(mi)">重新生成</button>
                <button @click="startEdit(m)">编辑</button>
                <button @click="store.deleteMessage(store.currentId!, m.id)">删除</button>
              </div>
            </div>
            <div v-if="editingId === m.id" class="edit-box">
              <textarea v-model="editContent" class="inline-input" style="min-height:80px;"></textarea>
              <div style="display:flex;gap:8px;margin-top:6px;">
                <button class="btn btn-sm btn-primary" @click="saveEdit(m)">保存</button>
                <button class="btn btn-sm" @click="editingId = null">取消</button>
              </div>
            </div>
            <div v-else class="bubble" v-html="md(m.content)" @click="onBubbleClick"></div>
          </div>
        </div>

        <div v-if="streaming" class="msg assistant">
          <div class="msg-avatar">AI</div>
          <div class="msg-body">
            <div class="msg-name">助手 <span style="font-size:11px;color:var(--text-faint)">生成中…</span></div>
            <div v-for="(t, ti) in streamTools" :key="ti" class="tool-call">
              <div class="tool-call-head" @click="toggleTool(ti)">
                <span>{{ toolIcon(t.name) }}</span>
                <span>{{ t.name === 'activate_skill' ? '加载技能' : t.name === 'run_command' ? '执行命令' : t.name === 'write_file' ? '写入文件' : t.name }}</span>
                <span class="tool-status">{{ t.status === 'running' ? '执行中' : '完成' }}</span>
                <span style="margin-left:auto;">{{ isToolExpanded(ti) ? '▾' : '▸' }}</span>
              </div>
              <div v-if="isToolExpanded(ti)" class="tool-call-body">
                <div><span class="tool-label">参数 </span>{{ t.args }}</div>
                <div v-if="t.result" style="margin-top:6px;"><span class="tool-label">结果 </span>{{ t.result }}</div>
              </div>
            </div>
            <div class="bubble typing" v-html="md(streamText)"></div>
            <div v-if="streamUsage && streamUsage.total_tokens" class="usage-bar">
              {{ streamUsage.total_tokens }} tokens · ${{ streamUsage.estimated_cost.toFixed(4) }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="composer">
      <div class="composer-inner">
        <div v-if="store.error" class="error-box">{{ store.error }}</div>
        <div class="composer-box">
          <textarea
            v-model="input"
            placeholder="输入消息，Enter 发送，Shift+Enter 换行"
            @keydown="onKeydown"
          ></textarea>
          <div class="composer-toolbar">
            <div class="toolbar-left">
              <div class="toolbar-menu">
                <button class="toolbar-btn" :class="{ open: modelMenuOpen }" @click="onModelButtonClick">
                  <span class="tb-icon">◇</span>
                  <span class="tb-label">{{ props.provider ? props.provider.model : '选择模型' }}</span>
                  <span class="tb-caret">▾</span>
                </button>
                <div v-if="modelMenuOpen" class="toolbar-popover model-popover" @click.stop>
                  <div class="popover-head">切换模型</div>
                  <button
                    v-for="p in store.config?.providers ?? []"
                    :key="p.id"
                    class="popover-item"
                    :class="{ selected: p.id === store.config?.active_provider_id }"
                    @click="selectProvider(p)"
                  >
                    <span class="pi-check">{{ p.id === store.config?.active_provider_id ? '✓' : '' }}</span>
                    <span class="pi-main">
                      <span class="pi-name">{{ p.name }}</span>
                      <span class="pi-sub">{{ p.model }} · {{ p.base_url }}</span>
                    </span>
                  </button>
                  <button class="popover-foot" @click="modelMenuOpen = false; emit('openSettings')">⚙️ 管理模型服务商</button>
                </div>
              </div>

              <div class="toolbar-menu">
                <button class="toolbar-btn" :class="{ open: skillMenuOpen }" @click="onSkillButtonClick">
                  <span class="tb-icon">🧩</span>
                  <span class="tb-label">技能</span>
                  <span class="tb-count">{{ enabledSkills.length }}</span>
                  <span class="tb-caret">▾</span>
                </button>
                <div v-if="skillMenuOpen" class="toolbar-popover skill-popover" @click.stop>
                  <div class="popover-head">已启用技能（对话时按需自动加载）</div>
                  <div v-if="!enabledSkills.length" class="popover-empty">还没有启用技能</div>
                  <div v-for="s in enabledSkills" :key="s.dir_name" class="popover-item static">
                    <span class="pi-check">✓</span>
                    <span class="pi-main">
                      <span class="pi-name">🧩 {{ s.meta.name }}</span>
                      <span class="pi-sub">{{ s.meta.description || '暂无描述' }}</span>
                    </span>
                  </div>
                  <button class="popover-foot" @click="skillMenuOpen = false; emit('openSkills')">＋ 管理 / 安装技能</button>
                </div>
              </div>
            </div>
            <div class="composer-actions">
              <button v-if="streaming" class="stop-btn" @click="store.cancel(store.currentId!)">停止生成</button>
              <button v-else class="send-btn" :disabled="!input.trim()" @click="send">发送</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
