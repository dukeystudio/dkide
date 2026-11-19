<script setup lang="ts">
import { ref, nextTick, watch, computed, onMounted, onUnmounted } from 'vue';
import { useChatStore } from '../store';
import { useSkillsStore } from '../skillsStore';
import { api } from '../api';
import { getCurrentWindow } from '@tauri-apps/api/window';
import ThemeMenu from './ThemeMenu.vue';
import { renderMarkdown, copyText } from '../markdown';
import type { Message, ToolCallView, GeneratedFile, Skill, TextAttachment } from '../types';

const emit = defineEmits<{ openSettings: [opts?: { focusAdd?: boolean; focusProviderId?: string }]; openSkills: [] }>();
const store = useChatStore();
const skills = useSkillsStore();
const input = ref('');
const attachments = ref<TextAttachment[]>([]);
const attachmentError = ref('');
const composerBoxEl = ref<HTMLElement | null>(null);
const dragOverComposer = ref(false);
const messagesEl = ref<HTMLElement | null>(null);
const expandedTools = ref<Set<string>>(new Set());
const editingId = ref<string | null>(null);
const editContent = ref('');
const copiedId = ref('');
const skillMenuOpen = ref(false);

const session = computed(() => store.currentSession);
const streaming = computed(() => session.value ? store.isStreaming(session.value.id) : false);
const streamText = computed(() => session.value ? store.textFor(session.value.id) : '');
const streamReasoning = computed(() => session.value ? store.reasoningFor(session.value.id) : '');
const streamTools = computed(() => session.value ? store.toolsFor(session.value.id) : []);
const streamFiles = computed(() => session.value ? store.filesFor(session.value.id) : []);
const streamUsage = computed(() => session.value ? store.usageFor(session.value.id) : null);
// 思考过程：生成中默认展开，生成完折叠（可手动点开）
const reasoningOpen = ref(true);
const workspaceOpen = ref(false);
const workspacePanelEl = ref<HTMLElement | null>(null);
const workspaceWidth = ref<number | null>(null);
const workspaceTreeWidth = ref<number | null>(null);
const resizingWorkspace = ref(false);
const resizingTree = ref(false);
const fileFilter = ref('');
const now = ref(Date.now());
let clockTimer: number | null = null;
let stopActiveResize: (() => void) | null = null;
let unlistenDragDrop: (() => void) | null = null;

const workspaceStyles = computed<Record<string, string>>(() => {
  const styles: Record<string, string> = {};
  if (workspaceWidth.value !== null) styles['--workspace-width'] = `${workspaceWidth.value}px`;
  if (workspaceTreeWidth.value !== null) styles['--workspace-tree-width'] = `${workspaceTreeWidth.value}px`;
  return styles;
});

function clampSize(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function startWorkspaceResize(event: MouseEvent) {
  const panel = workspacePanelEl.value;
  if (!panel) return;
  event.preventDefault();
  stopActiveResize?.();
  const container = panel.parentElement;
  const startX = event.clientX;
  const startWidth = panel.getBoundingClientRect().width;
  resizingWorkspace.value = true;
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
  const onMove = (moveEvent: MouseEvent) => {
    const available = container?.clientWidth ?? window.innerWidth;
    const maxWidth = Math.max(320, Math.min(720, available - 420));
    workspaceWidth.value = Math.round(clampSize(startWidth + startX - moveEvent.clientX, 320, maxWidth));
  };
  const finish = () => {
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', finish);
    resizingWorkspace.value = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    stopActiveResize = null;
  };
  window.addEventListener('mousemove', onMove);
  window.addEventListener('mouseup', finish);
  stopActiveResize = finish;
}

function startTreeResize(event: MouseEvent) {
  const panel = workspacePanelEl.value;
  const tree = panel?.querySelector('.workspace-tree') as HTMLElement | null;
  if (!panel || !tree) return;
  event.preventDefault();
  stopActiveResize?.();
  const startX = event.clientX;
  const startWidth = tree.getBoundingClientRect().width;
  resizingTree.value = true;
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
  const onMove = (moveEvent: MouseEvent) => {
    const maxWidth = Math.max(150, Math.min(360, panel.clientWidth - 240));
    workspaceTreeWidth.value = Math.round(clampSize(startWidth + moveEvent.clientX - startX, 150, maxWidth));
  };
  const finish = () => {
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', finish);
    resizingTree.value = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    stopActiveResize = null;
  };
  window.addEventListener('mousemove', onMove);
  window.addEventListener('mouseup', finish);
  stopActiveResize = finish;
}

function formatDuration(ms: number) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const pad = (value: number) => String(value).padStart(2, '0');
  return hours > 0 ? `${pad(hours)}:${pad(minutes)}:${pad(seconds)}` : `${pad(minutes)}:${pad(seconds)}`;
}
const generationStartedAt = computed(() => session.value ? store.startTimeFor(session.value.id) : 0);
const generationElapsed = computed(() => generationStartedAt.value ? formatDuration(now.value - generationStartedAt.value) : '00:00');
function toolDuration(tool: ToolCallView) {
  return formatDuration((tool.finished_at ?? now.value) - tool.started_at);
}

onMounted(() => {
  clockTimer = window.setInterval(() => { now.value = Date.now(); }, 1000);
  void setupDragDrop();
});
onUnmounted(() => {
  if (clockTimer !== null) window.clearInterval(clockTimer);
  stopActiveResize?.();
  unlistenDragDrop?.();
});
// 历史消息思考过程：按消息 id 记录展开状态（默认收起，点箭头展开）
const historyReasoningOpen = ref<Record<string, boolean>>({});
function toggleHistoryReasoning(id: string) {
  historyReasoningOpen.value[id] = !historyReasoningOpen.value[id];
}
// 文件预览
const previewFile = ref<GeneratedFile | null>(null);
const previewContent = ref('');
const previewLoading = ref(false);
const previewError = ref('');
const copiedFileId = ref('');
const copyTip = ref('');
const availableSkills = computed(() => skills.skills);
const selectedSkillCount = computed(() => availableSkills.value.filter(isSkillSelected).length);
const canSend = computed(() => {
  const parsed = extractSkillNames(input.value.trim());
  return !!(parsed.content || attachments.value.length);
});

function isSkillSelected(skill: Skill) {
  return input.value.includes(`@${skill.meta.name}`);
}

function toggleSkill(skill: Skill) {
  const token = `@${skill.meta.name}`;
  if (isSkillSelected(skill)) {
    input.value = input.value.split(token).join('').replace(/[ \t]+/g, ' ').trimStart();
  } else {
    input.value = `${token} ${input.value}`.trim();
  }
}

function extractSkillNames(content: string) {
  const skillNames: string[] = [];
  let cleaned = content;
  for (const skill of availableSkills.value) {
    const token = `@${skill.meta.name}`;
    if (!cleaned.includes(token)) continue;
    skillNames.push(skill.dir_name);
    cleaned = cleaned.split(token).join('');
  }
  return { content: cleaned.replace(/[ \t]+/g, ' ').trim(), skillNames };
}

function scrollToBottom(force = false) {
  nextTick(() => {
    const el = messagesEl.value;
    if (!el) return;
    // 非强制且用户已往上翻看（不在底部）时，不强行拉回，避免生成中跳屏
    if (!force) {
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
      if (!atBottom) return;
    }
    el.scrollTop = el.scrollHeight;
    // 双保险：markdown/图片渲染可能还没撑满高度，再滚一次
    requestAnimationFrame(() => { el.scrollTop = el.scrollHeight; });
  });
}

function onDocClick(e: MouseEvent) {
  const el = e.target as HTMLElement;
  if (!el.closest('.toolbar-menu')) {
    skillMenuOpen.value = false;
  }
}
onMounted(() => document.addEventListener('click', onDocClick));
onUnmounted(() => document.removeEventListener('click', onDocClick));

// 切换会话：强制滚到底（打开对话就停留在最后一条）
watch(
  () => store.currentId,
  () => scrollToBottom(true)
);
// 生成增量：贴底才跟随，不强制
watch(
  () => [streamText.value, streamTools.value.length, session.value?.messages.length],
  () => scrollToBottom(false)
);
// 挂载即滚到底
onMounted(() => scrollToBottom(true));

function md(content: string) { return renderMarkdown(content); }

async function send() {
  const parsed = extractSkillNames(input.value.trim());
  const text = parsed.content;
  if ((!text && !attachments.value.length) || streaming.value) return;
  const attachmentText = attachments.value.map((item) => {
    const truncatedTip = item.truncated ? '\n（内容过长已截断）' : '';
    return `【附件：${item.name}】${truncatedTip}\n${item.content}`;
  }).join('\n\n');
  const content = [text, attachmentText].filter(Boolean).join('\n\n');
  await store.send(content, () => {
    input.value = '';
    attachments.value = [];
    attachmentError.value = '';
  }, parsed.skillNames);
  scrollToBottom();
}

async function attachTextFile() {
  attachmentError.value = '';
  try {
    const path = await api.pickTextAttachment();
    if (!path) return;
    const attachment = await api.readTextAttachment(path);
    attachments.value.push(attachment);
  } catch (error) {
    const message = typeof error === 'string' ? error : (error as Error)?.message ?? String(error);
    attachmentError.value = message.replace(/^.*?(failed|error):\s*/i, '') || message;
  }
}

function removeAttachment(index: number) {
  attachments.value.splice(index, 1);
  attachmentError.value = '';
}

async function addAttachmentFromPath(path: string) {
  try {
    const attachment = await api.readTextAttachment(path);
    attachments.value.push(attachment);
    attachmentError.value = '';
  } catch (error) {
    const message = typeof error === 'string' ? error : (error as Error)?.message ?? String(error);
    attachmentError.value = message.replace(/^.*?(failed|error):\s*/i, '') || message;
  }
}

function isPositionInComposer(position?: { x: number; y: number }) {
  const box = composerBoxEl.value;
  if (!box || !position) return true;
  const rect = box.getBoundingClientRect();
  return position.x >= rect.left && position.x <= rect.right && position.y >= rect.top && position.y <= rect.bottom;
}

async function setupDragDrop() {
  try {
    const appWindow = getCurrentWindow();
    const scaleFactor = await appWindow.scaleFactor();
    unlistenDragDrop = await appWindow.onDragDropEvent((event) => {
      const payload = event.payload as {
        type: 'enter' | 'over' | 'leave' | 'drop';
        paths?: string[];
        position?: {
          x: number;
          y: number;
          toLogical?: (factor: number) => { x: number; y: number };
        };
      };
      const rawPosition = payload.position;
      const logicalPosition = rawPosition?.toLogical
        ? rawPosition.toLogical(scaleFactor)
        : rawPosition;
      const inComposer = isPositionInComposer(logicalPosition);
      if (payload.type === 'enter') {
        dragOverComposer.value = inComposer && (payload.paths?.length ?? 0) > 0;
      } else if (payload.type === 'over') {
        dragOverComposer.value = inComposer;
      } else if (payload.type === 'leave') {
        dragOverComposer.value = false;
      } else if (payload.type === 'drop') {
        dragOverComposer.value = false;
        if (!inComposer) return;
        payload.paths?.forEach((path) => { void addAttachmentFromPath(path); });
      }
    });
  } catch {
    dragOverComposer.value = false;
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    send();
  }
}

/** 原生 select 切换模型 */
const providersList = computed(() => store.config?.providers ?? []);
async function onModelChange(id: string) {
  const c = store.config;
  if (!c || !id || c.active_provider_id === id) return;
  await store.saveConfig({ ...c, active_provider_id: id });
}
/** 打开服务商弹窗并聚焦"添加服务商" */
function goAddProvider() {
  store.focusAddProvider = true;
  store.providerDialogFocusId = null;
  store.providerDialogOpen = true;
}

/** 打开服务商弹窗并定位到当前模型 */
function goEditProvider() {
  store.focusAddProvider = false;
  store.providerDialogFocusId = store.config?.active_provider_id ?? null;
  store.providerDialogOpen = true;
}

function onSkillButtonClick() {
  skillMenuOpen.value = !skillMenuOpen.value;
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
async function saveEdit(m: Message, onlyUpdate = false) {
  const ok = await store.editMessage(m.id, editContent.value, onlyUpdate);
  if (!ok) showCopyTip('未保存：已切换到其他会话，请回到原会话再编辑');
  editingId.value = null;
}

async function openFilePreview(f: GeneratedFile) {
  previewFile.value = f;
  previewContent.value = '';
  previewLoading.value = true;
  previewError.value = '';
  try {
    previewContent.value = await api.readFilePreview(f.abs_path);
  } catch (e: any) {
    previewError.value = typeof e === 'string' ? e : e?.message ?? String(e);
  } finally {
    previewLoading.value = false;
  }
}
function closeFilePreview() { previewFile.value = null; previewContent.value = ''; }
async function openFileInOs(f: GeneratedFile) {
  await api.openFileInOs(f.abs_path);
}
function showCopyTip(msg: string) {
  copyTip.value = msg;
  setTimeout(() => { if (copyTip.value === msg) copyTip.value = ''; }, 2500);
}
async function copyFileContent(f: GeneratedFile) {
  // 抽屉已加载且无错误就直接用；否则（二进制/越界/loading）重新读一次
  const fromPreview = f.abs_path === previewFile.value?.abs_path ? previewContent.value : '';
  let text = (!previewError.value && fromPreview) ? fromPreview : '';
  if (!text) {
    try { text = await api.readFilePreview(f.abs_path); }
    catch (e: any) {
      const reason = typeof e === 'string' ? e : e?.message ?? String(e);
      showCopyTip(`复制失败：${reason}`);
      return;
    }
  }
  await copyText(text);
  copiedFileId.value = f.abs_path;
  setTimeout(() => { if (copiedFileId.value === f.abs_path) copiedFileId.value = ''; }, 1500);
  // 截断内容给出提示（后端在结尾追加了"…已截断"标记）
  if (text.includes('…（内容过长已截断')) showCopyTip('已复制（内容过长，仅前 10 万字符）');
}
function fileIcon(f: GeneratedFile) {
  const ext = f.path.split('.').pop()?.toLowerCase() ?? '';
  const map: Record<string, string> = {
    ts: '🟦', tsx: '🟦', js: '🟨', jsx: '🟨', json: '🟨',
    md: '📝', mdx: '📝', txt: '📝',
    rs: '🦀', py: '🐍', go: '🐹', java: '☕', c: '⚙️', cpp: '⚙️', h: '⚙️',
    html: '🌐', htm: '🌐', css: '🎨', scss: '🎨', less: '🎨',
    svg: '🖼', png: '🖼', jpg: '🖼', jpeg: '🖼', gif: '🖼',
    sh: '⌨️', zsh: '⌨️', bat: '⌨️', ps1: '⌨️',
    yml: '📋', yaml: '📋', toml: '📋', ini: '📋',
    sql: '🗄', dockerfile: '🐳',
  };
  return map[ext] ?? '📄';
}
function fileName(f: GeneratedFile) {
  return f.path.split('/').pop() || f.path;
}

const filteredFiles = computed(() => {
  const keyword = fileFilter.value.trim().toLowerCase();
  if (!keyword) return streamFiles.value;
  return streamFiles.value.filter((f) => `${f.path} ${fileName(f)}`.toLowerCase().includes(keyword));
});

watch(streamFiles, (files) => {
  if (!files.length) {
    if (previewFile.value) closeFilePreview();
    workspaceOpen.value = false;
    return;
  }
  if (!files.some((f) => f.abs_path === previewFile.value?.abs_path)) {
    void openFilePreview(files[0]);
  }
  workspaceOpen.value = true;
});

function toggleWorkspace() {
  if (workspaceOpen.value) {
    workspaceOpen.value = false;
    return;
  }
  workspaceOpen.value = true;
  if (!previewFile.value && streamFiles.value.length) void openFilePreview(streamFiles.value[0]);
}

function locateFileFromTool(tool: ToolCallView) {
  const raw = `${tool.args} ${tool.result ?? ''}`;
  const matched = streamFiles.value.find((f) =>
    raw.includes(f.path) || raw.includes(f.abs_path) || raw.includes(fileName(f))
  );
  workspaceOpen.value = true;
  void openFilePreview(matched ?? streamFiles.value[streamFiles.value.length - 1]);
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
  <div class="chat-view">
    <div class="chat-column">
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
        <button
          class="topbar-files"
          :class="{ active: workspaceOpen }"
          title="打开或关闭生成文件面板"
          @click="toggleWorkspace"
        >📁 文件<span v-if="streamFiles.length" class="topbar-files-count">{{ streamFiles.length }}</span></button>
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
                <button v-if="m.role === 'user'" title="撤销到此：删除该条提问及其后所有回复" @click="store.undo(m.id)">↩ 撤销到此</button>
                <button @click="store.deleteMessage(store.currentId!, m.id)">删除</button>
              </div>
            </div>
            <div v-if="editingId === m.id" class="edit-box">
              <textarea v-model="editContent" class="inline-input" style="min-height:80px;"></textarea>
              <div style="display:flex;gap:8px;margin-top:6px;align-items:center;">
                <button v-if="m.role === 'user'" class="btn btn-sm btn-primary" @click="saveEdit(m)">保存并继续</button>
                <button v-else class="btn btn-sm btn-primary" @click="saveEdit(m, true)">保存</button>
                <button class="btn btn-sm" @click="editingId = null">取消</button>
                <span v-if="m.role === 'user'" style="font-size:11px;color:var(--text-faint)">保存后将从此重新生成</span>
              </div>
            </div>
            <template v-if="editingId !== m.id">
              <div v-if="m.role === 'assistant' && m.reasoning && store.config?.show_reasoning" class="reasoning" :class="{ open: historyReasoningOpen[m.id] }">
                <div class="reasoning-head" @click="toggleHistoryReasoning(m.id)">
                  <span>💭 思考过程</span>
                  <span class="reasoning-toggle">{{ historyReasoningOpen[m.id] ? '收起' : '展开' }}</span>
                </div>
                <div v-if="historyReasoningOpen[m.id]" class="reasoning-body" v-html="md(m.reasoning)"></div>
              </div>
              <div class="bubble" v-html="md(m.content)" @click="onBubbleClick"></div>
            </template>
          </div>
        </div>

        <div v-if="streaming" class="msg assistant">
          <div class="msg-avatar">AI</div>
          <div class="msg-body">
            <div class="msg-name">助手 <span class="stream-elapsed">生成中… {{ generationElapsed }}</span></div>
            <div v-if="store.config?.show_reasoning && (streamReasoning || streaming)" class="reasoning" :class="{ open: reasoningOpen }">
              <div class="reasoning-head" @click="reasoningOpen = !reasoningOpen">
                <span>💭 思考过程</span>
                <span class="reasoning-time">{{ generationElapsed }}</span>
                <span class="reasoning-toggle">{{ reasoningOpen ? '收起' : '展开' }}</span>
              </div>
              <div v-if="reasoningOpen" class="reasoning-body">
                <div v-if="streamReasoning" v-html="md(streamReasoning)"></div>
                <div v-else class="reasoning-placeholder">该模型未返回思考文本，本轮已用时 {{ generationElapsed }}</div>
              </div>
            </div>
            <div v-for="(t, ti) in streamTools" :key="ti" class="tool-call">
              <div class="tool-call-head" @click="toggleTool(ti)">
                <span>{{ toolIcon(t.name) }}</span>
                <span>{{ t.name === 'activate_skill' ? '加载技能' : t.name === 'run_command' ? '执行命令' : t.name === 'write_file' ? '写入文件' : t.name }}</span>
                <span class="tool-status">{{ t.status === 'running' ? '执行中' : '完成' }} {{ toolDuration(t) }}</span>
                <span style="margin-left:auto;">{{ isToolExpanded(ti) ? '▾' : '▸' }}</span>
              </div>
              <div v-if="isToolExpanded(ti)" class="tool-call-body">
                <div><span class="tool-label">参数 </span>{{ t.args }}</div>
                <div v-if="t.result" style="margin-top:6px;"><span class="tool-label">结果 </span>{{ t.result }}</div>
                <button
                  v-if="t.name === 'write_file' && streamFiles.length"
                  class="tool-file-link"
                  @click.stop="locateFileFromTool(t)"
                >📁 在文件面板中查看</button>
              </div>
            </div>
            <div class="bubble typing" v-html="md(streamText)"></div>
            <div v-if="streamUsage && streamUsage.total_tokens" class="usage-bar">
              {{ streamUsage.total_tokens }} tokens · ${{ streamUsage.estimated_cost.toFixed(4) }} · 用时 {{ generationElapsed }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="composer">
      <div class="composer-inner">
        <div v-if="store.error" class="error-box">{{ store.error }}</div>
        <div v-if="copyTip" class="copy-tip">{{ copyTip }}</div>
        <div ref="composerBoxEl" class="composer-box">
          <div v-if="dragOverComposer" class="drop-overlay">松开即可添加为文本附件</div>
          <div v-if="attachments.length" class="attachment-list">
            <span v-for="(item, index) in attachments" :key="`${item.name}-${index}`" class="attachment-chip">
              <span class="attachment-name" :title="item.name">文本 {{ item.name }}</span>
              <span v-if="item.truncated" class="attachment-warning">已截断</span>
              <button class="attachment-remove" title="移除附件" @click="removeAttachment(index)">✕</button>
            </span>
          </div>
          <div v-if="attachmentError" class="attachment-error">{{ attachmentError }}</div>
          <textarea
            v-model="input"
            placeholder="输入消息，Enter 发送，Shift+Enter 换行"
            @keydown="onKeydown"
          ></textarea>
          <div class="composer-toolbar">
            <div class="toolbar-left">
              <div class="toolbar-menu model-select-wrap">
                <span class="tb-icon">◇</span>
                <select
                  class="model-select"
                  :value="store.config?.active_provider_id"
                  :disabled="!providersList.length"
                  @change="onModelChange(($event.target as HTMLSelectElement).value)"
                >
                  <option v-if="!providersList.length" value="">未配置模型</option>
                  <option v-for="p in providersList" :key="p.id" :value="p.id">
                    {{ p.name }}
                  </option>
                </select>
                <button class="tb-add" title="添加模型" @click="goAddProvider">＋</button>
                <button class="tb-edit" title="编辑当前模型" @click="goEditProvider">编辑</button>
              </div>

              <div class="toolbar-menu">
                <button class="toolbar-btn" title="上传文本、代码或配置文件作为对话附件" @click="attachTextFile">
                  <span class="tb-icon">＋</span>
                  <span class="tb-label">文本</span>
                </button>
              </div>

              <div class="toolbar-menu">
                <button class="toolbar-btn" :class="{ open: skillMenuOpen }" @click="onSkillButtonClick">
                  <span class="tb-icon">🧩</span>
                  <span class="tb-label">技能</span>
                  <span class="tb-count">{{ selectedSkillCount }}</span>
                  <span class="tb-caret">▾</span>
                </button>
                <div v-if="skillMenuOpen" class="toolbar-popover skill-popover" @click.stop>
                  <div class="popover-head">选择本轮技能（输入框会显示 @技能名）</div>
                  <div v-if="!availableSkills.length" class="popover-empty">还没有安装技能</div>
                  <div
                    v-for="s in availableSkills"
                    :key="s.dir_name"
                    class="popover-item selectable"
                    :class="{ selected: isSkillSelected(s) }"
                    @click.stop="toggleSkill(s)"
                  >
                    <span class="pi-check">{{ isSkillSelected(s) ? '✓' : '' }}</span>
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
              <button v-else class="send-btn" :disabled="!canSend" @click="send">发送</button>
            </div>
          </div>
        </div>
      </div>
    </div>
    </div>

    <div
      v-if="workspaceOpen"
      class="panel-resizer workspace-resizer"
      :class="{ dragging: resizingWorkspace }"
      title="拖动调整文件面板宽度"
      @mousedown="startWorkspaceResize"
    ></div>

    <aside v-if="workspaceOpen" ref="workspacePanelEl" class="workspace-panel" :style="workspaceStyles">
      <div class="workspace-head">
        <span class="workspace-title">生成文件</span>
        <span class="workspace-count">{{ streamFiles.length }}</span>
        <button class="workspace-close" title="关闭文件面板" @click="workspaceOpen = false">✕</button>
      </div>
      <div v-if="!streamFiles.length" class="workspace-empty">本次对话还没有生成文件</div>
      <div v-else class="workspace-body">
        <div class="workspace-tree">
          <input v-model="fileFilter" class="workspace-filter" placeholder="筛选文件…" />
          <div v-if="!filteredFiles.length" class="workspace-empty">没有匹配的文件</div>
          <div
            v-for="f in filteredFiles"
            :key="f.abs_path"
            class="workspace-file"
            :class="{ active: previewFile?.abs_path === f.abs_path }"
            :title="f.path"
            @click="openFilePreview(f)"
          >
            <span class="wf-icon">{{ fileIcon(f) }}</span>
            <span class="wf-name">{{ fileName(f) }}</span>
          </div>
        </div>
        <div
          class="panel-resizer tree-resizer"
          :class="{ dragging: resizingTree }"
          title="拖动调整文件列表宽度"
          @mousedown="startTreeResize"
        ></div>
        <div class="workspace-preview">
          <div class="workspace-preview-head">
            <span class="wp-path">{{ previewFile?.path ?? '选择文件查看内容' }}</span>
            <template v-if="previewFile">
              <button class="wp-btn" :title="copiedFileId === previewFile.abs_path ? '已复制' : '复制文件内容到剪贴板'" @click="copyFileContent(previewFile)">{{ copiedFileId === previewFile.abs_path ? '已复制' : '复制' }}</button>
              <button class="wp-btn" title="用系统应用打开" @click="openFileInOs(previewFile)">打开</button>
            </template>
          </div>
          <div v-if="!previewFile" class="wp-empty">选择左侧文件查看内容</div>
          <div v-else-if="previewLoading" class="wp-body">加载中…</div>
          <div v-else-if="previewError" class="wp-body wp-error">{{ previewError }}</div>
          <pre v-else class="wp-body wp-code">{{ previewContent }}</pre>
        </div>
      </div>
    </aside>
  </div>
</template>
