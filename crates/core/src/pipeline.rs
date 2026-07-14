//! 构建流水线：生成 → dotnet build → godot 导出 pck → 部署。
//!
//! 所有步骤都接受一个行回调，CLI 直接打印，UI 转发成事件。

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::codegen;
use crate::config::ToolConfig;
use crate::live;
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
    generate_with(project_dir, false, log)
}

/// `live = true` 时生成实时模式源码（注入 Live 运行时，数值走 live.json）。
pub fn generate_with(project_dir: &Path, live_mode: bool, log: LogFn) -> Result<GenSummary> {
    let project = Project::load(project_dir)?;
    let out = codegen::generate_with(&project, &codegen::GenOptions { live: live_mode })?;
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
    // Godot 的内部 dotnet publish 也需要游戏程序集引用路径。
    if let Some(dir) = &cfg.sts2_dir {
        cmd.env("Sts2Dir", dir);
    }

    // Godot 某些导出错误仍会返回退出码 0，不能只依赖进程状态。
    let mut saw_error = false;
    {
        let mut export_log = |line: &str| {
            saw_error |= line.starts_with("ERROR:");
            log(line);
        };
        run_streamed(cmd, &mut export_log).context("godot 导出 pck 失败")?;
    }
    if saw_error {
        bail!("godot 导出日志包含错误，pck 不可用");
    }
    if !dest.exists() {
        bail!("godot 声称完成但未找到 {}", dest.display());
    }
    log(&format!("pck 已导出: {}", dest.display()));
    Ok(dest)
}

/// 一键：生成 + 编译 + 导出。成功后 mods 目录里就是完整可加载的 mod。
pub fn deploy(project_dir: &Path, cfg: &ToolConfig, log: LogFn) -> Result<()> {
    deploy_impl(project_dir, cfg, false, log)
}

/// 实时部署：deploy 的实时模式版本（生成含 Live 运行时的构建，并写入 live.json 基线）。
/// 之后保持游戏运行，用 push_live 推送数值/文本改动即可即时生效。
pub fn deploy_live(project_dir: &Path, cfg: &ToolConfig, log: LogFn) -> Result<()> {
    deploy_impl(project_dir, cfg, true, log)
}

fn deploy_impl(project_dir: &Path, cfg: &ToolConfig, live_mode: bool, log: LogFn) -> Result<()> {
    let summary = generate_with(project_dir, live_mode, log)?;
    log(&format!("生成完成: {} 个文件, {} 个素材", summary.files, summary.copies));
    build(project_dir, cfg, log)?;
    pack(project_dir, cfg, log)?;
    // live.json 与构建模式保持一致：实时构建写入基线，正式构建清掉残留
    if let Some(mods) = cfg.mods_dir() {
        let project = Project::load(project_dir)?;
        let live_path = mods.join(&project.manifest.id).join(live::LIVE_FILE);
        if live_mode {
            let fp = live::live_fingerprint(&project);
            let data = live::live_data(&project, &fp)?;
            std::fs::write(&live_path, &data.json)
                .with_context(|| format!("写入 {} 失败", live_path.display()))?;
            log(&format!(
                "实时数据已写入: {}（文本 {} 项 / 数值 {} 项）",
                live_path.display(),
                data.texts,
                data.nums
            ));
            log("实时会话就绪：保持游戏运行，推送的数值/文本改动即时生效；");
            log("  结构性改动（增删内容、改效果积木/触发器/图片等）仍需重新执行实时部署并重启游戏。");
        } else if live_path.exists() {
            let _ = std::fs::remove_file(&live_path);
            log("已清除旧的实时数据 live.json（本次为正式构建）");
        }
    }
    if cfg.sts2_dir.is_some() {
        log("部署完成，启动游戏即可测试。获取类指令只能在战斗中使用：");
        let project = Project::load(project_dir)?;
        let id = &project.manifest.id;
        for card in &project.cards {
            log(&format!(
                "  卡牌: card {}",
                crate::ids::content_id(id, "CARD", &card.class_name)
            ));
        }
        for power in &project.powers {
            log(&format!(
                "  能力: power {} 1 0",
                crate::ids::content_id(id, "POWER", &power.class_name)
            ));
        }
        for relic in &project.relics {
            log(&format!("  遗物 ID: {}", crate::ids::content_id(id, "RELIC", &relic.class_name)));
        }
        for potion in &project.potions {
            log(&format!("  药水 ID: {}", crate::ids::content_id(id, "POTION", &potion.class_name)));
        }
        for enc in &project.encounters {
            log(&format!(
                "  遭遇（地图任意节点可用）: fight {}",
                crate::ids::content_id(id, "ENCOUNTER", &enc.class_name)
            ));
        }
        for ev in &project.events {
            log(&format!(
                "  事件（地图任意节点可用）: event {}",
                crate::ids::content_id(id, "EVENT", &ev.class_name)
            ));
        }
        for ch in &project.characters {
            log(&format!(
                "  人物: 选人界面直接选择（ID: {}）",
                crate::ids::content_id(id, "CHARACTER", &ch.class_name)
            ));
        }
    } else {
        log("未配置游戏目录，产物在 build/ 下，请手动复制到游戏 mods 文件夹");
    }
    Ok(())
}

/// push_live 的结果。
pub struct LivePush {
    /// 项目结构相对上次实时部署已变化：推送的 live.json 对结构性改动不生效，
    /// 需要重新实时部署（并重启游戏）。
    pub needs_deploy: bool,
    /// 本次推送的本地化词条数 / 数值条数。
    pub texts: usize,
    pub nums: usize,
}

/// 推送实时数据：只重写 `mods/<id>/live.json`，不编译不导出（毫秒级）。
/// 游戏里的 Live 运行时监视该文件，数值/文本改动即时生效。
/// 保留上次实时部署写入的结构指纹作为基线，用于判断是否需要重新部署。
pub fn push_live(project: &Project, cfg: &ToolConfig) -> Result<LivePush> {
    project.validate()?;
    let id = &project.manifest.id;
    let mods = cfg.mods_dir().context("未配置游戏目录(sts2Dir)，实时推送需要部署目录")?;
    let mod_dir = mods.join(id);
    if !mod_dir.join(format!("{id}.dll")).exists() {
        bail!("游戏 mods 目录中没有 {id}.dll——请先执行一次实时部署（deploy --live）");
    }
    let live_path = mod_dir.join(live::LIVE_FILE);
    // 基线指纹 = 上次实时部署时的项目结构；文件缺失（如上次是正式构建）视为需要部署
    let baseline = std::fs::read_to_string(&live_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("fingerprint").and_then(|f| f.as_str()).map(String::from))
        .unwrap_or_default();
    let current = live::live_fingerprint(project);
    let data = live::live_data(project, &baseline)?;
    std::fs::write(&live_path, &data.json)
        .with_context(|| format!("写入 {} 失败", live_path.display()))?;
    Ok(LivePush {
        needs_deploy: baseline != current,
        texts: data.texts,
        nums: data.nums,
    })
}

/// 项目内的工坊工作区目录名（结构与官方 ModUploader 的 workspace 一致，
/// mod_id.txt 由上传器在首次发布后写入，应随项目提交）。
pub const WORKSHOP_DIR: &str = "workshop";

/// Steam 对预览图的硬性限制。
const PREVIEW_IMAGE_MAX: u64 = 1024 * 1024;

/// 组装 `<project>/workshop/` 工作区：content/ 三件套 + image.png + workshop.json。
/// artifacts_dir 是已部署的 `<游戏>/mods/<id>/` 目录。返回工作区路径。
pub fn assemble_workshop_workspace(
    project: &Project,
    project_dir: &Path,
    artifacts_dir: &Path,
    log: LogFn,
) -> Result<PathBuf> {
    let id = &project.manifest.id;
    let ws = project_dir.join(WORKSHOP_DIR);
    let content = ws.join("content");
    if content.exists() {
        std::fs::remove_dir_all(&content).with_context(|| format!("清理 {} 失败", content.display()))?;
    }
    std::fs::create_dir_all(&content)?;

    // mod 三件套：清单 + dll + pck（deploy 的产物）
    for file in [format!("{id}.json"), format!("{id}.dll"), format!("{id}.pck")] {
        let src = artifacts_dir.join(&file);
        if !src.exists() {
            bail!(
                "缺少 {}——请先 deploy 成功再发布（或检查游戏 mods 目录）",
                src.display()
            );
        }
        std::fs::copy(&src, content.join(&file))
            .with_context(|| format!("复制 {file} 失败"))?;
        log(&format!("工坊内容 {file}"));
    }

    // 预览图：Steam 要求名为 image.png 且 < 1MB
    let workshop = project.workshop.clone().unwrap_or_default();
    let image_dst = ws.join("image.png");
    match &workshop.preview_image {
        Some(rel) => {
            let src = project_dir.join(rel);
            if !src.exists() {
                bail!("预览图不存在: {rel}");
            }
            let size = std::fs::metadata(&src)?.len();
            if size >= PREVIEW_IMAGE_MAX {
                bail!(
                    "预览图 {rel} 为 {:.0} KB，超过 Steam 的 1MB 上限，请压缩后重试",
                    size as f64 / 1024.0
                );
            }
            std::fs::copy(&src, &image_dst)?;
            log(&format!("预览图 {rel} -> image.png"));
        }
        None => {
            if !image_dst.exists() {
                bail!(
                    "未设置预览图（workshop.previewImage），且 {} 不存在。\
                     Steam 要求必须有 png 预览图（< 1MB）",
                    image_dst.display()
                );
            }
            log("使用工作区里已有的 image.png");
        }
    }

    // workshop.json：tags/changeNote 每次写入；其余字段仅在明确设置或首次发布时写入，
    // 避免每次上传覆盖工坊网页上手工改过的内容（官方 README 约定：缺省字段=保持不变）
    let first_upload = !ws.join("mod_id.txt").exists();
    let mut json = serde_json::Map::new();
    json.insert("tags".into(), serde_json::json!(workshop.tags));
    json.insert("changeNote".into(), serde_json::json!(workshop.change_note));
    if !workshop.dependencies.is_empty() {
        json.insert("dependencies".into(), serde_json::json!(workshop.dependencies));
    }
    if !workshop.content_descriptors.is_empty() {
        json.insert("contentDescriptors".into(), serde_json::json!(workshop.content_descriptors));
    }
    let title = workshop.title.clone().or_else(|| first_upload.then(|| project.manifest.name.clone()));
    if let Some(t) = title {
        json.insert("title".into(), serde_json::json!(t));
    }
    let description = workshop
        .description
        .clone()
        .or_else(|| first_upload.then(|| project.manifest.description.clone()));
    if let Some(d) = description {
        json.insert("description".into(), serde_json::json!(d));
    }
    let visibility = workshop
        .visibility
        .clone()
        .or_else(|| first_upload.then(|| "private".to_string()));
    if let Some(v) = visibility {
        json.insert("visibility".into(), serde_json::json!(v));
    }
    std::fs::write(
        ws.join("workshop.json"),
        serde_json::to_string_pretty(&serde_json::Value::Object(json))? + "\n",
    )?;

    if first_upload {
        log("首次发布：workshop.json 含标题/描述/可见性（默认 private）");
        if workshop.tags.is_empty() {
            log("警告: tags 为空——工坊标签上传后无法在网页修改，建议先填好（如 Cards / schinese）");
        }
    } else {
        log("更新发布：仅写入 tags/changeNote 等明确设置的字段，其余保持工坊现值");
    }
    Ok(ws)
}

/// 发布到创意工坊：默认先 deploy，再组装工作区并调用官方 ModUploader。
/// 需要 Steam 客户端在运行且已登录。
pub fn publish(project_dir: &Path, cfg: &ToolConfig, skip_deploy: bool, log: LogFn) -> Result<()> {
    let uploader = cfg
        .mod_uploader_exe
        .as_ref()
        .context("未配置 ModUploader 路径，请先 sts2mod config set modUploaderExe <ModUploader可执行文件>\n（下载: https://github.com/megacrit/sts2-mod-uploader/releases）")?;
    let uploader_path = Path::new(uploader);
    if !uploader_path.exists() {
        bail!("ModUploader 路径不存在: {uploader}");
    }

    if skip_deploy {
        log("跳过构建（--skip-deploy），使用现有部署产物");
        // 实时模式构建含开发用运行时，不应发布到工坊
        if project_dir.join(crate::GEN_DIR).join("Scripts/Live/Live.cs").exists() {
            bail!(
                "现有构建产物是实时模式（含 Live 运行时），不能直接发布。\
                 请去掉 --skip-deploy 重新发布（会自动做正式构建）"
            );
        }
    } else {
        deploy(project_dir, cfg, log)?;
    }

    let project = Project::load(project_dir)?;
    let id = project.manifest.id.clone();
    let mods_dir = cfg.mods_dir().context("未配置游戏目录(sts2Dir)，发布需要 deploy 产物")?;
    let ws = assemble_workshop_workspace(&project, project_dir, &mods_dir.join(&id), log)?;

    // 上传器依赖其目录里的 steam_api 库与 steam_appid.txt，用其所在目录作为工作目录
    let mut cmd = Command::new(uploader_path);
    cmd.arg("upload").arg("-w").arg(&ws);
    if let Some(dir) = uploader_path.parent() {
        cmd.current_dir(dir);
    }
    log("调用官方 ModUploader（需要 Steam 客户端正在运行）…");
    run_streamed(cmd, log).context("ModUploader 上传失败")?;

    match std::fs::read_to_string(ws.join("mod_id.txt")) {
        Ok(mod_id) => {
            let mod_id = mod_id.trim();
            log(&format!(
                "发布成功: https://steamcommunity.com/sharedfiles/filedetails/?id={mod_id}"
            ));
            log("提醒: 首次发布默认 private，到工坊页面改可见性；workshop/mod_id.txt 请随项目保存");
        }
        Err(_) => log("上传器未生成 mod_id.txt，请检查上传器输出确认是否成功"),
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
            // 创意工坊订阅版：<steamapps>/workshop/content/<游戏appid>/<条目id>/STS2-RitsuLib.dll
            let workshop = root
                .parent()
                .and_then(|p| p.parent())
                .map(|steamapps| steamapps.join("workshop").join("content").join("2868840"))
                .and_then(|content| std::fs::read_dir(content).ok())
                .and_then(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .find(|item| item.join("STS2-RitsuLib.dll").is_file())
                });
            checks.push(Check {
                name: "RitsuLib".into(),
                ok: ritsu.exists() || workshop.is_some(),
                detail: if ritsu.exists() {
                    format!("{}", ritsu.display())
                } else if let Some(w) = workshop {
                    format!("创意工坊订阅: {}", w.display())
                } else {
                    "mods/STS2-RitsuLib 与创意工坊订阅均不存在（游戏内加载 mod 需要，编译不需要）".into()
                },
            });
        }
        None => checks.push(Check {
            name: "游戏目录".into(),
            ok: false,
            detail: "未配置 sts2Dir".into(),
        }),
    }

    checks.push(match &cfg.mod_uploader_exe {
        Some(exe) if Path::new(exe).exists() => {
            Check { name: "ModUploader".into(), ok: true, detail: exe.clone() }
        }
        Some(exe) => Check {
            name: "ModUploader".into(),
            ok: false,
            detail: format!("路径不存在: {exe}"),
        },
        None => Check {
            name: "ModUploader".into(),
            ok: true,
            detail: "未配置（仅发布工坊需要，可选）".into(),
        },
    });

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
