import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { api } from './api';
import { useSkillsStore } from './skillsStore';
import { useAuthStore } from './authStore';
import type { AppConfig, Approval, GeneratedFile, Message, Session, StreamBlock, StreamEvent, ToolCallView, UsageStats } from './types';

export const useChatStore = defineStore('chat', () => {
  const sessions = ref<Session[]>([]);
  const currentId = ref<string | null>(null);
  const config = ref<AppConfig | null>(null);
  const error = ref('');
  const searchQuery = ref('');
  const providerRequired = ref(false);
  /** 是否打开"服务商配置"弹窗 */
  const providerDialogOpen = ref(false);
  /** 打开弹窗时是否自动聚焦/滚动到"添加服务商"区 */
  const focusAddProvider = ref(false);
  /** 打开弹窗时需要编辑的服务商 id */
  const providerDialogFocusId = ref<string | null>(null);

  // 按会话隔离的流式状态
  const streamingIds = ref<Set<string>>(new Set());
  const streamText = ref<Record<string, string>>({});
  const streamReasoning = ref<Record<string, string>>({});
  const streamTools = ref<Record<string, ToolCallView[]>>({});
  const streamBlocks = ref<Record<string, StreamBlock[]>>({});
  const streamFiles = ref<Record<string, GeneratedFile[]>>({});
  const streamUsage = ref<Record<string, UsageStats>>({});
  const streamStartedAt = ref<Record<string, number>>({});

  const pendingApprovals = ref<Approval[]>([]);
  let approvalTimer: number | null = null;
  let streamBlockSequence = 0;

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
  function reasoningFor(id: string) { return streamReasoning.value[id] ?? ''; }
  function toolsFor(id: string) { return streamTools.value[id] ?? []; }
  function blocksFor(id: string) { return streamBlocks.value[id] ?? []; }
  function filesFor(id: string) { return streamFiles.value[id] ?? []; }
  function usageFor(id: string) { return streamUsage.value[id] ?? null; }
  function startTimeFor(id: string) { return streamStartedAt.value[id] ?? 0; }

  function appendStreamBlock(sessionId: string, block: StreamBlock) {
    streamBlocks.value[sessionId] = [...(streamBlocks.value[sessionId] ?? []), block];
  }

  function appendStreamText(sessionId: string, text: string) {
    const blocks = [...(streamBlocks.value[sessionId] ?? [])];
    const last = blocks[blocks.length - 1];
    if (last?.kind === 'text') {
      blocks[blocks.length - 1] = { ...last, text: (last.text ?? '') + text };
    } else {
      blocks.push({ id: ++streamBlockSequence, kind: 'text', text, started_at: Date.now() });
    }
    streamBlocks.value[sessionId] = blocks;
  }

  function appendStreamReasoning(sessionId: string, text: string) {
    const blocks = [...(streamBlocks.value[sessionId] ?? [])];
    const last = blocks[blocks.length - 1];
    if (last?.kind === 'reasoning') {
      blocks[blocks.length - 1] = { ...last, text: (last.text ?? '') + text };
    } else {
      blocks.push({ id: ++streamBlockSequence, kind: 'reasoning', text, started_at: Date.now() });
    }
    streamBlocks.value[sessionId] = blocks;
  }

  function appendStreamTool(sessionId: string, name: string, args: string) {
    appendStreamBlock(sessionId, {
      id: ++streamBlockSequence,
      kind: 'tool',
      name,
      args,
      status: 'running',
      started_at: Date.now(),
    });
  }

  function completeStreamTool(sessionId: string, name: string, result: string, status: 'done' | 'error') {
    const blocks = [...(streamBlocks.value[sessionId] ?? [])];
    for (let index = blocks.length - 1; index >= 0; index--) {
      const block = blocks[index];
      if (block.kind !== 'tool' || block.name !== name || block.status !== 'running') continue;
      blocks[index] = { ...block, result, status, finished_at: Date.now() };
      streamBlocks.value[sessionId] = blocks;
      return;
    }
  }

  function appendSubAgentBlock(sessionId: string, taskId: string, title: string) {
    appendStreamBlock(sessionId, {
      id: ++streamBlockSequence,
      task_id: taskId,
      kind: 'subagent',
      name: title,
      status: 'running',
      started_at: Date.now(),
    });
  }

  function completeSubAgentBlock(sessionId: string, taskId: string, result: string, status: 'done' | 'error') {
    const blocks = [...(streamBlocks.value[sessionId] ?? [])];
    for (let index = blocks.length - 1; index >= 0; index--) {
      const block = blocks[index];
      if (block.kind !== 'subagent' || block.task_id !== taskId || block.status !== 'running') continue;
      blocks[index] = { ...block, result, status, finished_at: Date.now() };
      streamBlocks.value[sessionId] = blocks;
      return;
    }
  }

  function finalizeRunningTools(sessionId: string, result: string) {
    const tools = streamTools.value[sessionId] || [];
    let changed = false;
    for (const tool of tools) {
      if (tool.status !== 'running') continue;
      tool.status = 'done';
      tool.result = result;
      tool.finished_at = Date.now();
      changed = true;
    }
    if (changed) streamTools.value[sessionId] = [...tools];
    const blocks = [...(streamBlocks.value[sessionId] ?? [])];
    let blocksChanged = false;
    for (const block of blocks) {
      if ((block.kind !== 'tool' && block.kind !== 'subagent') || block.status !== 'running') continue;
      block.status = 'done';
      block.result = result;
      block.finished_at = Date.now();
      blocksChanged = true;
    }
    if (blocksChanged) streamBlocks.value[sessionId] = blocks;
  }

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

  async function send(
    content: string,
    onAccepted?: () => void,
    skillNames: string[] | null = null,
    reuseLastUser = false,
    continueMessageId: string | null = null,
  ): Promise<boolean> {
    const session = currentSession.value;
    if (!session || isStreaming(session.id) || (!content.trim() && !continueMessageId)) return false;

    const auth = useAuthStore();
    if (!auth.requireLogin('登录后才能开始对话')) return false;
    if (!requireProvider()) return false;

    // 续写模式：不插入用户消息，续写指令由后端临时构造，不落库也不显示
    if (!continueMessageId) {
      const userMsg: Message = {
        id: `local-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
        role: 'user',
        content: content.trim(),
        created_at: Math.floor(Date.now() / 1000),
      };
      const lastUserMessage = [...session.messages].reverse().find((message) => message.role === 'user');
      const reuseExisting = reuseLastUser
        && lastUserMessage?.content === content.trim()
        && Date.now() - lastUserMessage.created_at * 1000 < 60_000;
      if (!reuseExisting) {
        session.messages.push(userMsg);
        await api.appendMessage(session.id, userMsg);
        if (session.title === '新会话') {
          await renameSession(session.id, content.trim().slice(0, 20));
        }
      }
    }
    onAccepted?.();

    const sid = session.id;
    streamingIds.value = new Set([...streamingIds.value, sid]);
    streamText.value[sid] = '';
    streamReasoning.value[sid] = '';
    streamTools.value[sid] = [];
    streamBlocks.value[sid] = [];
    streamFiles.value[sid] = [];
    streamStartedAt.value[sid] = Date.now();
    streamUsage.value[sid] = {
      prompt_tokens: 0, completion_tokens: 0, total_tokens: 0, requests: 0, estimated_cost: 0,
    };
    error.value = '';

    try {
      await api.chat(sid, (event: StreamEvent) => {
        switch (event.type) {
          case 'delta':
            streamText.value[sid] = (streamText.value[sid] || '') + event.text;
            appendStreamText(sid, event.text);
            break;
          case 'reasoning':
            streamReasoning.value[sid] = (streamReasoning.value[sid] || '') + event.text;
            appendStreamReasoning(sid, event.text);
            break;
          case 'tool':
            streamTools.value[sid] = [
              ...(streamTools.value[sid] || []),
              { name: event.name, args: event.args, status: 'running', started_at: Date.now() },
            ];
            appendStreamTool(sid, event.name, event.args);
            break;
          case 'tool_result': {
            const tools = streamTools.value[sid] || [];
            const status = event.result.startsWith('执行失败') ? 'error' : 'done';
            const idx = [...tools].reverse().findIndex((t) => t.name === event.name && t.status === 'running');
            if (idx >= 0) {
              const real = tools.length - 1 - idx;
              tools[real] = { ...tools[real], result: event.result, status, finished_at: Date.now() };
              streamTools.value[sid] = [...tools];
            }
            completeStreamTool(sid, event.name, event.result, status === 'error' ? 'error' : 'done');
            break;
          }
          case 'sub_agent_start':
            appendSubAgentBlock(sid, event.task_id, event.title);
            break;
          case 'sub_agent_done':
            completeSubAgentBlock(sid, event.task_id, event.summary, 'done');
            break;
          case 'sub_agent_error':
            completeSubAgentBlock(sid, event.task_id, event.message, 'error');
            break;
          case 'file':
            // 去重：同一文件多次写入只保留最新一次
            streamFiles.value[sid] = [
              ...(streamFiles.value[sid] || []).filter((f) => f.abs_path !== event.abs_path),
              { path: event.path, abs_path: event.abs_path, kind: event.kind },
            ];
            break;
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
            finalizeRunningTools(sid, '生成失败');
            break;
          case 'cancelled':
            streamText.value[sid] = (streamText.value[sid] || '') + '\n\n*[已停止生成]*';
            appendStreamText(sid, '\n\n*[已停止生成]*');
            finalizeRunningTools(sid, '已停止生成');
            break;
          case 'done':
            finalizeRunningTools(sid, '已完成');
            if (continueMessageId) {
              // 续写：把新片段合并进原消息，清掉截断标记（后端落库同样合并）
              const target = session.messages.find((m) => m.id === continueMessageId);
              if (target) {
                target.content = (target.content || '') + (event.content || '');
                const extraReasoning = event.reasoning || streamReasoning.value[sid] || '';
                if (extraReasoning.trim()) {
                  target.reasoning = target.reasoning
                    ? `${target.reasoning}\n${extraReasoning}`
                    : extraReasoning;
                }
                target.truncated = event.truncated || undefined;
              }
            } else {
              session.messages.push({
                id: `local-a-${Date.now()}`,
                role: 'assistant',
                content: event.content || '（模型本轮未返回内容。）',
                reasoning: event.reasoning || streamReasoning.value[sid] || undefined,
                truncated: event.truncated || undefined,
                created_at: Math.floor(Date.now() / 1000),
              });
            }
            break;
        }
      }, skillNames, continueMessageId);
      sessions.value = await api.listSessions();
      await refreshUsage();
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : e?.message ?? String(e);
      // 后端返回的错误字符串可能带 "command failed" 前缀，提取核心
      error.value = msg.replace(/^.*?(failed|error):\s*/i, '') || msg;
      finalizeRunningTools(sid, '生成失败');
    } finally {
      const next = new Set(streamingIds.value);
      next.delete(sid);
      streamingIds.value = next;
      delete streamText.value[sid];
      delete streamReasoning.value[sid];
      delete streamTools.value[sid];
      delete streamBlocks.value[sid];
      delete streamUsage.value[sid];
      // streamFiles 保留，供"生成文件"面板在生成结束后继续展示；下次 send 开头会清空
    }
    return true;
  }

  async function cancel(sid: string) {
    try {
      await api.cancelChat(sid);
    } catch { /* 后端可能已结束，继续本地收尾 */ }
    for (let attempt = 0; attempt < 80 && isStreaming(sid); attempt++) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    if (isStreaming(sid)) {
      const next = new Set(streamingIds.value);
      next.delete(sid);
      streamingIds.value = next;
    }
  }

  let undoing = false;
  /**
   * 撤销：删除指定 user 消息及其后的所有消息（含助手回复与工具结果）。
   * @param messageId 要撤销到的那条 user 消息 id；省略 = 撤最近一轮（最后一条 user 消息）。
   */
  async function undo(messageId?: string): Promise<boolean> {
    if (undoing) return false; // 防连点：上一次 listSessions 未 resolve 前不再触发，避免拿旧快照
    const session = currentSession.value;
    if (!session || isStreaming(session.id)) return false;
    let userIdx = -1;
    if (messageId) {
      userIdx = session.messages.findIndex((m) => m.id === messageId && m.role === 'user');
    } else {
      for (let i = session.messages.length - 1; i >= 0; i--) {
        if (session.messages[i].role === 'user') { userIdx = i; break; }
      }
    }
    if (userIdx < 0) return false;
    undoing = true;
    try {
      const toRemove = session.messages.slice(userIdx);
      for (const m of toRemove) {
        await api.deleteMessage(session.id, m.id);
      }
      session.messages = session.messages.slice(0, userIdx);
      sessions.value = await api.listSessions();
      return true;
    } finally {
      undoing = false;
    }
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

  /**
   * 从被截断的助手消息续写：走后端续写模式，指令只在内存里临时构造，
   * 不落库、不出现在对话里；新正文合并进原消息。
   * 仅当目标是最后一条消息时可续写（后面若有更新的用户消息，续写会打乱上下文）。
   */
  async function continueGeneration(messageIndex: number) {
    const session = currentSession.value;
    if (!session || isStreaming(session.id)) return;
    const m = session.messages[messageIndex];
    if (!m || m.role !== 'assistant' || !m.truncated || messageIndex !== session.messages.length - 1) return;
    await send('', undefined, null, false, m.id);
  }

  /**
   * 编辑某条用户消息并重新生成（对标 ChatGPT/Claude 编辑历史）。
   * 保存修改 → 删除该用户消息之后的所有回复 → 基于新内容重新生成。
   * onlyUpdate=true 时只保存不重新生成。
   */
  /** 返回 true 表示保存成功；false 表示目标消息不在当前会话（如切会话后）未保存。 */
  async function editMessage(messageId: string, newContent: string, onlyUpdate = false): Promise<boolean> {
    const session = currentSession.value;
    if (!session || isStreaming(session.id)) return false;
    const idx = session.messages.findIndex((m) => m.id === messageId);
    if (idx < 0) return false; // 已切到别的会话，findIndex 找不到 → 不静默吞掉
    session.messages[idx].content = newContent;
    await api.updateMessage(session.id, messageId, newContent);
    if (onlyUpdate) return true;

    // 删除该用户消息之后的所有回复（保留这条被改的用户消息）
    const after = session.messages.slice(idx + 1);
    for (const m of after) await api.deleteMessage(session.id, m.id);
    session.messages = session.messages.slice(0, idx + 1);

    // 重新生成：去掉这条被编辑的用户消息，send 会把它作为新消息追加并流式生成
    session.messages = session.messages.slice(0, idx);
    await send(newContent);
    return true;
  }

  async function saveConfig(next: AppConfig) {
    await api.saveConfig(next);
    config.value = next;
    applyTheme(next.theme);
  }

  return {
    sessions, currentId, currentSession, filteredSessions, config, error, searchQuery, providerRequired, providerDialogOpen, focusAddProvider, providerDialogFocusId,
    streamingIds, totalUsage, pendingApprovals,
    isStreaming, textFor, reasoningFor, toolsFor, blocksFor, filesFor, usageFor, startTimeFor,
    init, newSession, selectSession, deleteSession, renameSession, togglePin,
    deleteMessage, refreshUsage, send, cancel, regenerate, continueGeneration, undo, editMessage, saveConfig,
    respondApproval,
  };
});
