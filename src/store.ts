import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { api } from './api';
import { useSkillsStore } from './skillsStore';
import { useAuthStore } from './authStore';
import type { AppConfig, Approval, Message, Session, StreamEvent, ToolCallView, UsageStats } from './types';

export const useChatStore = defineStore('chat', () => {
  const sessions = ref<Session[]>([]);
  const currentId = ref<string | null>(null);
  const config = ref<AppConfig | null>(null);
  const error = ref('');
  const searchQuery = ref('');
  const providerRequired = ref(false);

  // 按会话隔离的流式状态
  const streamingIds = ref<Set<string>>(new Set());
  const streamText = ref<Record<string, string>>({});
  const streamTools = ref<Record<string, ToolCallView[]>>({});
  const streamUsage = ref<Record<string, UsageStats>>({});

  const pendingApprovals = ref<Approval[]>([]);
  let approvalTimer: number | null = null;

  const currentSession = computed(() =>
    sessions.value.find((s) => s.id === currentId.value) ?? null
  );
  const filteredSessions = computed(() => {
    const q = searchQuery.value.toLowerCase();
    const list = q
      ? sessions.value.filter((s) => s.title.toLowerCase().includes(q))
      : sessions.value;
    return [...list].sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updated_at - a.updated_at);
  });
  const totalUsage = ref<UsageStats>({
    prompt_tokens: 0, completion_tokens: 0, total_tokens: 0, requests: 0, estimated_cost: 0,
  });

  function isStreaming(id: string) { return streamingIds.value.has(id); }
  function textFor(id: string) { return streamText.value[id] ?? ''; }
  function toolsFor(id: string) { return streamTools.value[id] ?? []; }
  function usageFor(id: string) { return streamUsage.value[id] ?? null; }

  async function init() {
    config.value = await api.getConfig();
    applyTheme(config.value.theme);
    sessions.value = await api.listSessions();
    if (sessions.value.length > 0) {
      currentId.value = sessions.value[0].id;
    } else {
      await newSession();
    }
    await Promise.all([useSkillsStore().load(), refreshUsage()]);
    startApprovalPolling();
  }

  function applyTheme(theme: string) {
    const dark = theme === 'dark' ||
      (theme === 'system' && window.matchMedia?.('(prefers-color-scheme: dark)').matches);
    document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
  }

  function startApprovalPolling() {
    if (approvalTimer) return;
    approvalTimer = window.setInterval(async () => {
      try {
        const pending = await api.listApprovals();
        pendingApprovals.value = pending;
      } catch { /* ignore */ }
    }, 1000);
  }

  async function respondApproval(id: string, approved: boolean) {
    await api.respondApproval(id, approved);
    pendingApprovals.value = pendingApprovals.value.filter((a) => a.id !== id);
  }

  async function newSession() {
    const s = await api.createSession();
    sessions.value.unshift(s);
    currentId.value = s.id;
  }

  async function selectSession(id: string) { currentId.value = id; }

  async function deleteSession(id: string) {
    await api.deleteSession(id);
    sessions.value = sessions.value.filter((s) => s.id !== id);
    if (currentId.value === id) {
      currentId.value = sessions.value[0]?.id ?? null;
      if (!currentId.value) await newSession();
    }
  }

  async function renameSession(id: string, title: string) {
    await api.renameSession(id, title);
    const s = sessions.value.find((x) => x.id === id);
    if (s) s.title = title;
  }

  async function togglePin(id: string) {
    const pinned = await api.togglePinSession(id);
    const s = sessions.value.find((x) => x.id === id);
    if (s) s.pinned = pinned;
  }

  async function deleteMessage(sessionId: string, messageId: string) {
    await api.deleteMessage(sessionId, messageId);
    const s = sessions.value.find((x) => x.id === sessionId);
    if (s) s.messages = s.messages.filter((m) => m.id !== messageId);
  }

  async function refreshUsage() {
    totalUsage.value = await api.getTotalUsage();
  }

  function requireProvider(): boolean {
    const c = config.value;
    if (!c) { providerRequired.value = true; return false; }
    const active = c.providers.find((p) => p.id === c.active_provider_id) ?? c.providers[0];
    if (active?.api_key?.trim()) return true;
    providerRequired.value = true;
    return false;
  }

  async function send(content: string): Promise<boolean> {
    const session = currentSession.value;
    if (!session || isStreaming(session.id) || !content.trim()) return false;

    const auth = useAuthStore();
    if (!auth.requireLogin('登录后才能开始对话')) return false;
    if (!requireProvider()) return false;

    const userMsg: Message = {
      id: `local-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
      role: 'user',
      content: content.trim(),
      created_at: Math.floor(Date.now() / 1000),
    };
    session.messages.push(userMsg);
    await api.appendMessage(session.id, userMsg);
    if (session.title === '新会话') {
      await renameSession(session.id, content.trim().slice(0, 20));
    }

    const sid = session.id;
    streamingIds.value = new Set([...streamingIds.value, sid]);
    streamText.value[sid] = '';
    streamTools.value[sid] = [];
    streamUsage.value[sid] = {
      prompt_tokens: 0, completion_tokens: 0, total_tokens: 0, requests: 0, estimated_cost: 0,
    };
    error.value = '';

    try {
      await api.chat(sid, (event: StreamEvent) => {
        switch (event.type) {
          case 'delta':
            streamText.value[sid] = (streamText.value[sid] || '') + event.text;
            break;
          case 'tool':
            streamTools.value[sid] = [
              ...(streamTools.value[sid] || []),
              { name: event.name, args: event.args, status: 'running' },
            ];
            break;
          case 'tool_result': {
            const tools = streamTools.value[sid] || [];
            const idx = [...tools].reverse().findIndex((t) => t.name === event.name && t.status === 'running');
            if (idx >= 0) {
              const real = tools.length - 1 - idx;
              tools[real] = { ...tools[real], result: event.result, status: 'done' };
              streamTools.value[sid] = [...tools];
            }
            break;
          }
          case 'approval_required':
            pendingApprovals.value = [
              ...pendingApprovals.value,
              { id: event.id, tool: event.tool, summary: event.summary },
            ];
            break;
          case 'usage':
            streamUsage.value[sid] = {
              prompt_tokens: event.prompt_tokens,
              completion_tokens: event.completion_tokens,
              total_tokens: event.prompt_tokens + event.completion_tokens,
              requests: 1,
              estimated_cost: event.estimated_cost,
            };
            break;
          case 'error':
            error.value = event.message;
            break;
          case 'cancelled':
            streamText.value[sid] = (streamText.value[sid] || '') + '\n\n*[已停止生成]*';
            break;
          case 'done':
            session.messages.push({
              id: `local-a-${Date.now()}`,
              role: 'assistant',
              content: event.content,
              created_at: Math.floor(Date.now() / 1000),
            });
            break;
        }
      });
      sessions.value = await api.listSessions();
      await refreshUsage();
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : e?.message ?? String(e);
      // 后端返回的错误字符串可能带 "command failed" 前缀，提取核心
      error.value = msg.replace(/^.*?(failed|error):\s*/i, '') || msg;
    } finally {
      const next = new Set(streamingIds.value);
      next.delete(sid);
      streamingIds.value = next;
      delete streamText.value[sid];
      delete streamTools.value[sid];
      delete streamUsage.value[sid];
    }
    return true;
  }

  async function cancel(sid: string) {
    await api.cancelChat(sid);
  }

  async function regenerate(messageIndex: number) {
    const session = currentSession.value;
    if (!session || isStreaming(session.id)) return;
    const auth = useAuthStore();
    if (!auth.requireLogin('登录后才能重新生成')) return;
    if (!requireProvider()) return;
    // 找到该助手消息之前最近的用户消息及其位置
    let userIdx = -1;
    for (let i = messageIndex - 1; i >= 0; i--) {
      if (session.messages[i].role === 'user') { userIdx = i; break; }
    }
    if (userIdx < 0) return;
    const userContent = session.messages[userIdx].content;
    // 删除用户消息及其后的所有消息，send() 会重新添加用户消息并生成
    const toRemove = session.messages.slice(userIdx);
    for (const m of toRemove) await api.deleteMessage(session.id, m.id);
    session.messages = session.messages.slice(0, userIdx);
    await send(userContent);
  }

  async function editMessage(messageId: string, newContent: string) {
    const session = currentSession.value;
    if (!session) return;
    const idx = session.messages.findIndex((m) => m.id === messageId);
    if (idx < 0) return;
    session.messages[idx].content = newContent;
    await api.updateMessage(session.id, messageId, newContent);
  }

  async function saveConfig(next: AppConfig) {
    await api.saveConfig(next);
    config.value = next;
    applyTheme(next.theme);
  }

  return {
    sessions, currentId, currentSession, filteredSessions, config, error, searchQuery, providerRequired,
    streamingIds, totalUsage, pendingApprovals,
    isStreaming, textFor, toolsFor, usageFor,
    init, newSession, selectSession, deleteSession, renameSession, togglePin,
    deleteMessage, refreshUsage, send, cancel, regenerate, editMessage, saveConfig,
    respondApproval,
  };
});
