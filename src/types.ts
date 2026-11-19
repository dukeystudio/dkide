export interface Provider {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  model: string;
  /** 思考强度：null 使用服务商默认值，例如 low / medium / high / xhigh */
  reasoning_effort: string | null;
  /** 是否声明支持工具调用 */
  tool_calling: boolean;
  /** 是否声明支持图片输入 */
  image_input: boolean;
  /** 是否声明支持推理/思考 */
  reasoning: boolean;
  /** 输入上下文 token 数：null 使用服务商默认值 */
  input_tokens: number | null;
  /** 输出 token 数：null 使用服务商默认值 */
  output_tokens: number | null;
}

export interface AppConfig {
  providers: Provider[];
  active_provider_id: string | null;
  enabled_skills: string[];
  system_prompt: string;
  workspace_path: string;
  version: number;
  theme: 'light' | 'dark' | 'system';
  tool_policy: 'ask' | 'auto' | 'deny';
  marketplace_url: string;
  cost_per_million: number;
  token_budget: number;
  /** 思考开关：null=模型默认，true/false 强制 */
  enable_thinking: boolean | null;
  /** 是否在对话中展示思考过程（前端展示开关） */
  show_reasoning: boolean;
  /** 温度（null 用模型默认） */
  temperature: number | null;
  /** 最大输出 token（null 用模型默认） */
  max_tokens: number | null;
}

export type Role = 'system' | 'user' | 'assistant' | 'tool';

export interface Message {
  id: string;
  role: Role;
  content: string;
  /** 模型思考过程（可选） */
  reasoning?: string;
  /** 正文因输出长度上限被截断（可续写） */
  truncated?: boolean;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  created_at: number;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface UsageStats {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  requests: number;
  estimated_cost: number;
}

export interface Session {
  id: string;
  title: string;
  messages: Message[];
  created_at: number;
  updated_at: number;
  pinned: boolean;
  usage: UsageStats;
}

export interface SkillMeta {
  name: string;
  description: string;
  version: string;
  author: string;
  allowed_tools: string[];
}

export interface Skill {
  meta: SkillMeta;
  instructions: string;
  path: string;
  dir_name: string;
  enabled: boolean;
}

export interface MarketSkill {
  name: string;
  description: string;
  author: string;
  url: string;
}

export interface Approval {
  id: string;
  tool: string;
  summary: string;
}

export interface AuthSession {
  account: string;
  login_at: number;
  coin: number;
  expire_display: string;
}

export interface UserQuota {
  coin: number;
  expire_display: string;
  is_member: boolean;
}

export interface ToolCallView {
  name: string;
  args: string;
  result?: string;
  status: 'running' | 'done' | 'error';
  started_at: number;
  finished_at?: number;
}

export interface StreamBlock {
  id: number;
  task_id?: string;
  kind: 'text' | 'reasoning' | 'tool' | 'subagent';
  text?: string;
  name?: string;
  args?: string;
  result?: string;
  status?: 'running' | 'done' | 'error';
  started_at: number;
  finished_at?: number;
}

export const TEXT_ATTACHMENT_EXTENSIONS = [
  'txt', 'text', 'md', 'markdown', 'csv', 'tsv', 'json', 'jsonl', 'xml',
  'yaml', 'yml', 'toml', 'ini', 'cfg', 'conf', 'properties', 'env', 'log',
  'sql', 'html', 'htm', 'css', 'scss', 'js', 'jsx', 'ts', 'tsx', 'vue',
  'py', 'rb', 'go', 'rs', 'java', 'c', 'h', 'cpp', 'hpp', 'cs', 'php',
  'sh', 'bash', 'zsh', 'ps1', 'bat', 'cmd', 'lua', 'swift', 'kt', 'scala', 'r',
] as const;

export const TEXT_ATTACHMENT_FILENAMES = [
  '.env', 'dockerfile', 'makefile', 'readme', 'license', 'gitignore',
] as const;

export interface TextAttachment {
  name: string;
  content: string;
  truncated: boolean;
}

/** 本轮生成/修改的文件 */
export interface GeneratedFile {
  path: string;
  abs_path: string;
  kind: string;
}

export type StreamEvent =
  | { type: 'delta'; text: string }
  | { type: 'reasoning'; text: string }
  | { type: 'tool'; name: string; args: string }
  | { type: 'file'; path: string; abs_path: string; kind: string }
  | { type: 'tool_result'; name: string; result: string }
  | { type: 'sub_agent_start'; task_id: string; title: string }
  | { type: 'sub_agent_done'; task_id: string; summary: string }
  | { type: 'sub_agent_error'; task_id: string; message: string }
  | { type: 'approval_required'; id: string; tool: string; summary: string }
  | { type: 'usage'; prompt_tokens: number; completion_tokens: number; estimated_cost: number }
  | { type: 'cancelled' }
  | { type: 'done'; content: string; reasoning?: string; truncated?: boolean }
  | { type: 'error'; message: string };
