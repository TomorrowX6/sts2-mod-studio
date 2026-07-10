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
    Deploy,
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
                "build/\nsts2mod.local.json\n",
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
        Cmd::Deploy => {
            let cfg = config::load_merged(Some(&cli.project))?;
            pipeline::deploy(&cli.project, &cfg, &mut log)
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
        other => bail!("未知配置项: {other}（可用: sts2Dir / godotExe / dotnet / pckArch）"),
    }
    Ok(())
}
