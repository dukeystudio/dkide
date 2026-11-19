use crate::models::AppConfig;

pub const CONFIG_VERSION: u32 = 2;

/// 配置版本迁移：把旧版本配置补全为当前版本结构。
pub fn migrate(mut config: serde_json::Value) -> AppConfig {
    let version = config
        .get("version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;

    if version < 2 {
        // v1 -> v2：新增 theme / tool_policy / marketplace / usage_settings
        let obj = config.as_object_mut();
        if let Some(o) = obj {
            o.entry("theme").or_insert(serde_json::json!("light"));
            o.entry("tool_policy").or_insert(serde_json::json!("ask"));
            o.entry("marketplace_url").or_insert(serde_json::json!(""));
            o.entry("cost_per_million").or_insert(serde_json::json!(0.0));
            o.insert("version".into(), serde_json::json!(CONFIG_VERSION));
        }
    }

    serde_json::from_value(config).unwrap_or_default()
}
