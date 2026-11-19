use std::path::PathBuf;

use crate::config;
use crate::error::{AppError, Result};
use crate::logger;
use crate::models::{AppConfig, AuthSession, Session};

pub struct Store {
    pub dir: PathBuf,
    config_path: PathBuf,
    sessions_path: PathBuf,
    auth_path: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            config_path: dir.join("config.json"),
            sessions_path: dir.join("sessions.json"),
            auth_path: dir.join("auth.json"),
            dir,
        }
    }

    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::create_dir_all(self.dir.join("skills"))?;
        std::fs::create_dir_all(self.dir.join("logs"))?;
        if !self.config_path.exists() {
            self.save_config(&AppConfig::default())?;
        }
        if !self.sessions_path.exists() {
            self.save_sessions(&[])?;
        }
        Ok(())
    }

    pub fn load_config(&self) -> Result<AppConfig> {
        if !self.config_path.exists() {
            return Ok(AppConfig::default());
        }
        let text = std::fs::read_to_string(&self.config_path)?;
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => Ok(config::migrate(value)),
            Err(e) => {
                // 配置损坏时不让应用崩溃，备份后使用默认配置
                logger::error(format!("配置文件解析失败，已备份并使用默认配置: {}", e));
                let backup = self.dir.join("config.corrupted.json");
                let _ = std::fs::copy(&self.config_path, backup);
                Ok(AppConfig::default())
            }
        }
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        let text = serde_json::to_string_pretty(config)?;
        atomic_write(&self.config_path, &text)
    }

    pub fn load_sessions(&self) -> Result<Vec<Session>> {
        if !self.sessions_path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&self.sessions_path)?;
        serde_json::from_str(&text).map_err(|e| {
            logger::error(format!("会话文件解析失败: {}", e));
            AppError(format!("会话文件解析失败: {}", e))
        })
    }

    pub fn save_sessions(&self, sessions: &[Session]) -> Result<()> {
        let text = serde_json::to_string_pretty(sessions)?;
        atomic_write(&self.sessions_path, &text)
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.dir.join("skills")
    }

    pub fn load_auth(&self) -> Result<Option<AuthSession>> {
        if !self.auth_path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&self.auth_path)?;
        serde_json::from_str(&text).map(Some).map_err(|e| {
            logger::error(format!("会话文件解析失败: {}", e));
            AppError(format!("会话文件解析失败: {}", e))
        })
    }

    pub fn save_auth(&self, auth: &AuthSession) -> Result<()> {
        let text = serde_json::to_string_pretty(auth)?;
        atomic_write(&self.auth_path, &text)
    }

    pub fn clear_auth(&self) -> Result<()> {
        if self.auth_path.exists() {
            std::fs::remove_file(&self.auth_path)?;
        }
        Ok(())
    }
}

/// 原子写入：先写临时文件再 rename，避免崩溃导致文件损坏。
fn atomic_write(path: &PathBuf, content: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    match std::fs::rename(&tmp, path) {
        Ok(_) => Ok(()),
        Err(e) => {
            // Windows 上 rename 覆盖已有文件会失败，退回到直接写
            logger::warn(format!("原子 rename 失败，回退直接写入: {}", e));
            std::fs::write(path, content)?;
            Ok(())
        }
    }
}
