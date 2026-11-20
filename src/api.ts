import { invoke, Channel } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { TEXT_ATTACHMENT_EXTENSIONS, TEXT_ATTACHMENT_FILENAMES } from './types';
import type {
  AppConfig,
  Approval,
  AuthSession,
  MarketSkill,
  Message,
  Provider,
  Session,
  Skill,
  StreamEvent,
  TextAttachment,
  UsageStats,
  UserQuota,
} from './types';

export const api = {
  authLogin: (username: string, password: string) =>
    invoke<AuthSession>('auth_login', { username, password }),
  authRegister: (email: string, password: string) =>
    invoke<string>('auth_register', { email, password }),
  authVerify: () => invoke<AuthSession | null>('auth_verify'),
  authLogout: () => invoke<void>('auth_logout'),
  authSession: () => invoke<AuthSession | null>('auth_session'),
  authQuota: () => invoke<UserQuota>('auth_quota'),

  getConfig: () => invoke<AppConfig>('get_config'),
  saveConfig: (config: AppConfig) => invoke<void>('save_config', { config }),
  getProjectRoot: () => invoke<string>('get_project_root'),
  createProject: (parent: string, name: string) => invoke<string>('create_project', { parent, name }),
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
  updateMessage: (sessionId: string, messageId: string, content: string) =>
    invoke<void>('update_message', { sessionId, messageId, content }),
  exportSession: async (sessionId: string, title: string) => {
    const path = await save({
      defaultPath: `${title.replace(/[\\/:*?"<>|]/g, '_')}.md`,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    });
    if (path) await invoke<void>('export_session', { sessionId, path });
    return !!path;
  },

  chat: (sessionId: string, onEvent: (e: StreamEvent) => void, skillNames: string[] | null = null, continueMessageId: string | null = null): Promise<string> => {
    const channel = new Channel<StreamEvent>();
    channel.onmessage = onEvent;
    return invoke<string>('chat', { sessionId, channel, skillNames, continueMessageId });
  },
  cancelChat: (sessionId: string) => invoke<void>('cancel_chat', { sessionId }),

  readFilePreview: (absPath: string) => invoke<string>('read_file_for_preview', { absPath }),
  pickTextAttachment: async () => {
    const selected = await open({
      multiple: false,
      filters: [{
        name: '文本与代码文件',
        extensions: [...TEXT_ATTACHMENT_EXTENSIONS, ...TEXT_ATTACHMENT_FILENAMES],
      }],
    });
    return typeof selected === 'string' ? selected : null;
  },
  readTextAttachment: (absPath: string) => invoke<TextAttachment>('read_text_attachment', { absPath }),
  openFileInOs: (absPath: string) => invoke<void>('open_file_in_os', { absPath }),
  revealFileInOs: (absPath: string) => invoke<void>('reveal_file_in_os', { absPath }),
  recordMessageFiles: (sessionId: string, files: unknown[]) => invoke<void>('record_message_files', { sessionId, files }),

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
