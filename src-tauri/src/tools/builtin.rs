use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::ipc::Channel;

use crate::approval::ApprovalBroker;
use crate::cancel::CancelRegistry;
use crate::error::{AppError, Result};
use crate::llm::types::StreamEvent;
use crate::logger;

/// 工具策略
#[derive(Debug, Clone, PartialEq)]
pub enum ToolPolicy {
    Ask,
    Auto,
    Deny,
}

impl ToolPolicy {
    pub fn from_str(s: &str) -> Self {
        match s {
            "auto" => ToolPolicy::Auto,
            "deny" => ToolPolicy::Deny,
            _ => ToolPolicy::Ask,
        }
    }
}

/// 内置工具的工作上下文。所有文件操作都限制在 workspace 目录内。
pub struct ToolContext {
    pub workspace: PathBuf,
    pub policy: ToolPolicy,
    pub approvals: ApprovalBroker,
    pub channel: Option<Channel<StreamEvent>>,
    cancels: Option<CancelRegistry>,
    session_id: Option<String>,
}

impl ToolContext {
    pub fn new(workspace: PathBuf, policy: ToolPolicy, approvals: ApprovalBroker) -> Self {
        Self { workspace, policy, approvals, channel: None, cancels: None, session_id: None }
    }

    pub fn with_channel(mut self, channel: Channel<StreamEvent>) -> Self {
        self.channel = Some(channel);
        self
    }

    pub fn with_cancel(mut self, cancels: CancelRegistry, session_id: String) -> Self {
        self.cancels = Some(cancels);
        self.session_id = Some(session_id);
        self
    }

    fn is_cancelled(&self) -> bool {
        let Some(cancels) = &self.cancels else { return false };
        let Some(session_id) = &self.session_id else { return false };
        cancels.is_cancelled(session_id)
    }

    fn resolve(&self, path: &str) -> Result<PathBuf> {
        let p = Path::new(path);
        let combined = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.workspace.join(p)
        };
        let normalized = normalize(&combined);
        if !normalized.starts_with(&self.workspace) {
            return Err(AppError(format!(
                "路径越界：{}（工作目录为 {}）",
                path,
                self.workspace.display()
            )));
        }
        Ok(normalized)
    }

    /// 危险操作需要用户审批
    fn require_approval(&self, tool: &str, summary: impl Into<String>) -> Result<()> {
        match self.policy {
            ToolPolicy::Deny => return Err(AppError("工具策略禁止执行该操作".into())),
            ToolPolicy::Auto => return Ok(()),
            ToolPolicy::Ask => {}
        }
        let id = self.approvals.request(tool, summary);
        if let Some(ref channel) = self.channel {
            let pending = self.approvals.list_pending().into_iter().find(|p| p.id == id);
            if let Some(p) = pending {
                let _ = channel.send(StreamEvent::ApprovalRequired {
                    id: p.id,
                    tool: p.tool,
                    summary: p.summary,
                });
            }
        }
        if !self.approvals.wait(&id, Duration::from_secs(120)) {
            return Err(AppError("操作未获批准或审批超时".into()));
        }
        Ok(())
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => { out.pop(); }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// 命令黑名单：这些命令即使审批通过也拒绝执行。
fn is_blocked_command(command: &str) -> Option<&'static str> {
    let c = command.to_lowercase();
    let dangerous = [
        ("rm -rf /", "删除根目录"),
        ("rm -rf ~", "删除主目录"),
        ("sudo ", "提权命令"),
        ("mkfs", "格式化磁盘"),
        ("dd if=", "磁盘写入"),
        ("shutdown", "关机"),
        ("reboot", "重启"),
        (":(){", "fork 炸弹"),
        ("curl | sh", "远程脚本执行"),
        ("wget -qo- | sh", "远程脚本执行"),
        ("chmod 777", "危险权限"),
    ];
    for (pattern, reason) in dangerous {
        if c.contains(pattern) {
            return Some(reason);
        }
    }
    if uses_python_interpreter(&c) {
        return Some("已禁用 Python：请使用 Rust 工具链（cargo clippy/check/test）或系统原生命令");
    }
    None
}

/// 判断命令是否把 python 当作解释器调用（在某个命令段的开头），
/// 而非常规参数（如 grep 后跟文本 "python" 不会命中）。
fn uses_python_interpreter(lower: &str) -> bool {
    for seg in split_command_segments(lower) {
        // 段开头的第一个词才是被调用的命令（后面是参数）。
        let first = seg.split_whitespace().next().unwrap_or("");
        // 去掉路径前缀只看命令名本身，覆盖 /usr/bin/python、./venv/bin/python
        let base = first.rsplit('/').next().unwrap_or(first);
        for name in ["python3", "python2", "python"] {
            if base == name
                || (base.starts_with(name) && base.as_bytes().get(name.len()) == Some(&b'.'))
            {
                return true;
            }
        }
    }
    false
}

/// 把 shell 命令切成"命令段"：每个段以 行首 / | / && / || / ; 之后开始。
fn split_command_segments(lower: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    let bytes = lower.as_bytes();
    let flush = |out: &mut Vec<String>, cur: &mut String| {
        let s = cur.trim().to_string();
        if !s.is_empty() { out.push(s); }
        cur.clear();
    };
    while i < bytes.len() {
        if lower[i..].starts_with("&&") || lower[i..].starts_with("||") || bytes[i] == b';' || bytes[i] == b'|' {
            flush(&mut out, &mut cur);
            let len = if lower[i..].starts_with("&&") || lower[i..].starts_with("||") { 2 } else { 1 };
            i += len;
        } else {
            cur.push(lower[i..i + 1].chars().next().unwrap());
            i += 1;
        }
    }
    flush(&mut out, &mut cur);
    out
}

/// 执行内置工具，返回要回填给模型的文本结果。
pub fn exec_tool(ctx: &ToolContext, name: &str, args: &str) -> Result<String> {
    let parsed: serde_json::Value =
        serde_json::from_str(args).map_err(|e| AppError(format!("工具参数不是合法 JSON: {}", e)))?;

    let result = match name {
        "read_file" => {
            let path = parsed.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| AppError("缺少 path 参数".into()))?;
            let full = ctx.resolve(path)?;
            let content = std::fs::read_to_string(&full)
                .map_err(|e| AppError(format!("读取文件失败: {}", e)))?;
            Ok(truncate_chars(&content, 20000))
        }
        "write_file" => {
            let path = parsed.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| AppError("缺少 path 参数".into()))?;
            let content = parsed.get("content").and_then(|v| v.as_str())
                .ok_or_else(|| AppError("缺少 content 参数".into()))?;
            ctx.require_approval("write_file", format!("写入文件: {}", path))?;
            let full = ctx.resolve(path)?;
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full, content)
                .map_err(|e| AppError(format!("写入文件失败: {}", e)))?;
            logger::tool("write_file", format!("written: {}", full.display()));
            // 通知前端"生成文件"面板：给出相对路径与绝对路径
            let rel = full.strip_prefix(&ctx.workspace)
                .unwrap_or(&full)
                .to_string_lossy()
                .to_string();
            if let Some(ref channel) = ctx.channel {
                let _ = channel.send(StreamEvent::File {
                    path: rel,
                    abs_path: full.to_string_lossy().to_string(),
                    kind: "write_file".into(),
                });
            }
            Ok(format!("已写入: {}", full.display()))
        }
        "list_files" => {
            let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let full = ctx.resolve(path)?;
            let entries = std::fs::read_dir(&full)
                .map_err(|e| AppError(format!("列出目录失败: {}", e)))?;
            let mut lines = Vec::new();
            for entry in entries.flatten() {
                let ty = entry.file_type().ok();
                let mark = if ty.map(|t| t.is_dir()).unwrap_or(false) { "/" } else { "" };
                lines.push(format!("{}{}", entry.file_name().to_string_lossy(), mark));
            }
            lines.sort();
            Ok(lines.join("\n"))
        }
        "run_command" => {
            let command = parsed.get("command").and_then(|v| v.as_str())
                .ok_or_else(|| AppError("缺少 command 参数".into()))?;
            if let Some(reason) = is_blocked_command(command) {
                logger::warn(format!("拦截危险命令 ({}): {}", reason, command));
                return Err(AppError(format!("命令被安全策略拦截：{}", reason)));
            }
            ctx.require_approval("run_command", format!("执行命令: {}", command))?;
            logger::tool("run_command", command);
            let repaired_command = repair_mojibake(command);
            run_command_shell(&repaired_command, &ctx.workspace, ctx)
        }
        _ => Err(AppError(format!("未知内置工具: {}", name))),
    };

    if let Some(ref channel) = ctx.channel {
        let display = match &result {
            Ok(text) => text.clone(),
            Err(error) => format!("执行失败：{}", error.0),
        };
        let _ = channel.send(StreamEvent::ToolResult {
            name: name.to_string(),
            result: truncate_chars(&display, 500),
        });
    }
    result
}

fn strip_powershell_prefix(command: &str) -> &str {
    let mut rest = command.trim();
    let first_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let first = &rest[..first_end];
    if !first.eq_ignore_ascii_case("powershell")
        && !first.eq_ignore_ascii_case("powershell.exe")
        && !first.eq_ignore_ascii_case("pwsh")
        && !first.eq_ignore_ascii_case("pwsh.exe")
    {
        return rest;
    }
    rest = rest[first_end..].trim_start();
    loop {
        let token_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let token = &rest[..token_end];
        let lower = token.to_ascii_lowercase();
        if matches!(lower.as_str(), "-nop" | "-noprofile" | "-noninteractive" | "-c" | "-command") {
            rest = rest[token_end..].trim_start();
        } else {
            break;
        }
    }
    if rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"') {
        rest = &rest[1..rest.len() - 1];
    }
    rest
}

fn normalize_shell_command(command: &str) -> (bool, &str) {
    let trimmed = command.trim();
    let lower = trimmed.to_ascii_lowercase();
    let starts_powershell = lower.starts_with("powershell.exe")
        || lower.starts_with("powershell ")
        || lower.starts_with("pwsh.exe")
        || lower.starts_with("pwsh ");
    let looks_powershell = starts_powershell
        || trimmed.contains("& '")
        || lower.contains("get-childitem")
        || lower.contains("write-output")
        || lower.contains("2>null")
        || trimmed.contains('$');
    let cleaned = if starts_powershell { strip_powershell_prefix(trimmed) } else { trimmed };
    (looks_powershell, cleaned)
}

#[cfg(target_os = "windows")]
fn kill_child_tree(child: &mut std::process::Child) {
    let pid = child.id().to_string();
    let _ = child.kill();
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .output();
    let _ = child.wait();
}

#[cfg(not(target_os = "windows"))]
fn kill_child_tree(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn run_command_shell(command: &str, cwd: &Path, ctx: &ToolContext) -> Result<String> {
    let (use_powershell, cleaned) = normalize_shell_command(command);

    #[cfg(target_os = "windows")]
    let shell_command = if use_powershell {
        format!("[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;$OutputEncoding=[System.Text.Encoding]::UTF8;{}", cleaned)
    } else {
        format!("chcp.com 65001 >nul && {}", cleaned)
    };

    #[cfg(target_os = "windows")]
    let mut child = if use_powershell {
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", shell_command.as_str()])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    } else {
        std::process::Command::new("cmd")
            .args(["/C", shell_command.as_str()])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    }
    .map_err(|e| AppError(format!("启动命令失败: {}", e)))?;

    #[cfg(not(target_os = "windows"))]
    let mut child = std::process::Command::new("sh")
        .args(["-c", cleaned])
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError(format!("启动命令失败: {}", e)))?;

    let stdout_arc = Arc::new(Mutex::new(String::new()));
    let stderr_arc = Arc::new(Mutex::new(String::new()));
    let mut stdout_reader = child.stdout.take().ok_or_else(|| AppError("无法读取命令标准输出".into()))?;
    let mut stderr_reader = child.stderr.take().ok_or_else(|| AppError("无法读取命令错误输出".into()))?;
    let stdout_handle = {
        let buffer = Arc::clone(&stdout_arc);
        thread::spawn(move || {
            let mut text = String::new();
            let _ = stdout_reader.read_to_string(&mut text);
            *buffer.lock().unwrap() = text;
        })
    };
    let stderr_handle = {
        let buffer = Arc::clone(&stderr_arc);
        thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr_reader.read_to_string(&mut text);
            *buffer.lock().unwrap() = text;
        })
    };

    let timeout = Duration::from_secs(30);
    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if ctx.is_cancelled() {
                    kill_child_tree(&mut child);
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(AppError("命令已被用户中止".into()));
                }
                if start.elapsed() > timeout {
                    kill_child_tree(&mut child);
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(AppError("命令执行超时（30 秒），已终止进程树".into()));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(AppError(format!("等待命令失败: {}", error))),
        }
    };

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();
    let stdout = repair_mojibake(&stdout_arc.lock().unwrap().clone());
    let stderr = repair_mojibake(&stderr_arc.lock().unwrap().clone());
    let mut result = String::new();
    result.push_str(&format!("退出码: {}\n", status.code().unwrap_or(-1)));
    if !stdout.is_empty() {
        result.push_str("--- stdout ---\n");
        result.push_str(&stdout);
    }
    if !stderr.is_empty() {
        result.push_str("--- stderr ---\n");
        result.push_str(&stderr);
    }
    Ok(truncate_chars(&result, 20000))
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32, 0x3400..=0x9FFF | 0xF900..=0xFAFF)
}

fn repair_mojibake(text: &str) -> String {
    let has_latin1 = text.chars().any(|ch| ('\u{0080}'..='\u{00FF}').contains(&ch));
    let has_cjk = text.chars().any(is_cjk);
    if !has_latin1 || !has_cjk {
        return text.to_string();
    }
    let mut bytes = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let code = ch as u32;
        if (0x80..=0xFF).contains(&code) {
            bytes.push(code as u8);
        } else {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
        }
    }
    match String::from_utf8(bytes) {
        Ok(repaired) if !repaired.contains('\u{FFFD}') => repaired,
        _ => text.to_string(),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}…（内容过长已截断）", &s[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::new(PathBuf::from("/proj"), ToolPolicy::Auto, ApprovalBroker::default())
    }

    #[test]
    fn resolves_within_workspace() {
        assert_eq!(ctx().resolve("src/a.rs").unwrap(), PathBuf::from("/proj/src/a.rs"));
    }

    #[test]
    fn blocks_path_traversal() {
        assert!(ctx().resolve("../etc/passwd").is_err());
    }

    #[test]
    fn read_write_file_roundtrip() {
        let dir = std::env::temp_dir().join(format!("dkide-tool-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let c = ToolContext::new(dir.clone(), ToolPolicy::Auto, ApprovalBroker::default());
        let args = r#"{"path":"a.txt","content":"hello"}"#;
        assert!(exec_tool(&c, "write_file", args).is_ok());
        assert_eq!(exec_tool(&c, "read_file", r#"{"path":"a.txt"}"#).unwrap(), "hello");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn blocks_dangerous_commands() {
        assert!(is_blocked_command("rm -rf /").is_some());
        assert!(is_blocked_command("sudo rm -rf /var").is_some());
        assert!(is_blocked_command("ls -la").is_none());
    }

    #[test]
    fn blocks_python_interpreter() {
        // 作为解释器调用 → 拦截
        assert!(is_blocked_command("python3 script.py").is_some());
        assert!(is_blocked_command("python script.py").is_some());
        assert!(is_blocked_command("python3.11 script.py").is_some());
        assert!(is_blocked_command("cargo test && python3 analyze.py").is_some());
        assert!(is_blocked_command("echo hi | python3").is_some());
        // 非常规参数 → 不拦
        assert!(is_blocked_command("grep python src/main.rs").is_none());
        assert!(is_blocked_command("ls python-files").is_none());
        assert!(is_blocked_command("cargo clippy").is_none());
    }
}
