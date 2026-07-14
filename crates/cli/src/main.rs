//! sts2mod —— 《杀戮尖塔2》mod 制作流水线命令行。
//!
//! 典型流程：
//! ```text
//! sts2mod config set sts2Dir "D:/Steam/steamapps/common/Slay the Spire 2"
//! sts2mod config set godotExe "D:/godot/Godot_v4.5.1-stable_mono_win64.exe"
//! sts2mod new MyMod
//! cd MyMod && sts2mod deploy
//! ```

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use sts2mod_core::{config, model, pipeline};

#[derive(Parser)]
#[command(name = "sts2mod", about = "杀戮尖塔2 mod 制作流水线", version)]
struct Cli {
    /// 项目目录（默认当前目录）
    #[arg(short = 'C', long = "project", global = true, default_value = ".")]
    project: PathBuf,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 新建一个 mod 项目（含一张示例卡牌）
    New {
        /// 项目目录名，同时作为 mod id
        dir: String,
        /// mod 显示名称（默认同 id）
        #[arg(long)]
        name: Option<String>,
    },
    /// 从 project.stsmod.json 生成 C#/Godot 源码到 build/godot
    Generate,
    /// 生成并编译 dll（需要已配置游戏目录）
    Build,
    /// 导出 pck（需要已配置 Godot 路径；需先 generate）
    Pack,
    /// 一键：生成 + 编译 + 导出，直接部署进游戏 mods 目录
    Deploy {
        /// 实时模式：构建内置 Live 运行时并写入 live.json，
        /// 之后配合 `sts2mod live` 或 Studio 实时会话，数值/文本改动无需重启游戏
        #[arg(long)]
        live: bool,
    },
    /// 实时会话：监视 project.stsmod.json，改动即推送进游戏（数值/文本即时生效）。
    /// 首次运行（或结构已变化）会自动先做一次实时部署
    Live {
        /// 检测到结构性改动时自动重新完整部署（仍需重启游戏加载新 dll/pck）
        #[arg(long)]
        auto_deploy: bool,
    },
    /// 发布到创意工坊：deploy + 组装 workshop/ 工作区 + 调用官方 ModUploader
    Publish {
        /// 跳过构建，直接用游戏 mods 目录里的现有产物
        #[arg(long)]
        skip_deploy: bool,
    },
    /// 导入已有 mod（部署目录，含 <id>.json 与可选 pck）为新项目骨架
    Import {
        /// 已有 mod 的目录，如 <游戏>/mods/SomeMod 或工坊订阅目录
        mod_dir: PathBuf,
        /// 生成的新项目目录
        out_dir: PathBuf,
    },
    /// 检查工具链与项目状态
    Doctor,
    /// 查看/修改全局配置（sts2Dir / godotExe / dotnet / pckArch）
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// 列出当前配置
    List,
    /// 设置一项，例如 sts2mod config set sts2Dir "C:/.../Slay the Spire 2"
    Set { key: String, value: String },
    /// 清除一项
    Unset { key: String },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("错误: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let mut log = |line: &str| println!("{line}");
    match cli.command {
        Cmd::New { dir, name } => {
            let target = PathBuf::from(&dir);
            if target.join(sts2mod_core::PROJECT_FILE).exists() {
                bail!("{dir} 下已有项目文件");
            }
            let id = target
                .file_name()
                .context("非法目录名")?
                .to_string_lossy()
                .to_string();
            let project = model::starter_project(&id, name.as_deref().unwrap_or(&id));
            std::fs::create_dir_all(target.join(sts2mod_core::ASSETS_DIR).join("cards"))?;
            std::fs::create_dir_all(target.join(sts2mod_core::CUSTOM_SRC_DIR))?;
            project.save(&target)?;
            std::fs::write(
                target.join(".gitignore"),
                "build/\nworkshop/content/\nsts2mod.local.json\n",
            )?;
            println!("已创建项目 {dir}/");
            println!("  - 编辑 {} 定义内容", sts2mod_core::PROJECT_FILE);
            println!("  - 卡图放入 assets/cards/，自定义 C# 放入 src/");
            println!("  - cd {dir} && sts2mod deploy 一键构建部署");
            Ok(())
        }
        Cmd::Generate => {
            let s = pipeline::generate(&cli.project, &mut log)?;
            println!("生成完成: {} 个文件, {} 个素材 -> {}", s.files, s.copies, s.out_dir.display());
            Ok(())
        }
        Cmd::Build => {
            let cfg = config::load_merged(Some(&cli.project))?;
            pipeline::generate(&cli.project, &mut log)?;
            pipeline::build(&cli.project, &cfg, &mut log)
        }
        Cmd::Pack => {
            let cfg = config::load_merged(Some(&cli.project))?;
            pipeline::pack(&cli.project, &cfg, &mut log)?;
            Ok(())
        }
        Cmd::Deploy { live } => {
            let cfg = config::load_merged(Some(&cli.project))?;
            if live {
                pipeline::deploy_live(&cli.project, &cfg, &mut log)
            } else {
                pipeline::deploy(&cli.project, &cfg, &mut log)
            }
        }
        Cmd::Live { auto_deploy } => run_live(&cli.project, auto_deploy, &mut log),
        Cmd::Publish { skip_deploy } => {
            let cfg = config::load_merged(Some(&cli.project))?;
            pipeline::publish(&cli.project, &cfg, skip_deploy, &mut log)
        }
        Cmd::Import { mod_dir, out_dir } => {
            let summary = sts2mod_core::import::import_mod(&mod_dir, &out_dir, &mut log)?;
            println!("{summary}");
            Ok(())
        }
        Cmd::Doctor => {
            let cfg = config::load_merged(Some(&cli.project))?;
            let project_dir = cli.project.join(sts2mod_core::PROJECT_FILE).exists().then_some(cli.project.as_path());
            let mut all_ok = true;
            for c in pipeline::doctor(project_dir, &cfg) {
                println!("{} {}: {}", if c.ok { "✔" } else { "✘" }, c.name, c.detail);
                all_ok &= c.ok;
            }
            if !all_ok {
                std::process::exit(2);
            }
            Ok(())
        }
        Cmd::Config { action } => {
            let mut cfg = config::load_global()?;
            match action {
                ConfigCmd::List => {
                    println!("{}", serde_json::to_string_pretty(&cfg)?);
                    println!("(配置文件: {})", config::global_config_path()?.display());
                }
                ConfigCmd::Set { key, value } => {
                    set_key(&mut cfg, &key, Some(value))?;
                    config::save_global(&cfg)?;
                    println!("已保存");
                }
                ConfigCmd::Unset { key } => {
                    set_key(&mut cfg, &key, None)?;
                    config::save_global(&cfg)?;
                    println!("已清除");
                }
            }
            Ok(())
        }
    }
}

/// 实时会话：先保证有匹配的实时部署，然后监视项目文件，改动即推送。
fn run_live(project_dir: &std::path::Path, auto_deploy: bool, log: &mut dyn FnMut(&str)) -> Result<()> {
    let cfg = config::load_merged(Some(project_dir))?;
    let project_file = project_dir.join(sts2mod_core::PROJECT_FILE);
    if !project_file.exists() {
        bail!("{} 不存在（请在项目目录运行，或用 -C 指定）", project_file.display());
    }

    // 首次：推送一次；没有实时部署或结构已变化就先完整实时部署
    let needs_initial_deploy = match model::Project::load(project_dir) {
        Ok(p) => match pipeline::push_live(&p, &cfg) {
            Ok(push) => push.needs_deploy,
            Err(_) => true,
        },
        Err(e) => return Err(e),
    };
    if needs_initial_deploy {
        println!("首次实时部署（构建内置 Live 运行时）…");
        pipeline::deploy_live(project_dir, &cfg, log)?;
        println!("请（重新）启动游戏。之后保持本命令运行，改动会自动推送。");
    }

    println!("实时会话中：监视 {}（Ctrl+C 退出）", project_file.display());
    println!("  数值/文本改动 → 保存后即时生效（游戏内换个界面或重新悬浮即可看到）");
    println!("  结构性改动   → {}", if auto_deploy {
        "自动重新完整部署（完成后需重启游戏）"
    } else {
        "提示需要重新部署（可加 --auto-deploy 自动执行）"
    });

    let mtime_of = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut last = mtime_of(&project_file);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let now = mtime_of(&project_file);
        if now == last {
            continue;
        }
        last = now;
        let project = match model::Project::load(project_dir) {
            Ok(p) => p,
            Err(e) => {
                println!("⚠ 项目文件暂不可用（{e:#}），等待下次保存…");
                continue;
            }
        };
        match pipeline::push_live(&project, &cfg) {
            Ok(push) if push.needs_deploy => {
                if auto_deploy {
                    println!("检测到结构性改动，自动重新部署…");
                    if let Err(e) = pipeline::deploy_live(project_dir, &cfg, log) {
                        println!("⚠ 重新部署失败: {e:#}");
                    } else {
                        println!("重新部署完成——请重启游戏加载新构建。");
                    }
                } else {
                    println!("⚠ 检测到结构性改动（增删内容/效果/触发器等）：实时推送对其不生效，请重新执行 sts2mod deploy --live 并重启游戏");
                }
            }
            Ok(push) => {
                println!("已推送 {}（文本 {} 项 / 数值 {} 项）", chrono_now(), push.texts, push.nums);
            }
            Err(e) => println!("⚠ 推送失败: {e:#}"),
        }
    }
}

/// 本地时间 HH:MM:SS（不引额外依赖，够用即可）。
fn chrono_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 简化处理：UTC+8（本工具主要面向中文社区；仅用于日志显示）
    let secs = (now + 8 * 3600) % 86400;
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn set_key(cfg: &mut config::ToolConfig, key: &str, value: Option<String>) -> Result<()> {
    match key {
        "sts2Dir" => cfg.sts2_dir = value,
        "godotExe" => cfg.godot_exe = value,
        "dotnet" => cfg.dotnet = value,
        "pckArch" => {
            if let Some(v) = &value {
                if v != "x86_64" && v != "msil" {
                    bail!("pckArch 只能是 x86_64 或 msil");
                }
            }
            cfg.pck_arch = value;
        }
        "modUploaderExe" => cfg.mod_uploader_exe = value,
        other => bail!("未知配置项: {other}（可用: sts2Dir / godotExe / dotnet / pckArch / modUploaderExe）"),
    }
    Ok(())
}
