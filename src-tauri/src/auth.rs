use tauri::State;

use crate::error::{AppError, Result};
use crate::logger;
use crate::models::{AuthSession, UserQuota};
use crate::state::AppState;

/// 与 moha 共用的账号服务（表单协议，三端共用同一套账号）。
pub const AUTH_API_URL: &str = "https://auth.dukey.cn/soft/mohaapi.ashx";
const TOKEN_SERVICE: &str = "com.dkide.auth";
const TOKEN_USER: &str = "moha_token";

/// POST 表单到账号服务，返回原始文本。
async fn post_form(http: &reqwest::Client, form: &[(&str, &str)], timeout_secs: u64) -> Result<String> {
    let client = if timeout_secs != 300 {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?
    } else {
        http.clone()
    };
    let resp = client
        .post(AUTH_API_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(form)
        .send()
        .await
        .map_err(|e| AppError(format!("连接账号服务失败: {}", e)))?;
    let text = resp.text().await.map_err(|e| AppError(format!("读取账号服务响应失败: {}", e)))?;
    Ok(text.trim().to_string())
}

/// 把服务端原始错误归一化成中文提示（对标 moha 的 normalizeAuthMessage）。
fn normalize_message(message: &str) -> String {
    let raw = message.trim();
    let lower = raw.to_lowercase();
    if raw.is_empty() { return "账号或密码错误，请检查后重试".into(); }
    if raw.len() > 200 || raw.starts_with('<') || lower.starts_with("<!doctype") {
        return "账号服务返回异常，请稍后重试".into();
    }
    if lower.contains("expire") { return "账号已到期，请续费后再登录".into(); }
    if lower.contains("invalid user") || lower.contains("user not found") || lower.contains("nouser") {
        return "账号不存在，请检查后重试".into();
    }
    if lower.contains("password") || lower.contains("pwd") { return "密码错误，请重新输入".into(); }
    if matches!(lower.as_str(), "-1" | "error" | "fail" | "failed" | "false" | "null" | "no") {
        return "账号或密码错误，请检查后重试".into();
    }
    raw.to_string()
}

fn parse_token(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if lower == "ok" {
        Some("authorized".into())
    } else if text.contains("token:") {
        Some(text.split("token:").nth(1).unwrap_or("").trim().to_string())
    } else {
        None
    }
}

fn save_token(state: &AppState, token: &str) -> Result<()> {
    state.secure_store.set_named(TOKEN_SERVICE, TOKEN_USER, token)
}

fn load_token(state: &AppState) -> Option<String> {
    state.secure_store.get_named(TOKEN_SERVICE, TOKEN_USER)
}

/// 登录：ac=gettokenlogin，登录成功后 token 存入系统钥匙串。
#[tauri::command]
pub async fn auth_login(
    state: State<'_, AppState>,
    http: State<'_, reqwest::Client>,
    username: String,
    password: String,
) -> Result<AuthSession> {
    let text = post_form(&http, &[
        ("ac", "gettokenlogin"),
        ("user", &username),
        ("pwd", &password),
    ], 15).await?;

    let token = match parse_token(&text) {
        Some(t) => t,
        None => return Err(AppError(normalize_message(&text))),
    };
    save_token(&state, &token)?;

    let quota = fetch_quota(&http, &username).await.unwrap_or_default();
    let session = AuthSession {
        account: username,
        login_at: chrono::Utc::now().timestamp(),
        coin: quota.coin,
        expire_display: quota.expire_display,
    };
    state.store.save_auth(&session)?;
    logger::info(format!("user logged in: {}", session.account));
    Ok(session)
}

/// 注册：ac=register（moha 需要机器码 signkey，这里用固定标识，服务端不强绑则可通过）。
#[tauri::command]
pub async fn auth_register(
    state: State<'_, AppState>,
    http: State<'_, reqwest::Client>,
    email: String,
    password: String,
) -> Result<String> {
    let text = post_form(&http, &[
        ("ac", "register"),
        ("email", &email),
        ("pwd", &password),
    ], 15).await?;
    let lower = text.to_lowercase();
    if lower.contains("success") || lower == "ok" || lower.contains("注册成功") {
        let _ = &state;
        Ok("注册成功，请登录".into())
    } else {
        Err(AppError(normalize_message(&text)))
    }
}

/// 启动时验票：ac=verifylogin，网络异常不强制登出。
#[tauri::command]
pub async fn auth_verify(
    state: State<'_, AppState>,
    http: State<'_, reqwest::Client>,
) -> Result<Option<AuthSession>> {
    let session = match state.store.load_auth()? {
        Some(s) => s,
        None => return Ok(None),
    };
    let token = match load_token(&state) {
        Some(t) => t,
        None => {
            state.store.clear_auth().ok();
            return Ok(None);
        }
    };
    let text = match post_form(&http, &[
        ("ac", "verifylogin"),
        ("user", &session.account),
        ("token", &token),
    ], 15).await {
        Ok(t) => t,
        Err(_) => return Ok(Some(session)), // 网络异常：不强制登出
    };
    let lower = text.to_lowercase();
    if lower == "ok" || lower.contains("success") {
        let quota = fetch_quota(&http, &session.account).await.unwrap_or_default();
        let mut fresh = session;
        fresh.coin = quota.coin;
        fresh.expire_display = quota.expire_display;
        state.store.save_auth(&fresh)?;
        Ok(Some(fresh))
    } else {
        state.store.clear_auth().ok();
        let _ = state.secure_store.remove_named(TOKEN_SERVICE, TOKEN_USER);
        Ok(None)
    }
}

/// 登出：清除本地会话与 token。
#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> Result<()> {
    state.store.clear_auth()?;
    let _ = state.secure_store.remove_named(TOKEN_SERVICE, TOKEN_USER);
    logger::info("user logged out");
    Ok(())
}

/// 读取本地会话（不校验，供 UI 快速显示）。
#[tauri::command]
pub fn auth_session(state: State<'_, AppState>) -> Result<Option<AuthSession>> {
    state.store.load_auth()
}

/// 查询配额：ac=getuserinfo。
#[tauri::command]
pub async fn auth_quota(
    state: State<'_, AppState>,
    http: State<'_, reqwest::Client>,
) -> Result<UserQuota> {
    let session = state.store.load_auth()?
        .ok_or_else(|| AppError("未登录".into()))?;
    let quota = fetch_quota(&http, &session.account).await?;
    let mut fresh = session;
    fresh.coin = quota.coin;
    fresh.expire_display = quota.expire_display.clone();
    state.store.save_auth(&fresh)?;
    Ok(quota)
}

async fn fetch_quota(http: &reqwest::Client, username: &str) -> Result<UserQuota> {
    let text = post_form(http, &[("ac", "getuserinfo"), ("user", username)], 15).await?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    let coin = json.get("coin")
        .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64()))
        .unwrap_or(0.0);
    let expire_display = json.get("expire_display").or_else(|| json.get("expire_at"))
        .and_then(|v| v.as_str()).unwrap_or("").to_string();
    Ok(UserQuota {
        coin,
        is_member: !expire_display.is_empty(),
        expire_display,
    })
}
