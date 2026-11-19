import { invoke, Channel } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import type {
  AppConfig,
  Approval,
  MarketSkill,
  Message,
  Provider,
  Session,
  Skill,
  StreamEvent,
  UsageStats,
} from './types';

export const api = {
  getConfig: () => invoke<AppConfig>('get_config'),
  saveConfig: (config: AppConfig) => invoke<void>('save_config', { config }),
  testProvider: (provider: Provider) => invoke<string>('test_provider', { provider }),

  listSessions: () => invoke<Session[]>('list_sessions'),
  createSession: () => invoke<Session>('create_session'),
  deleteSession: (id: string) => invoke<void>('delete_session', { id }),
  renameSession: (id: string, title: string) => invoke<void>('rename_session', { id, title }),
  togglePinSession: (id: string) => invoke<boolean>('toggle_pin_session', { id }),
  appendMessage: (sessionId: string, message: Message) =>
    invoke<void>('append_message', { sessionId, message }),
  deleteMessage: (sessionId: string, messageId: string) =>
    invoke<void>('delete_message', { sessionId, messageId }),
  exportSession: async (sessionId: string, title: string) => {
    const path = await save({
      defaultPath: `${title.replace(/[\\/:*?"<>|]/g, '_')}.md`,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    });
    if (path) await invoke<void>('export_session', { sessionId, path });
    return !!path;
  },

  chat: (sessionId: string, onEvent: (e: StreamEvent) => void): Promise<string> => {
    const channel = new Channel<StreamEvent>();
    channel.onmessage = onEvent;
    return invoke<string>('chat', { sessionId, channel });
  },
  cancelChat: (sessionId: string) => invoke<void>('cancel_chat', { sessionId }),

  listApprovals: () => invoke<Approval[]>('list_approvals'),
  respondApproval: (id: string, approved: boolean) =>
    invoke<void>('respond_approval', { id, approved }),

  getTotalUsage: () => invoke<UsageStats>('get_total_usage'),

  listSkills: () => invoke<Skill[]>('list_skills'),
  installSkillFromDir: (path: string) => invoke<Skill>('install_skill_from_dir', { path }),
  installSkillFromZip: (path: string) => invoke<Skill[]>('install_skill_from_zip', { path }),
  installSkillFromUrl: (url: string) => invoke<Skill[]>('install_skill_from_url', { url }),
  uninstallSkill: (dirName: string) => invoke<void>('uninstall_skill', { dirName }),
  openSkillsDir: () => invoke<void>('open_skills_dir'),
  listMarketplace: () => invoke<MarketSkill[]>('list_marketplace'),

  getAppInfo: () => invoke<{ name: string; english_name: string; version: string; description: string }>('get_app_info'),
};
