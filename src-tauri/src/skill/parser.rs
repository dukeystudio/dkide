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

/// 加载技能目录下 references 中的参考文件（文件名, 全文）。
/// 工作流类技能（例如魔哈漫剧）的真正规则在这些文件里；全部塞进激活指令会
/// 超出模型上下文，因此这里返回文件清单与全文，由 read_skill_reference 按需取用。
const MAX_REFERENCE_FILE_BYTES: usize = 300 * 1024;

pub fn load_skill_reference_files(dir: &Path) -> Vec<(String, String)> {
    let refs_dir = dir.join("references");
    let mut files: Vec<_> = match std::fs::read_dir(&refs_dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter(|entry| {
                matches!(
                    entry.path().extension().and_then(|ext| ext.to_str()),
                    Some("txt") | Some("md")
                )
            })
            .collect(),
        Err(_) => return Vec::new(),
    };
    files.sort_by_key(|entry| entry.file_name());

    let mut result = Vec::new();
    for file in files {
        let name = file.file_name().to_string_lossy().to_string();
        let raw = match std::fs::read(file.path()) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let text = decode_utf8_or_gbk(&raw);
        let content = if text.len() > MAX_REFERENCE_FILE_BYTES {
            let mut cut = MAX_REFERENCE_FILE_BYTES.min(text.len());
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            format!("{}…（单个参考文件过长，已截断）", &text[..cut])
        } else {
            text
        };
        crate::logger::info(&format!(
            "[skill] 读取参考文件 {} bytes={}",
            name,
            content.len()
        ));
        result.push((name, content));
    }
    result
}

/// 生成参考文件清单，附在激活指令末尾，告诉模型如何按需读取全文。
fn format_reference_index(files: &[(String, String)]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let mut output = String::from(
        "\n\n--- 本技能参考文件（系统已就绪）---\n执行任一工位前，必须先用 read_skill_reference(skill_name, file_name) 读取对应参考文件全文并严格遵循；禁止再用 read_file / run_command 查找技能目录。\n",
    );
    for (name, content) in files {
        output.push_str(&format!(
            "- references/{}（{} 字符）\n",
            name,
            content.chars().count()
        ));
    }
    output
}

/// 优先按 UTF-8 解码；否则回退 GBK（Windows 中文文本常见编码）。
pub fn decode_utf8_or_gbk(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    let (cow, _, _) = encoding_rs::GBK.decode(bytes);
    cow.into_owned()
}

/// 从某个目录加载技能（读取其中的 SKILL.md 及其 references 参考文件）。
pub fn load_skill_from_dir(dir: &Path, enabled: bool) -> Result<Skill> {
    let skill_md = dir.join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md)?;
    let (meta, body) = parse_skill_md(&content)?;
    let references = load_skill_reference_files(dir);
    let instructions = format!("{}{}", body, format_reference_index(&references));
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
