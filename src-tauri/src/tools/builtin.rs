use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use tauri::ipc::Channel;

use crate::approval::ApprovalBroker;
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
}

impl ToolContext {
    pub fn new(workspace: PathBuf, policy: ToolPolicy, approvals: ApprovalBroker) -> Self {
        Self { workspace, policy, approvals, channel: None }
    }

    pub fn with_channel(mut self, channel: Channel<StreamEvent>) -> Self {
        self.channel = Some(channel);
        self
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
    None
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
            run_command_shell(command, &ctx.workspace)
        }
        _ => Err(AppError(format!("未知内置工具: {}", name))),
    };

    if let Ok(ref text) = result {
        if let Some(ref channel) = ctx.channel {
            let _ = channel.send(StreamEvent::ToolResult {
                name: name.to_string(),
                result: truncate_chars(text, 500),
            });
        }
    }
    result
}

fn run_command_shell(command: &str, cwd: &Path) -> Result<String> {
    #[cfg(target_os = "windows")]
    let mut child = std::process::Command::new("cmd")
        .args(["/C", command])
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError(format!("启动命令失败: {}", e)))?;

    #[cfg(not(target_os = "windows"))]
    let mut child = std::process::Command::new("sh")
        .args(["-c", command])
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError(format!("启动命令失败: {}", e)))?;

    // 带超时的等待：轮询 try_wait，30 秒后 kill
    let timeout = Duration::from_secs(30);
    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError("命令执行超时（30 秒）".into()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(AppError(format!("等待命令失败: {}", e))),
        }
    };

    let output = child
        .wait_with_output()
        .map_err(|e| AppError(format!("读取命令输出失败: {}", e)))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
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
}
