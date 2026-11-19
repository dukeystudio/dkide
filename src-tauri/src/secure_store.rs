use std::path::Path;

use crate::error::{AppError, Result};
use crate::logger;

const SERVICE: &str = "com.dkide.apikey";

/// API Key 安全存储：优先使用系统钥匙串，失败时回退到本地混淆文件。
pub struct SecureStore {
    fallback_path: std::path::PathBuf,
}

impl SecureStore {
    pub fn new(dir: &Path) -> Self {
        Self { fallback_path: dir.join("keys.dat") }
    }

    pub fn get(&self, id: &str) -> Option<String> {
        self.get_named(SERVICE, id)
    }

    /// 用自定义 service/user 读取（如账号 token），带本地回退。
    pub fn get_named(&self, service: &str, user: &str) -> Option<String> {
        if let Ok(entry) = keyring::Entry::new(service, user) {
            if let Ok(key) = entry.get_password() {
                return Some(key);
            }
        }
        self.read_fallback().ok()?.remove(&format!("{}:{}", service, user))
    }

    /// 用自定义 service/user 写入（如账号 token）。
    pub fn set_named(&self, service: &str, user: &str, key: &str) -> Result<()> {
        match keyring::Entry::new(service, user) {
            Ok(entry) => {
                if entry.set_password(key).is_ok() {
                    self.remove_from_fallback(&format!("{}:{}", service, user)).ok();
                    return Ok(());
                }
                logger::warn(format!("系统钥匙串写入失败，使用本地加密回退: {}:{}", service, user));
            }
            Err(e) => logger::warn(format!("钥匙串不可用: {}", e)),
        }
        let mut map = self.read_fallback().unwrap_or_default();
        map.insert(format!("{}:{}", service, user), key.to_string());
        self.write_fallback(&map)
    }

    /// 用自定义 service/user 删除。
    pub fn remove_named(&self, service: &str, user: &str) -> Result<()> {
        if let Ok(entry) = keyring::Entry::new(service, user) {
            let _ = entry.delete_credential();
        }
        self.remove_from_fallback(&format!("{}:{}", service, user))
    }

    pub fn set(&self, id: &str, key: &str) -> Result<()> {
        if key.is_empty() {
            self.remove(id)?;
            return Ok(());
        }
        match keyring::Entry::new(SERVICE, id) {
            Ok(entry) => {
                if entry.set_password(key).is_ok() {
                    self.remove_from_fallback(id).ok();
                    return Ok(());
                }
                logger::warn(format!("系统钥匙串写入失败，使用本地加密回退: {}", id));
            }
            Err(e) => logger::warn(format!("钥匙串不可用: {}", e)),
        }
        let mut map = self.read_fallback().unwrap_or_default();
        map.insert(id.to_string(), key.to_string());
        self.write_fallback(&map)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        if let Ok(entry) = keyring::Entry::new(SERVICE, id) {
            let _ = entry.delete_credential();
        }
        self.remove_from_fallback(id)
    }

    fn read_fallback(&self) -> Result<std::collections::HashMap<String, String>> {
        if !self.fallback_path.exists() { return Ok(Default::default()); }
        let raw = std::fs::read_to_string(&self.fallback_path)?;
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw.as_bytes())
            .map_err(|e| AppError(format!("密钥文件解码失败: {}", e)))?;
        let plain_bytes: Vec<u8> = decoded.into_iter().map(|b| b ^ 0xA5).collect();
        let plain = String::from_utf8(plain_bytes)
            .map_err(|e| AppError(format!("密钥文件解码失败: {}", e)))?;
        serde_json::from_str(&plain).map_err(|e| AppError(e.to_string()))
    }

    fn write_fallback(&self, map: &std::collections::HashMap<String, String>) -> Result<()> {
        let plain = serde_json::to_string(map)?;
        let encoded: Vec<u8> = plain.bytes().map(|b| b ^ 0xA5).collect();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &encoded);
        std::fs::write(&self.fallback_path, b64)?;
        Ok(())
    }

    fn remove_from_fallback(&self, id: &str) -> Result<()> {
        let mut map = self.read_fallback().unwrap_or_default();
        if map.remove(id).is_some() {
            self.write_fallback(&map)?;
        }
        Ok(())
    }
}
