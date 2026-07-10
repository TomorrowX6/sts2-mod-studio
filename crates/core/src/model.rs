//! `project.stsmod.json` 的数据模型。
//!
//! 这是工具的“唯一事实来源”：UI 和 CLI 都读写这个结构，
//! 代码生成器把它翻译成 RitsuLib 风格的 C# 源码与资源文件。

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// 项目文件格式版本，用于将来迁移。
    pub format_version: u32,
    pub manifest: Manifest,
    /// C# 命名空间根，默认取 manifest.id。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csharp_namespace: Option<String>,
    #[serde(default)]
    pub cards: Vec<CardDef>,
}

/// 对应游戏要求的 `{modid}.json` 清单（camelCase 为工具内格式，
/// 生成时转成游戏要求的 snake_case 字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub version: String,
    pub min_game_version: String,
    #[serde(default = "default_true")]
    pub affects_gameplay: bool,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub id: String,
    pub min_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDef {
    /// C# 类名（PascalCase），同时决定内容物 ID 与默认资源文件名。
    pub class_name: String,
    /// 卡池："Colorless" 或完整卡池类名（如自定义池 `TestCardPool`）。
    #[serde(default = "default_pool")]
    pub pool: String,
    /// Attack / Skill / Power / Status / Curse
    pub card_type: String,
    /// Basic / Common / Uncommon / Rare / Special
    pub rarity: String,
    /// AnyEnemy / AllEnemies / Self / None 等（原样写入 TargetType.X）
    pub target: String,
    pub energy_cost: i32,
    #[serde(default = "default_true")]
    pub show_in_library: bool,
    /// 卡图，项目目录内的相对路径（一般在 assets/ 下）。可空 = 暂无卡图。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait: Option<String>,
    #[serde(default)]
    pub vars: Vec<VarDef>,
    /// 打出时效果（积木序列，按顺序生成 OnPlay 内的 await 调用）。
    #[serde(default)]
    pub on_play: Vec<Effect>,
    /// 本地化文本：语言代码（zhs / en / ...）→ 文本。
    #[serde(default)]
    pub text: BTreeMap<String, CardText>,
}

/// 卡牌数值（DynamicVar）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VarDef {
    /// Damage / Block / Cards / Energy / Repeat / Heal / HpLoss / MaxHp /
    /// Gold / Stars 等标准种类，或 "Power"（配合 `power` 字段）。
    pub kind: String,
    /// kind = "Power" 时的能力类名，如 StrengthPower。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<String>,
    pub value: i64,
    /// ValueProp 组合：Move / Unpowered / Unblockable / SkipHurtAnim。
    /// 为空时 Damage/Block 默认 Move。
    #[serde(default)]
    pub props: Vec<String>,
    /// 升级时增加量，0 = 升级不改动该值。
    #[serde(default)]
    pub upgrade: i64,
}

/// 打出效果积木。amount 来源：`var`（引用 vars 中的数值）或 `amount` 字面量。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Effect {
    /// 对目标造成伤害（使用 DamageVar，默认名 "Damage"）。
    Damage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
    },
    /// 抽牌（使用 CardsVar，默认名 "Cards"）。
    Draw {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
    },
    /// 给予能力。to_self=true 给自己，否则给卡牌目标。
    #[serde(rename_all = "camelCase")]
    ApplyPower {
        power: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<i64>,
        #[serde(default)]
        to_self: bool,
    },
    /// 逃生舱：原样插入一段 C# 代码（OnPlay 方法体内）。
    Custom { code: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardText {
    pub title: String,
    pub description: String,
}

fn default_true() -> bool {
    true
}

fn default_pool() -> String {
    "Colorless".into()
}

impl Project {
    pub fn load(project_dir: &Path) -> Result<Self> {
        let path = project_dir.join(crate::PROJECT_FILE);
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("读取项目文件失败: {}", path.display()))?;
        let project: Project =
            serde_json::from_str(&raw).with_context(|| format!("解析 {} 失败", path.display()))?;
        project.validate()?;
        Ok(project)
    }

    pub fn save(&self, project_dir: &Path) -> Result<()> {
        self.validate()?;
        let path = project_dir.join(crate::PROJECT_FILE);
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw + "\n")
            .with_context(|| format!("写入项目文件失败: {}", path.display()))?;
        Ok(())
    }

    pub fn namespace(&self) -> String {
        self.csharp_namespace
            .clone()
            .unwrap_or_else(|| self.manifest.id.clone())
    }

    pub fn validate(&self) -> Result<()> {
        let id = &self.manifest.id;
        if id.is_empty() {
            bail!("manifest.id 不能为空");
        }
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            bail!("manifest.id 只能包含字母、数字、'-'、'_'（当前: {id}）");
        }
        if !id.chars().next().unwrap().is_ascii_alphabetic() {
            bail!("manifest.id 必须以字母开头（当前: {id}）");
        }
        for part in [
            ("version", &self.manifest.version),
            ("minGameVersion", &self.manifest.min_game_version),
        ] {
            if !is_semver3(part.1) {
                bail!("manifest.{} 必须是 X.X.X 三段语义化版本（当前: {}）", part.0, part.1);
            }
        }
        let mut seen = std::collections::HashSet::new();
        for card in &self.cards {
            if !is_pascal_case_ident(&card.class_name) {
                bail!("卡牌类名必须是 PascalCase 的合法 C# 标识符（当前: {}）", card.class_name);
            }
            if !seen.insert(card.class_name.clone()) {
                bail!("卡牌类名重复: {}", card.class_name);
            }
            for v in &card.vars {
                if v.kind == "Power" && v.power.is_none() {
                    bail!("卡牌 {}: kind=Power 的数值必须填 power 字段", card.class_name);
                }
            }
        }
        Ok(())
    }
}

fn is_semver3(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn is_pascal_case_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 新建项目时的模板：带一张示例攻击牌。
pub fn starter_project(id: &str, name: &str) -> Project {
    let mut text = BTreeMap::new();
    text.insert(
        "zhs".to_string(),
        CardText {
            title: "示例打击".into(),
            description: "造成{Damage:diff()}点伤害。".into(),
        },
    );
    text.insert(
        "en".to_string(),
        CardText {
            title: "Sample Strike".into(),
            description: "Deal {Damage:diff()} damage.".into(),
        },
    );
    Project {
        format_version: 1,
        manifest: Manifest {
            id: id.to_string(),
            name: name.to_string(),
            author: String::new(),
            description: String::new(),
            version: "0.1.0".into(),
            min_game_version: "0.107.1".into(),
            affects_gameplay: true,
            dependencies: vec![Dependency {
                id: "STS2-RitsuLib".into(),
                min_version: "0.2.27".into(),
            }],
        },
        csharp_namespace: None,
        cards: vec![CardDef {
            class_name: "SampleStrike".into(),
            pool: "Colorless".into(),
            card_type: "Attack".into(),
            rarity: "Common".into(),
            target: "AnyEnemy".into(),
            energy_cost: 1,
            show_in_library: true,
            portrait: None,
            vars: vec![VarDef {
                kind: "Damage".into(),
                power: None,
                value: 9,
                props: vec!["Move".into()],
                upgrade: 3,
            }],
            on_play: vec![Effect::Damage { var: None }],
            text,
        }],
    }
}
