use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    /// 技能声明需要的工具权限，例如 ["read_file", "write_file"]
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub meta: SkillMeta,
    /// SKILL.md 正文（frontmatter 之后的部分），即给模型的操作指令
    pub instructions: String,
    /// 技能目录的绝对路径
    pub path: String,
    /// 目录名，作为唯一标识
    pub dir_name: String,
    /// 是否启用
    pub enabled: bool,
}

/// 解析 SKILL.md：支持 `---` 包裹的 YAML frontmatter + Markdown 正文。
pub fn parse_skill_md(content: &str) -> Result<(SkillMeta, String)> {
    let content = content.trim_start_matches('\u{feff}');
    let mut meta = SkillMeta::default();
    let body: String;

    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let yaml = &rest[..end];
            meta = serde_yaml::from_str(yaml)
                .map_err(|e| AppError(format!("技能 frontmatter 解析失败: {}", e)))?;
            body = rest[end + 4..].trim().to_string();
        } else {
            body = content.to_string();
        }
    } else {
        body = content.to_string();
    }

    if meta.name.is_empty() {
        return Err(AppError("技能缺少 name 字段".into()));
    }
    Ok((meta, body))
}

/// 从某个目录加载技能（读取其中的 SKILL.md）。
pub fn load_skill_from_dir(dir: &Path, enabled: bool) -> Result<Skill> {
    let skill_md = dir.join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md)?;
    let (meta, instructions) = parse_skill_md(&content)?;
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(Skill {
        meta,
        instructions,
        path: dir.to_string_lossy().to_string(),
        dir_name,
        enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let md = "---\nname: 测试技能\ndescription: 用于测试\nversion: 1.0.0\nallowed_tools: [read_file, write_file]\n---\n\n# 指令正文\n第一步。";
        let (meta, body) = parse_skill_md(md).unwrap();
        assert_eq!(meta.name, "测试技能");
        assert_eq!(meta.description, "用于测试");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.allowed_tools, vec!["read_file", "write_file"]);
        assert!(body.starts_with("# 指令正文"));
    }

    #[test]
    fn rejects_missing_name() {
        let md = "---\ndescription: 没有名字\n---\n正文";
        assert!(parse_skill_md(md).is_err());
    }
}
