//! 代码生成：Project → RitsuLib 风格的 C# + Godot 工程源码树。
//!
//! 生成的目录结构与官方教程一致：
//! ```text
//! build/godot/
//! ├── {id}.csproj
//! ├── {id}.json              (游戏 mod 清单)
//! ├── project.godot
//! ├── export_presets.cfg
//! ├── Scripts/
//! │   ├── Entry.cs
//! │   ├── Cards/{ClassName}.cs
//! │   └── Custom/…            (用户 src/ 逃生舱，原样复制)
//! └── {id}/
//!     ├── images/cards/…      (从 assets/ 复制)
//!     └── localization/{lang}/cards.json
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::{bail, Result};
use serde_json::json;

use crate::ids;
use crate::model::{CardDef, Effect, Project, VarDef};

pub struct GeneratedFile {
    pub rel_path: PathBuf,
    pub content: String,
}

/// 需要从项目目录复制进生成工程的二进制素材。
pub struct AssetCopy {
    /// 相对项目目录的源路径（项目文件里写的）。
    pub src_rel: String,
    /// 相对生成工程根的目标路径。
    pub dst_rel: PathBuf,
}

pub struct GenOutput {
    pub files: Vec<GeneratedFile>,
    pub copies: Vec<AssetCopy>,
    pub warnings: Vec<String>,
}

/// DynamicVar 的标准种类 → 游戏内对应的 Var 类与占位符名。
const STANDARD_VAR_KINDS: &[&str] = &[
    "Damage", "Block", "Cards", "Energy", "Repeat", "Heal", "HpLoss", "MaxHp", "Gold", "Stars",
    "Summon", "Forge",
];

pub fn generate(project: &Project) -> Result<GenOutput> {
    project.validate()?;
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    let mut copies = Vec::new();
    let id = &project.manifest.id;
    let ns = project.namespace();

    files.push(GeneratedFile {
        rel_path: format!("{id}.json").into(),
        content: game_manifest_json(project)?,
    });
    files.push(GeneratedFile {
        rel_path: format!("{id}.csproj").into(),
        content: csproj(id),
    });
    files.push(GeneratedFile {
        rel_path: "project.godot".into(),
        content: project_godot(id),
    });
    files.push(GeneratedFile {
        rel_path: ".gitignore".into(),
        content: ".godot/\nbin/\nobj/\n".into(),
    });
    files.push(GeneratedFile {
        rel_path: "Scripts/Entry.cs".into(),
        content: entry_cs(id, &ns),
    });

    // 图片素材：有则登记复制，无则提示走 RitsuLib 占位图
    let push_asset = |copies: &mut Vec<AssetCopy>,
                          warnings: &mut Vec<String>,
                          label: &str,
                          class_name: &str,
                          category: &str,
                          src: &Option<String>| {
        if let Some(src) = src {
            let ext = std::path::Path::new(src)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            copies.push(AssetCopy {
                src_rel: src.clone(),
                dst_rel: format!("{id}/images/{category}/{class_name}.{ext}").into(),
            });
        } else {
            warnings.push(format!(
                "{label} {class_name} 未设置图片，游戏内将显示 RitsuLib 占位图"
            ));
        }
    };

    for card in &project.cards {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Cards/{}.cs", card.class_name).into(),
            content: card_cs(project, card, &mut warnings)?,
        });
        push_asset(&mut copies, &mut warnings, "卡牌", &card.class_name, "cards", &card.portrait);
    }
    for relic in &project.relics {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Relics/{}.cs", relic.class_name).into(),
            content: relic_cs(project, relic, &mut warnings)?,
        });
        push_asset(&mut copies, &mut warnings, "遗物", &relic.class_name, "relics", &relic.icon);
    }
    for power in &project.powers {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Powers/{}.cs", power.class_name).into(),
            content: power_cs(project, power, &mut warnings)?,
        });
        push_asset(&mut copies, &mut warnings, "能力", &power.class_name, "powers", &power.icon);
    }
    for potion in &project.potions {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Potions/{}.cs", potion.class_name).into(),
            content: potion_cs(project, potion, &mut warnings)?,
        });
        push_asset(&mut copies, &mut warnings, "药水", &potion.class_name, "potions", &potion.image);
    }

    for (lang, file, content) in localization_files(project)? {
        files.push(GeneratedFile {
            rel_path: format!("{id}/localization/{lang}/{file}").into(),
            content,
        });
    }

    files.push(GeneratedFile {
        rel_path: "export_presets.cfg".into(),
        content: export_presets(id, "x86_64"),
    });

    Ok(GenOutput { files, copies, warnings })
}

/// 游戏要求的 `{modid}.json`（snake_case 字段）。
fn game_manifest_json(project: &Project) -> Result<String> {
    let m = &project.manifest;
    let deps: Vec<_> = m
        .dependencies
        .iter()
        .map(|d| json!({ "id": d.id, "min_version": d.min_version }))
        .collect();
    let v = json!({
        "id": m.id,
        "name": m.name,
        "author": m.author,
        "description": m.description,
        "version": m.version,
        "min_game_version": m.min_game_version,
        "has_pck": true,
        "has_dll": true,
        "dependencies": deps,
        "affects_gameplay": m.affects_gameplay,
    });
    Ok(serde_json::to_string_pretty(&v)? + "\n")
}

fn csproj(id: &str) -> String {
    format!(
        r#"<Project Sdk="Godot.NET.Sdk/4.5.1">
  <PropertyGroup>
    <TargetFramework>net9.0</TargetFramework>
    <ImplicitUsings>true</ImplicitUsings>
    <LangVersion>13.0</LangVersion>
    <Nullable>enable</Nullable>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <AssemblyName>{id}</AssemblyName>

    <!-- 由 sts2mod 构建时通过 -p:Sts2Dir=... 传入，也可在此写死 -->
    <Sts2DataDirName Condition="'$(Sts2DataDirName)' == ''">data_sts2_windows_x86_64</Sts2DataDirName>
    <Sts2DataDir>$(Sts2Dir)/$(Sts2DataDirName)</Sts2DataDir>
  </PropertyGroup>

  <ItemGroup>
    <Reference Include="sts2">
      <HintPath>$(Sts2DataDir)/sts2.dll</HintPath>
      <Private>false</Private>
    </Reference>

    <Reference Include="0Harmony">
      <HintPath>$(Sts2DataDir)/0Harmony.dll</HintPath>
      <Private>false</Private>
    </Reference>

    <PackageReference Include="STS2.RitsuLib" Version="*" />
  </ItemGroup>

  <!-- 构建后把 dll 与清单复制到游戏 mods 目录 -->
  <Target Name="CopyMod" AfterTargets="PostBuildEvent" Condition="'$(Sts2Dir)' != '' And '$(IsInnerGodotExport)' != 'true'">
    <Message Text="Copying mod to Slay the Spire 2 mods folder..." Importance="high" />
    <MakeDir Directories="$(Sts2Dir)/mods/{id}/" />
    <Copy SourceFiles="$(TargetPath)" DestinationFolder="$(Sts2Dir)/mods/{id}/" />
    <Copy SourceFiles="$(MSBuildProjectDirectory)/{id}.json" DestinationFolder="$(Sts2Dir)/mods/{id}/" />
  </Target>
</Project>
"#
    )
}

fn project_godot(id: &str) -> String {
    format!(
        r#"; 由 sts2mod 生成，勿手改（每次生成会覆盖）
config_version=5

[application]

config/name="{id}"
config/features=PackedStringArray("4.5", "C#", "Mobile")

[dotnet]

project/assembly_name="{id}"

[rendering]

renderer/rendering_method="mobile"
"#
    )
}

fn export_presets(id: &str, arch: &str) -> String {
    format!(
        r#"[preset.0]

name="Windows Desktop"
platform="Windows Desktop"
runnable=true
advanced_options=false
dedicated_server=false
custom_features=""
export_filter="all_resources"
include_filter=""
exclude_filter="{id}.json"
export_path=""
patches=PackedStringArray()
encryption_include_filters=""
encryption_exclude_filters=""
seed=0
encrypt_pck=false
encrypt_directory=false
script_export_mode=2

[preset.0.options]

custom_template/debug=""
custom_template/release=""
debug/export_console_wrapper=1
binary_format/embed_pck=false
texture_format/s3tc_bptc=true
texture_format/etc2_astc=false
binary_format/architecture="{arch}"
codesign/enable=false
"#
    )
}

fn entry_cs(id: &str, ns: &str) -> String {
    format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using System.Reflection;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;
using STS2RitsuLib;
using STS2RitsuLib.Interop;

namespace {ns};

[ModInitializer(nameof(Init))]
public class Entry
{{
    public const string ModId = "{id}";
    public static readonly Logger Logger = RitsuLibFramework.CreateLogger(ModId);

    public static void Init()
    {{
        var assembly = Assembly.GetExecutingAssembly();
        RitsuLibFramework.EnsureGodotScriptsRegistered(assembly, Logger);
        ModTypeDiscoveryHub.RegisterModAssembly(ModId, assembly);
    }}
}}
"#
    )
}

fn pool_type(pool: &str) -> String {
    match pool {
        "Colorless" => "ColorlessCardPool".into(),
        other => other.into(),
    }
}

/// 数值在生成代码中的访问表达式。标准种类走便捷属性，其余走索引器。
fn var_accessor(name: &str) -> String {
    if STANDARD_VAR_KINDS.contains(&name) {
        format!("DynamicVars.{name}")
    } else {
        format!("DynamicVars[\"{name}\"]")
    }
}

/// 一个 VarDef 在描述占位符 / 代码访问中的名字。
fn var_name(v: &VarDef) -> String {
    if v.kind == "Power" {
        v.power.clone().unwrap_or_default()
    } else {
        v.kind.clone()
    }
}

fn var_ctor(v: &VarDef) -> Result<String> {
    if v.kind == "Power" {
        let power = v.power.as_deref().unwrap_or_default();
        return Ok(format!("new PowerVar<{power}>({})", v.value));
    }
    if !STANDARD_VAR_KINDS.contains(&v.kind.as_str()) {
        bail!("未知的数值种类: {}（支持: {} 或 Power）", v.kind, STANDARD_VAR_KINDS.join("/"));
    }
    let mut props: Vec<String> = v.props.iter().map(|p| format!("ValueProp.{p}")).collect();
    if props.is_empty() && (v.kind == "Damage" || v.kind == "Block") {
        props.push("ValueProp.Move".into());
    }
    if props.is_empty() {
        Ok(format!("new {}Var({})", v.kind, v.value))
    } else {
        Ok(format!("new {}Var({}, {})", v.kind, v.value, props.join(" | ")))
    }
}

/// 效果生成上下文：同一积木在不同宿主（卡牌/遗物/药水/能力）里可用性与表达式不同。
/// 表达式均为经过教程或反编译验证的写法；None 表示该宿主不支持此积木。
struct EffectCtx<'a> {
    /// 用于报错信息，如 "卡牌 TestCard"。
    label: String,
    /// damage（攻击）积木的目标表达式——仅卡牌（链式 FromCard(this) 只对卡牌成立）。
    attack_target: Option<&'a str>,
    /// directDamage / playVfx 的目标表达式（卡牌与有目标药水可用）。
    damage_target: Option<&'a str>,
    /// draw 积木的 Player 表达式。
    draw_player: Option<&'a str>,
    /// applyPower toSelf=true 时的 Creature 表达式。
    apply_self: &'a str,
    /// applyPower toSelf=false 时的目标表达式（无目标宿主为 None）。
    apply_target: Option<&'a str>,
    /// applyPower 的施加来源表达式。
    apply_source: &'a str,
    /// var/amount 都未填时的默认数值表达式（能力用 Amount）。
    default_amount: Option<&'a str>,
    /// 自己的 Creature 表达式（block/heal/vfx 用）。
    self_creature: &'a str,
    /// 金币归属的 Player 表达式（能力上下文没有，为 None）。
    gold_player: Option<&'a str>,
}

/// 数值表达式：优先 var 访问器，其次字面量，最后默认 var 名（或上下文默认值）。
fn amount_of(
    ctx: &EffectCtx,
    var: &Option<String>,
    amount: &Option<i64>,
    default_var: &str,
    accessor: &str,
) -> String {
    match (var, amount) {
        (Some(v), _) => format!("{}{accessor}", var_accessor(v)),
        (None, Some(n)) => n.to_string(),
        (None, None) => match ctx.default_amount {
            Some(expr) => expr.to_string(),
            None => format!("{}{accessor}", var_accessor(default_var)),
        },
    }
}

fn props_expr(props: &[String], default: &str) -> String {
    if props.is_empty() {
        default.to_string()
    } else {
        props.iter().map(|p| format!("ValueProp.{p}")).collect::<Vec<_>>().join(" | ")
    }
}

fn effect_code(ctx: &EffectCtx, effect: &Effect, warnings: &mut Vec<String>) -> Result<String> {
    Ok(match effect {
        Effect::Damage { var } => {
            let Some(target) = ctx.attack_target else {
                bail!(
                    "{}: damage（攻击）积木需要敌人目标——仅限 target 为敌人的卡牌；其他场景用 directDamage 或 custom",
                    ctx.label
                );
            };
            let name = var.clone().unwrap_or_else(|| "Damage".into());
            format!(
                "await DamageCmd.Attack({}.BaseValue)\n    .FromCard(this)\n    .Targeting({target})\n    .Execute(choiceContext);",
                var_accessor(&name)
            )
        }
        Effect::Draw { var } => {
            let Some(player) = ctx.draw_player else {
                bail!("{}: 该宿主上下文中没有 Player，无法使用 draw 积木", ctx.label);
            };
            let name = var.clone().unwrap_or_else(|| "Cards".into());
            format!(
                "await CardPileCmd.Draw(choiceContext, {}.IntValue, {player});",
                var_accessor(&name)
            )
        }
        Effect::ApplyPower { power, var, amount, to_self } => {
            let amount_expr = match (var, amount) {
                (Some(v), _) => format!("{}.IntValue", var_accessor(v)),
                (None, Some(n)) => n.to_string(),
                (None, None) => match ctx.default_amount {
                    Some(expr) => expr.to_string(),
                    // 未指定时默认引用与能力同名的 PowerVar
                    None => format!("{}.IntValue", var_accessor(power)),
                },
            };
            let target = if *to_self {
                ctx.apply_self.to_string()
            } else {
                match ctx.apply_target {
                    Some(t) => t.to_string(),
                    None => {
                        warnings.push(format!(
                            "{}: applyPower 无目标可选，已改为给自己",
                            ctx.label
                        ));
                        ctx.apply_self.to_string()
                    }
                }
            };
            format!(
                "await PowerCmd.Apply<{power}>(choiceContext, {target}, {amount_expr}, {}, null);",
                ctx.apply_source
            )
        }
        Effect::Block { var, amount } => {
            let amt = amount_of(ctx, var, amount, "Block", ".BaseValue");
            format!(
                "await CreatureCmd.GainBlock({}, {amt}, ValueProp.Move, null);",
                ctx.self_creature
            )
        }
        Effect::Heal { var, amount } => {
            let amt = amount_of(ctx, var, amount, "Heal", ".BaseValue");
            format!("await CreatureCmd.Heal({}, {amt});", ctx.self_creature)
        }
        Effect::DirectDamage { var, amount, props, to_self } => {
            let amt = amount_of(ctx, var, amount, "Damage", ".BaseValue");
            let target = if *to_self {
                ctx.self_creature.to_string()
            } else {
                match ctx.damage_target {
                    Some(t) => t.to_string(),
                    None => bail!("{}: directDamage 需要目标，该宿主没有目标（可勾选 toSelf 或用 custom）", ctx.label),
                }
            };
            format!(
                "await CreatureCmd.Damage(choiceContext, [{target}], {amt}, {}, {});",
                props_expr(props, "ValueProp.Unblockable | ValueProp.Unpowered"),
                ctx.self_creature
            )
        }
        Effect::GainGold { var, amount } => {
            let Some(player) = ctx.gold_player else {
                bail!("{}: 该宿主上下文中没有 Player，无法使用 gainGold 积木", ctx.label);
            };
            let amt = amount_of(ctx, var, amount, "Gold", ".IntValue");
            format!("await PlayerCmd.GainGold({amt}, {player});")
        }
        Effect::PlaySfx { event } => format!("SfxCmd.Play(\"{}\");", event.replace('"', "")),
        Effect::PlayVfx { path, on_self } => {
            let target = if *on_self {
                ctx.self_creature.to_string()
            } else {
                ctx.damage_target.unwrap_or(ctx.self_creature).to_string()
            };
            format!("VfxCmd.PlayOnCreature({target}, \"{}\");", path.replace('"', ""))
        }
        Effect::If { when, then, otherwise } => {
            let then_body = render_block(ctx, then, warnings)?;
            let mut code = format!("if ({})\n{{\n{then_body}\n}}", when.trim());
            if !otherwise.is_empty() {
                let else_body = render_block(ctx, otherwise, warnings)?;
                code.push_str(&format!("\nelse\n{{\n{else_body}\n}}"));
            }
            code
        }
        Effect::Repeat { times, body } => {
            let inner = render_block(ctx, body, warnings)?;
            format!("for (var i = 0; i < {times}; i++)\n{{\n{inner}\n}}")
        }
        Effect::Custom { code } => code.trim_end().to_string(),
    })
}

/// 嵌套块体：相对缩进 4 空格（供 if/repeat 使用）。
fn render_block(ctx: &EffectCtx, effects: &[Effect], warnings: &mut Vec<String>) -> Result<String> {
    if effects.is_empty() {
        return Ok("    // （空）".to_string());
    }
    let mut parts = Vec::new();
    for e in effects {
        let code = effect_code(ctx, e, warnings)?;
        parts.push(
            code.lines()
                .map(|l| if l.is_empty() { l.to_string() } else { format!("    {l}") })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    Ok(parts.join("\n"))
}

/// 把效果序列渲染成方法体（8 空格基础缩进）。
fn render_effects(ctx: &EffectCtx, effects: &[Effect], warnings: &mut Vec<String>) -> Result<String> {
    let mut parts = Vec::new();
    for e in effects {
        let code = effect_code(ctx, e, warnings)?;
        parts.push(
            code.lines()
                .map(|l| if l.is_empty() { l.to_string() } else { format!("        {l}") })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    Ok(parts.join("\n"))
}

/// 逃生舱代码块：原样插入类体（4 空格缩进）。
fn render_extra_code(extra: &Option<String>) -> String {
    match extra {
        Some(code) if !code.trim().is_empty() => {
            let body = code
                .trim_end()
                .lines()
                .map(|l| if l.is_empty() { l.to_string() } else { format!("    {l}") })
                .collect::<Vec<_>>()
                .join("\n");
            format!("\n    // ---- 自定义代码（extraCode）----\n{body}\n")
        }
        _ => String::new(),
    }
}

fn card_cs(project: &Project, card: &CardDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &card.class_name;
    let pool = pool_type(&card.pool);

    let portrait_ext = card
        .portrait
        .as_deref()
        .and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str()))
        .unwrap_or("png");

    let mut vars_src = String::new();
    for (i, v) in card.vars.iter().enumerate() {
        let sep = if i + 1 < card.vars.len() { "," } else { "" };
        writeln!(vars_src, "        {}{sep}", var_ctor(v)?).unwrap();
    }

    let has_target = !matches!(card.target.as_str(), "None" | "Self");
    let ctx = EffectCtx {
        label: format!("卡牌 {class}"),
        attack_target: has_target.then_some("cardPlay.Target!"),
        damage_target: has_target.then_some("cardPlay.Target!"),
        draw_player: Some("Owner"),
        apply_self: "Owner",
        apply_target: has_target.then_some("cardPlay.Target!"),
        apply_source: "Owner",
        default_amount: None,
        self_creature: "Owner.Creature",
        gold_player: Some("Owner"),
    };
    let on_play = if card.on_play.is_empty() {
        if card.card_type == "Attack" {
            warnings.push(format!("卡牌 {class}: 攻击牌没有任何打出效果"));
        }
        String::new()
    } else {
        let body = render_effects(&ctx, &card.on_play, warnings)?;
        format!(
            "\n    // 打出时的效果\n    protected override async Task OnPlay(PlayerChoiceContext choiceContext, CardPlay cardPlay)\n    {{\n{body}\n    }}\n"
        )
    };

    let upgrades: Vec<&VarDef> = card.vars.iter().filter(|v| v.upgrade != 0).collect();
    let on_upgrade = if upgrades.is_empty() {
        String::new()
    } else {
        let body = upgrades
            .iter()
            .map(|v| format!("        {}.UpgradeValueBy({});", var_accessor(&var_name(v)), v.upgrade))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n    // 升级效果\n    protected override void OnUpgrade()\n    {{\n{body}\n    }}\n")
    };

    let canonical_vars = if card.vars.is_empty() {
        String::new()
    } else {
        format!(
            "\n    // 卡牌基础数值\n    protected override IEnumerable<DynamicVar> CanonicalVars => [\n{vars_src}    ];\n"
        )
    };

    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.HoverTips;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Models.CardPools;
using MegaCrit.Sts2.Core.Models.Cards;
using MegaCrit.Sts2.Core.Models.Powers;
using MegaCrit.Sts2.Core.ValueProps;
using STS2RitsuLib.Cards.DynamicVars;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;

namespace {ns}.Cards;

[RegisterCard(typeof({pool}))]
public class {class} : ModCardTemplate
{{
    private const int energyCost = {cost};
    private const CardType type = CardType.{card_type};
    private const CardRarity rarity = CardRarity.{rarity};
    private const TargetType targetType = TargetType.{target};
    private const bool shouldShowInCardLibrary = {show};

    // 卡图资源
    public override CardAssetProfile AssetProfile => new(
        PortraitPath: "res://{id}/images/cards/{class}.{portrait_ext}"
    );
{canonical_vars}
    public {class}() : base(energyCost, type, rarity, targetType, shouldShowInCardLibrary)
    {{
    }}
{on_play}{on_upgrade}{extra}}}
"#,
        cost = card.energy_cost,
        card_type = card.card_type,
        rarity = card.rarity,
        target = card.target,
        show = card.show_in_library,
        extra = render_extra_code(&card.extra_code),
    ))
}

/// 触发器规格：钩子签名与上下文表达式（均来自教程原文或 RitsuLib 反编译验证）。
struct TriggerSpec {
    name: &'static str,
    sig: &'static str,
    /// 方法体开头的守卫代码（如己方回合过滤），已含缩进的完整行。
    prelude: Option<&'static str>,
    draw_player: Option<&'static str>,
    apply_self: &'static str,
    apply_target: Option<&'static str>,
    apply_source: &'static str,
    default_amount: Option<&'static str>,
    self_creature: &'static str,
    gold_player: Option<&'static str>,
}

/// 遗物可用触发器。
const RELIC_TRIGGERS: &[TriggerSpec] = &[TriggerSpec {
    name: "AfterPlayerTurnStart",
    sig: "public override async Task AfterPlayerTurnStart(PlayerChoiceContext choiceContext, Player player)",
    prelude: None,
    draw_player: Some("player"),
    apply_self: "player.Creature",
    apply_target: None,
    apply_source: "player.Creature",
    default_amount: None,
    self_creature: "player.Creature",
    gold_player: Some("player"),
}];

/// 能力可用触发器。
const POWER_TRIGGERS: &[TriggerSpec] = &[
    TriggerSpec {
        name: "AfterCardDrawn",
        sig: "public override async Task AfterCardDrawn(PlayerChoiceContext choiceContext, CardModel card, bool fromHandDraw)",
        prelude: None,
        draw_player: None,
        apply_self: "Owner",
        apply_target: None,
        apply_source: "Owner",
        default_amount: Some("Amount"),
        self_creature: "Owner",
        gold_player: None,
    },
    // 己方回合结束后（真实钩子 AfterSideTurnEnd + 己方过滤，
    // side == Owner.Side 惯用法来自 RitsuLib 源码）
    TriggerSpec {
        name: "AfterOwnerTurnEnd",
        sig: "public override async Task AfterSideTurnEnd(PlayerChoiceContext choiceContext, CombatSide side, IEnumerable<Creature> participants)",
        prelude: Some("        if (side != Owner.Side)\n        {\n            return;\n        }\n"),
        draw_player: None,
        apply_self: "Owner",
        apply_target: None,
        apply_source: "Owner",
        default_amount: Some("Amount"),
        self_creature: "Owner",
        gold_player: None,
    },
];

fn render_triggers(
    kind_label: &str,
    class_name: &str,
    triggers: &[crate::model::TriggerDef],
    specs: &[TriggerSpec],
    warnings: &mut Vec<String>,
) -> Result<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    for t in triggers {
        let Some(spec) = specs.iter().find(|s| s.name == t.trigger) else {
            let available: Vec<&str> = specs.iter().map(|s| s.name).collect();
            bail!(
                "{kind_label} {class_name}: 不支持的触发器 {}（可用: {}；其他钩子请写在 extraCode 里）",
                t.trigger,
                available.join("/")
            );
        };
        if !seen.insert(t.trigger.clone()) {
            bail!("{kind_label} {class_name}: 触发器 {} 重复", t.trigger);
        }
        if t.effects.is_empty() {
            warnings.push(format!("{kind_label} {class_name}: 触发器 {} 没有任何效果", t.trigger));
            continue;
        }
        let ctx = EffectCtx {
            label: format!("{kind_label} {class_name} 触发器 {}", t.trigger),
            attack_target: None,
            damage_target: None,
            draw_player: spec.draw_player,
            apply_self: spec.apply_self,
            apply_target: spec.apply_target,
            apply_source: spec.apply_source,
            default_amount: spec.default_amount,
            self_creature: spec.self_creature,
            gold_player: spec.gold_player,
        };
        let body = render_effects(&ctx, &t.effects, warnings)?;
        out.push_str(&format!(
            "\n    // 触发器: {}\n    {}\n    {{\n{}{}\n    }}\n",
            t.trigger,
            spec.sig,
            spec.prelude.unwrap_or(""),
            body
        ));
    }
    Ok(out)
}

/// CanonicalVars 属性块（vars 为空时返回空串）。
fn render_canonical_vars(vars: &[VarDef]) -> Result<String> {
    if vars.is_empty() {
        return Ok(String::new());
    }
    let mut src = String::new();
    for (i, v) in vars.iter().enumerate() {
        let sep = if i + 1 < vars.len() { "," } else { "" };
        writeln!(src, "        {}{sep}", var_ctor(v)?).unwrap();
    }
    Ok(format!(
        "\n    // 基础数值\n    protected override IEnumerable<DynamicVar> CanonicalVars => [\n{src}    ];\n"
    ))
}

fn asset_ext(path: &Option<String>) -> &str {
    path.as_deref()
        .and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str()))
        .unwrap_or("png")
}

fn relic_cs(project: &Project, relic: &crate::model::RelicDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &relic.class_name;
    let pool = match relic.pool.as_str() {
        "Shared" => "SharedRelicPool".to_string(),
        other => other.to_string(),
    };
    let ext = asset_ext(&relic.icon);
    let triggers = render_triggers("遗物", class, &relic.triggers, RELIC_TRIGGERS, warnings)?;
    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using Godot;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.Entities.Relics;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Models.RelicPools;
using MegaCrit.Sts2.Core.Saves.Runs;
using MegaCrit.Sts2.Core.ValueProps;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;

namespace {ns}.Relics;

[RegisterRelic(typeof({pool}))]
public class {class} : ModRelicTemplate
{{
    // 稀有度
    public override RelicRarity Rarity => RelicRarity.{rarity};

    // 图标资源（小图 / 轮廓 / 大图）
    public override RelicAssetProfile AssetProfile => new(
        IconPath: "res://{id}/images/relics/{class}.{ext}",
        IconOutlinePath: "res://{id}/images/relics/{class}.{ext}",
        BigIconPath: "res://{id}/images/relics/{class}.{ext}"
    );
{vars}{triggers}{extra}}}
"#,
        rarity = relic.rarity,
        vars = render_canonical_vars(&relic.vars)?,
        extra = render_extra_code(&relic.extra_code),
    ))
}

fn power_cs(project: &Project, power: &crate::model::PowerDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &power.class_name;
    let ext = asset_ext(&power.icon);
    let triggers = render_triggers("能力", class, &power.triggers, POWER_TRIGGERS, warnings)?;
    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Models.Cards;
using MegaCrit.Sts2.Core.ValueProps;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;

namespace {ns}.Powers;

[RegisterPower]
public class {class} : ModPowerTemplate
{{
    // 类型：Buff 或 Debuff
    public override PowerType Type => PowerType.{power_type};
    // 叠加类型：Counter 可叠加，Single 不可叠加
    public override PowerStackType StackType => PowerStackType.{stack_type};

    // 图标资源（小图 / 大图）
    public override PowerAssetProfile AssetProfile => new(
        IconPath: "res://{id}/images/powers/{class}.{ext}",
        BigIconPath: "res://{id}/images/powers/{class}.{ext}"
    );
{triggers}{extra}}}
"#,
        power_type = power.power_type,
        stack_type = power.stack_type,
        extra = render_extra_code(&power.extra_code),
    ))
}

fn potion_cs(project: &Project, potion: &crate::model::PotionDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &potion.class_name;
    let pool = match potion.pool.as_str() {
        "Shared" => "SharedPotionPool".to_string(),
        other => other.to_string(),
    };
    let ext = asset_ext(&potion.image);
    let has_target = !matches!(potion.target.as_str(), "None" | "Self");
    let ctx = EffectCtx {
        label: format!("药水 {class}"),
        attack_target: None,
        damage_target: has_target.then_some("target!"),
        draw_player: Some("Owner"),
        apply_self: "Owner.Creature",
        apply_target: has_target.then_some("target!"),
        apply_source: "Owner.Creature",
        default_amount: None,
        self_creature: "Owner.Creature",
        gold_player: Some("Owner"),
    };
    let on_use = if potion.on_use.is_empty() {
        warnings.push(format!("药水 {class}: 没有任何使用效果"));
        String::new()
    } else {
        let body = render_effects(&ctx, &potion.on_use, warnings)?;
        format!(
            "\n    // 使用时的效果\n    protected override async Task OnUse(PlayerChoiceContext choiceContext, Creature? target)\n    {{\n{body}\n    }}\n"
        )
    };
    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Potions;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.HoverTips;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Models.Cards;
using MegaCrit.Sts2.Core.ValueProps;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;

namespace {ns}.Potions;

[RegisterPotion(typeof({pool}))]
public class {class} : ModPotionTemplate
{{
    // 稀有度
    public override PotionRarity Rarity => PotionRarity.{rarity};
    // 使用方式，CombatOnly 表示只能在战斗中使用
    public override PotionUsage Usage => PotionUsage.{usage};
    // 目标类型
    public override TargetType TargetType => TargetType.{target};

    // 药水图（本体 / 轮廓）
    public override PotionAssetProfile AssetProfile => new(
        ImagePath: "res://{id}/images/potions/{class}.{ext}",
        OutlinePath: "res://{id}/images/potions/{class}.{ext}"
    );
{vars}{on_use}{extra}}}
"#,
        rarity = potion.rarity,
        usage = potion.usage,
        target = potion.target,
        vars = render_canonical_vars(&potion.vars)?,
        extra = render_extra_code(&potion.extra_code),
    ))
}

/// 每种语言、每个类别一个 json：(语言, 文件名, 内容)。
/// 键名规则：{内容ID}.title / .description，遗物另有 .flavor，能力另有 .smartDescription。
fn localization_files(project: &Project) -> Result<Vec<(String, String, String)>> {
    // (lang, 文件名) → 键值表
    let mut buckets: BTreeMap<(String, &'static str), serde_json::Map<String, serde_json::Value>> =
        BTreeMap::new();
    let mod_id = &project.manifest.id;

    for card in &project.cards {
        let cid = ids::content_id(mod_id, "CARD", &card.class_name);
        for (lang, text) in &card.text {
            let map = buckets.entry((lang.clone(), "cards.json")).or_default();
            map.insert(format!("{cid}.title"), json!(text.title));
            map.insert(format!("{cid}.description"), json!(text.description));
        }
    }
    for relic in &project.relics {
        let cid = ids::content_id(mod_id, "RELIC", &relic.class_name);
        for (lang, text) in &relic.text {
            let map = buckets.entry((lang.clone(), "relics.json")).or_default();
            map.insert(format!("{cid}.title"), json!(text.title));
            map.insert(format!("{cid}.description"), json!(text.description));
            if !text.flavor.is_empty() {
                map.insert(format!("{cid}.flavor"), json!(text.flavor));
            }
        }
    }
    for power in &project.powers {
        let cid = ids::content_id(mod_id, "POWER", &power.class_name);
        for (lang, text) in &power.text {
            let map = buckets.entry((lang.clone(), "powers.json")).or_default();
            map.insert(format!("{cid}.title"), json!(text.title));
            map.insert(format!("{cid}.description"), json!(text.description));
            if !text.smart_description.is_empty() {
                map.insert(format!("{cid}.smartDescription"), json!(text.smart_description));
            }
        }
    }
    for potion in &project.potions {
        let cid = ids::content_id(mod_id, "POTION", &potion.class_name);
        for (lang, text) in &potion.text {
            let map = buckets.entry((lang.clone(), "potions.json")).or_default();
            map.insert(format!("{cid}.title"), json!(text.title));
            map.insert(format!("{cid}.description"), json!(text.description));
        }
    }

    buckets
        .into_iter()
        .map(|((lang, file), map)| {
            Ok((
                lang,
                file.to_string(),
                serde_json::to_string_pretty(&serde_json::Value::Object(map))? + "\n",
            ))
        })
        .collect()
}
