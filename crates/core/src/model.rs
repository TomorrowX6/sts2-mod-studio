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
    #[serde(default)]
    pub relics: Vec<RelicDef>,
    #[serde(default)]
    pub powers: Vec<PowerDef>,
    #[serde(default)]
    pub potions: Vec<PotionDef>,
    #[serde(default)]
    pub monsters: Vec<MonsterDef>,
    #[serde(default)]
    pub encounters: Vec<EncounterDef>,
    #[serde(default)]
    pub events: Vec<EventDef>,
    #[serde(default)]
    pub characters: Vec<CharacterDef>,
    /// 自定义卡牌关键词（如"唯一"——消耗/虚无这类卡牌属性）。
    #[serde(default)]
    pub keywords: Vec<KeywordDef>,
    /// 自定义卡牌标签名（PascalCase，如 Heavy；打击木偶等按 tag 判定）。
    #[serde(default)]
    pub card_tags: Vec<String>,
    /// 创意工坊发布信息（`sts2mod publish` 用）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workshop: Option<WorkshopDef>,
}

/// 自定义卡牌关键词（RegisterOwnedCardKeyword）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeywordDef {
    /// 标识（PascalCase，如 Unique），卡牌里按此名引用。
    pub name: String,
    /// 描述前的小图标（可空）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// 关键词文本插入卡牌描述的位置：
    /// None / BeforeCardDescription / AfterCardDescription。
    #[serde(default = "default_placement")]
    pub placement: String,
    /// 语言代码 → { title, description }。
    #[serde(default)]
    pub text: BTreeMap<String, CardText>,
}

fn default_placement() -> String {
    "BeforeCardDescription".into()
}

/// 创意工坊发布配置。对应官方 ModUploader 的 workshop.json——
/// 除 tags/changeNote 外，留空的字段发布时不写入，保持工坊网页上的现值不被覆盖。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkshopDef {
    /// 预览图（项目内相对路径，必须是 png 且 < 1MB；工坊硬性要求）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_image: Option<String>,
    /// 工坊标签（上传后无法在网页修改，务必先想好）。
    /// 常用：Characters / Cards / Relics / QoL / schinese / English。
    pub tags: Vec<String>,
    /// 本次更新说明（每次发布都会写入）。
    pub change_note: String,
    /// 可见性：private / public / unlisted / friends_only。
    /// 仅首次发布写入，之后请在工坊网页改（避免覆盖）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    /// 工坊标题/描述。留空 = 首次发布用清单的 name/description，之后不覆盖。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 依赖的工坊条目 ID（工坊 URL 里的数字）。
    pub dependencies: Vec<u64>,
    /// 成人内容描述符：nudity / frequent_violence / adult_only /
    /// gratuitous_nudity / general_mature。
    pub content_descriptors: Vec<String>,
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
    /// 卡牌关键词：原版名（如 Exhaust）或本项目自定义关键词名。
    #[serde(default)]
    pub keywords: Vec<String>,
    /// 卡牌标签：原版名（如 Strike / Defend）或本项目自定义 tag 名。
    #[serde(default)]
    pub tags: Vec<String>,
    /// 悬浮提示：展示本项目其他卡牌 / 能力（引用类名）。
    #[serde(default)]
    pub hover_tip_cards: Vec<String>,
    #[serde(default)]
    pub hover_tip_powers: Vec<String>,
    /// 逃生舱：原样插入类体的 C# 代码（额外字段、钩子重写等）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

/// 触发器 = 钩子方法 + 依次执行的效果。钩子名必须在该内容类型的白名单里。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerDef {
    /// 钩子名，如遗物的 "AfterPlayerTurnStart"、能力的 "AfterCardDrawn"。
    pub trigger: String,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelicDef {
    pub class_name: String,
    /// "Shared" → SharedRelicPool，或自定义池类名。
    #[serde(default = "default_shared")]
    pub pool: String,
    /// Common / Uncommon / Rare / Boss / Shop / Special
    pub rarity: String,
    /// 图标（项目内相对路径），三处图标（小图/轮廓/大图）共用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub vars: Vec<VarDef>,
    #[serde(default)]
    pub triggers: Vec<TriggerDef>,
    /// title / description / flavor
    #[serde(default)]
    pub text: BTreeMap<String, RelicText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelicText {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub flavor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerDef {
    pub class_name: String,
    /// Buff / Debuff
    pub power_type: String,
    /// Counter（可叠加）/ Single（不可叠加）
    pub stack_type: String,
    /// 图标，小图/大图共用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub triggers: Vec<TriggerDef>,
    /// title / description / smartDescription
    #[serde(default)]
    pub text: BTreeMap<String, PowerText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerText {
    pub title: String,
    /// 静态描述（卡牌悬浮提示等场景）。
    pub description: String,
    /// 动态描述，可用 {Amount} 显示层数。留空则不生成该键。
    #[serde(default)]
    pub smart_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PotionDef {
    pub class_name: String,
    /// "Shared" → SharedPotionPool，或自定义池类名。
    #[serde(default = "default_shared")]
    pub pool: String,
    /// Common / Uncommon / Rare
    pub rarity: String,
    /// CombatOnly / Anywhere 等 PotionUsage 枚举值。
    pub usage: String,
    /// TargetType，如 Self / AnyEnemy / None。
    pub target: String,
    /// 药水图（本体与轮廓共用）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default)]
    pub vars: Vec<VarDef>,
    /// 使用时效果（对应 OnUse）。
    #[serde(default)]
    pub on_use: Vec<Effect>,
    /// title / description
    #[serde(default)]
    pub text: BTreeMap<String, CardText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

fn default_shared() -> String {
    "Shared".into()
}

// ---------- M4: 怪物 / 遭遇 / 事件 / 人物 ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterDef {
    pub class_name: String,
    /// 初始血量区间（游戏在区间内随机）。
    pub min_hp: i32,
    pub max_hp: i32,
    /// 战斗模型图片（内置模板场景使用）。可空 = 隐形占位。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// 自定义 tscn 场景（项目内相对路径），设置后覆盖内置模板。
    /// 场景需含 Visuals/Bounds/IntentPos/CenterPos/TalkPos 唯一名节点。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    /// 招式列表，按顺序循环（意图1→意图2→…→意图1）。
    #[serde(default)]
    pub moves: Vec<MonsterMoveDef>,
    /// 语言代码 → 怪物名。
    #[serde(default)]
    pub text: BTreeMap<String, MonsterText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterText {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterMoveDef {
    /// 状态 ID，大写蛇形（如 BASIC_ATTACK），同时是本地化键的一部分。
    pub name: String,
    /// 头顶显示的意图（可多个并列）。
    #[serde(default)]
    pub intents: Vec<IntentDef>,
    /// 实际执行效果。怪物没有 DynamicVars，数值须用固定值。
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// 语言代码 → 意图标题（必填，游戏内悬浮显示）。
    #[serde(default)]
    pub title: BTreeMap<String, String>,
    /// 语言代码 → 出招时说的话（留空不说话）。
    #[serde(default)]
    pub banter: BTreeMap<String, String>,
}

/// 怪物意图（头顶图标）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IntentDef {
    /// 攻击意图，显示伤害数字（SingleAttackIntent）。
    Attack { amount: i64 },
    /// 防御意图（DefendIntent）。
    Defend,
    /// 自定义意图表达式，原样插入，如 `new BuffIntent()`。
    Custom { code: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterDef {
    pub class_name: String,
    /// 注册到哪些幕：Overgrowth（密林）/ Hive（蜂巢）/ Glory（荣耀）。
    #[serde(default)]
    pub acts: Vec<String>,
    /// 房间类型（RoomType 枚举）：Monster / Elite / Boss。
    #[serde(default = "default_room_monster")]
    pub room_type: String,
    /// 是否属于弱怪池（前几场战斗的敌人）。
    #[serde(default)]
    pub is_weak: bool,
    /// 出场怪物（引用本项目怪物类名）。多于一个时自动生成槽位场景。
    pub monsters: Vec<EncounterMonster>,
    /// 摄像机缩放（场景太大时调小，如 0.8）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_scaling: Option<f64>,
    /// 语言代码 → 文本。
    #[serde(default)]
    pub text: BTreeMap<String, EncounterText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterMonster {
    /// 本项目怪物类名。
    pub monster: String,
    /// 槽位名（可空，自动按 first/second/… 分配）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterText {
    pub title: String,
    /// 被该遭遇击败时的死亡文本，可用 {character} 与 {encounter} 占位。
    #[serde(default)]
    pub loss: String,
}

fn default_room_monster() -> String {
    "Monster".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventDef {
    pub class_name: String,
    /// 注册到哪些幕；空 = 共享事件（所有幕都可能出现）。
    #[serde(default)]
    pub acts: Vec<String>,
    /// 事件立绘图片。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// 事件数值（选项描述里可用 {Damage} {Gold} 等占位）。
    #[serde(default)]
    pub vars: Vec<VarDef>,
    /// 出现条件（C# 布尔表达式，runState 可用），空 = 总是允许。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// 页面列表。第一页必须是 INITIAL；没有选项的页面 = 结束页。
    pub pages: Vec<EventPage>,
    /// 语言代码 → 事件标题。
    #[serde(default)]
    pub title: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    /// 页面键，大写蛇形（INITIAL / CHOOSE_TYPE / …）。
    pub key: String,
    /// 语言代码 → 页面描述。
    #[serde(default)]
    pub description: BTreeMap<String, String>,
    /// 页面选项；空 = 结束页（显示描述后事件结束）。
    #[serde(default)]
    pub options: Vec<EventOptionDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventOptionDef {
    /// 选项键，大写蛇形（TAKE_DAMAGE / …）。
    pub key: String,
    /// 语言代码 → 选项标题。
    #[serde(default)]
    pub title: BTreeMap<String, String>,
    /// 语言代码 → 选项描述（可空）。
    #[serde(default)]
    pub description: BTreeMap<String, String>,
    /// 选中后依次执行的效果。
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// 之后跳到哪一页（页面键）。含 startCombat 效果时可空。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goto: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterDef {
    pub class_name: String,
    /// 主题色 #RRGGBB（名字颜色、能量轮廓、卡框色调等统一使用）。
    #[serde(default = "default_char_color")]
    pub color: String,
    /// Masculine / Feminine / Neutral。
    #[serde(default = "default_gender")]
    pub gender: String,
    pub starting_hp: i32,
    pub starting_gold: i32,
    /// 未提供的资源自动回退到该原版人物：
    /// Ironclad / Silent / Defect / Regent / Necrobinder。
    #[serde(default = "default_base_character")]
    pub base: String,
    /// 战斗模型图片（内置模板场景）。可空 = 用原版人物模型。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat_image: Option<String>,
    /// 头像（左上角与统计页）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait: Option<String>,
    /// 选人界面图标 / 锁定态图标 / 地图标记。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select_icon_locked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_marker: Option<String>,
    /// 能量图标：描述内联 24x24 / 表盘与卡牌角标 74x74。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_icon_big: Option<String>,
    /// 初始卡组（引用本项目卡牌类名）。
    #[serde(default)]
    pub starting_deck: Vec<StartingCard>,
    /// 初始遗物（引用本项目遗物类名）。
    #[serde(default)]
    pub starting_relics: Vec<String>,
    /// 语言代码 → 文本（其余人称代词等键按性别自动生成默认值）。
    #[serde(default)]
    pub text: BTreeMap<String, CharacterText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartingCard {
    /// 本项目卡牌类名。
    pub card: String,
    #[serde(default = "default_card_count")]
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterText {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

fn default_char_color() -> String {
    "#8080FF".into()
}

fn default_gender() -> String {
    "Neutral".into()
}

fn default_base_character() -> String {
    "Ironclad".into()
}

fn default_card_count() -> i64 {
    1
}

/// 卡牌数值（DynamicVar）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VarDef {
    /// Damage / Block / Cards / Energy / Repeat / Heal / HpLoss / MaxHp /
    /// Gold / Stars 等标准种类，"Power"（配合 `power` 字段），
    /// 或 "Custom"（配合 `name` 字段，生成 ModCardVars.Int）。
    pub kind: String,
    /// kind = "Power" 时的能力类名，如 StrengthPower。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<String>,
    /// kind = "Custom" 时的变量名（PascalCase，如 Leech），描述里 {Leech} 引用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// kind = "Custom" 时的悬浮提示文本（语言 → {title, description}）。
    /// 非空时生成 WithSharedTooltip 与 static_hover_tips.json。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tooltip: BTreeMap<String, CardText>,
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
    /// 对目标造成伤害（使用 DamageVar，默认名 "Damage"；怪物招式里用固定值 amount）。
    Damage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<i64>,
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
    /// 获得格挡（CreatureCmd.GainBlock，默认 ValueProp.Move，吃敏捷）。默认数值名 "Block"。
    Block {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<i64>,
    },
    /// 治疗自己（CreatureCmd.Heal）。默认数值名 "Heal"。
    Heal {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<i64>,
    },
    /// 直接伤害（CreatureCmd.Damage，非攻击：无攻击动画、默认不可格挡不吃力量，
    /// 适合"失去生命"或穿透伤害）。props 为空时默认 Unblockable|Unpowered。
    #[serde(rename_all = "camelCase")]
    DirectDamage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<i64>,
        #[serde(default)]
        props: Vec<String>,
        #[serde(default)]
        to_self: bool,
    },
    /// 获得金币（PlayerCmd.GainGold）。默认数值名 "Gold"。
    GainGold {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<i64>,
    },
    /// 播放音效（SfxCmd.Play），event 形如 "event:/sfx/block_gain"。
    PlaySfx { event: String },
    /// 播放特效（VfxCmd.PlayOnCreature），path 形如 "vfx/vfx_bloody_impact"。
    #[serde(rename_all = "camelCase")]
    PlayVfx {
        path: String,
        /// true 在自己身上播放，false 在目标身上（无目标宿主自动改为自己）。
        #[serde(default)]
        on_self: bool,
    },
    /// 条件分支：when 为 C# 布尔表达式，then/else 为效果序列（可嵌套）。
    If {
        when: String,
        #[serde(default)]
        then: Vec<Effect>,
        #[serde(default, rename = "else", skip_serializing_if = "Vec::is_empty")]
        otherwise: Vec<Effect>,
    },
    /// 重复执行 times 次（可嵌套）。
    Repeat {
        times: i64,
        #[serde(default, rename = "do")]
        body: Vec<Effect>,
    },
    /// 失去金币（事件专用，PlayerCmd.LoseGold）。默认数值名 "Gold"。
    LoseGold {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<i64>,
    },
    /// 事件奖励：选 count 张牌（CardReward）。
    #[serde(rename_all = "camelCase")]
    RewardCards {
        #[serde(default = "default_reward_cards")]
        count: i64,
    },
    /// 事件奖励：一瓶药水（PotionReward）。
    RewardPotion,
    /// 事件内进入战斗并结束事件（encounter = 本项目遭遇类名）。
    #[serde(rename_all = "camelCase")]
    StartCombat { encounter: String },
    /// 逃生舱：原样插入一段 C# 代码（方法体内）。
    Custom { code: String },
}

fn default_reward_cards() -> i64 {
    3
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
        let mut check = |label: &str, class_name: &str, vars: &[VarDef]| -> Result<()> {
            if !is_pascal_case_ident(class_name) {
                bail!("{label}类名必须是 PascalCase 的合法 C# 标识符（当前: {class_name}）");
            }
            if !seen.insert(class_name.to_string()) {
                bail!("类名重复: {class_name}（所有内容的类名需全局唯一）");
            }
            for v in vars {
                if v.kind == "Power" && v.power.is_none() {
                    bail!("{label} {class_name}: kind=Power 的数值必须填 power 字段");
                }
                if v.kind == "Custom" {
                    match &v.name {
                        Some(n) if is_pascal_case_ident(n) => {}
                        Some(n) => bail!(
                            "{label} {class_name}: 自定义数值名必须是 PascalCase（当前: {n}）"
                        ),
                        None => bail!("{label} {class_name}: kind=Custom 的数值必须填 name 字段"),
                    }
                }
            }
            Ok(())
        };
        for c in &self.cards {
            check("卡牌", &c.class_name, &c.vars)?;
        }
        for r in &self.relics {
            check("遗物", &r.class_name, &r.vars)?;
        }
        for p in &self.powers {
            check("能力", &p.class_name, &[])?;
        }
        for p in &self.potions {
            check("药水", &p.class_name, &p.vars)?;
        }
        for m in &self.monsters {
            check("怪物", &m.class_name, &[])?;
        }
        for e in &self.encounters {
            check("遭遇", &e.class_name, &[])?;
        }
        for e in &self.events {
            check("事件", &e.class_name, &e.vars)?;
        }
        for c in &self.characters {
            check("人物", &c.class_name, &[])?;
        }
        self.validate_workshop()?;
        self.validate_keywords_tags()?;
        self.validate_m4()
    }

    /// 自定义关键词 / 标签及卡牌上的引用。
    fn validate_keywords_tags(&self) -> Result<()> {
        let mut kw_names = std::collections::HashSet::new();
        for kw in &self.keywords {
            if !is_pascal_case_ident(&kw.name) {
                bail!("关键词名必须是 PascalCase（当前: {}）", kw.name);
            }
            if !kw_names.insert(kw.name.as_str()) {
                bail!("关键词名重复: {}", kw.name);
            }
            const PLACEMENTS: &[&str] = &["None", "BeforeCardDescription", "AfterCardDescription"];
            if !PLACEMENTS.contains(&kw.placement.as_str()) {
                bail!(
                    "关键词 {}: placement 必须是 {} 之一（当前: {}）",
                    kw.name, PLACEMENTS.join("/"), kw.placement
                );
            }
        }
        let mut tag_names = std::collections::HashSet::new();
        for t in &self.card_tags {
            if !is_pascal_case_ident(t) {
                bail!("卡牌标签名必须是 PascalCase（当前: {t}）");
            }
            if !tag_names.insert(t.as_str()) {
                bail!("卡牌标签名重复: {t}");
            }
        }
        let card_names: std::collections::HashSet<&str> =
            self.cards.iter().map(|c| c.class_name.as_str()).collect();
        let power_names: std::collections::HashSet<&str> =
            self.powers.iter().map(|p| p.class_name.as_str()).collect();
        for c in &self.cards {
            for k in &c.keywords {
                if !is_pascal_case_ident(k) {
                    bail!("卡牌 {}: 关键词名必须是 PascalCase（当前: {k}）", c.class_name);
                }
            }
            for t in &c.tags {
                if !is_pascal_case_ident(t) {
                    bail!("卡牌 {}: 标签名必须是 PascalCase（当前: {t}）", c.class_name);
                }
            }
            for r in &c.hover_tip_cards {
                if !card_names.contains(r.as_str()) {
                    bail!(
                        "卡牌 {}: 悬浮提示引用了不存在的卡牌 {r}（仅支持本项目卡牌，原版内容请用 extraCode）",
                        c.class_name
                    );
                }
            }
            for r in &c.hover_tip_powers {
                if !power_names.contains(r.as_str()) {
                    bail!(
                        "卡牌 {}: 悬浮提示引用了不存在的能力 {r}（仅支持本项目能力，原版内容请用 extraCode）",
                        c.class_name
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_workshop(&self) -> Result<()> {
        let Some(w) = &self.workshop else { return Ok(()) };
        if let Some(v) = &w.visibility {
            const VIS: &[&str] = &["private", "public", "unlisted", "friends_only"];
            if !VIS.contains(&v.as_str()) {
                bail!("workshop.visibility 必须是 {} 之一（当前: {v}）", VIS.join("/"));
            }
        }
        const DESCRIPTORS: &[&str] =
            &["nudity", "frequent_violence", "adult_only", "gratuitous_nudity", "general_mature"];
        for d in &w.content_descriptors {
            if !DESCRIPTORS.contains(&d.as_str()) {
                bail!("workshop.contentDescriptors 含未知值 {d}（可用: {}）", DESCRIPTORS.join("/"));
            }
        }
        if let Some(img) = &w.preview_image {
            if !img.to_ascii_lowercase().ends_with(".png") {
                bail!("workshop.previewImage 必须是 png 文件（Steam 要求 image.png，当前: {img}）");
            }
        }
        Ok(())
    }

    /// 怪物 / 遭遇 / 事件 / 人物的结构与引用校验。
    fn validate_m4(&self) -> Result<()> {
        for m in &self.monsters {
            let class = &m.class_name;
            if m.min_hp < 1 || m.max_hp < m.min_hp {
                bail!("怪物 {class}: 血量区间无效（需 1 <= minHp <= maxHp，当前 {}~{}）", m.min_hp, m.max_hp);
            }
            let mut seen = std::collections::HashSet::new();
            for mv in &m.moves {
                if !is_upper_snake(&mv.name) {
                    bail!("怪物 {class}: 招式名必须是大写蛇形（如 BASIC_ATTACK，当前: {}）", mv.name);
                }
                if !seen.insert(mv.name.clone()) {
                    bail!("怪物 {class}: 招式名重复: {}", mv.name);
                }
            }
        }
        let monster_names: std::collections::HashSet<&str> =
            self.monsters.iter().map(|m| m.class_name.as_str()).collect();
        for e in &self.encounters {
            let class = &e.class_name;
            if e.monsters.is_empty() {
                bail!("遭遇 {class}: 至少需要一个怪物");
            }
            for em in &e.monsters {
                if !monster_names.contains(em.monster.as_str()) {
                    bail!("遭遇 {class}: 引用了不存在的怪物 {}（需先在怪物列表中创建）", em.monster);
                }
            }
            let slots: Vec<&String> = e.monsters.iter().filter_map(|m| m.slot.as_ref()).collect();
            let mut seen = std::collections::HashSet::new();
            for s in &slots {
                if !seen.insert(s.as_str()) {
                    bail!("遭遇 {class}: 槽位名重复: {s}");
                }
            }
            for act in &e.acts {
                if !is_pascal_case_ident(act) {
                    bail!("遭遇 {class}: 幕名必须是 PascalCase 类名（Overgrowth/Hive/Glory，当前: {act}）");
                }
            }
        }
        let encounter_names: std::collections::HashSet<&str> =
            self.encounters.iter().map(|e| e.class_name.as_str()).collect();
        for ev in &self.events {
            let class = &ev.class_name;
            if ev.pages.is_empty() {
                bail!("事件 {class}: 至少需要一个页面（INITIAL）");
            }
            if ev.pages[0].key != "INITIAL" {
                bail!("事件 {class}: 第一页的键必须是 INITIAL（当前: {}）", ev.pages[0].key);
            }
            if ev.pages[0].options.is_empty() {
                bail!("事件 {class}: INITIAL 页必须至少有一个选项");
            }
            let mut page_keys = std::collections::HashSet::new();
            for p in &ev.pages {
                if !is_upper_snake(&p.key) {
                    bail!("事件 {class}: 页面键必须是大写蛇形（当前: {}）", p.key);
                }
                if !page_keys.insert(p.key.clone()) {
                    bail!("事件 {class}: 页面键重复: {}", p.key);
                }
            }
            for act in &ev.acts {
                if !is_pascal_case_ident(act) {
                    bail!("事件 {class}: 幕名必须是 PascalCase 类名（当前: {act}）");
                }
            }
            for p in &ev.pages {
                let mut opt_keys = std::collections::HashSet::new();
                for o in &p.options {
                    if !is_upper_snake(&o.key) {
                        bail!("事件 {class} 页 {}: 选项键必须是大写蛇形（当前: {}）", p.key, o.key);
                    }
                    if !opt_keys.insert(o.key.clone()) {
                        bail!("事件 {class} 页 {}: 选项键重复: {}", p.key, o.key);
                    }
                    let mut has_combat = false;
                    walk_effects(&o.effects, &mut |e| {
                        if let Effect::StartCombat { encounter } = e {
                            has_combat = true;
                            if !encounter_names.contains(encounter.as_str()) {
                                return Err(anyhow::anyhow!(
                                    "事件 {class} 选项 {}.{}: startCombat 引用了不存在的遭遇 {encounter}",
                                    p.key, o.key
                                ));
                            }
                        }
                        Ok(())
                    })?;
                    match &o.goto {
                        Some(g) => {
                            if !ev.pages.iter().any(|pp| &pp.key == g) {
                                bail!("事件 {class} 选项 {}.{}: 跳转目标页 {g} 不存在", p.key, o.key);
                            }
                        }
                        None => {
                            if !has_combat {
                                bail!(
                                    "事件 {class} 选项 {}.{}: 必须设置跳转页，或包含 startCombat 效果",
                                    p.key, o.key
                                );
                            }
                        }
                    }
                }
            }
        }
        let card_names: std::collections::HashSet<&str> =
            self.cards.iter().map(|c| c.class_name.as_str()).collect();
        let relic_names: std::collections::HashSet<&str> =
            self.relics.iter().map(|r| r.class_name.as_str()).collect();
        for ch in &self.characters {
            let class = &ch.class_name;
            if parse_hex_color(&ch.color).is_none() {
                bail!("人物 {class}: color 必须是 #RRGGBB 十六进制颜色（当前: {}）", ch.color);
            }
            const BASES: &[&str] = &["Ironclad", "Silent", "Defect", "Regent", "Necrobinder"];
            if !BASES.contains(&ch.base.as_str()) {
                bail!("人物 {class}: base 必须是 {} 之一（当前: {}）", BASES.join("/"), ch.base);
            }
            if !["Masculine", "Feminine", "Neutral"].contains(&ch.gender.as_str()) {
                bail!("人物 {class}: gender 必须是 Masculine/Feminine/Neutral（当前: {}）", ch.gender);
            }
            if ch.starting_hp < 1 {
                bail!("人物 {class}: 初始血量必须 >= 1");
            }
            for sc in &ch.starting_deck {
                if !card_names.contains(sc.card.as_str()) {
                    bail!("人物 {class}: 初始卡组引用了不存在的卡牌 {}（需先在卡牌列表中创建）", sc.card);
                }
                if sc.count < 1 {
                    bail!("人物 {class}: 初始卡 {} 数量必须 >= 1", sc.card);
                }
            }
            for r in &ch.starting_relics {
                if !relic_names.contains(r.as_str()) {
                    bail!("人物 {class}: 初始遗物引用了不存在的遗物 {r}（需先在遗物列表中创建）");
                }
            }
        }
        Ok(())
    }
}

/// 递归遍历效果树（含 if/repeat 嵌套）。
pub fn walk_effects(effects: &[Effect], f: &mut dyn FnMut(&Effect) -> Result<()>) -> Result<()> {
    for e in effects {
        f(e)?;
        match e {
            Effect::If { then, otherwise, .. } => {
                walk_effects(then, f)?;
                walk_effects(otherwise, f)?;
            }
            Effect::Repeat { body, .. } => walk_effects(body, f)?,
            _ => {}
        }
    }
    Ok(())
}

fn is_upper_snake(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// "#RRGGBB" → (r, g, b)，每分量 0.0~1.0。
pub fn parse_hex_color(s: &str) -> Option<(f32, f32, f32)> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some((
        byte(0)? as f32 / 255.0,
        byte(2)? as f32 / 255.0,
        byte(4)? as f32 / 255.0,
    ))
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
                // 怪物/人物脚手架（ModMonsterTemplate 等）需要较新的 RitsuLib
                id: "STS2-RitsuLib".into(),
                min_version: "0.4.54".into(),
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
                name: None,
                tooltip: BTreeMap::new(),
                value: 9,
                props: vec!["Move".into()],
                upgrade: 3,
            }],
            on_play: vec![Effect::Damage { var: None, amount: None }],
            keywords: vec![],
            tags: vec![],
            hover_tip_cards: vec![],
            hover_tip_powers: vec![],
            text,
            extra_code: None,
        }],
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
    }
}
