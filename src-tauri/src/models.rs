use serde::{Deserialize, Serialize};

use crate::usage::UsageStats;

/// 登录会话（对应 moha 账号体系）。token 存钥匙串，不存明文密码。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthSession {
    pub account: String,
    pub login_at: i64,
    pub coin: f64,
    pub expire_display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserQuota {
    pub coin: f64,
    pub expire_display: String,
    pub is_member: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// 内置 Agnes（国内版）服务信息。默认开箱即用，无需用户填 key。
pub const BUILTIN_AGNES_ID: &str = "builtin-agnes";
pub const BUILTIN_AGNES_NAME: &str = "Agnes 3.0 Flash（国内）";
pub const BUILTIN_AGNES_BASE_URL: &str = "https://api.agnes-ai.cn/v1";
pub const BUILTIN_AGNES_MODEL: &str = "agnes-3.0-flash";
/// 内置 Agnes 的 API Key（cpk- 前缀，随版本内置，不走用户钥匙串）
pub const BUILTIN_AGNES_API_KEY: &str = "cpk-ydvjoVPqdKxGW9SAohxEJuEbzLSiuNIE0cXnYjsvp8gwJZDF";

/// 判断某个 base_url 是否指向内置 Agnes 服务
pub fn is_builtin_agnes(base_url: &str) -> bool {
    base_url.trim().to_lowercase().starts_with("https://api.agnes-ai.cn")
}

impl Default for Provider {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "新中转站".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: "gpt-4o-mini".into(),
        }
    }
}

/// 构造内置 Agnes provider（固定 id，作为默认 provider）
pub fn builtin_provider() -> Provider {
    Provider {
        id: BUILTIN_AGNES_ID.into(),
        name: BUILTIN_AGNES_NAME.into(),
        base_url: BUILTIN_AGNES_BASE_URL.into(),
        api_key: String::new(), // 内置 key 由 load/save_config 逻辑自动补齐
        model: BUILTIN_AGNES_MODEL.into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub providers: Vec<Provider>,
    pub active_provider_id: Option<String>,
    /// 全局启用的技能名（目录名）。为空表示启用全部。
    #[serde(default)]
    pub enabled_skills: Vec<String>,
    #[serde(default)]
    pub system_prompt: String,
    /// 工具（文件读写/终端）的工作目录，为空时使用应用数据目录。
    #[serde(default)]
    pub workspace_path: String,
    /// 配置结构版本
    #[serde(default = "default_version")]
    pub version: u32,
    /// 界面主题：light / dark / system
    #[serde(default)]
    pub theme: String,
    /// 工具策略：ask（每次询问）/ auto（自动放行）/ deny（禁用命令工具）
    #[serde(default)]
    pub tool_policy: String,
    /// 技能市场索引 URL
    #[serde(default)]
    pub marketplace_url: String,
    /// 自定义每百万 token 单价（0 表示用内置定价表）
    #[serde(default)]
    pub cost_per_million: f64,
    /// 历史上下文 token 预算
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,
}

fn default_version() -> u32 { crate::config::CONFIG_VERSION }
fn default_token_budget() -> usize { 24000 }

impl Default for AppConfig {
    fn default() -> Self {
        let builtin = builtin_provider();
        Self {
            providers: vec![builtin.clone()],
            active_provider_id: Some(builtin.id.clone()),
            enabled_skills: Vec::new(),
            system_prompt: "你是一个乐于助人的 AI 助手。".into(),
            workspace_path: String::new(),
            version: crate::config::CONFIG_VERSION,
            theme: "light".into(),
            tool_policy: "ask".into(),
            marketplace_url: String::new(),
            cost_per_million: 0.0,
            token_budget: default_token_budget(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub created_at: i64,
}

impl Message {
    #[allow(dead_code)]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub usage: UsageStats,
}

impl Session {
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "新会话".into(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            pinned: false,
            usage: UsageStats::default(),
        }
    }
}
