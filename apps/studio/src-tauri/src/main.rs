//! STS2 Mod Studio —— Tauri 后端。
//! 只是 sts2mod-core 的薄封装：加载/保存项目、跑流水线、读写配置。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use serde::Serialize;
use sts2mod_core::{config, model, pipeline, Project};
use tauri::Emitter;

type CmdResult<T> = Result<T, String>;

fn err_str<E: std::fmt::Display>(e: E) -> String {
    format!("{e}")
}

#[tauri::command]
fn load_project(dir: String) -> CmdResult<serde_json::Value> {
    let project = Project::load(Path::new(&dir)).map_err(|e| format!("{e:#}"))?;
    serde_json::to_value(&project).map_err(err_str)
}

#[tauri::command]
fn save_project(dir: String, project: serde_json::Value) -> CmdResult<()> {
    let project: Project = serde_json::from_value(project).map_err(err_str)?;
    project.save(Path::new(&dir)).map_err(|e| format!("{e:#}"))
}

/// 在 dir 下新建项目骨架，id 取目录名。
#[tauri::command]
fn new_project(dir: String) -> CmdResult<serde_json::Value> {
    let target = PathBuf::from(&dir);
    if target.join(sts2mod_core::PROJECT_FILE).exists() {
        return Err("该目录下已有项目文件".into());
    }
    let id = target
        .file_name()
        .ok_or("非法目录名")?
        .to_string_lossy()
        .to_string();
    let project = model::starter_project(&id, &id);
    std::fs::create_dir_all(target.join(sts2mod_core::ASSETS_DIR).join("cards")).map_err(err_str)?;
    std::fs::create_dir_all(target.join(sts2mod_core::CUSTOM_SRC_DIR)).map_err(err_str)?;
    project.save(&target).map_err(|e| format!("{e:#}"))?;
    std::fs::write(target.join(".gitignore"), "build/\nworkshop/content/\nsts2mod.local.json\n").map_err(err_str)?;
    serde_json::to_value(&project).map_err(err_str)
}

/// 执行流水线步骤，日志通过 "pipeline-log" 事件推给前端。
#[tauri::command]
async fn run_step(window: tauri::Window, dir: String, step: String) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let project_dir = PathBuf::from(&dir);
        let cfg = config::load_merged(Some(&project_dir)).map_err(|e| format!("{e:#}"))?;
        let mut log = |line: &str| {
            let _ = window.emit("pipeline-log", line);
        };
        let result = match step.as_str() {
            "generate" => pipeline::generate(&project_dir, &mut log).map(|s| {
                log(&format!("生成完成: {} 个文件, {} 个素材", s.files, s.copies));
            }),
            "build" => pipeline::generate(&project_dir, &mut log)
                .and_then(|_| pipeline::build(&project_dir, &cfg, &mut log)),
            "pack" => pipeline::pack(&project_dir, &cfg, &mut log).map(|_| ()),
            "deploy" => pipeline::deploy(&project_dir, &cfg, &mut log),
            "publish" => pipeline::publish(&project_dir, &cfg, false, &mut log),
            other => Err(anyhow::anyhow!("未知步骤: {other}")),
        };
        result.map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(err_str)?
}

#[derive(Serialize)]
struct CheckDto {
    name: String,
    ok: bool,
    detail: String,
}

#[tauri::command]
fn doctor(dir: Option<String>) -> CmdResult<Vec<CheckDto>> {
    let project_dir = dir.map(PathBuf::from);
    let cfg = config::load_merged(project_dir.as_deref()).map_err(|e| format!("{e:#}"))?;
    Ok(pipeline::doctor(project_dir.as_deref(), &cfg)
        .into_iter()
        .map(|c| CheckDto { name: c.name, ok: c.ok, detail: c.detail })
        .collect())
}

/// 把用户选择的图片复制进项目 assets/{category}/{ClassName}.{ext}，返回相对路径。
#[tauri::command]
fn import_asset(dir: String, category: String, class_name: String, src: String) -> CmdResult<String> {
    const CATEGORIES: &[&str] = &["cards", "relics", "powers", "potions", "monsters", "events", "characters", "workshop"];
    if !CATEGORIES.contains(&category.as_str()) {
        return Err(format!("未知素材类别: {category}"));
    }
    if class_name.is_empty() || !class_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!("非法类名: {class_name}"));
    }
    let src_path = Path::new(&src);
    let ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let rel = format!("assets/{category}/{class_name}.{ext}");
    let dst = Path::new(&dir).join(&rel);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(err_str)?;
    }
    std::fs::copy(src_path, &dst).map_err(|e| format!("复制图片失败: {e}"))?;
    Ok(rel)
}

/// 导入已有 mod（部署产物目录）为 out_dir 下的新项目，日志走 pipeline-log 事件。
#[tauri::command]
async fn import_mod(window: tauri::Window, mod_dir: String, out_dir: String) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut log = |line: &str| {
            let _ = window.emit("pipeline-log", line);
        };
        let summary = sts2mod_core::import::import_mod(
            Path::new(&mod_dir),
            Path::new(&out_dir),
            &mut log,
        )
        .map_err(|e| format!("{e:#}"))?;
        Ok(summary.to_string())
    })
    .await
    .map_err(err_str)?
}

/// 读取项目内图片为 data URL（预览用）。仅允许项目目录内的相对路径。
#[tauri::command]
fn read_image(dir: String, rel: String) -> CmdResult<String> {
    use base64::Engine as _;
    if rel.contains("..") || Path::new(&rel).is_absolute() {
        return Err(format!("非法路径: {rel}"));
    }
    let path = Path::new(&dir).join(&rel);
    let bytes = std::fs::read(&path).map_err(|e| format!("读取 {rel} 失败: {e}"))?;
    let mime = match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "image/png",
    };
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command]
fn get_config() -> CmdResult<config::ToolConfig> {
    config::load_global().map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn set_config(cfg: config::ToolConfig) -> CmdResult<()> {
    config::save_global(&cfg).map_err(|e| format!("{e:#}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_project,
            save_project,
            new_project,
            run_step,
            doctor,
            import_asset,
            import_mod,
            read_image,
            get_config,
            set_config
        ])
        .run(tauri::generate_context!())
        .expect("启动 STS2 Mod Studio 失败");
}
