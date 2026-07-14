//! 实时预览（live 模式）：数据生成与结构指纹。
//!
//! live 模式下生成的 mod 内置 `Live` 运行时（见 codegen::live_cs），
//! 监视游戏 `mods/<id>/live.json`。本模块负责生成这份文件：
//!
//! - `num`：所有"可热更数值"的 路径 → 当前值。路径与生成代码里
//!   `Live.Int("路径", 字面量)` 的调用一一对应（一致性由测试保证）。
//! - `loc`：全部本地化词条（语言 → 表 → 键 → 文本），运行时用
//!   `LocTable.MergeWith` 并进游戏本地化表。
//! - `fingerprint`：项目的**结构指纹**——把可热更的数值与文本抹平后取哈希。
//!   编辑只改数值/文本时指纹不变（推送即生效）；增删内容、改效果积木、
//!   改 C# 表达式等结构性改动会改变指纹（需要重新部署）。

use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::{json, Value};

use crate::codegen;
use crate::model::{Effect, IntentDef, Project, VarDef};

/// 部署目录里的实时数据文件名（`mods/<id>/live.json`）。
pub const LIVE_FILE: &str = "live.json";

/// 生成好的实时数据。
pub struct LiveData {
    pub json: String,
    /// 本地化词条总数（所有语言、所有表）。
    pub texts: usize,
    /// 可热更数值总数。
    pub nums: usize,
}

/// 项目语言代码 → 游戏三字码。游戏按 `res://{mod}/localization/{游戏语言码}/`
/// 加载（sts2.dll ModManager.GetModdedLocTables 反编译确认），
/// 常见笔误 "en" 必须映射成 "eng" 才会被加载。
pub fn normalize_lang(lang: &str) -> String {
    match lang {
        "en" => "eng".into(),
        other => other.into(),
    }
}

/// 生成 live.json。`fingerprint` 由调用方提供：
/// 部署时写入当前指纹（作为基线）；推送时保留部署基线不变。
pub fn live_data(project: &Project, fingerprint: &str) -> Result<LiveData> {
    let nums = live_numbers(project);
    let num_count = nums.len();
    let mut loc: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    let mut texts = 0;
    for ((lang, file), map) in codegen::localization_tables(project)? {
        texts += map.len();
        let table = file.trim_end_matches(".json").to_string();
        loc.entry(lang).or_default().insert(table, Value::Object(map));
    }
    let v = json!({
        "formatVersion": 1,
        "modId": project.manifest.id,
        "fingerprint": fingerprint,
        "loc": loc,
        "num": nums,
    });
    Ok(LiveData {
        json: serde_json::to_string_pretty(&v)? + "\n",
        texts,
        nums: num_count,
    })
}

/// 一个 VarDef 在路径与生成代码中的名字（与 codegen::var_name 一致）。
fn var_name(v: &VarDef) -> String {
    match v.kind.as_str() {
        "Power" => v.power.clone().unwrap_or_default(),
        "Custom" => v.name.clone().unwrap_or_default(),
        _ => v.kind.clone(),
    }
}

/// 所有可热更数值：路径 → 当前值。
/// 路径必须与 codegen 在 live 模式下发出的 `Live.Int("…")` 完全一致。
pub fn live_numbers(project: &Project) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for c in &project.cards {
        let pre = format!("card.{}", c.class_name);
        out.insert(format!("{pre}.cost"), c.energy_cost as i64);
        var_numbers(&pre, &c.vars, true, &mut out);
        effect_numbers(&format!("{pre}.onPlay"), &c.on_play, &mut out);
    }
    for r in &project.relics {
        let pre = format!("relic.{}", r.class_name);
        var_numbers(&pre, &r.vars, false, &mut out);
        for t in &r.triggers {
            effect_numbers(&format!("{pre}.trigger.{}", t.trigger), &t.effects, &mut out);
        }
    }
    for p in &project.powers {
        let pre = format!("power.{}", p.class_name);
        for t in &p.triggers {
            effect_numbers(&format!("{pre}.trigger.{}", t.trigger), &t.effects, &mut out);
        }
    }
    for p in &project.potions {
        let pre = format!("potion.{}", p.class_name);
        var_numbers(&pre, &p.vars, false, &mut out);
        effect_numbers(&format!("{pre}.onUse"), &p.on_use, &mut out);
    }
    for m in &project.monsters {
        let pre = format!("monster.{}", m.class_name);
        out.insert(format!("{pre}.minHp"), m.min_hp as i64);
        out.insert(format!("{pre}.maxHp"), m.max_hp as i64);
        for mv in &m.moves {
            for (j, it) in mv.intents.iter().enumerate() {
                if let IntentDef::Attack { amount } = it {
                    out.insert(format!("{pre}.moves.{}.intent.{j}", mv.name), *amount);
                }
            }
            effect_numbers(&format!("{pre}.moves.{}.effects", mv.name), &mv.effects, &mut out);
        }
    }
    for ev in &project.events {
        let pre = format!("event.{}", ev.class_name);
        var_numbers(&pre, &ev.vars, false, &mut out);
        for p in &ev.pages {
            for o in &p.options {
                effect_numbers(
                    &format!("{pre}.pages.{}.options.{}.effects", p.key, o.key),
                    &o.effects,
                    &mut out,
                );
            }
        }
    }
    for ch in &project.characters {
        let pre = format!("character.{}", ch.class_name);
        out.insert(format!("{pre}.startingHp"), ch.starting_hp as i64);
        out.insert(format!("{pre}.startingGold"), ch.starting_gold as i64);
        for (i, sc) in ch.starting_deck.iter().enumerate() {
            out.insert(format!("{pre}.deck.{i}"), sc.count);
        }
    }
    out
}

/// 数值（DynamicVar）：基础值恒可热更；升级增量仅卡牌会生成 OnUpgrade。
fn var_numbers(prefix: &str, vars: &[VarDef], upgrades: bool, out: &mut BTreeMap<String, i64>) {
    for v in vars {
        let name = var_name(v);
        out.insert(format!("{prefix}.var.{name}"), v.value);
        if upgrades && v.upgrade != 0 {
            out.insert(format!("{prefix}.var.{name}.up"), v.upgrade);
        }
    }
}

/// 效果树中的字面量数值。条件与 codegen 完全一致：
/// 引用 var 的数值走 DynamicVars（实例化时已热更），只有字面量需要路径。
fn effect_numbers(prefix: &str, effects: &[Effect], out: &mut BTreeMap<String, i64>) {
    for (i, e) in effects.iter().enumerate() {
        let p = format!("{prefix}.{i}");
        match e {
            Effect::Damage { var, amount }
            | Effect::Block { var, amount }
            | Effect::Heal { var, amount }
            | Effect::GainGold { var, amount }
            | Effect::LoseGold { var, amount }
            | Effect::DirectDamage { var, amount, .. }
            | Effect::ApplyPower { var, amount, .. } => {
                if var.is_none() {
                    if let Some(n) = amount {
                        out.insert(format!("{p}.amount"), *n);
                    }
                }
            }
            Effect::If { then, otherwise, .. } => {
                effect_numbers(&format!("{p}.then"), then, out);
                effect_numbers(&format!("{p}.else"), otherwise, out);
            }
            Effect::Repeat { times, body } => {
                out.insert(format!("{p}.times"), *times);
                effect_numbers(&format!("{p}.do"), body, out);
            }
            Effect::RewardCards { count } => {
                out.insert(format!("{p}.count"), *count);
            }
            Effect::Draw { .. }
            | Effect::PlaySfx { .. }
            | Effect::PlayVfx { .. }
            | Effect::RewardPotion
            | Effect::StartCombat { .. }
            | Effect::Custom { .. } => {}
        }
    }
}

/// 结构指纹：把可热更字段抹平后对项目 JSON 取哈希。
/// 指纹相同 = 推送 live.json 即可生效；不同 = 需要完整重新部署。
pub fn live_fingerprint(project: &Project) -> String {
    let mut v = serde_json::to_value(project).expect("Project 序列化不会失败");
    if let Some(obj) = v.as_object_mut() {
        // 工坊信息与构建无关
        obj.remove("workshop");
        // 清单是结构性的（写进游戏清单 json），整体保留
        let manifest = obj.remove("manifest");
        for (_, val) in obj.iter_mut() {
            strip_tunables(val);
        }
        if let Some(m) = manifest {
            obj.insert("manifest".into(), m);
        }
    }
    let canonical = serde_json::to_string(&v).expect("Value 序列化不会失败");
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

/// 递归抹平可热更字段。
/// - 文本类键（本地化映射/字符串）置空：改文本不触发重新部署
/// - 数值类键置空：改数值不触发重新部署
///
/// 注意"存在性"仍然参与指纹：比如给效果新增 amount 字段（字面量 ↔ 引用 var
/// 的切换）会改变生成代码的形状，正确地要求重新部署。
fn strip_tunables(v: &mut Value) {
    const TEXT_KEYS: &[&str] = &[
        "text", "title", "description", "banter", "tooltip", "flavor", "smartDescription", "loss",
    ];
    const NUM_KEYS: &[&str] = &[
        "value", "upgrade", "amount", "times", "count", "energyCost", "minHp", "maxHp",
        "startingHp", "startingGold",
    ];
    match v {
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                if TEXT_KEYS.contains(&k.as_str()) {
                    *val = Value::Null;
                } else if NUM_KEYS.contains(&k.as_str()) && val.is_number() {
                    *val = Value::Null;
                } else {
                    strip_tunables(val);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_tunables(item);
            }
        }
        _ => {}
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
