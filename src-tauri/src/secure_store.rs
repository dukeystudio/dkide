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
        // 不能只凭 set_password 返回 Ok 就认定持久化成功：
        // keyring 在没有对应原生后端时（如未开启 apple-native 的 macOS）会降级为
        // 纯内存 mock，set_password 永远 Ok，但进程重启后数据全丢。
        // 因此写入后用一个【全新的 Entry】读回验证——mock 每次 Entry::new 都是空的，
        // 读不回来；真正的系统钥匙串则可以。只有确认持久化后才删除本地回退条目。
        let mut persisted = false;
        match keyring::Entry::new(service, user) {
            Ok(entry) => {
                if entry.set_password(key).is_ok() {
                    persisted = keyring::Entry::new(service, user)
                        .and_then(|e| e.get_password())
                        .map(|v| v == key)
                        .unwrap_or(false);
                    if !persisted {
                        logger::warn(format!(
                            "系统钥匙串未真正持久化（可能为内存后端），改用本地加密回退: {}:{}",
                            service, user
                        ));
                    }
                } else {
                    logger::warn(format!("系统钥匙串写入失败，使用本地加密回退: {}:{}", service, user));
                }
            }
            Err(e) => logger::warn(format!("钥匙串不可用: {}", e)),
        }

        let fallback_key = format!("{}:{}", service, user);
        if persisted {
            // 确认系统钥匙串可读，清掉可能残留的旧回退条目
            self.remove_from_fallback(&fallback_key).ok();
            return Ok(());
        }
        // 后端不可靠：写入本地回退文件（XOR 混淆），保证重启后仍可读
        let mut map = self.read_fallback().unwrap_or_default();
        map.insert(fallback_key, key.to_string());
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
        self.set_named(SERVICE, id, key)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        self.remove_named(SERVICE, id)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> (SecureStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("dkide-secure-{}-{}", tag, std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        (SecureStore::new(&dir), dir)
    }

    #[test]
    fn fallback_roundtrip_and_remove() {
        let (store, dir) = temp_store("roundtrip");
        let path = dir.join("keys.dat");
        let _ = std::fs::remove_file(&path);

        let mut map = std::collections::HashMap::new();
        map.insert("svc:user".to_string(), "secret-value".to_string());
        store.write_fallback(&map).unwrap();

        let loaded = store.read_fallback().unwrap();
        assert_eq!(loaded.get("svc:user").map(String::as_str), Some("secret-value"));

        store.remove_from_fallback("svc:user").unwrap();
        let loaded = store.read_fallback().unwrap();
        assert_eq!(loaded.get("svc:user"), None);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn fallback_file_is_obfuscated() {
        let (store, dir) = temp_store("obfuscate");
        let path = dir.join("keys.dat");
        let _ = std::fs::remove_file(&path);

        let mut map = std::collections::HashMap::new();
        map.insert("svc:user".to_string(), "plaintext-secret".to_string());
        store.write_fallback(&map).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("plaintext-secret"), "回退文件不应含明文密钥");
        assert!(!raw.contains("svc:user"), "回退文件不应含明文键名");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
