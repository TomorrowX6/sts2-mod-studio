//! 导入已有 mod（部署产物目录：`<id>.json` + 可选 `<id>.pck`）为项目骨架。
//!
//! 能恢复的：清单、本地化文本（卡牌/遗物/能力/药水按 RitsuLib 键规则反推成
//! 内容条目）、pck 里的图片素材。不能恢复的：C# 逻辑（在 dll 里，不可逆）——
//! 效果/数值留空，由用户在编辑器里重填或写 extraCode。

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::ids;
use crate::model::{
    CardDef, CardText, Dependency, Manifest, PotionDef, PowerDef, PowerText, Project, RelicDef,
    RelicText,
};
use crate::pck::Pck;
use crate::pipeline::LogFn;

/// 游戏清单（snake_case，与生成方向相反）。
#[derive(Deserialize)]
struct GameManifest {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    version: String,
    #[serde(default)]
    min_game_version: String,
    #[serde(default = "default_true")]
    affects_gameplay: bool,
    #[serde(default)]
    dependencies: Vec<GameDependency>,
}

#[derive(Deserialize)]
struct GameDependency {
    id: String,
    #[serde(default)]
    min_version: String,
}

fn default_true() -> bool {
    true
}

pub struct ImportSummary {
    pub mod_id: String,
    pub cards: usize,
    pub relics: usize,
    pub powers: usize,
    pub potions: usize,
    pub images: usize,
    pub localization_files: usize,
    /// 无法归类的本地化键数量（怪物/事件等复杂内容不做脚手架）。
    pub skipped_keys: usize,
}

impl fmt::Display for ImportSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "导入完成: {}（{} 卡牌 / {} 遗物 / {} 能力 / {} 药水，{} 张图片，{} 个本地化文件）",
            self.mod_id, self.cards, self.relics, self.powers, self.potions, self.images,
            self.localization_files
        )?;
        if self.skipped_keys > 0 {
            writeln!(
                f,
                "  {} 个本地化键未归类（怪物/遭遇/事件/人物等不做脚手架，原文件在 assets/imported/localization/ 供参考）",
                self.skipped_keys
            )?;
        }
        write!(
            f,
            "  注意: C# 逻辑（效果/数值/触发器）在 dll 里无法导入，已留空——请在编辑器里重填"
        )
    }
}

/// 导入 mod_dir（部署产物目录）为 out_dir 下的新项目。
pub fn import_mod(mod_dir: &Path, out_dir: &Path, log: LogFn) -> Result<ImportSummary> {
    if !mod_dir.is_dir() {
        bail!("mod 目录不存在: {}", mod_dir.display());
    }
    if out_dir.join(crate::PROJECT_FILE).exists() {
        bail!("{} 下已有项目文件，不能覆盖", out_dir.display());
    }

    // 找游戏清单：目录里带 id+version 字段的 json
    let manifest = find_game_manifest(mod_dir)?;
    let id = manifest.id.clone();
    log(&format!("清单: {} v{}（{}）", id, manifest.version, manifest.name));

    let mut project = Project {
        format_version: 1,
        manifest: Manifest {
            id: id.clone(),
            name: manifest.name,
            author: manifest.author,
            description: manifest.description,
            version: normalize_semver(&manifest.version),
            min_game_version: if manifest.min_game_version.is_empty() {
                "0.107.1".into()
            } else {
                normalize_semver(&manifest.min_game_version)
            },
            affects_gameplay: manifest.affects_gameplay,
            dependencies: manifest
                .dependencies
                .into_iter()
                .map(|d| Dependency { id: d.id, min_version: normalize_semver(&d.min_version) })
                .collect(),
        },
        csharp_namespace: None,
        cards: vec![],
        relics: vec![],
        powers: vec![],
        potions: vec![],
        monsters: vec![],
        encounters: vec![],
        events: vec![],
        characters: vec![],
        keywords: vec![],
        card_tags: vec![],
        workshop: None,
    };

    let mut summary = ImportSummary {
        mod_id: id.clone(),
        cards: 0,
        relics: 0,
        powers: 0,
        potions: 0,
        images: 0,
        localization_files: 0,
        skipped_keys: 0,
    };

    std::fs::create_dir_all(out_dir.join(crate::CUSTOM_SRC_DIR))?;

    // pck：抽本地化与图片
    let pck_path = mod_dir.join(format!("{id}.pck"));
    // (lang, 类别文件名) → 键值表
    let mut loc: BTreeMap<(String, String), serde_json::Map<String, serde_json::Value>> =
        BTreeMap::new();
    // 图片：类别/文件名（含扩展名）→ 项目内相对路径
    let mut images: BTreeMap<String, String> = BTreeMap::new();

    if pck_path.exists() {
        let mut pck = Pck::open(&pck_path)?;
        let mod_prefix_path = format!("{id}/");
        let entries = pck.entries.clone();
        for entry in &entries {
            let Some(rel) = entry.path.strip_prefix(&mod_prefix_path) else { continue };
            if let Some(loc_rel) = rel.strip_prefix("localization/") {
                // localization/{lang}/{file}.json
                let mut parts = loc_rel.splitn(2, '/');
                let (Some(lang), Some(file)) = (parts.next(), parts.next()) else { continue };
                if !file.ends_with(".json") {
                    continue;
                }
                let bytes = pck.read(entry)?;
                let dst = out_dir
                    .join(crate::ASSETS_DIR)
                    .join("imported/localization")
                    .join(lang)
                    .join(file);
                write_file(&dst, &bytes)?;
                summary.localization_files += 1;
                match serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes) {
                    Ok(map) => {
                        loc.insert((lang.to_string(), file.to_string()), map);
                    }
                    Err(e) => log(&format!("警告: {} 解析失败（{e}），已原样保存", entry.path)),
                }
            } else if let Some(img_rel) = rel.strip_prefix("images/") {
                if let Some(src_rel) = img_rel.strip_suffix(".import") {
                    // 图片经 Godot 导入为 .ctex（无损 WebP/PNG 内嵌），
                    // 通过 .import 元数据定位 ctex 并抠出原图
                    let meta = String::from_utf8_lossy(&pck.read(entry)?).into_owned();
                    let Some(ctex_path) = parse_import_dest(&meta) else {
                        log(&format!("警告: {} 无法解析导入元数据，跳过", entry.path));
                        continue;
                    };
                    let Some(ctex_entry) = entries.iter().find(|e| e.path == ctex_path) else {
                        log(&format!("警告: 找不到纹理 {ctex_path}，跳过 {src_rel}"));
                        continue;
                    };
                    let ctex = pck.read(ctex_entry)?;
                    match crate::pck::extract_ctex_image(&ctex) {
                        Some((ext, bytes)) => {
                            let out_rel = replace_ext(src_rel, ext);
                            let dst = out_dir
                                .join(crate::ASSETS_DIR)
                                .join("imported/images")
                                .join(&out_rel);
                            write_file(&dst, &bytes)?;
                            images.insert(
                                out_rel.clone(),
                                format!("{}/imported/images/{out_rel}", crate::ASSETS_DIR),
                            );
                            summary.images += 1;
                        }
                        None => log(&format!(
                            "警告: {src_rel} 是 VRAM 压缩纹理，无法还原为图片（跳过）"
                        )),
                    }
                } else if !img_rel.ends_with(".remap") && !img_rel.ends_with(".md5") {
                    // 原样打包的图片（如 svg 或未导入的文件）
                    let bytes = pck.read(entry)?;
                    let dst = out_dir.join(crate::ASSETS_DIR).join("imported/images").join(img_rel);
                    write_file(&dst, &bytes)?;
                    images.insert(
                        img_rel.to_string(),
                        format!("{}/imported/images/{img_rel}", crate::ASSETS_DIR),
                    );
                    summary.images += 1;
                }
            }
        }
        log(&format!(
            "pck: 抽取 {} 个本地化文件、{} 张图片",
            summary.localization_files, summary.images
        ));
    } else {
        log(&format!("未找到 {id}.pck，仅导入清单"));
    }

    // 本地化 → 内容脚手架
    let prefix = ids::mod_prefix(&id);
    scaffold_content(&mut project, &loc, &images, &prefix, &mut summary, log);

    if mod_dir.join(format!("{id}.dll")).exists() {
        log("提醒: dll 中的 C# 逻辑无法导入，卡牌等内容的效果/数值已留空");
    }

    project.save(out_dir)?;
    let gitignore = out_dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, "build/\nworkshop/content/\nsts2mod.local.json\n")?;
    }
    log(&format!("项目已写入 {}", out_dir.display()));
    Ok(summary)
}

fn find_game_manifest(mod_dir: &Path) -> Result<GameManifest> {
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(mod_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else { continue };
        if let Ok(m) = serde_json::from_str::<GameManifest>(&raw) {
            // 清单文件名应与 id 一致，最优；否则也接受
            let exact = path.file_stem().and_then(|s| s.to_str()) == Some(m.id.as_str());
            candidates.push((exact, m));
        }
    }
    candidates.sort_by_key(|(exact, _)| !*exact);
    match candidates.into_iter().next() {
        Some((_, m)) => Ok(m),
        None => bail!(
            "{} 下没有找到 mod 清单（形如 <id>.json，含 id 与 version 字段）",
            mod_dir.display()
        ),
    }
}

/// 从 .import 元数据里解析导入产物路径（path="res://.godot/imported/….ctex"）。
fn parse_import_dest(meta: &str) -> Option<String> {
    for line in meta.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("path=\"") {
            let path = rest.split('"').next()?;
            return Some(path.strip_prefix("res://").unwrap_or(path).to_string());
        }
    }
    None
}

fn replace_ext(path: &str, ext: &str) -> String {
    match path.rsplit_once('.') {
        Some((stem, _)) => format!("{stem}.{ext}"),
        None => format!("{path}.{ext}"),
    }
}

/// 游戏侧版本号偶见两段（如 "1.0"），补成三段过校验。
fn normalize_semver(v: &str) -> String {
    let n = v.split('.').filter(|p| !p.is_empty()).count();
    match n {
        0 => "0.1.0".into(),
        1 => format!("{v}.0.0"),
        2 => format!("{v}.0"),
        _ => v.into(),
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes).with_context(|| format!("写入 {} 失败", path.display()))?;
    Ok(())
}

/// 从本地化键反推内容条目。键规则：`{MOD前缀}_{类别}_{类名蛇形}.{字段}`。
fn scaffold_content(
    project: &mut Project,
    loc: &BTreeMap<(String, String), serde_json::Map<String, serde_json::Value>>,
    images: &BTreeMap<String, String>,
    prefix: &str,
    summary: &mut ImportSummary,
    log: LogFn,
) {
    // 类别文件 → (类别 ID 段, 图片子目录)
    const CATEGORIES: &[(&str, &str, &str)] = &[
        ("cards.json", "CARD", "cards"),
        ("relics.json", "RELIC", "relics"),
        ("powers.json", "POWER", "powers"),
        ("potions.json", "POTION", "potions"),
    ];

    for (file, category, img_dir) in CATEGORIES {
        // 类名蛇形 → 语言 → 字段 → 文本
        let mut by_stem: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> =
            BTreeMap::new();
        let key_prefix = format!("{prefix}_{category}_");
        for ((lang, f), map) in loc {
            if f != file {
                continue;
            }
            for (key, value) in map {
                let Some(rest) = key.strip_prefix(&key_prefix) else {
                    summary.skipped_keys += 1;
                    continue;
                };
                let Some((stem, field)) = rest.split_once('.') else {
                    summary.skipped_keys += 1;
                    continue;
                };
                let Some(text) = value.as_str() else { continue };
                by_stem
                    .entry(stem.to_string())
                    .or_default()
                    .entry(lang.clone())
                    .or_default()
                    .insert(field.to_string(), text.to_string());
            }
        }

        for (stem, langs) in by_stem {
            let class_name = ids::pascal_of(&stem);
            let image = find_image(images, img_dir, &class_name);
            let text_of = |lang_map: &BTreeMap<String, String>, field: &str| {
                lang_map.get(field).cloned().unwrap_or_default()
            };
            match *category {
                "CARD" => {
                    let text = langs
                        .iter()
                        .map(|(lang, m)| {
                            (lang.clone(), CardText {
                                title: text_of(m, "title"),
                                description: text_of(m, "description"),
                            })
                        })
                        .collect();
                    project.cards.push(CardDef {
                        class_name,
                        pool: "Colorless".into(),
                        card_type: "Skill".into(),
                        rarity: "Common".into(),
                        target: "None".into(),
                        energy_cost: 1,
                        show_in_library: true,
                        portrait: image,
                        vars: vec![],
                        on_play: vec![],
                        keywords: vec![],
                        tags: vec![],
                        hover_tip_cards: vec![],
                        hover_tip_powers: vec![],
                        text,
                        extra_code: None,
                    });
                    summary.cards += 1;
                }
                "RELIC" => {
                    let text = langs
                        .iter()
                        .map(|(lang, m)| {
                            (lang.clone(), RelicText {
                                title: text_of(m, "title"),
                                description: text_of(m, "description"),
                                flavor: text_of(m, "flavor"),
                            })
                        })
                        .collect();
                    project.relics.push(RelicDef {
                        class_name,
                        pool: "Shared".into(),
                        rarity: "Common".into(),
                        icon: image,
                        vars: vec![],
                        triggers: vec![],
                        text,
                        extra_code: None,
                    });
                    summary.relics += 1;
                }
                "POWER" => {
                    let text = langs
                        .iter()
                        .map(|(lang, m)| {
                            (lang.clone(), PowerText {
                                title: text_of(m, "title"),
                                description: text_of(m, "description"),
                                smart_description: text_of(m, "smartDescription"),
                            })
                        })
                        .collect();
                    project.powers.push(PowerDef {
                        class_name,
                        power_type: "Buff".into(),
                        stack_type: "Counter".into(),
                        icon: image,
                        triggers: vec![],
                        text,
                        extra_code: None,
                    });
                    summary.powers += 1;
                }
                "POTION" => {
                    let text = langs
                        .iter()
                        .map(|(lang, m)| {
                            (lang.clone(), CardText {
                                title: text_of(m, "title"),
                                description: text_of(m, "description"),
                            })
                        })
                        .collect();
                    project.potions.push(PotionDef {
                        class_name,
                        pool: "Shared".into(),
                        rarity: "Common".into(),
                        usage: "CombatOnly".into(),
                        target: "Self".into(),
                        image,
                        vars: vec![],
                        on_use: vec![],
                        text,
                        extra_code: None,
                    });
                    summary.potions += 1;
                }
                _ => unreachable!(),
            }
        }
    }

    // 其余本地化文件（monsters/encounters/events/characters/ancients…）计入未归类
    for ((_, file), map) in loc {
        if !CATEGORIES.iter().any(|(f, ..)| f == file) {
            summary.skipped_keys += map.len();
        }
    }

    if summary.cards + summary.relics + summary.powers + summary.potions > 0 {
        log(&format!(
            "脚手架: {} 卡牌 / {} 遗物 / {} 能力 / {} 药水（类型/数值/效果为默认值，需重填）",
            summary.cards, summary.relics, summary.powers, summary.potions
        ));
    }
}

/// 在抽取的图片里找 `{img_dir}/{Class}.*`。
fn find_image(
    images: &BTreeMap<String, String>,
    img_dir: &str,
    class_name: &str,
) -> Option<String> {
    const EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "svg"];
    EXTS.iter()
        .find_map(|ext| images.get(&format!("{img_dir}/{class_name}.{ext}")).cloned())
}
