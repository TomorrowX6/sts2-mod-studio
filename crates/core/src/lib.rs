//! sts2mod-core：《杀戮尖塔2》mod 图形化制作工具的核心库。
//!
//! 职责：
//! - 定义工具自己的项目文件格式（`project.stsmod.json`）
//! - 从项目文件生成完整的 C# + Godot mod 源码树（基于 RitsuLib）
//! - 驱动构建流水线：dotnet build → godot 导出 pck → 部署到游戏 mods 目录

pub mod codegen;
pub mod config;
pub mod ids;
pub mod model;
pub mod pipeline;

pub use model::Project;

/// 工具项目文件名（位于项目目录根部）。
pub const PROJECT_FILE: &str = "project.stsmod.json";
/// 生成的 Godot/C# 工程所在的子目录（每次生成时整体重建）。
pub const GEN_DIR: &str = "build/godot";
/// 用户自定义 C# 逃生舱目录：其中的 .cs 会被原样复制进生成工程。
pub const CUSTOM_SRC_DIR: &str = "src";
/// 项目内素材目录（卡图等，按项目文件中的相对路径引用）。
pub const ASSETS_DIR: &str = "assets";
