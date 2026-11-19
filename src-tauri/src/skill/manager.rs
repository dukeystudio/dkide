use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};
use crate::skill::parser::load_skill_from_dir;
use crate::skill::Skill;

/// 技能管理器：负责扫描、安装、卸载、读取技能。
pub struct SkillManager {
    /// 用户级技能目录（跨项目）
    global_dir: PathBuf,
    /// 当前工作区技能目录（可选）
    workspace_dir: Option<PathBuf>,
}

impl SkillManager {
    pub fn new(global_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&global_dir);
        Self {
            global_dir,
            workspace_dir: None,
        }
    }

    pub fn set_workspace(&mut self, workspace: PathBuf) {
        let dir = workspace.join(".dkide").join("skills");
        let _ = std::fs::create_dir_all(&dir);
        self.workspace_dir = Some(dir);
    }

    /// 扫描所有技能目录，返回技能列表。
    pub fn list(&self, enabled_skills: &[String]) -> Result<Vec<Skill>> {
        let mut skills = Vec::new();
        for dir in self.scan_dirs() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let dir_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    match load_skill_from_dir(&path, true) {
                        Ok(mut skill) => {
                            skill.enabled = is_enabled(&skill, enabled_skills);
                            crate::logger::info(&format!(
                                "[skill] 扫描技能 name={} dir_name={} enabled={} instructions={}bytes",
                                skill.meta.name, skill.dir_name, skill.enabled, skill.instructions.len()
                            ));
                            skills.push(skill);
                        }
                        Err(e) => crate::logger::warn(&format!("[skill] 跳过技能 {}: {}", dir_name, e)),
                    }
                }
            }
        }
        Ok(skills)
    }

    /// 从本地目录安装技能：把目录复制到全局技能目录。
    pub fn install_from_dir(&self, src: &Path) -> Result<Skill> {
        if !src.is_dir() {
            return Err(AppError("源路径不是目录".into()));
        }
        let dir_name = src
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError("无法获取目录名".into()))?
            .to_string();
        let dest = self.global_dir.join(&dir_name);
        if dest.exists() {
            return Err(AppError(format!("技能 {} 已存在", dir_name)));
        }
        copy_dir_all(src, &dest)?;
        load_skill_from_dir(&dest, true)
    }

    /// 从 zip 文件安装技能：解压到全局技能目录。
    pub fn install_from_zip(&self, zip_path: &Path) -> Result<Vec<Skill>> {
        let file = std::fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError(format!("无法打开 zip: {}", e)))?;

        // 找出 zip 内包含 SKILL.md 的顶层目录（条目名按 UTF-8/GBK 修复后再用）
        let mut root: Option<String> = None;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| AppError(e.to_string()))?;
            let name = decode_zip_entry_name(entry.name_raw());
            if name.ends_with("SKILL.md") {
                let parts: Vec<&str> = name.split('/').collect();
                if parts.len() >= 2 {
                    root = Some(parts[0].to_string());
                }
            }
        }
        let root = root.ok_or_else(|| AppError("zip 中未找到 SKILL.md".into()))?;
        crate::logger::info(&format!("[skill] 安装 zip 顶层目录 name={}", root));
        let dest = self.global_dir.join(&root);
        if dest.exists() {
            return Err(AppError(format!("技能 {} 已存在", root)));
        }
        std::fs::create_dir_all(&dest)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| AppError(e.to_string()))?;
            let name = decode_zip_entry_name(entry.name_raw());
            if !name.starts_with(&format!("{}/", root)) {
                continue;
            }
            let relative = name[root.len() + 1..].to_string();
            if relative.is_empty() {
                continue;
            }
            // 防 Zip Slip：拒绝包含 ".." 的条目
            if relative.split('/').any(|part| part == "..") {
                return Err(AppError(format!("zip 包含非法路径: {}", name)));
            }
            let out_path = dest.join(&relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        self.list(&[])
    }

    /// 从 URL 下载 zip 并安装技能（异步由于要在 command 中 await，这里用 tokio 的 block_on 不合适；
    /// 因此下载逻辑放在 command 层，本方法只负责把已下载的字节写成临时 zip 再安装）。
    pub fn install_from_zip_bytes(&self, bytes: &[u8]) -> Result<Vec<Skill>> {
        let tmp = std::env::temp_dir().join(format!(
            "dkide-skill-{}.zip",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&tmp, bytes)?;
        let result = self.install_from_zip(&tmp);
        let _ = std::fs::remove_file(&tmp);
        result
    }

    /// 卸载技能（仅全局目录）。
    pub fn uninstall(&self, dir_name: &str) -> Result<()> {
        let path = self.global_dir.join(dir_name);
        if !path.exists() {
            return Err(AppError(format!("技能 {} 不存在", dir_name)));
        }
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    pub fn global_dir(&self) -> &Path {
        &self.global_dir
    }

    fn scan_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.global_dir.clone()];
        if let Some(ref w) = self.workspace_dir {
            dirs.push(w.clone());
        }
        dirs
    }
}

/// zip 条目名解码：先按 UTF-8，失败回退 GBK（Windows 中文 zip 常见）。
fn decode_zip_entry_name(raw: &[u8]) -> String {
    crate::skill::parser::decode_utf8_or_gbk(raw)
}

/// 启用判断：同时兼容技能名（meta.name）与目录名（dir_name），
/// 避免历史配置里保存的是其中任意一种时技能被误判为未启用。
fn is_enabled(skill: &Skill, enabled_skills: &[String]) -> bool {
    if enabled_skills.is_empty() {
        true
    } else {
        enabled_skills
            .iter()
            .any(|s| s == &skill.meta.name || s == &skill.dir_name)
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
