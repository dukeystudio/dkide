use std::collections::HashMap;

use futures::StreamExt;
use tauri::ipc::Channel;

use crate::cancel::CancelRegistry;
use crate::error::{AppError, Result};
use crate::llm::types::{
    consume_sse_buffer, StreamEvent, StreamResult, ToolDefinition, ToolFunction, WireFunctionCall,
    WireMessage, WireToolCall,
};
use crate::logger;
use crate::models::{Message, Provider, Role};
use crate::tools::{exec_tool, ToolContext};
use crate::usage::{model_price, trim_history, UsageStats};

const MAX_TOOL_ROUNDS: usize = 12;

fn resolve_answer_content(content: String, reasoning: &str) -> String {
    if !content.trim().is_empty() {
        return content;
    }
    if reasoning.trim().is_empty() {
        "（模型本轮未返回内容。）".into()
    } else {
        "（模型本轮未返回正文，仅返回了思考过程。）".into()
    }
}

fn tool_call_signature(name: &str, arguments: &str) -> String {
    format!("{}:{}", name, arguments)
}

fn has_repeated_tool_call(messages: &[WireMessage], signatures: &[String]) -> bool {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for message in messages {
        if let Some(calls) = &message.tool_calls {
            for call in calls {
                let signature = tool_call_signature(&call.function.name, &call.function.arguments);
                *counts.entry(signature).or_insert(0) += 1;
            }
        }
    }
    signatures.iter().any(|signature| counts.get(signature).copied().unwrap_or(0) >= 2)
}

pub struct ChatEngine<'a> {
    provider: &'a Provider,
    system_prompt: &'a str,
    skills: &'a [(String, String)],
    skill_instructions: &'a HashMap<String, String>,
    tool_ctx: &'a ToolContext,
    cancels: &'a CancelRegistry,
    session_id: &'a str,
    http: &'a reqwest::Client,
    token_budget: usize,
    custom_price: f64,
    enable_thinking: Option<bool>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    read_only: bool,
}

impl<'a> ChatEngine<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: &'a Provider,
        system_prompt: &'a str,
        skills: &'a [(String, String)],
        skill_instructions: &'a HashMap<String, String>,
        tool_ctx: &'a ToolContext,
        cancels: &'a CancelRegistry,
        session_id: &'a str,
        http: &'a reqwest::Client,
        token_budget: usize,
        custom_price: f64,
        enable_thinking: Option<bool>,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Self {
        Self {
            provider, system_prompt, skills, skill_instructions, tool_ctx,
            cancels, session_id, http, token_budget, custom_price,
            enable_thinking, temperature, max_tokens, read_only: false,
        }
    }

    pub fn with_read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub async fn run(
        &self,
        history: &[Message],
        channel: &Channel<StreamEvent>,
    ) -> Result<(String, String, UsageStats, bool)> {
        let messages = self.build_messages(history);
        let tools = if self.provider.tool_calling { self.build_tools() } else { Vec::new() };
        let price = if self.custom_price > 0.0 { self.custom_price } else { model_price(&self.provider.model) };
        let mut total_usage = UsageStats::default();
        let mut all_reasoning = String::new();

        for _ in 0..MAX_TOOL_ROUNDS {
            if self.cancels.is_cancelled(self.session_id) {
                let _ = channel.send(StreamEvent::Cancelled);
                return Ok((String::new(), String::new(), total_usage, false));
            }

            let result = self.stream_once(&messages, &tools, channel).await?;
            if !result.reasoning.is_empty() {
                if !all_reasoning.is_empty() { all_reasoning.push('\n'); }
                all_reasoning.push_str(&result.reasoning);
            }

            let prompt = if result.prompt_tokens > 0 { result.prompt_tokens } else { UsageStats::estimate(&serde_json::to_string(&messages).unwrap_or_default()) };
            let completion = if result.completion_tokens > 0 { result.completion_tokens } else { UsageStats::estimate(&result.content) };
            total_usage.add(prompt, completion, price);
            let _ = channel.send(StreamEvent::Usage {
                prompt_tokens: total_usage.prompt_tokens,
                completion_tokens: total_usage.completion_tokens,
                estimated_cost: total_usage.estimated_cost,
            });
            logger::llm(format!("model={} prompt={} completion={}", self.provider.model, prompt, completion));

            if result.tool_calls.is_empty() {
                let mut truncated = result.is_truncated();
                let mut content = result.content;
                if content.trim().is_empty() && !all_reasoning.trim().is_empty() {
                    logger::llm("empty answer after reasoning, retrying without thinking");
                    if let Ok(retry) = self.stream_once_with_thinking(&messages, &tools, channel, Some(false), None).await {
                        if !retry.content.trim().is_empty() {
                            truncated = retry.is_truncated();
                            content = retry.content;
                            let prompt = if retry.prompt_tokens > 0 { retry.prompt_tokens } else { UsageStats::estimate(&serde_json::to_string(&messages).unwrap_or_default()) };
                            let completion = if retry.completion_tokens > 0 { retry.completion_tokens } else { UsageStats::estimate(&content) };
                            total_usage.add(prompt, completion, price);
                            let _ = channel.send(StreamEvent::Usage {
                                prompt_tokens: total_usage.prompt_tokens,
                                completion_tokens: total_usage.completion_tokens,
                                estimated_cost: total_usage.estimated_cost,
                            });
                        }
                    }
                }
                let answer = resolve_answer_content(content, &all_reasoning);
                let _ = channel.send(StreamEvent::Done { content: answer.clone(), reasoning: all_reasoning.clone(), truncated });
                return Ok((answer, all_reasoning, total_usage, truncated));
            }

            let wire_calls: Vec<WireToolCall> = result.tool_calls.iter().map(|c| WireToolCall {
                id: if c.id.is_empty() { format!("call_{}", c.index) } else { c.id.clone() },
                kind: "function".into(),
                function: WireFunctionCall { name: c.name.clone(), arguments: sanitize_arguments(&c.arguments) },
            }).collect();
            let signatures: Vec<String> = wire_calls.iter()
                .map(|call| tool_call_signature(&call.function.name, &call.function.arguments))
                .collect();
            if has_repeated_tool_call(&messages, &signatures) {
                logger::llm("repeated tool calls detected, forcing final answer");
                return self.force_final_answer(&messages, channel, &all_reasoning, total_usage).await;
            }
            let mut next = messages.clone();
            next.push(WireMessage {
                role: "assistant".into(),
                content: serde_json::json!(if result.content.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(result.content.clone()) }),
                tool_calls: Some(wire_calls),
                tool_call_id: None,
            });

            for call in &result.tool_calls {
                if self.cancels.is_cancelled(self.session_id) {
                    let _ = channel.send(StreamEvent::Cancelled);
                    return Ok((String::new(), String::new(), total_usage, false));
                }
                let call_id = if call.id.is_empty() { format!("call_{}", call.index) } else { call.id.clone() };
                let _ = channel.send(StreamEvent::Tool {
                    name: call.name.clone(),
                    args: call.arguments.clone(),
                });
                let output = self.execute_tool(&call.name, &call.arguments);
                // 工具错误仅回填给模型，不重复推送 error 事件
                next.push(WireMessage {
                    role: "tool".into(),
                    content: serde_json::json!(output),
                    tool_calls: None,
                    tool_call_id: Some(call_id),
                });
            }
            // 下一轮使用包含工具结果的上下文（携带已累积的思考过程）
            return self.run_with_messages(&next, &tools, channel, total_usage, all_reasoning, 1).await;
        }

        let msg = "工具调用轮数超过上限，已停止。".to_string();
        let _ = channel.send(StreamEvent::Error { message: msg.clone() });
        Ok((msg, all_reasoning, total_usage, false))
    }

    /// 递归处理后续工具轮次（acc_reasoning 携带前序轮次累积的思考过程）
    fn run_with_messages<'b>(
        &'b self,
        messages: &'b [WireMessage],
        tools: &'b [ToolDefinition],
        channel: &'b Channel<StreamEvent>,
        mut total_usage: UsageStats,
        mut acc_reasoning: String,
        round: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(String, String, UsageStats, bool)>> + Send + 'b>> {
        Box::pin(async move {
            if round >= MAX_TOOL_ROUNDS || self.cancels.is_cancelled(self.session_id) {
                let _ = channel.send(StreamEvent::Cancelled);
                return Ok((String::new(), String::new(), total_usage, false));
            }
            let price = if self.custom_price > 0.0 { self.custom_price } else { model_price(&self.provider.model) };
            let result = self.stream_once(messages, tools, channel).await?;
            if !result.reasoning.is_empty() {
                if !acc_reasoning.is_empty() { acc_reasoning.push('\n'); }
                acc_reasoning.push_str(&result.reasoning);
            }
            let prompt = if result.prompt_tokens > 0 { result.prompt_tokens } else { UsageStats::estimate(&serde_json::to_string(messages).unwrap_or_default()) };
            let completion = if result.completion_tokens > 0 { result.completion_tokens } else { UsageStats::estimate(&result.content) };
            total_usage.add(prompt, completion, price);
            let _ = channel.send(StreamEvent::Usage {
                prompt_tokens: total_usage.prompt_tokens,
                completion_tokens: total_usage.completion_tokens,
                estimated_cost: total_usage.estimated_cost,
            });

            if result.tool_calls.is_empty() {
                let mut truncated = result.is_truncated();
                let mut content = result.content;
                if content.trim().is_empty() && !acc_reasoning.trim().is_empty() {
                    logger::llm("empty answer after tool rounds, retrying without thinking");
                    if let Ok(retry) = self.stream_once_with_thinking(messages, tools, channel, Some(false), None).await {
                        if !retry.content.trim().is_empty() {
                            truncated = retry.is_truncated();
                            content = retry.content;
                            let prompt = if retry.prompt_tokens > 0 { retry.prompt_tokens } else { UsageStats::estimate(&serde_json::to_string(messages).unwrap_or_default()) };
                            let completion = if retry.completion_tokens > 0 { retry.completion_tokens } else { UsageStats::estimate(&content) };
                            total_usage.add(prompt, completion, price);
                            let _ = channel.send(StreamEvent::Usage {
                                prompt_tokens: total_usage.prompt_tokens,
                                completion_tokens: total_usage.completion_tokens,
                                estimated_cost: total_usage.estimated_cost,
                            });
                        }
                    }
                }
                let answer = resolve_answer_content(content, &acc_reasoning);
                let _ = channel.send(StreamEvent::Done { content: answer.clone(), reasoning: acc_reasoning.clone(), truncated });
                return Ok((answer, acc_reasoning, total_usage, truncated));
            }

            let wire_calls: Vec<WireToolCall> = result.tool_calls.iter().map(|c| WireToolCall {
                id: if c.id.is_empty() { format!("call_{}", c.index) } else { c.id.clone() },
                kind: "function".into(),
                function: WireFunctionCall { name: c.name.clone(), arguments: sanitize_arguments(&c.arguments) },
            }).collect();
            let signatures: Vec<String> = wire_calls.iter()
                .map(|call| tool_call_signature(&call.function.name, &call.function.arguments))
                .collect();
            if has_repeated_tool_call(messages, &signatures) {
                logger::llm("repeated tool calls detected in later round, forcing final answer");
                return self.force_final_answer(messages, channel, &acc_reasoning, total_usage).await;
            }
            let mut next = messages.to_vec();
            next.push(WireMessage {
                role: "assistant".into(),
                content: serde_json::json!(result.content),
                tool_calls: Some(wire_calls),
                tool_call_id: None,
            });
            for call in &result.tool_calls {
                let call_id = if call.id.is_empty() { format!("call_{}", call.index) } else { call.id.clone() };
                let _ = channel.send(StreamEvent::Tool { name: call.name.clone(), args: call.arguments.clone() });
                let output = self.execute_tool(&call.name, &call.arguments);
                next.push(WireMessage {
                    role: "tool".into(), content: serde_json::json!(output),
                    tool_calls: None, tool_call_id: Some(call_id),
                });
            }
            self.run_with_messages(&next, tools, channel, total_usage, acc_reasoning, round + 1).await
        })
    }

    async fn stream_once(
        &self,
        messages: &[WireMessage],
        tools: &[ToolDefinition],
        channel: &Channel<StreamEvent>,
    ) -> Result<StreamResult> {
        let reasoning_effort = self.provider.reasoning_effort.as_ref().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
        });
        self.stream_once_with_thinking(messages, tools, channel, self.enable_thinking, reasoning_effort).await
    }

    async fn stream_once_with_thinking(
        &self,
        messages: &[WireMessage],
        tools: &[ToolDefinition],
        channel: &Channel<StreamEvent>,
        enable_thinking: Option<bool>,
        reasoning_effort: Option<String>,
    ) -> Result<StreamResult> {
        let url = format!("{}/chat/completions", self.provider.base_url.trim_end_matches('/'));
        let req = crate::llm::types::ChatRequest {
            model: self.provider.model.clone(),
            messages: messages.to_vec(),
            stream: true,
            tools: tools.to_vec(),
            enable_thinking,
            reasoning_effort,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        };
        let response = self.http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.provider.api_key))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(AppError(format!("模型请求失败 ({}): {}", status, truncate(&text, 500))));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut result = StreamResult::default();

        while let Some(chunk) = stream.next().await {
            if self.cancels.is_cancelled(self.session_id) {
                break;
            }
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            for piece in consume_sse_buffer(&mut buffer, &mut result) {
                match piece {
                    crate::llm::types::SsePiece::Text(t) => {
                        let _ = channel.send(StreamEvent::Delta { text: t });
                    }
                    crate::llm::types::SsePiece::Reasoning(t) => {
                        let _ = channel.send(StreamEvent::Reasoning { text: t });
                    }
                }
            }
        }
        Ok(result)
    }

    async fn force_final_answer(
        &self,
        messages: &[WireMessage],
        channel: &Channel<StreamEvent>,
        reasoning: &str,
        mut total_usage: UsageStats,
    ) -> Result<(String, String, UsageStats, bool)> {
        let mut forced = messages.to_vec();
        forced.push(WireMessage {
            role: "user".into(),
            content: serde_json::json!("检测到重复工具调用，请立即停止调用工具，直接基于已有信息给出最终回答。"),
            tool_calls: None,
            tool_call_id: None,
        });
        let price = if self.custom_price > 0.0 { self.custom_price } else { model_price(&self.provider.model) };
        match self.stream_once_with_thinking(&forced, &[], channel, Some(false), None).await {
            Ok(result) => {
                let prompt = if result.prompt_tokens > 0 { result.prompt_tokens } else { UsageStats::estimate(&serde_json::to_string(&forced).unwrap_or_default()) };
                let completion = if result.completion_tokens > 0 { result.completion_tokens } else { UsageStats::estimate(&result.content) };
                total_usage.add(prompt, completion, price);
                let _ = channel.send(StreamEvent::Usage {
                    prompt_tokens: total_usage.prompt_tokens,
                    completion_tokens: total_usage.completion_tokens,
                    estimated_cost: total_usage.estimated_cost,
                });
                let answer = resolve_answer_content(result.content, reasoning);
                let _ = channel.send(StreamEvent::Done { content: answer.clone(), reasoning: reasoning.to_string(), truncated: false });
                Ok((answer, reasoning.to_string(), total_usage, false))
            }
            Err(error) => {
                let answer = format!("检测到重复工具调用，已停止自动尝试。{}\n请补充说明需要生成的具体内容。", error.0);
                let _ = channel.send(StreamEvent::Done { content: answer.clone(), reasoning: reasoning.to_string(), truncated: false });
                Ok((answer, reasoning.to_string(), total_usage, false))
            }
        }
    }

    fn build_messages(&self, history: &[Message]) -> Vec<WireMessage> {
        let texts: Vec<String> = history.iter().map(|m| m.content.clone()).collect();
        let trimmed = trim_history(history, &texts, self.token_budget, 20);
        let mut messages = Vec::new();
        messages.push(WireMessage {
            role: "system".into(),
            content: serde_json::json!(self.system_prompt()),
            tool_calls: None,
            tool_call_id: None,
        });
        for msg in &trimmed {
            if matches!(msg.role, Role::Tool) { continue; }
            let role = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => continue,
            };
            messages.push(WireMessage {
                role: role.into(),
                content: serde_json::json!(msg.content),
                tool_calls: None,
                tool_call_id: None,
            });
        }
        messages
    }

    fn system_prompt(&self) -> String {
        let mut prompt = self.system_prompt.to_string();
        if self.provider.tool_calling {
            prompt.push_str(&format!(
                "\n\n## 工作环境\n文件工具的工作目录为: {}\n你可以使用 read_file / write_file / list_files / run_command 工具。危险操作会请求用户批准。\n\n## 机检（静态检查）约定\n做代码检查/静态分析只走 Rust 工具链：cargo clippy（lint）、cargo check（编译检查）、cargo test（测试）。禁止使用 Python（python/python3）执行机检或任何脚本任务；通用命令优先用 grep/ripgrep/awk 或系统原生工具。\n",
                self.tool_ctx.workspace.display()
            ));
        } else {
            prompt.push_str("\n\n## 工具能力\n当前服务商已关闭工具调用，你只能基于对话内已有信息回答。\n");
        }
        let image_capability = if self.provider.image_input {
            "该服务商已声明支持图片输入"
        } else {
            "该服务商未声明支持图片输入，请勿假设你能看到图片"
        };
        prompt.push_str(&format!("\n\n## 图片输入\n{}\n", image_capability));
        if !self.skills.is_empty() {
            prompt.push_str("\n\n## 可用技能\n");
            prompt.push_str("当任务匹配某个技能时，必须先调用 activate_skill 加载其指令，再按指令用 read_skill_reference 读取所需参考文件全文并严格遵循。禁止用 read_file / list_files / run_command 查找技能目录或参考文件。\n");
            for (name, desc) in self.skills {
                prompt.push_str(&format!("- {}：{}\n", name, desc));
            }
        }
        prompt
    }

    fn build_tools(&self) -> Vec<ToolDefinition> {
        let mut tools = vec![
            definition("read_file", "读取工作目录内文本文件内容。", serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            })),
            definition("write_file", "写入文件（需用户批准）。", serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                "required": ["path", "content"]
            })),
            definition("list_files", "列出目录内容。", serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": []
            })),
            definition("run_command", "执行 shell 命令（需用户批准，危险命令会被拦截）。", serde_json::json!({
                "type": "object",
                "properties": { "command": { "type": "string" } },
                "required": ["command"]
            })),
        ];
        if !self.skills.is_empty() {
            tools.push(definition("activate_skill", "加载并激活指定技能的详细指令（含参考文件清单）。", serde_json::json!({
                "type": "object",
                "properties": { "skill_name": { "type": "string" } },
                "required": ["skill_name"]
            })));
            tools.push(definition("read_skill_reference", "按文件名读取已激活技能的参考文件全文。", serde_json::json!({
                "type": "object",
                "properties": {
                    "skill_name": { "type": "string" },
                    "file_name": { "type": "string" }
                },
                "required": ["skill_name", "file_name"]
            })));
        }
        if self.read_only {
            tools.retain(|tool| matches!(tool.function.name.as_str(), "read_file" | "list_files" | "activate_skill" | "read_skill_reference"));
        }
        tools
    }

    fn execute_tool(&self, name: &str, args: &str) -> String {
        match name {
            "activate_skill" => {
                let parsed: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
                let skill_name = parsed.get("skill_name").and_then(|v| v.as_str()).unwrap_or("");
                let result = self.skill_instructions.get(skill_name).cloned()
                    .unwrap_or_else(|| format!("激活技能失败：找不到技能 {}", skill_name));
                crate::logger::info(&format!(
                    "[skill] 激活技能 name={} found={} chars={}",
                    skill_name,
                    !result.starts_with("激活技能失败"),
                    result.chars().count()
                ));
                result
            }
            "read_skill_reference" => {
                let parsed: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
                let skill_name = parsed.get("skill_name").and_then(|v| v.as_str()).unwrap_or("");
                let file_name = parsed.get("file_name").and_then(|v| v.as_str()).unwrap_or("");
                let key = format!("__ref__:{}:{}", skill_name, file_name);
                let result = self.skill_instructions.get(&key).cloned()
                    .unwrap_or_else(|| format!("找不到技能 {} 的参考文件 {}；可用文件见 activate_skill 返回的清单", skill_name, file_name));
                crate::logger::info(&format!(
                    "[skill] 读取参考文件 skill={} file={} found={} chars={}",
                    skill_name,
                    file_name,
                    !result.starts_with("找不到"),
                    result.chars().count()
                ));
                result
            }
            "read_file" | "write_file" | "list_files" | "run_command" => {
                match exec_tool(self.tool_ctx, name, args) {
                    Ok(text) => text,
                    Err(e) => format!("工具执行失败：{}", e),
                }
            }
            _ => format!("未知工具: {}", name),
        }
    }
}

/// 把模型返回的 arguments 规范化为合法 JSON 字符串。
/// 上游（OpenAI 兼容）要求 tool_call.arguments 必须是合法 JSON；
/// 模型偶发返回空串/被截断/非 JSON，直接回传会触发 400 "arguments must be valid JSON"。
fn sanitize_arguments(args: &str) -> String {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return "{}".into();
    }
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        trimmed.to_string()
    } else {
        // 兜底：原样回传会让上游报错，这里降级为空对象
        "{}".into()
    }
}

fn definition(name: &str, description: &str, parameters: serde_json::Value) -> ToolDefinition {
    ToolDefinition {
        kind: "function".into(),
        function: ToolFunction { name: name.into(), description: description.into(), parameters },
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_arguments;

    #[test]
    fn valid_json_kept() {
        assert_eq!(sanitize_arguments("{\"a\":1}"), "{\"a\":1}");
        assert_eq!(sanitize_arguments(""), "{}");
        assert_eq!(sanitize_arguments("   "), "{}");
    }

    #[test]
    fn invalid_json_falls_back() {
        assert_eq!(sanitize_arguments("not json"), "{}");
        assert_eq!(sanitize_arguments("{\"a\":"), "{}");
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.chars().count() > max {
        &s[..s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len())]
    } else {
        s
    }
}
