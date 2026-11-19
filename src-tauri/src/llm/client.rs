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
            enable_thinking, temperature, max_tokens,
        }
    }

    pub async fn run(
        &self,
        history: &[Message],
        channel: &Channel<StreamEvent>,
    ) -> Result<(String, String, UsageStats)> {
        let messages = self.build_messages(history);
        let tools = self.build_tools();
        let price = if self.custom_price > 0.0 { self.custom_price } else { model_price(&self.provider.model) };
        let mut total_usage = UsageStats::default();
        let mut all_reasoning = String::new();

        for _ in 0..MAX_TOOL_ROUNDS {
            if self.cancels.is_cancelled(self.session_id) {
                let _ = channel.send(StreamEvent::Cancelled);
                return Ok((String::new(), all_reasoning, total_usage));
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
                let _ = channel.send(StreamEvent::Done { content: result.content.clone(), reasoning: all_reasoning.clone() });
                return Ok((result.content, all_reasoning, total_usage));
            }

            let wire_calls: Vec<WireToolCall> = result.tool_calls.iter().map(|c| WireToolCall {
                id: if c.id.is_empty() { format!("call_{}", c.index) } else { c.id.clone() },
                kind: "function".into(),
                function: WireFunctionCall { name: c.name.clone(), arguments: sanitize_arguments(&c.arguments) },
            }).collect();
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
                    return Ok((String::new(), all_reasoning, total_usage));
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
        Ok((msg, all_reasoning, total_usage))
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(String, String, UsageStats)>> + Send + 'b>> {
        Box::pin(async move {
            if round >= MAX_TOOL_ROUNDS || self.cancels.is_cancelled(self.session_id) {
                let _ = channel.send(StreamEvent::Cancelled);
                return Ok((String::new(), acc_reasoning, total_usage));
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
                let _ = channel.send(StreamEvent::Done { content: result.content.clone(), reasoning: acc_reasoning.clone() });
                return Ok((result.content, acc_reasoning, total_usage));
            }

            let mut next = messages.to_vec();
            next.push(WireMessage {
                role: "assistant".into(),
                content: serde_json::json!(result.content),
                tool_calls: Some(result.tool_calls.iter().map(|c| WireToolCall {
                    id: if c.id.is_empty() { format!("call_{}", c.index) } else { c.id.clone() },
                    kind: "function".into(),
                    function: WireFunctionCall { name: c.name.clone(), arguments: sanitize_arguments(&c.arguments) },
                }).collect()),
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
        let url = format!("{}/chat/completions", self.provider.base_url.trim_end_matches('/'));
        let req = crate::llm::types::ChatRequest {
            model: self.provider.model.clone(),
            messages: messages.to_vec(),
            stream: true,
            tools: tools.to_vec(),
            enable_thinking: self.enable_thinking,
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
        prompt.push_str(&format!(
            "\n\n## 工作环境\n文件工具的工作目录为: {}\n你可以使用 read_file / write_file / list_files / run_command 工具。危险操作会请求用户批准。\n\n## 机检（静态检查）约定\n做代码检查/静态分析只走 Rust 工具链：cargo clippy（lint）、cargo check（编译检查）、cargo test（测试）。禁止使用 Python（python/python3）执行机检或任何脚本任务；通用命令优先用 grep/ripgrep/awk 或系统原生工具。\n",
            self.tool_ctx.workspace.display()
        ));
        if !self.skills.is_empty() {
            prompt.push_str("\n\n## 可用技能\n");
            prompt.push_str("当任务匹配某个技能时，调用 activate_skill 工具加载其完整指令并遵循执行。\n");
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
            tools.push(definition("activate_skill", "加载并激活指定技能的详细指令。", serde_json::json!({
                "type": "object",
                "properties": { "skill_name": { "type": "string" } },
                "required": ["skill_name"]
            })));
        }
        tools
    }

    fn execute_tool(&self, name: &str, args: &str) -> String {
        match name {
            "activate_skill" => {
                let parsed: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
                let skill_name = parsed.get("skill_name").and_then(|v| v.as_str()).unwrap_or("");
                self.skill_instructions.get(skill_name).cloned()
                    .unwrap_or_else(|| format!("激活技能失败：找不到技能 {}", skill_name))
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
