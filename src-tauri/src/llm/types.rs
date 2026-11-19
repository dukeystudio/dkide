use serde::{Deserialize, Serialize};

/// 推送给前端的流式事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// 模型输出的文本增量
    Delta { text: String },
    /// 开始执行某个工具（技能）
    Tool { name: String, args: String },
    /// 工具执行结果
    ToolResult { name: String, result: String },
    /// 需要用户审批的命令
    ApprovalRequired { id: String, tool: String, summary: String },
    /// 用量统计
    Usage { prompt_tokens: u64, completion_tokens: u64, estimated_cost: f64 },
    /// 用户取消生成
    Cancelled,
    /// 本轮回答结束
    Done { content: String },
    /// 出错
    Error { message: String },
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<WireMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct WireMessage {
    pub role: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<WireToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WireToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: WireFunctionCall,
}

#[derive(Debug, Clone, Serialize)]
pub struct WireFunctionCall {
    pub name: String,
    pub arguments: String,
}

/// SSE 返回的单个 chunk
#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<UsageInfo>,
}

#[derive(Debug, Deserialize)]
struct UsageInfo {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Debug, Deserialize, Default)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    function: DeltaFunction,
}

#[derive(Debug, Deserialize, Default)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// 一次流式响应的累积结果
#[derive(Debug, Default)]
pub struct StreamResult {
    pub content: String,
    pub tool_calls: Vec<AccumToolCall>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Default, Clone)]
pub struct AccumToolCall {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// 逐块解析 SSE 数据，返回本块产生的文本增量，并累积工具调用。
pub fn consume_sse_buffer(
    buffer: &mut String,
    result: &mut StreamResult,
) -> Vec<String> {
    let mut texts = Vec::new();
    while let Some(pos) = buffer.find("\n\n") {
        let raw: String = buffer.drain(..=pos + 1).collect();
        for line in raw.lines() {
            let data = match line.strip_prefix("data:") {
                Some(d) => d.trim(),
                None => continue,
            };
            if data == "[DONE]" {
                continue;
            }
            let chunk: StreamChunk = match serde_json::from_str(data) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(ref usage) = chunk.usage {
                result.prompt_tokens = usage.prompt_tokens;
                result.completion_tokens = usage.completion_tokens;
            }
            for choice in &chunk.choices {
                let delta = &choice.delta;
                if let Some(ref text) = delta.content {
                    if !text.is_empty() {
                        result.content.push_str(text);
                        texts.push(text.clone());
                    }
                }
                if let Some(ref calls) = delta.tool_calls {
                    for call in calls {
                        let acc = match result
                            .tool_calls
                            .iter_mut()
                            .find(|c| c.index == call.index)
                        {
                            Some(c) => c,
                            None => {
                                result.tool_calls.push(AccumToolCall {
                                    index: call.index,
                                    ..Default::default()
                                });
                                result.tool_calls.last_mut().unwrap()
                            }
                        };
                        if let Some(ref id) = call.id {
                            acc.id.push_str(id);
                        }
                        if let Some(ref name) = call.function.name {
                            acc.name.push_str(name);
                        }
                        if let Some(ref args) = call.function.arguments {
                            acc.arguments.push_str(args);
                        }
                    }
                }
            }
        }
    }
    texts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_deltas() {
        let mut buffer = String::new();
        let mut result = StreamResult::default();
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n");
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n");
        let texts = consume_sse_buffer(&mut buffer, &mut result);
        assert_eq!(texts, vec!["你", "好"]);
        assert_eq!(result.content, "你好");
    }

    #[test]
    fn accumulates_streamed_tool_calls() {
        let mut buffer = String::new();
        let mut result = StreamResult::default();
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"activate_skill\",\"arguments\":\"\"}}]}}]}\n\n");
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"skill_name\\\":\\\"x\\\"}\"}}]}}]}\n\n");
        consume_sse_buffer(&mut buffer, &mut result);
        assert_eq!(result.tool_calls.len(), 1);
        let call = &result.tool_calls[0];
        assert_eq!(call.id, "call_1");
        assert_eq!(call.name, "activate_skill");
        assert_eq!(call.arguments, "{\"skill_name\":\"x\"}");
    }
}
