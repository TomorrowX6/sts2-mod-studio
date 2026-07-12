//! 机器级 / 项目级工具链配置。
//!
//! 路径这类每台机器不同的东西不进 `project.stsmod.json`：
//! - 全局配置：`~/.config/sts2mod/config.json`（Windows 为 %APPDATA%/sts2mod）
//! - 项目覆盖：项目目录下 `sts2mod.local.json`（应加入 .gitignore）

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolConfig {
    /// 《杀戮尖塔2》根目录（含 data_sts2_* 与 mods/）。
    pub sts2_dir: Option<String>,
    /// Godot 4.5.1 Mono 可执行文件路径。
    pub godot_exe: Option<String>,
    /// dotnet 可执行文件，默认 PATH 上的 "dotnet"。
    pub dotnet: Option<String>,
    /// pck 导出架构：x86_64（默认）或 msil（兼容 mac）。
    pub pck_arch: Option<String>,
    /// 官方 ModUploader 可执行文件路径（megacrit/sts2-mod-uploader）。
    pub mod_uploader_exe: Option<String>,
}

pub const LOCAL_CONFIG_FILE: &str = "sts2mod.local.json";

pub fn global_config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("无法确定用户配置目录")?;
    Ok(dir.join("sts2mod").join("config.json"))
}

pub fn load_global() -> Result<ToolConfig> {
    let path = global_config_path()?;
    load_file(&path)
}

pub fn save_global(cfg: &ToolConfig) -> Result<()> {
    let path = global_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(cfg)? + "\n")
        .with_context(|| format!("写入 {} 失败", path.display()))?;
    Ok(())
}

fn load_file(path: &Path) -> Result<ToolConfig> {
    if !path.exists() {
        return Ok(ToolConfig::default());
    }
    let raw = std::fs::read_to_string(path).with_context(|| format!("读取 {} 失败", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("解析 {} 失败", path.display()))
}

/// 全局配置 + 项目本地覆盖合并。
pub fn load_merged(project_dir: Option<&Path>) -> Result<ToolConfig> {
    let mut cfg = load_global()?;
    if let Some(dir) = project_dir {
        let local = load_file(&dir.join(LOCAL_CONFIG_FILE))?;
        if local.sts2_dir.is_some() {
            cfg.sts2_dir = local.sts2_dir;
        }
        if local.godot_exe.is_some() {
            cfg.godot_exe = local.godot_exe;
        }
        if local.dotnet.is_some() {
            cfg.dotnet = local.dotnet;
        }
        if local.pck_arch.is_some() {
            cfg.pck_arch = local.pck_arch;
        }
        if local.mod_uploader_exe.is_some() {
            cfg.mod_uploader_exe = local.mod_uploader_exe;
        }
    }
    Ok(cfg)
}

impl ToolConfig {
    pub fn dotnet_cmd(&self) -> String {
        self.dotnet.clone().unwrap_or_else(|| "dotnet".into())
    }

    pub fn mods_dir(&self) -> Option<PathBuf> {
        self.sts2_dir.as_ref().map(|d| Path::new(d).join("mods"))
    }
}
