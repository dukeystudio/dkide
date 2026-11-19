use std::path::PathBuf;
use std::time::Duration;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::approval::PendingApproval;
use crate::error::{AppError, Result};
use crate::llm::{ChatEngine, StreamEvent};
use crate::logger;
use crate::models::{AppConfig, Message, Provider, Session};
use crate::skill::Skill;
use crate::state::AppState;
use crate::tools::{ToolContext, ToolPolicy};
use crate::usage::UsageStats;

fn active_provider(state: &AppState) -> Result<Provider> {
    let config = state.config.lock().unwrap();
    let provider = if let Some(ref id) = config.active_provider_id {
        config.providers.iter().find(|p| &p.id == id)
    } else {
        config.providers.first()
    };
    provider.cloned().ok_or_else(|| AppError("没有配置任何模型中转站".into()))
}

// ---------- 配置 ----------

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig> {
    Ok(state.config.lock().unwrap().clone())
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<()> {
    let (workspace_changed, removed_provider_ids) = {
        let old = state.config.lock().unwrap();
        let removed = old.providers.iter()
            .filter(|p| !config.providers.iter().any(|n| n.id == p.id))
            .map(|p| p.id.clone())
            .collect::<Vec<_>>();
        (old.workspace_path != config.workspace_path, removed)
    };
    *state.config.lock().unwrap() = config.clone();
    state.save_config()?;
    // 清理已删除服务商在钥匙串里残留的 API Key
    for id in removed_provider_ids {
        let _ = state.secure_store.remove(&id);
    }
    // 工作目录变更时同步技能管理器；清空时回退到应用数据目录
    if workspace_changed {
        let mut mgr = state.skill_manager.lock().unwrap();
        let path = if config.workspace_path.trim().is_empty() {
            state.store.skills_dir()
        } else {
            PathBuf::from(&config.workspace_path)
        };
        mgr.set_workspace(path);
    }
    Ok(())
}

#[tauri::command]
pub async fn test_provider(provider: Provider, http: State<'_, reqwest::Client>) -> Result<String> {
    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let resp = http.post(&url)
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": provider.model,
            "messages": [{"role": "user", "content": "说OK"}],
            "stream": false,
            "max_tokens": 16,
        }))
        .send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.is_success() {
        Ok("连接成功".into())
    } else {
        // 修复：按字符截断，避免按字节切在 UTF-8 字符中间导致 panic
        let safe: String = text.chars().take(300).collect();
        Err(AppError(format!("连接失败 ({}): {}", status, safe)))
    }
}

// ---------- 会话 ----------

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<Session>> {
    let mut sessions = state.sessions.lock().unwrap().clone();
    sessions.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated_at.cmp(&a.updated_at)));
    Ok(sessions)
}

#[tauri::command]
pub fn create_session(state: State<'_, AppState>) -> Result<Session> {
    let session = Session::new();
    state.sessions.lock().unwrap().push(session.clone());
    state.save_sessions()?;
    Ok(session)
}

#[tauri::command]
pub fn delete_session(state: State<'_, AppState>, id: String) -> Result<()> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.retain(|s| s.id != id);
    drop(sessions);
    state.save_sessions()
}

#[tauri::command]
pub fn rename_session(state: State<'_, AppState>, id: String, title: String) -> Result<()> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
        session.title = title;
        session.updated_at = chrono::Utc::now().timestamp();
    }
    drop(sessions);
    state.save_sessions()
}

#[tauri::command]
pub fn toggle_pin_session(state: State<'_, AppState>, id: String) -> Result<bool> {
    let mut sessions = state.sessions.lock().unwrap();
    let mut pinned = false;
    if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
        session.pinned = !session.pinned;
        pinned = session.pinned;
    }
    drop(sessions);
    state.save_sessions()?;
    Ok(pinned)
}

#[tauri::command]
pub fn append_message(state: State<'_, AppState>, session_id: String, message: Message) -> Result<()> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
        if session.title == "新会话" && matches!(message.role, crate::models::Role::User) {
            session.title = message.content.chars().take(20).collect();
        }
        session.messages.push(message);
        session.updated_at = chrono::Utc::now().timestamp();
    }
    drop(sessions);
    state.save_sessions()
}

#[tauri::command]
pub fn delete_message(state: State<'_, AppState>, session_id: String, message_id: String) -> Result<()> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
        session.messages.retain(|m| m.id != message_id);
    }
    drop(sessions);
    state.save_sessions()
}

/// 按 id 原位更新消息内容（编辑消息用），避免 append 产生重复消息
#[tauri::command]
pub fn update_message(
    state: State<'_, AppState>,
    session_id: String,
    message_id: String,
    content: String,
) -> Result<()> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
        if let Some(msg) = session.messages.iter_mut().find(|m| m.id == message_id) {
            msg.content = content;
            session.updated_at = chrono::Utc::now().timestamp();
        }
    }
    drop(sessions);
    state.save_sessions()
}

#[tauri::command]
pub fn export_session(state: State<'_, AppState>, session_id: String, path: String) -> Result<()> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.iter().find(|s| s.id == session_id)
        .ok_or_else(|| AppError("会话不存在".into()))?;
    let mut md = format!("# {}\n\n", session.title);
    md.push_str(&format!("> 导出时间: {}\n\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
    for msg in &session.messages {
        let who = match msg.role {
            crate::models::Role::User => "## 用户",
            crate::models::Role::Assistant => "## 助手",
            _ => continue,
        };
        md.push_str(&format!("{}\n\n{}\n\n---\n\n", who, msg.content));
    }
    if session.usage.requests > 0 {
        md.push_str(&format!("\n*用量: {} tokens, ${:.4}*\n", session.usage.total_tokens, session.usage.estimated_cost));
    }
    std::fs::write(&path, md)?;
    logger::info(format!("exported session {} to {}", session_id, path));
    Ok(())
}

// ---------- 对话 ----------

#[tauri::command]
pub async fn chat(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    channel: Channel<StreamEvent>,
) -> Result<String> {
    let provider = active_provider(&state)?;
    state.cancels.clear(&session_id);

    let (history, system_prompt, enabled_skills, workspace_path, token_budget, custom_price) = {
        let config = state.config.lock().unwrap();
        let sessions = state.sessions.lock().unwrap();
        let history = sessions.iter().find(|s| s.id == session_id)
            .map(|s| s.messages.clone()).unwrap_or_default();
        (history, config.system_prompt.clone(), config.enabled_skills.clone(),
         config.workspace_path.clone(), config.token_budget, config.cost_per_million)
    };

    let workspace = if workspace_path.trim().is_empty() {
        app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&workspace_path)
    };
    let _ = std::fs::create_dir_all(&workspace);

    let (skill_summaries, skill_instructions) = {
        let mgr = state.skill_manager.lock().unwrap();
        let skills: Vec<_> = mgr.list(&enabled_skills)?.into_iter().filter(|s| s.enabled).collect();
        let summaries: Vec<(String, String)> = skills.iter()
            .map(|s| (s.meta.name.clone(), s.meta.description.clone())).collect();
        let mut instructions = std::collections::HashMap::new();
        for s in &skills {
            let text = format!("# 技能: {}\n\n描述: {}\n\n{}", s.meta.name, s.meta.description, s.instructions);
            instructions.insert(s.dir_name.clone(), text.clone());
            instructions.insert(s.meta.name.clone(), text);
        }
        (summaries, instructions)
    };

    let policy = {
        let config = state.config.lock().unwrap();
        ToolPolicy::from_str(&config.tool_policy)
    };
    let tool_ctx = ToolContext::new(workspace, policy, state.approvals.clone())
        .with_channel(channel.clone());

    let engine = ChatEngine::new(
        &provider, &system_prompt, &skill_summaries, &skill_instructions,
        &tool_ctx, &state.cancels, &session_id, &state.http, token_budget, custom_price,
    );

    let (answer, usage) = engine.run(&history, &channel).await?;
    state.cancels.clear(&session_id);

    let mut sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
        if !answer.is_empty() {
            session.messages.push(Message::assistant(answer.clone()));
        }
        session.usage.add(usage.prompt_tokens, usage.completion_tokens, 0.0);
        session.usage.estimated_cost = usage.estimated_cost;
        session.updated_at = chrono::Utc::now().timestamp();
    }
    drop(sessions);
    let _ = state.save_sessions();
    Ok(answer)
}

#[tauri::command]
pub fn cancel_chat(state: State<'_, AppState>, session_id: String) -> Result<()> {
    state.cancels.cancel(&session_id);
    Ok(())
}

// ---------- 审批 ----------

#[tauri::command]
pub fn list_approvals(state: State<'_, AppState>) -> Vec<PendingApproval> {
    state.approvals.list_pending()
}

#[tauri::command]
pub fn respond_approval(state: State<'_, AppState>, id: String, approved: bool) -> Result<()> {
    state.approvals.respond(&id, approved);
    Ok(())
}

// ---------- 用量 ----------

#[tauri::command]
pub fn get_total_usage(state: State<'_, AppState>) -> Result<UsageStats> {
    let sessions = state.sessions.lock().unwrap();
    let mut total = UsageStats::default();
    for s in sessions.iter() {
        total.prompt_tokens += s.usage.prompt_tokens;
        total.completion_tokens += s.usage.completion_tokens;
        total.total_tokens += s.usage.total_tokens;
        total.requests += s.usage.requests;
        total.estimated_cost += s.usage.estimated_cost;
    }
    Ok(total)
}

// ---------- 技能 ----------

#[tauri::command]
pub fn list_skills(state: State<'_, AppState>) -> Result<Vec<Skill>> {
    let config = state.config.lock().unwrap();
    let mgr = state.skill_manager.lock().unwrap();
    mgr.list(&config.enabled_skills)
}

#[tauri::command]
pub fn install_skill_from_dir(state: State<'_, AppState>, path: String) -> Result<Skill> {
    let mgr = state.skill_manager.lock().unwrap();
    mgr.install_from_dir(&PathBuf::from(path))
}

#[tauri::command]
pub fn install_skill_from_zip(state: State<'_, AppState>, path: String) -> Result<Vec<Skill>> {
    let mgr = state.skill_manager.lock().unwrap();
    mgr.install_from_zip(&PathBuf::from(path))
}

#[tauri::command]
pub async fn install_skill_from_url(
    state: State<'_, AppState>,
    url: String,
    http: State<'_, reqwest::Client>,
) -> Result<Vec<Skill>> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError("URL 必须以 http:// 或 https:// 开头".into()));
    }
    let resp = http.get(&url).timeout(Duration::from_secs(60)).send().await
        .map_err(|e| AppError(format!("下载失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError(format!("下载失败: HTTP {}", resp.status())));
    }
    let bytes = resp.bytes().await.map_err(|e| AppError(format!("读取内容失败: {}", e)))?;
    let mgr = state.skill_manager.lock().unwrap();
    mgr.install_from_zip_bytes(&bytes)
}

#[tauri::command]
pub fn uninstall_skill(state: State<'_, AppState>, dir_name: String) -> Result<()> {
    let mgr = state.skill_manager.lock().unwrap();
    mgr.uninstall(&dir_name)
}

#[tauri::command]
pub fn open_skills_dir(state: State<'_, AppState>) -> Result<()> {
    let dir = state.skill_manager.lock().unwrap().global_dir().to_path_buf();
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(dir).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer").arg(dir).spawn()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(dir).spawn()?;
    Ok(())
}

// ---------- 技能市场 ----------

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct MarketSkill {
    pub name: String,
    pub description: String,
    pub author: String,
    pub url: String,
}

#[tauri::command]
pub async fn list_marketplace(http: State<'_, reqwest::Client>, state: State<'_, AppState>) -> Result<Vec<MarketSkill>> {
    let url = {
        let config = state.config.lock().unwrap();
        config.marketplace_url.clone()
    };
    if url.trim().is_empty() {
        // 未配置技能市场索引时不返回虚假的内置条目，避免用户点击安装必然失败
        return Ok(Vec::new());
    }
    let resp = http.get(&url).timeout(Duration::from_secs(20)).send().await
        .map_err(|e| AppError(format!("获取技能市场失败: {}", e)))?;
    let skills: Vec<MarketSkill> = resp.json().await
        .map_err(|e| AppError(format!("解析技能市场失败: {}", e)))?;
    Ok(skills)
}

// ---------- 系统 ----------

#[tauri::command]
pub fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "都客AI助手",
        "english_name": "dkide",
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
    })
}
