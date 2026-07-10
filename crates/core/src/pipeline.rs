//! 构建流水线：生成 → dotnet build → godot 导出 pck → 部署。
//!
//! 所有步骤都接受一个行回调，CLI 直接打印，UI 转发成事件。

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::codegen;
use crate::config::ToolConfig;
use crate::model::Project;

pub type LogFn<'a> = &'a mut dyn FnMut(&str);

pub struct GenSummary {
    pub files: usize,
    pub copies: usize,
    pub warnings: Vec<String>,
    pub out_dir: PathBuf,
}

/// 生成源码树到 `<project>/build/godot`（整体重建，保证无陈旧文件）。
pub fn generate(project_dir: &Path, log: LogFn) -> Result<GenSummary> {
    let project = Project::load(project_dir)?;
    let out = codegen::generate(&project)?;
    let gen_dir = project_dir.join(crate::GEN_DIR);

    if gen_dir.exists() {
        std::fs::remove_dir_all(&gen_dir)
            .with_context(|| format!("清理 {} 失败", gen_dir.display()))?;
    }
    std::fs::create_dir_all(&gen_dir)?;

    for f in &out.files {
        let path = gen_dir.join(&f.rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &f.content)
            .with_context(|| format!("写入 {} 失败", path.display()))?;
        log(&format!("生成 {}", f.rel_path.display()));
    }

    let mut copied = 0;
    for c in &out.copies {
        let src = project_dir.join(&c.src_rel);
        let dst = gen_dir.join(&c.dst_rel);
        if !src.exists() {
            log(&format!("警告: 素材不存在，跳过复制: {}", c.src_rel));
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dst)
            .with_context(|| format!("复制素材 {} 失败", c.src_rel))?;
        log(&format!("素材 {} -> {}", c.src_rel, c.dst_rel.display()));
        copied += 1;
    }

    // 自定义 C# 逃生舱：src/**/*.cs 原样复制进 Scripts/Custom/
    let custom_dir = project_dir.join(crate::CUSTOM_SRC_DIR);
    if custom_dir.is_dir() {
        copied += copy_cs_tree(&custom_dir, &gen_dir.join("Scripts/Custom"), log)?;
    }

    for w in &out.warnings {
        log(&format!("警告: {w}"));
    }
    Ok(GenSummary {
        files: out.files.len(),
        copies: copied,
        warnings: out.warnings,
        out_dir: gen_dir,
    })
}

fn copy_cs_tree(src: &Path, dst: &Path, log: LogFn) -> Result<usize> {
    let mut n = 0;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            n += copy_cs_tree(&path, &dst.join(entry.file_name()), log)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("cs") {
            std::fs::create_dir_all(dst)?;
            std::fs::copy(&path, dst.join(entry.file_name()))?;
            log(&format!("自定义代码 {}", path.display()));
            n += 1;
        }
    }
    Ok(n)
}

/// dotnet build（csproj 的 CopyMod target 会把 dll+json 复制进 mods 目录）。
pub fn build(project_dir: &Path, cfg: &ToolConfig, log: LogFn) -> Result<()> {
    let gen_dir = project_dir.join(crate::GEN_DIR);
    if !gen_dir.join("project.godot").exists() {
        bail!("尚未生成源码，请先执行 generate");
    }
    let mut cmd = Command::new(cfg.dotnet_cmd());
    cmd.arg("build").arg("-nologo").current_dir(&gen_dir);
    if let Some(dir) = &cfg.sts2_dir {
        cmd.arg(format!("-p:Sts2Dir={dir}"));
    } else {
        log("警告: 未配置游戏目录(sts2Dir)，无法引用 sts2.dll，编译大概率失败。请先 sts2mod config set sts2Dir <路径>");
    }
    run_streamed(cmd, log).context("dotnet build 失败")
}

/// godot 无头导出 pck，直接写进 mods 目录（未配置游戏目录时写到 build/out/）。
pub fn pack(project_dir: &Path, cfg: &ToolConfig, log: LogFn) -> Result<PathBuf> {
    let project = Project::load(project_dir)?;
    let id = &project.manifest.id;
    let gen_dir = project_dir.join(crate::GEN_DIR);
    if !gen_dir.join("project.godot").exists() {
        bail!("尚未生成源码，请先执行 generate");
    }
    let godot = cfg
        .godot_exe
        .as_ref()
        .context("未配置 Godot 路径，请先 sts2mod config set godotExe <godot可执行文件>")?;

    let dest_dir = match cfg.mods_dir() {
        Some(mods) => mods.join(id),
        None => project_dir.join("build/out"),
    };
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(format!("{id}.pck"));

    let mut cmd = Command::new(godot);
    cmd.arg("--headless")
        .arg("--path")
        .arg(&gen_dir)
        .arg("--export-pack")
        .arg("Windows Desktop")
        .arg(&dest)
        // 与官方 mod 模板一致：阻止导出钩子递归触发构建
        .env("IsInnerGodotExport", "true")
        .env("MSBUILDDISABLENODEREUSE", "1");
    run_streamed(cmd, log).context("godot 导出 pck 失败")?;
    if !dest.exists() {
        bail!("godot 声称完成但未找到 {}", dest.display());
    }
    log(&format!("pck 已导出: {}", dest.display()));
    Ok(dest)
}

/// 一键：生成 + 编译 + 导出。成功后 mods 目录里就是完整可加载的 mod。
pub fn deploy(project_dir: &Path, cfg: &ToolConfig, log: LogFn) -> Result<()> {
    let summary = generate(project_dir, log)?;
    log(&format!("生成完成: {} 个文件, {} 个素材", summary.files, summary.copies));
    build(project_dir, cfg, log)?;
    pack(project_dir, cfg, log)?;
    if cfg.sts2_dir.is_some() {
        log("部署完成，启动游戏即可测试。注意 card 指令只能在战斗中使用：");
        let project = Project::load(project_dir)?;
        for card in &project.cards {
            log(&format!(
                "  战斗中按 ~ 输入: card {}",
                crate::ids::content_id(&project.manifest.id, "CARD", &card.class_name)
            ));
        }
    } else {
        log("未配置游戏目录，产物在 build/ 下，请手动复制到游戏 mods 文件夹");
    }
    Ok(())
}

pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

/// 环境自检：dotnet / godot / 游戏目录 / RitsuLib。
pub fn doctor(project_dir: Option<&Path>, cfg: &ToolConfig) -> Vec<Check> {
    let mut checks = Vec::new();

    let dotnet = cfg.dotnet_cmd();
    checks.push(match capture(&dotnet, &["--version"]) {
        Ok(v) => Check { name: ".NET SDK".into(), ok: true, detail: format!("{dotnet} {}", v.trim()) },
        Err(e) => Check { name: ".NET SDK".into(), ok: false, detail: format!("未找到 {dotnet}: {e}") },
    });

    checks.push(match &cfg.godot_exe {
        Some(exe) if Path::new(exe).exists() => match capture(exe, &["--version"]) {
            Ok(v) => {
                let v = v.trim().to_string();
                let ok = v.contains("mono");
                Check {
                    name: "Godot".into(),
                    ok,
                    detail: if ok { v } else { format!("{v}（警告: 需要 Mono/.NET 版本）") },
                }
            }
            Err(e) => Check { name: "Godot".into(), ok: false, detail: format!("无法运行: {e}") },
        },
        Some(exe) => Check { name: "Godot".into(), ok: false, detail: format!("路径不存在: {exe}") },
        None => Check { name: "Godot".into(), ok: false, detail: "未配置 godotExe".into() },
    });

    match &cfg.sts2_dir {
        Some(dir) => {
            let root = Path::new(dir);
            let data_dir = std::fs::read_dir(root).ok().and_then(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .find(|e| e.file_name().to_string_lossy().starts_with("data_sts2_"))
                    .map(|e| e.path())
            });
            match data_dir {
                Some(d) if d.join("sts2.dll").exists() => {
                    checks.push(Check {
                        name: "游戏目录".into(),
                        ok: true,
                        detail: format!("sts2.dll 位于 {}", d.display()),
                    });
                }
                _ => checks.push(Check {
                    name: "游戏目录".into(),
                    ok: false,
                    detail: format!("{dir} 下未找到 data_sts2_*/sts2.dll"),
                }),
            }
            let ritsu = root.join("mods").join("STS2-RitsuLib");
            checks.push(Check {
                name: "RitsuLib".into(),
                ok: ritsu.exists(),
                detail: if ritsu.exists() {
                    format!("{}", ritsu.display())
                } else {
                    "mods/STS2-RitsuLib 不存在（游戏内加载 mod 需要，编译不需要）".into()
                },
            });
        }
        None => checks.push(Check {
            name: "游戏目录".into(),
            ok: false,
            detail: "未配置 sts2Dir".into(),
        }),
    }

    if let Some(dir) = project_dir {
        checks.push(match Project::load(dir) {
            Ok(p) => Check {
                name: "项目文件".into(),
                ok: true,
                detail: format!("{} v{}（{} 张卡牌）", p.manifest.id, p.manifest.version, p.cards.len()),
            },
            Err(e) => Check { name: "项目文件".into(), ok: false, detail: format!("{e:#}") },
        });
    }
    checks
}

fn capture(cmd: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(cmd).args(args).output()?;
    if !out.status.success() {
        bail!("退出码 {:?}", out.status.code());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 运行子进程，stdout/stderr 逐行回调。
fn run_streamed(mut cmd: Command, log: LogFn) -> Result<()> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    log(&format!("$ {cmd:?}"));
    let mut child = cmd.spawn().with_context(|| format!("无法启动 {cmd:?}"))?;

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let mut handles = Vec::new();
    for reader in [
        child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        handles.push(std::thread::spawn(move || {
            for line in std::io::BufReader::new(reader).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        }));
    }
    drop(tx);
    for line in rx {
        log(&line);
    }
    for h in handles {
        let _ = h.join();
    }
    let status = child.wait()?;
    if !status.success() {
        bail!("命令失败，退出码 {:?}", status.code());
    }
    Ok(())
}
