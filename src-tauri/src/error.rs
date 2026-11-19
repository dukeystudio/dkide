use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppError(pub String);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError(s.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError(friendly_request_error(&e))
    }
}

/// 把 reqwest 底层错误转成用户可读提示，避免直接暴露英文 URL 报错。
pub fn friendly_request_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "请求超时：网络响应过慢或服务商暂时不可用，请稍后重试".into()
    } else if error.is_connect() {
        "无法连接服务商：请检查网络、代理或 Base URL".into()
    } else if error.is_body() {
        "读取响应失败：服务商提前断开连接，请重试".into()
    } else if error.is_request() {
        format!("请求发送失败：{}", error)
    } else {
        format!("网络请求失败：{}", error)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
