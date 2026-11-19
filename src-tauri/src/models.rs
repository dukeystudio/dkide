use serde::{Deserialize, Serialize};

use crate::usage::UsageStats;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
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
        Self {
            providers: vec![Provider::default()],
            active_provider_id: None,
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
