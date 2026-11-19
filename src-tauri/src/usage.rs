use serde::{Deserialize, Serialize};

/// token 用量与费用统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub requests: u64,
    pub estimated_cost: f64,
}

impl UsageStats {
    pub fn add(&mut self, prompt: u64, completion: u64, price_per_million: f64) {
        self.prompt_tokens += prompt;
        self.completion_tokens += completion;
        self.total_tokens += prompt + completion;
        self.requests += 1;
        self.estimated_cost +=
            (prompt + completion) as f64 / 1_000_000.0 * price_per_million;
    }

    /// 粗估 token 数（约 4 字符 1 token），用于中转站未返回 usage 时兜底。
    pub fn estimate(text: &str) -> u64 {
        ((text.chars().count() + 3) / 4) as u64
    }
}

/// 各模型每百万 token 参考价（USD，输入/输出平均）。可在设置中覆盖。
pub fn model_price(model: &str) -> f64 {
    let m = model.to_lowercase();
    if m.contains("gpt-4o") && !m.contains("mini") { 5.0 }
    else if m.contains("gpt-4o-mini") { 0.3 }
    else if m.contains("gpt-4") { 30.0 }
    else if m.contains("gpt-3.5") { 1.0 }
    else if m.contains("claude-3.5") || m.contains("claude-3-5") { 6.0 }
    else if m.contains("claude") { 4.0 }
    else if m.contains("deepseek") { 1.0 }
    else if m.contains("qwen") || m.contains("通义") { 2.0 }
    else { 2.0 }
}

/// 对话历史 token 预算裁剪：保留系统提示语义与最近若干轮，丢弃过老消息。
pub fn trim_history<T: Clone>(messages: &[T], texts: &[String], max_tokens: usize, keep_recent: usize) -> Vec<T> {
    if messages.len() <= keep_recent {
        return messages.to_vec();
    }
    let estimated: usize = texts.iter().map(|t| UsageStats::estimate(t) as usize).sum();
    if estimated <= max_tokens {
        return messages.to_vec();
    }
    let start = messages.len() - keep_recent;
    let mut result = Vec::new();
    // 保留第一条用户消息作为上下文锚点
    result.push(messages[0].clone());
    if start > 1 {
        result.push(messages[1].clone());
    }
    result.extend_from_slice(&messages[start..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_accumulates() {
        let mut u = UsageStats::default();
        u.add(1000, 500, 10.0);
        assert_eq!(u.total_tokens, 1500);
        assert!((u.estimated_cost - 0.015).abs() < 1e-6);
    }

    #[test]
    fn trims_when_over_budget() {
        let msgs: Vec<i32> = (0..20).collect();
        let texts: Vec<String> = (0..20).map(|_| "x".repeat(4000)).collect();
        let result = trim_history(&msgs, &texts, 100, 4);
        assert!(result.len() < msgs.len());
        assert_eq!(result[0], 0);
        assert_eq!(*result.last().unwrap(), 19);
    }
}
