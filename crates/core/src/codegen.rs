//! 代码生成：Project → RitsuLib 风格的 C# + Godot 工程源码树。
//!
//! 生成的目录结构与官方教程一致：
//! ```text
//! build/godot/
//! ├── {id}.csproj
//! ├── {id}.sln
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
use crate::model::{
    parse_hex_color, walk_effects, CardDef, CharacterDef, Effect, EncounterDef, EventDef,
    IntentDef, MonsterDef, Project, VarDef,
};

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
        rel_path: format!("{id}.sln").into(),
        content: solution(id),
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

    for monster in &project.monsters {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Monsters/{}.cs", monster.class_name).into(),
            content: monster_cs(project, monster, &mut warnings)?,
        });
        if let Some(custom) = &monster.scene {
            // 自定义场景原样复制
            copies.push(AssetCopy {
                src_rel: custom.clone(),
                dst_rel: format!("{id}/scenes/{}.tscn", scene_stem(&monster.class_name)).into(),
            });
        } else {
            files.push(GeneratedFile {
                rel_path: format!("{id}/scenes/{}.tscn", scene_stem(&monster.class_name)).into(),
                content: creature_scene_tscn(
                    &monster.class_name,
                    monster.image.as_ref().map(|img| {
                        format!("res://{id}/images/monsters/{}.{}", monster.class_name, ext_of(img))
                    }),
                ),
            });
            if monster.image.is_none() {
                warnings.push(format!(
                    "怪物 {} 未设置图片，战斗中将不可见（血条与意图仍显示）",
                    monster.class_name
                ));
            }
        }
        if monster.image.is_some() {
            push_asset(&mut copies, &mut warnings, "怪物", &monster.class_name, "monsters", &monster.image);
        }
    }
    for enc in &project.encounters {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Encounters/{}.cs", enc.class_name).into(),
            content: encounter_cs(project, enc, &mut warnings)?,
        });
        if enc.monsters.len() > 1 {
            files.push(GeneratedFile {
                rel_path: format!("{id}/scenes/{}.tscn", scene_stem(&enc.class_name)).into(),
                content: encounter_scene_tscn(&enc.class_name, &encounter_slots(enc)),
            });
        }
    }
    for ev in &project.events {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Events/{}.cs", ev.class_name).into(),
            content: event_cs(project, ev, &mut warnings)?,
        });
        push_asset(&mut copies, &mut warnings, "事件", &ev.class_name, "events", &ev.image);
    }
    for ch in &project.characters {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Characters/{}Pools.cs", ch.class_name).into(),
            content: character_pools_cs(project, ch)?,
        });
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Characters/{}.cs", ch.class_name).into(),
            content: character_cs(project, ch, &mut warnings)?,
        });
        for f in character_scenes(project, ch) {
            files.push(f);
        }
        let char_assets: &[(&str, &Option<String>, &str)] = &[
            ("战斗模型图", &ch.combat_image, "Combat"),
            ("头像", &ch.portrait, "Portrait"),
            ("选人图标", &ch.select_icon, "Select"),
            ("选人锁定图标", &ch.select_icon_locked, "SelectLocked"),
            ("地图标记", &ch.map_marker, "Marker"),
            ("能量图标(24x24)", &ch.energy_icon, "Energy"),
            ("能量图标(74x74)", &ch.energy_icon_big, "EnergyBig"),
        ];
        for (label, src, suffix) in char_assets {
            if let Some(src) = src {
                let ext = ext_of(src);
                copies.push(AssetCopy {
                    src_rel: src.to_string(),
                    dst_rel: format!("{id}/images/characters/{}{suffix}.{ext}", ch.class_name).into(),
                });
            } else {
                warnings.push(format!(
                    "人物 {} 未设置{label}，将回退到原版 {} 的资源",
                    ch.class_name, ch.base
                ));
            }
        }
        warnings.push(format!(
            "人物 {}: 先古对话（ancients.json）为占位文本，正式发布前请在生成目录参考格式后通过自定义方式覆盖",
            ch.class_name
        ));
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

fn solution(id: &str) -> String {
    format!(
        r#"Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 17
VisualStudioVersion = 17.0.31903.59
MinimumVisualStudioVersion = 10.0.40219.1
Project("{{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}}") = "{id}", "{id}.csproj", "{{A7611677-F3D4-46B1-AC17-E9908C2EF7AD}}"
EndProject
Global
	GlobalSection(SolutionConfigurationPlatforms) = preSolution
		Debug|Any CPU = Debug|Any CPU
		Release|Any CPU = Release|Any CPU
	EndGlobalSection
	GlobalSection(ProjectConfigurationPlatforms) = postSolution
		{{A7611677-F3D4-46B1-AC17-E9908C2EF7AD}}.Debug|Any CPU.ActiveCfg = Debug|Any CPU
		{{A7611677-F3D4-46B1-AC17-E9908C2EF7AD}}.Debug|Any CPU.Build.0 = Debug|Any CPU
		{{A7611677-F3D4-46B1-AC17-E9908C2EF7AD}}.Release|Any CPU.ActiveCfg = Release|Any CPU
		{{A7611677-F3D4-46B1-AC17-E9908C2EF7AD}}.Release|Any CPU.Build.0 = Release|Any CPU
	EndGlobalSection
	GlobalSection(SolutionProperties) = preSolution
		HideSolutionNode = FALSE
	EndGlobalSection
EndGlobal
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

/// 效果生成上下文：同一积木在不同宿主（卡牌/遗物/药水/能力/怪物/事件）里可用性与表达式不同。
/// 表达式均为经过教程或反编译验证的写法；None 表示该宿主不支持此积木。
struct EffectCtx<'a> {
    /// 用于报错信息，如 "卡牌 TestCard"。
    label: String,
    /// damage（攻击）积木的目标表达式——仅卡牌（链式 FromCard(this) 只对卡牌成立）。
    attack_target: Option<&'a str>,
    /// damage 积木改用怪物链式写法（FromMonster(this)，自动选目标）。
    monster_attack: bool,
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
    /// PlayerChoiceContext 表达式：方法参数 choiceContext，
    /// 或无参数上下文里的 new ThrowingPlayerChoiceContext()（教程惯用法）。
    choice_ctx: &'a str,
    /// 事件上下文：directDamage 用教程验证的单目标重载，
    /// 并解锁 loseGold / rewardCards / rewardPotion / startCombat。
    event_style: bool,
    /// 该宿主是否有 DynamicVars（怪物没有，数值只能用固定值）。
    vars_allowed: bool,
}

/// 数值表达式：优先 var 访问器，其次字面量，最后默认 var 名（或上下文默认值）。
fn amount_of(
    ctx: &EffectCtx,
    var: &Option<String>,
    amount: &Option<i64>,
    default_var: &str,
    accessor: &str,
) -> Result<String> {
    if var.is_some() && !ctx.vars_allowed {
        bail!("{}: 该宿主没有 DynamicVars，数值请用固定值 amount", ctx.label);
    }
    Ok(match (var, amount) {
        (Some(v), _) => format!("{}{accessor}", var_accessor(v)),
        (None, Some(n)) => n.to_string(),
        (None, None) => match ctx.default_amount {
            Some(expr) => expr.to_string(),
            None => {
                if !ctx.vars_allowed {
                    bail!("{}: 该宿主没有 DynamicVars，请填固定值 amount", ctx.label);
                }
                format!("{}{accessor}", var_accessor(default_var))
            }
        },
    })
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
        Effect::Damage { var, amount } => {
            if ctx.monster_attack {
                // 怪物攻击链（教程原文写法）：自动选目标、带攻击音效与命中特效
                if var.is_some() {
                    bail!("{}: 怪物没有 DynamicVars，damage 请用固定值 amount", ctx.label);
                }
                let Some(n) = amount else {
                    bail!("{}: 怪物的 damage 积木需要固定值 amount", ctx.label);
                };
                format!(
                    "await DamageCmd.Attack({n})\n    .FromMonster(this)\n    .WithAttackerFx(null, AttackSfx)\n    .WithHitFx(\"vfx/vfx_attack_blunt\")\n    .Execute(null);"
                )
            } else {
                let Some(target) = ctx.attack_target else {
                    bail!(
                        "{}: damage（攻击）积木需要敌人目标——仅限 target 为敌人的卡牌；其他场景用 directDamage 或 custom",
                        ctx.label
                    );
                };
                let amt = match (var, amount) {
                    (Some(v), _) => format!("{}.BaseValue", var_accessor(v)),
                    (None, Some(n)) => n.to_string(),
                    (None, None) => format!("{}.BaseValue", var_accessor("Damage")),
                };
                format!(
                    "await DamageCmd.Attack({amt})\n    .FromCard(this)\n    .Targeting({target})\n    .Execute({});",
                    ctx.choice_ctx
                )
            }
        }
        Effect::Draw { var } => {
            let Some(player) = ctx.draw_player else {
                bail!("{}: 该宿主上下文中没有 Player，无法使用 draw 积木", ctx.label);
            };
            let name = var.clone().unwrap_or_else(|| "Cards".into());
            format!(
                "await CardPileCmd.Draw({}, {}.IntValue, {player});",
                ctx.choice_ctx,
                var_accessor(&name)
            )
        }
        Effect::ApplyPower { power, var, amount, to_self } => {
            let amount_expr = match (var, amount) {
                (Some(v), _) => {
                    if !ctx.vars_allowed {
                        bail!("{}: 该宿主没有 DynamicVars，applyPower 请用固定层数", ctx.label);
                    }
                    format!("{}.IntValue", var_accessor(v))
                }
                (None, Some(n)) => n.to_string(),
                (None, None) => match ctx.default_amount {
                    Some(expr) => expr.to_string(),
                    None => {
                        if !ctx.vars_allowed {
                            bail!("{}: 该宿主没有 DynamicVars，applyPower 请填固定层数", ctx.label);
                        }
                        // 未指定时默认引用与能力同名的 PowerVar
                        format!("{}.IntValue", var_accessor(power))
                    }
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
                "await PowerCmd.Apply<{power}>({}, {target}, {amount_expr}, {}, null);",
                ctx.choice_ctx, ctx.apply_source
            )
        }
        Effect::Block { var, amount } => {
            let amt = amount_of(ctx, var, amount, "Block", ".BaseValue")?;
            format!(
                "await CreatureCmd.GainBlock({}, {amt}, ValueProp.Move, null);",
                ctx.self_creature
            )
        }
        Effect::Heal { var, amount } => {
            let amt = amount_of(ctx, var, amount, "Heal", ".BaseValue")?;
            format!("await CreatureCmd.Heal({}, {amt});", ctx.self_creature)
        }
        Effect::DirectDamage { var, amount, props, to_self } => {
            if ctx.event_style {
                // 事件教程验证写法：单目标重载，直接传 DynamicVar 或字面量
                if !to_self {
                    bail!("{}: 事件里的 directDamage 只能对玩家自己（勾选 toSelf）", ctx.label);
                }
                let amt = match (var, amount) {
                    (Some(v), _) => var_accessor(v),
                    (None, Some(n)) => n.to_string(),
                    (None, None) => var_accessor("Damage"),
                };
                format!(
                    "await CreatureCmd.Damage({}, {}, {amt}, null, null);",
                    ctx.choice_ctx, ctx.self_creature
                )
            } else {
                let amt = amount_of(ctx, var, amount, "Damage", ".BaseValue")?;
                let target = if *to_self {
                    ctx.self_creature.to_string()
                } else {
                    match ctx.damage_target {
                        Some(t) => t.to_string(),
                        None => bail!("{}: directDamage 需要目标，该宿主没有目标（可勾选 toSelf 或用 custom）", ctx.label),
                    }
                };
                format!(
                    "await CreatureCmd.Damage({}, [{target}], {amt}, {}, {});",
                    ctx.choice_ctx,
                    props_expr(props, "ValueProp.Unblockable | ValueProp.Unpowered"),
                    ctx.self_creature
                )
            }
        }
        Effect::GainGold { var, amount } => {
            let Some(player) = ctx.gold_player else {
                bail!("{}: 该宿主上下文中没有 Player，无法使用 gainGold 积木", ctx.label);
            };
            let amt = amount_of(ctx, var, amount, "Gold", ".IntValue")?;
            format!("await PlayerCmd.GainGold({amt}, {player});")
        }
        Effect::LoseGold { var, amount } => {
            if !ctx.event_style {
                bail!("{}: loseGold 积木仅事件可用", ctx.label);
            }
            let amt = amount_of(ctx, var, amount, "Gold", ".BaseValue")?;
            format!("await PlayerCmd.LoseGold({amt}, Owner!, GoldLossType.Stolen);")
        }
        Effect::RewardCards { count } => {
            if !ctx.event_style {
                bail!("{}: rewardCards 积木仅事件可用", ctx.label);
            }
            format!(
                "await RewardsCmd.OfferCustom(Owner!, [new CardReward(CardCreationOptions.ForNonCombatWithDefaultOdds([Owner!.Character.CardPool]), {count}, Owner)]);"
            )
        }
        Effect::RewardPotion => {
            if !ctx.event_style {
                bail!("{}: rewardPotion 积木仅事件可用", ctx.label);
            }
            "await RewardsCmd.OfferCustom(Owner!, [new PotionReward(Owner!)]);".to_string()
        }
        Effect::StartCombat { encounter } => {
            if !ctx.event_style {
                bail!("{}: startCombat 积木仅事件可用", ctx.label);
            }
            format!(
                "EnterCombatWithoutExitingEvent<{encounter}>([], shouldResumeAfterCombat: false);"
            )
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
        monster_attack: false,
        damage_target: has_target.then_some("cardPlay.Target!"),
        draw_player: Some("Owner"),
        apply_self: "Owner.Creature",
        apply_target: has_target.then_some("cardPlay.Target!"),
        apply_source: "Owner.Creature",
        default_amount: None,
        self_creature: "Owner.Creature",
        gold_player: Some("Owner"),
        choice_ctx: "choiceContext",
        event_style: false,
        vars_allowed: true,
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
            monster_attack: false,
            damage_target: None,
            draw_player: spec.draw_player,
            apply_self: spec.apply_self,
            apply_target: spec.apply_target,
            apply_source: spec.apply_source,
            default_amount: spec.default_amount,
            self_creature: spec.self_creature,
            gold_player: spec.gold_player,
            choice_ctx: "choiceContext",
            event_style: false,
            vars_allowed: true,
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
    path.as_deref().map(ext_of).unwrap_or("png")
}

fn ext_of(path: &str) -> &str {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
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
using MegaCrit.Sts2.Core.Models.Powers;
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
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Powers;
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
        monster_attack: false,
        damage_target: has_target.then_some("target!"),
        draw_player: Some("Owner"),
        apply_self: "Owner.Creature",
        apply_target: has_target.then_some("target!"),
        apply_source: "Owner.Creature",
        default_amount: None,
        self_creature: "Owner.Creature",
        gold_player: Some("Owner"),
        choice_ctx: "choiceContext",
        event_style: false,
        vars_allowed: true,
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
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Potions;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.HoverTips;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Models.Cards;
using MegaCrit.Sts2.Core.Models.PotionPools;
using MegaCrit.Sts2.Core.Models.Powers;
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

// ---------- M4: 怪物 / 遭遇 / 事件 / 人物 ----------

/// 场景文件名：类名 → 小写蛇形（TestMonster → test_monster）。
fn scene_stem(class_name: &str) -> String {
    ids::upper_snake(class_name).to_lowercase()
}

use crate::ids::pascal_of;

/// UPPER_SNAKE → camelCase（BASIC_ATTACK → basicAttack）。
fn camel_of(key: &str) -> String {
    let p = pascal_of(key);
    let mut c = p.chars();
    match c.next() {
        Some(head) => format!("{}{}", head.to_ascii_lowercase(), c.as_str()),
        None => p,
    }
}

/// 效果方法体：出现 await 用 async 方法，否则普通方法 + Task.CompletedTask，
/// 避免 CS1998 警告。sig_head 形如 "private{空格或 async }Task Name(args)"。
fn task_method(sig: &str, args: &str, body: &str, indent_comment: &str) -> String {
    if body.contains("await ") {
        format!("{indent_comment}    private async Task {sig}({args})\n    {{\n{body}\n    }}\n")
    } else {
        format!(
            "{indent_comment}    private Task {sig}({args})\n    {{\n{body}\n        return Task.CompletedTask;\n    }}\n"
        )
    }
}

fn intent_expr(label: &str, intent: &IntentDef) -> Result<String> {
    Ok(match intent {
        IntentDef::Attack { amount } => {
            if *amount < 0 {
                bail!("{label}: 攻击意图数值不能为负");
            }
            format!("new SingleAttackIntent({amount})")
        }
        IntentDef::Defend => "new DefendIntent()".into(),
        IntentDef::Custom { code } => {
            let code = code.trim().trim_end_matches(',').to_string();
            if code.is_empty() {
                bail!("{label}: 自定义意图代码不能为空");
            }
            code
        }
    })
}

fn monster_cs(project: &Project, m: &MonsterDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &m.class_name;
    let cid = ids::content_id(id, "MONSTER", class);
    let scene_res = format!("res://{id}/scenes/{}.tscn", scene_stem(class));

    if m.moves.is_empty() && m.extra_code.is_none() {
        bail!("怪物 {class}: 至少需要一个招式（或在 extraCode 里自行重写 GenerateMoveStateMachine）");
    }

    // 状态机：按顺序循环所有招式
    let mut machine = String::new();
    let mut methods = String::new();
    if !m.moves.is_empty() {
        let mut decls = String::new();
        for mv in &m.moves {
            let label = format!("怪物 {class} 招式 {}", mv.name);
            let var = camel_of(&mv.name);
            let method = format!("{}Move", pascal_of(&mv.name));
            let mut intents = Vec::new();
            for it in &mv.intents {
                intents.push(format!("            {}", intent_expr(&label, it)?));
            }
            if intents.is_empty() {
                warnings.push(format!("{label}: 没有意图，头顶不会显示图标"));
            }
            let intents_src = if intents.is_empty() {
                String::new()
            } else {
                format!(",\n{}", intents.join(",\n"))
            };
            decls.push_str(&format!(
                "        var {var} = new MoveState(\n            \"{}\",\n            {method}{intents_src}\n        );\n",
                mv.name
            ));

            // 招式执行方法
            let ctx = EffectCtx {
                label: label.clone(),
                attack_target: None,
                monster_attack: true,
                damage_target: Some("targets[0]"),
                draw_player: None,
                apply_self: "Creature",
                apply_target: Some("targets[0]"),
                apply_source: "Creature",
                default_amount: None,
                self_creature: "Creature",
                gold_player: None,
                choice_ctx: "new ThrowingPlayerChoiceContext()",
                event_style: false,
                vars_allowed: false,
            };
            let mut body = String::new();
            if !mv.banter.is_empty() {
                body.push_str(&format!(
                    "        TalkCmd.Play(L10NMonsterLookup(\"{cid}.moves.{}.banter\"), Creature, VfxColor.Blue);\n",
                    mv.name
                ));
            }
            if mv.effects.is_empty() {
                warnings.push(format!("{label}: 没有任何效果"));
            } else {
                body.push_str(&render_effects(&ctx, &mv.effects, warnings)?);
            }
            let body = body.trim_end_matches('\n');
            methods.push_str(&format!("\n{}", task_method(
                &format!("{method}"),
                "IReadOnlyList<Creature> targets",
                body,
                &format!("    // 招式: {}\n", mv.name),
            )));
        }
        let vars: Vec<String> = m.moves.iter().map(|mv| camel_of(&mv.name)).collect();
        machine = format!(
            "\n    protected override MonsterMoveStateMachine GenerateMoveStateMachine()\n    {{\n{decls}\n        // 按顺序循环出招\n        return ModMonsterMoveStateMachines.Cycle({});\n    }}\n",
            vars.join(", ")
        );
    }

    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Models.Powers;
using MegaCrit.Sts2.Core.MonsterMoves.Intents;
using MegaCrit.Sts2.Core.MonsterMoves.MonsterMoveStateMachine;
using MegaCrit.Sts2.Core.Nodes.Combat;
using MegaCrit.Sts2.Core.Nodes.Vfx;
using MegaCrit.Sts2.Core.ValueProps;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;
using STS2RitsuLib.Scaffolding.Godot;
using STS2RitsuLib.Scaffolding.MonsterMoves;

namespace {ns}.Monsters;

[RegisterMonster]
public class {class} : ModMonsterTemplate
{{
    // 初始血量区间（游戏在区间内随机取值）
    public override int MinInitialHp => {min};
    public override int MaxInitialHp => {max};

    // 怪物场景
    public override MonsterAssetProfile AssetProfile => new(
        VisualsScenePath: "{scene_res}"
    );

    // 自动转换怪物场景，不需要手动挂脚本（RitsuLib 惯用法）
    protected override NCreatureVisuals? TryCreateCreatureVisuals() =>
        RitsuGodotNodeFactories.CreateFromScenePath<NCreatureVisuals>(AssetProfile.VisualsScenePath!);
{machine}{methods}{extra}}}
"#,
        min = m.min_hp,
        max = m.max_hp,
        extra = render_extra_code(&m.extra_code),
    ))
}

/// 遭遇的最终槽位表：用户填的名字优先，否则 m1/m2/…。单怪不需要槽位。
fn encounter_slots(enc: &EncounterDef) -> Vec<String> {
    enc.monsters
        .iter()
        .enumerate()
        .map(|(i, m)| m.slot.clone().unwrap_or_else(|| format!("m{}", i + 1)))
        .collect()
}

fn encounter_cs(project: &Project, enc: &EncounterDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &enc.class_name;
    let multi = enc.monsters.len() > 1;

    let registrations = if enc.acts.is_empty() {
        warnings.push(format!(
            "遭遇 {class}: 未指定幕，将注册为全局遭遇（依赖 IsValidForAct / 控制台 fight 进入）"
        ));
        "[RegisterGlobalEncounter]".to_string()
    } else {
        enc.acts
            .iter()
            .map(|a| format!("[RegisterActEncounter(typeof({a}))]"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut uniq = Vec::new();
    for m in &enc.monsters {
        if !uniq.contains(&m.monster) {
            uniq.push(m.monster.clone());
        }
    }
    let all_possible = uniq
        .iter()
        .map(|m| format!("ModelDb.Monster<{m}>()"))
        .collect::<Vec<_>>()
        .join(", ");

    let scene_block = if multi {
        let slots = encounter_slots(enc);
        let slot_list = slots.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
        format!(
            "\n    // 多怪物遭遇场景（Marker2D 标注每个怪站哪）\n    public override EncounterAssetProfile AssetProfile => new(\n        EncounterScenePath: \"res://{id}/scenes/{}.tscn\"\n    );\n\n    // 怪物槽位名\n    public override IReadOnlyList<string> Slots => [{slot_list}];\n",
            scene_stem(class)
        )
    } else {
        String::new()
    };

    let camera = match enc.camera_scaling {
        Some(s) => format!(
            "\n    // 场景太大时调整缩放\n    public override float GetCameraScaling() => {s}f;\n"
        ),
        None => String::new(),
    };

    let gen_monsters = enc
        .monsters
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let slot = if multi {
                format!("\"{}\"", m.slot.clone().unwrap_or_else(|| format!("m{}", i + 1)))
            } else {
                "null".to_string()
            };
            format!("        (ModelDb.Monster<{}>().ToMutable(), {slot})", m.monster)
        })
        .collect::<Vec<_>>()
        .join(",\n");

    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Acts;
using MegaCrit.Sts2.Core.Rooms;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;
using {ns}.Monsters;

namespace {ns}.Encounters;

{registrations}
public class {class} : ModEncounterTemplate
{{
    // 该遭遇可能出现的所有怪物（图鉴等用途）
    public override IEnumerable<MonsterModel> AllPossibleMonsters => [{all_possible}];

    // 房间类型
    public override RoomType RoomType => RoomType.{room};

    // 是否属于弱怪池（前几场战斗）
    public override bool IsWeak => {weak};
{scene_block}{camera}
    // 实际生成的怪物（ToMutable 表示战斗中的可变数据）
    protected override IReadOnlyList<(MonsterModel, string?)> GenerateMonsters() => [
{gen_monsters}
    ];
{extra}}}
"#,
        room = enc.room_type,
        weak = enc.is_weak,
        extra = render_extra_code(&enc.extra_code),
    ))
}

fn event_cs(project: &Project, ev: &EventDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &ev.class_name;
    let ext = asset_ext(&ev.image);

    let registrations = if ev.acts.is_empty() {
        "[RegisterSharedEvent]".to_string()
    } else {
        ev.acts
            .iter()
            .map(|a| format!("[RegisterActEvent(typeof({a}))]"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let asset_block = if ev.image.is_some() {
        format!(
            "\n    // 事件立绘\n    public override EventAssetProfile AssetProfile => new(\n        InitialPortraitPath: \"res://{id}/images/events/{class}.{ext}\"\n    );\n"
        )
    } else {
        String::new()
    };

    let vars_block = render_canonical_vars(&ev.vars)?;

    let condition_block = match &ev.condition {
        Some(cond) if !cond.trim().is_empty() => format!(
            "\n    // 出现条件\n    public override bool IsAllowed(IRunState runState) => {};\n",
            cond.trim()
        ),
        _ => String::new(),
    };

    // 战斗事件：任一选项含 startCombat 时需要 LayoutType/CanonicalEncounter
    let mut combat_encounter: Option<String> = None;
    for p in &ev.pages {
        for o in &p.options {
            walk_effects(&o.effects, &mut |e| {
                if let Effect::StartCombat { encounter } = e {
                    if combat_encounter.is_none() {
                        combat_encounter = Some(encounter.clone());
                    }
                }
                Ok(())
            })?;
        }
    }
    let combat_block = match &combat_encounter {
        Some(enc) => format!(
            "\n    // 战斗事件\n    public override EventLayoutType LayoutType => EventLayoutType.Combat;\n    public override EncounterModel CanonicalEncounter => ModelDb.Encounter<{enc}>();\n"
        ),
        None => String::new(),
    };
    let encounters_using = if combat_encounter.is_some() {
        format!("using {ns}.Encounters;\n")
    } else {
        String::new()
    };

    // 选项方法名：INITIAL 页直接用选项名，其余页加页名前缀防撞
    let method_name = |page: &str, opt: &str| -> String {
        if page == "INITIAL" {
            pascal_of(opt)
        } else {
            format!("{}{}", pascal_of(page), pascal_of(opt))
        }
    };
    {
        let mut seen = std::collections::HashSet::new();
        for p in &ev.pages {
            for o in &p.options {
                if !seen.insert(method_name(&p.key, &o.key)) {
                    bail!(
                        "事件 {class}: 选项 {}.{} 生成的方法名与其他选项冲突，请改名",
                        p.key, o.key
                    );
                }
            }
        }
    }

    let option_list = |page_key: &str, options: &[crate::model::EventOptionDef]| -> String {
        options
            .iter()
            .map(|o| {
                let key_expr = if page_key == "INITIAL" {
                    format!("InitialOptionKey(\"{}\")", o.key)
                } else {
                    format!("ModOptionKey(\"{page_key}\", \"{}\")", o.key)
                };
                format!("        new EventOption(this, {}, {key_expr})", method_name(page_key, &o.key))
            })
            .collect::<Vec<_>>()
            .join(",\n")
    };

    // 初始选项
    let initial = &ev.pages[0];
    let initial_block = format!(
        "\n    // 初始页选项\n    protected override IReadOnlyList<EventOption> GenerateInitialOptions() =>\n    [\n{}\n    ];\n",
        option_list("INITIAL", &initial.options)
    );

    let ctx_for = |label: String| EffectCtx {
        label,
        attack_target: None,
        monster_attack: false,
        damage_target: None,
        draw_player: None,
        apply_self: "Owner!.Creature",
        apply_target: None,
        apply_source: "Owner!.Creature",
        default_amount: None,
        self_creature: "Owner!.Creature",
        gold_player: Some("Owner!"),
        choice_ctx: "new ThrowingPlayerChoiceContext()",
        event_style: true,
        vars_allowed: true,
    };

    // 选项方法 + 非初始页的 ShowPage 方法
    let mut methods = String::new();
    for p in &ev.pages {
        if p.key != "INITIAL" && !p.options.is_empty() {
            methods.push_str(&format!(
                "\n    // 页面: {key}\n    private void ShowPage{pascal}()\n    {{\n        SetEventState(PageDescription(\"{key}\"), [\n{opts}\n        ]);\n    }}\n",
                key = p.key,
                pascal = pascal_of(&p.key),
                opts = option_list(&p.key, &p.options)
                    .lines()
                    .map(|l| format!("    {l}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }
        for o in &p.options {
            let label = format!("事件 {class} 选项 {}.{}", p.key, o.key);
            let ctx = ctx_for(label.clone());
            let mut body = String::new();
            if !o.effects.is_empty() {
                body.push_str(&render_effects(&ctx, &o.effects, warnings)?);
                body.push('\n');
            }
            let mut went_combat = false;
            walk_effects(&o.effects, &mut |e| {
                if matches!(e, Effect::StartCombat { .. }) {
                    went_combat = true;
                }
                Ok(())
            })?;
            match &o.goto {
                Some(g) => {
                    let target = ev.pages.iter().find(|pp| &pp.key == g).unwrap();
                    if target.options.is_empty() {
                        body.push_str(&format!("        SetEventFinished(PageDescription(\"{g}\"));"));
                    } else {
                        body.push_str(&format!("        ShowPage{}();", pascal_of(g)));
                    }
                }
                None => {
                    debug_assert!(went_combat, "validate 已保证无 goto 必有 startCombat");
                }
            }
            let body = body.trim_end_matches('\n').to_string();
            methods.push_str(&format!("\n{}", task_method(
                &method_name(&p.key, &o.key),
                "",
                &body,
                &format!("    // 选项: {}.{}\n", p.key, o.key),
            )));
        }
    }

    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Gold;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Acts;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.ValueProps;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;
{encounters_using}
namespace {ns}.Events;

{registrations}
public sealed class {class} : ModEventTemplate
{{{asset_block}{vars_block}{condition_block}{combat_block}{initial_block}{methods}{extra}}}
"#,
        extra = render_extra_code(&ev.extra_code),
    ))
}

/// 人物主题色的浮点表达式。
fn color_expr(color: &str) -> String {
    let (r, g, b) = parse_hex_color(color).expect("validate 已校验颜色格式");
    format!("new({r:.3}f, {g:.3}f, {b:.3}f)")
}

/// 卡池 EnergyColorName：全局唯一的小写标识。
fn energy_color_name(project: &Project, ch: &CharacterDef) -> String {
    format!(
        "{}_{}",
        ids::mod_prefix(&project.manifest.id).to_lowercase(),
        ids::upper_snake(&ch.class_name).to_lowercase()
    )
}

fn character_pools_cs(project: &Project, ch: &CharacterDef) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &ch.class_name;
    let energy = energy_color_name(project, ch);
    let color = color_expr(&ch.color);
    let (r, g, b) = parse_hex_color(&ch.color).unwrap();

    let mut icons = String::new();
    if ch.energy_icon.is_some() {
        let ext = asset_ext(&ch.energy_icon);
        icons.push_str(&format!(
            "\n    // 描述内联能量图标（24x24）\n    public override string? TextEnergyIconPath => \"res://{id}/images/characters/{class}Energy.{ext}\";"
        ));
    }
    if ch.energy_icon_big.is_some() {
        let ext = asset_ext(&ch.energy_icon_big);
        icons.push_str(&format!(
            "\n    // 悬浮提示与卡牌角标能量图标（74x74）\n    public override string? BigEnergyIconPath => \"res://{id}/images/characters/{class}EnergyBig.{ext}\";"
        ));
    }

    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using Godot;
using STS2RitsuLib.Scaffolding.Content;
using STS2RitsuLib.Utils;

namespace {ns}.Characters;

// 人物专属卡池：卡牌用 [RegisterCard(typeof({class}CardPool))] 加入
public class {class}CardPool : TypeListCardPoolModel
{{
    public override string Title => "{energy}";
    public override string EnergyColorName => "{energy}";{icons}

    // 卡池主题色
    public override Color DeckEntryCardColor => {color};
    public override Color EnergyOutlineColor => {color};

    // 原版卡框换色（自定义卡框请改用 CreateUnmodulatedHsvShaderMaterial）
    private static readonly Material? _frameMaterial =
        MaterialUtils.CreateReplaceHueShaderMaterial({r:.3}f, {g:.3}f, {b:.3}f);
    public override Material? PoolFrameMaterial => _frameMaterial;

    public override bool IsColorless => false;
}}

// 人物专属遗物池：遗物用 [RegisterRelic(typeof({class}RelicPool))] 加入
public class {class}RelicPool : TypeListRelicPoolModel
{{
    public override string EnergyColorName => "{energy}";{icons}
}}

// 人物专属药水池：药水用 [RegisterPotion(typeof({class}PotionPool))] 加入
public class {class}PotionPool : TypeListPotionPoolModel
{{
    public override string EnergyColorName => "{energy}";{icons}
}}
"#
    ))
}

fn character_cs(project: &Project, ch: &CharacterDef, warnings: &mut Vec<String>) -> Result<String> {
    let id = &project.manifest.id;
    let ns = project.namespace();
    let class = &ch.class_name;
    let color = color_expr(&ch.color);
    let stem = scene_stem(class);

    // Scenes 子集
    let mut scene_args = Vec::new();
    if ch.combat_image.is_some() {
        scene_args.push(format!("VisualsPath: \"res://{id}/scenes/{stem}.tscn\""));
    }
    if ch.energy_icon_big.is_some() {
        scene_args.push(format!("EnergyCounterPath: \"res://{id}/scenes/{stem}_energy.tscn\""));
    }
    // Ui 子集（选人背景始终生成：纯色场景无外部依赖）
    let mut ui_args = Vec::new();
    if let Some(p) = &ch.portrait {
        let ext = ext_of(p);
        ui_args.push(format!("IconTexturePath: \"res://{id}/images/characters/{class}Portrait.{ext}\""));
        ui_args.push(format!("IconPath: \"res://{id}/scenes/{stem}_icon.tscn\""));
    }
    ui_args.push(format!("CharacterSelectBgPath: \"res://{id}/scenes/{stem}_bg.tscn\""));
    if let Some(p) = &ch.select_icon {
        let ext = ext_of(p);
        ui_args.push(format!("CharacterSelectIconPath: \"res://{id}/images/characters/{class}Select.{ext}\""));
    }
    if let Some(p) = &ch.select_icon_locked {
        let ext = ext_of(p);
        ui_args.push(format!(
            "CharacterSelectLockedIconPath: \"res://{id}/images/characters/{class}SelectLocked.{ext}\""
        ));
    }
    if let Some(p) = &ch.map_marker {
        let ext = ext_of(p);
        ui_args.push(format!("MapMarkerPath: \"res://{id}/images/characters/{class}Marker.{ext}\""));
    }

    let mut profile_args = Vec::new();
    if !scene_args.is_empty() {
        profile_args.push(format!(
            "            Scenes: new(\n                {}\n            )",
            scene_args.join(",\n                ")
        ));
    }
    profile_args.push(format!(
        "            Ui: new(\n                {}\n            )",
        ui_args.join(",\n                ")
    ));

    let visuals_override = if ch.combat_image.is_some() {
        "\n    // 自动转换人物场景，不需要手动挂脚本（RitsuLib 惯用法）\n    protected override NCreatureVisuals? TryCreateCreatureVisuals() =>\n        RitsuGodotNodeFactories.CreateFromScenePath<NCreatureVisuals>(AssetProfile.Scenes!.VisualsPath!);\n".to_string()
    } else {
        String::new()
    };

    let deck_block = if ch.starting_deck.is_empty() {
        warnings.push(format!("人物 {class}: 初始卡组为空，进游戏后没有手牌来源"));
        String::new()
    } else {
        let entries = ch
            .starting_deck
            .iter()
            .map(|sc| format!("        new(typeof({}), {})", sc.card, sc.count))
            .collect::<Vec<_>>()
            .join(",\n");
        format!(
            "\n    // 初始卡组\n    protected override IEnumerable<StartingDeckEntry> StartingDeckEntries => [\n{entries}\n    ];\n"
        )
    };

    let relics_block = if ch.starting_relics.is_empty() {
        String::new()
    } else {
        let entries = ch
            .starting_relics
            .iter()
            .map(|r| format!("        typeof({r})"))
            .collect::<Vec<_>>()
            .join(",\n");
        format!(
            "\n    // 初始遗物\n    protected override IEnumerable<Type> StartingRelicTypes => [\n{entries}\n    ];\n"
        )
    };

    let cards_using = if ch.starting_deck.is_empty() {
        String::new()
    } else {
        format!("using {ns}.Cards;\n")
    };
    let relics_using = if ch.starting_relics.is_empty() {
        String::new()
    } else {
        format!("using {ns}.Relics;\n")
    };

    Ok(format!(
        r#"// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using Godot;
using MegaCrit.Sts2.Core.Entities.Characters;
using MegaCrit.Sts2.Core.Nodes.Combat;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Characters;
using STS2RitsuLib.Scaffolding.Godot;
{cards_using}{relics_using}
namespace {ns}.Characters;

[RegisterCharacter]
public class {class} : ModCharacterTemplate<{class}CardPool, {class}RelicPool, {class}PotionPool>
{{
    // 主题色：角色名 / 能量轮廓 / 地图绘制
    public override Color NameColor => {color};
    public override Color EnergyLabelOutlineColor => {color};
    public override Color MapDrawingColor => {color};

    // 性别（本地化人称）
    public override CharacterGender Gender => CharacterGender.{gender};

    // 初始血量与金币
    public override int StartingHp => {hp};
    public override int StartingGold => {gold};

    // 资源清单：未提供的项自动回退到原版 {base}
    public override CharacterAssetProfile AssetProfile => CharacterAssetProfiles.Merge(
        CharacterAssetProfiles.{base}(),
        new(
{profile_args}
        ));

    // 攻击 / 施法动画延迟（对齐动画用）
    public override float AttackAnimDelay => 0f;
    public override float CastAnimDelay => 0f;

    // 不要求时间线小故事
    public override bool RequiresEpochAndTimeline => false;
{visuals_override}{deck_block}{relics_block}
    // 攻击建筑师时的特效池
    public override List<string> GetArchitectAttackVfx() => [
        "vfx/vfx_attack_blunt",
        "vfx/vfx_heavy_blunt",
        "vfx/vfx_attack_slash",
        "vfx/vfx_bloody_impact",
        "vfx/vfx_rock_shatter"
    ];
{extra}}}
"#,
        gender = ch.gender,
        hp = ch.starting_hp,
        gold = ch.starting_gold,
        base = ch.base,
        profile_args = profile_args.join(",\n"),
        extra = render_extra_code(&ch.extra_code),
    ))
}

/// 战斗生物场景（怪物 / 人物通用内置模板，教程附赠资源的结构）。
fn creature_scene_tscn(class_name: &str, texture_res: Option<String>) -> String {
    let (head, visuals) = match &texture_res {
        Some(res) => (
            format!(
                "[gd_scene load_steps=2 format=3]\n\n[ext_resource type=\"Texture2D\" path=\"{res}\" id=\"1_img\"]\n"
            ),
            "[node name=\"Visuals\" type=\"Sprite2D\" parent=\".\"]\nunique_name_in_owner = true\nposition = Vector2(0, -73)\ntexture = ExtResource(\"1_img\")\n".to_string(),
        ),
        None => (
            "[gd_scene format=3]\n".to_string(),
            "[node name=\"Visuals\" type=\"Node2D\" parent=\".\"]\nunique_name_in_owner = true\nposition = Vector2(0, -73)\n".to_string(),
        ),
    };
    format!(
        r#"{head}
[node name="{class_name}" type="Node2D"]

{visuals}
[node name="Bounds" type="Control" parent="."]
unique_name_in_owner = true
layout_mode = 3
anchors_preset = 0
offset_left = -70.0
offset_top = -140.0
offset_right = 70.0

[node name="IntentPos" type="Marker2D" parent="."]
unique_name_in_owner = true
position = Vector2(0, -159)

[node name="CenterPos" type="Marker2D" parent="."]
unique_name_in_owner = true
position = Vector2(0, -72)

[node name="TalkPos" type="Marker2D" parent="."]
unique_name_in_owner = true
position = Vector2(0, -144)
"#
    )
}

/// 多怪遭遇场景：每个槽位一个 Marker2D，最多 4 个一排（参考教程附赠布局）。
fn encounter_scene_tscn(class_name: &str, slots: &[String]) -> String {
    let mut nodes = String::new();
    for (i, slot) in slots.iter().enumerate() {
        let row = i / 4;
        let col = i % 4;
        let x = 880 + 295 * col;
        let y: i64 = 697 - 329 * row as i64;
        nodes.push_str(&format!(
            "\n[node name=\"{slot}\" type=\"Marker2D\" parent=\".\"]\nposition = Vector2({x}, {y})\n"
        ));
    }
    format!(
        r#"[gd_scene format=3]

[node name="{class_name}" type="Control"]
layout_mode = 3
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
grow_horizontal = 2
grow_vertical = 2
mouse_filter = 2
{nodes}"#
    )
}

/// 人物相关的生成场景：战斗模型 / 能量表盘 / 头像 / 选人背景。
fn character_scenes(project: &Project, ch: &CharacterDef) -> Vec<GeneratedFile> {
    let id = &project.manifest.id;
    let class = &ch.class_name;
    let stem = scene_stem(class);
    let (r, g, b) = parse_hex_color(&ch.color).unwrap();
    let mut files = Vec::new();

    if let Some(img) = &ch.combat_image {
        let ext = ext_of(img);
        files.push(GeneratedFile {
            rel_path: format!("{id}/scenes/{stem}.tscn").into(),
            content: creature_scene_tscn(
                class,
                Some(format!("res://{id}/images/characters/{class}Combat.{ext}")),
            ),
        });
    }

    if let Some(img) = &ch.energy_icon_big {
        let ext = ext_of(img);
        files.push(GeneratedFile {
            rel_path: format!("{id}/scenes/{stem}_energy.tscn").into(),
            content: format!(
                r#"[gd_scene load_steps=2 format=3]

[ext_resource type="Texture2D" path="res://{id}/images/characters/{class}EnergyBig.{ext}" id="1_energy"]

[node name="{class}EnergyCounter" type="Control"]
layout_mode = 3
anchors_preset = 0
offset_right = 128.0
offset_bottom = 128.0

[node name="EnergyVfxBack" type="Node2D" parent="."]
unique_name_in_owner = true
position = Vector2(64, 64)

[node name="Layers" type="Control" parent="."]
unique_name_in_owner = true
layout_mode = 1
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
grow_horizontal = 2
grow_vertical = 2
mouse_filter = 2

[node name="RotationLayers" type="Control" parent="Layers"]
unique_name_in_owner = true
anchors_preset = 0
offset_right = 40.0
offset_bottom = 40.0
mouse_filter = 2

[node name="Layer1" type="TextureRect" parent="Layers"]
layout_mode = 1
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
grow_horizontal = 2
grow_vertical = 2
texture = ExtResource("1_energy")
expand_mode = 1

[node name="EnergyVfxFront" type="Node2D" parent="."]
unique_name_in_owner = true
position = Vector2(64, 64)

[node name="Label" type="Label" parent="."]
layout_mode = 1
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
offset_left = 16.0
offset_top = -29.0
offset_right = -16.0
offset_bottom = 29.0
grow_horizontal = 2
grow_vertical = 2
theme_override_colors/font_color = Color(1, 0.9647059, 0.8862745, 1)
theme_override_colors/font_shadow_color = Color(0, 0, 0, 0.1882353)
theme_override_colors/font_outline_color = Color({r:.4}, {g:.4}, {b:.4}, 1)
theme_override_constants/shadow_offset_x = 3
theme_override_constants/shadow_offset_y = 2
theme_override_constants/outline_size = 16
theme_override_constants/shadow_outline_size = 16
theme_override_font_sizes/font_size = 36
text = "3/3"
horizontal_alignment = 1
vertical_alignment = 1
"#
            ),
        });
    }

    if let Some(img) = &ch.portrait {
        let ext = ext_of(img);
        files.push(GeneratedFile {
            rel_path: format!("{id}/scenes/{stem}_icon.tscn").into(),
            content: format!(
                r#"[gd_scene load_steps=2 format=3]

[ext_resource type="Texture2D" path="res://{id}/images/characters/{class}Portrait.{ext}" id="1_icon"]

[node name="{class}Icon" type="TextureRect"]
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
grow_horizontal = 2
grow_vertical = 2
mouse_filter = 2
texture = ExtResource("1_icon")
expand_mode = 1
stretch_mode = 5
"#
            ),
        });
    }

    // 选人背景：主题色纯色场景（无外部资源依赖，始终生成）
    files.push(GeneratedFile {
        rel_path: format!("{id}/scenes/{stem}_bg.tscn").into(),
        content: format!(
            r#"[gd_scene format=3]

[node name="{class}Bg" type="Control"]
layout_mode = 3
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
grow_horizontal = 2
grow_vertical = 2
mouse_filter = 2

[node name="ColorRect" type="ColorRect" parent="."]
layout_mode = 1
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
grow_horizontal = 2
grow_vertical = 2
color = Color({dr:.4}, {dg:.4}, {db:.4}, 1)
"#,
            dr = r * 0.35,
            dg = g * 0.35,
            db = b * 0.35,
        ),
    });

    files
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

    for m in &project.monsters {
        let cid = ids::content_id(mod_id, "MONSTER", &m.class_name);
        for (lang, text) in &m.text {
            let map = buckets.entry((lang.clone(), "monsters.json")).or_default();
            map.insert(format!("{cid}.name"), json!(text.name));
        }
        for mv in &m.moves {
            for (lang, title) in &mv.title {
                let map = buckets.entry((lang.clone(), "monsters.json")).or_default();
                map.insert(format!("{cid}.moves.{}.title", mv.name), json!(title));
            }
            for (lang, banter) in &mv.banter {
                if banter.is_empty() {
                    continue;
                }
                let map = buckets.entry((lang.clone(), "monsters.json")).or_default();
                map.insert(format!("{cid}.moves.{}.banter", mv.name), json!(banter));
            }
        }
    }
    for e in &project.encounters {
        let cid = ids::content_id(mod_id, "ENCOUNTER", &e.class_name);
        for (lang, text) in &e.text {
            let map = buckets.entry((lang.clone(), "encounters.json")).or_default();
            map.insert(format!("{cid}.title"), json!(text.title));
            if !text.loss.is_empty() {
                map.insert(format!("{cid}.loss"), json!(text.loss));
            }
        }
    }
    for ev in &project.events {
        let cid = ids::content_id(mod_id, "EVENT", &ev.class_name);
        for (lang, title) in &ev.title {
            let map = buckets.entry((lang.clone(), "events.json")).or_default();
            map.insert(format!("{cid}.title"), json!(title));
        }
        for p in &ev.pages {
            for (lang, desc) in &p.description {
                let map = buckets.entry((lang.clone(), "events.json")).or_default();
                map.insert(format!("{cid}.pages.{}.description", p.key), json!(desc));
            }
            for o in &p.options {
                for (lang, title) in &o.title {
                    let map = buckets.entry((lang.clone(), "events.json")).or_default();
                    map.insert(format!("{cid}.pages.{}.options.{}.title", p.key, o.key), json!(title));
                }
                for (lang, desc) in &o.description {
                    if desc.is_empty() {
                        continue;
                    }
                    let map = buckets.entry((lang.clone(), "events.json")).or_default();
                    map.insert(
                        format!("{cid}.pages.{}.options.{}.description", p.key, o.key),
                        json!(desc),
                    );
                }
            }
        }
    }
    for ch in &project.characters {
        let cid = ids::content_id(mod_id, "CHARACTER", &ch.class_name);
        for (lang, text) in &ch.text {
            let zhs = lang == "zhs";
            let map = buckets.entry((lang.clone(), "characters.json")).or_default();
            let title = &text.title;
            map.insert(format!("{cid}.title"), json!(title));
            map.insert(format!("{cid}.titleObject"), json!(title));
            map.insert(format!("{cid}.description"), json!(text.description));
            map.insert(
                format!("{cid}.cardsModifierTitle"),
                json!(if zhs { format!("{title}卡牌") } else { format!("{title} Cards") }),
            );
            map.insert(
                format!("{cid}.cardsModifierDescription"),
                json!(if zhs {
                    format!("{title}的卡牌现在会出现在奖励和商店中。")
                } else {
                    format!("{title}'s cards now appear in rewards and shops.")
                }),
            );
            // 人称代词按性别生成默认值
            let (subj, obj, poss) = match (zhs, ch.gender.as_str()) {
                (true, "Masculine") => ("他", "他", "他的"),
                (true, "Feminine") => ("她", "她", "她的"),
                (true, _) => ("TA", "TA", "TA的"),
                (false, "Masculine") => ("he", "him", "his"),
                (false, "Feminine") => ("she", "her", "her"),
                (false, _) => ("they", "them", "their"),
            };
            map.insert(format!("{cid}.pronounSubject"), json!(subj));
            map.insert(format!("{cid}.pronounObject"), json!(obj));
            map.insert(format!("{cid}.pronounPossessive"), json!(poss));
            map.insert(format!("{cid}.possessiveAdjective"), json!(poss));
            let filler = if zhs { "……" } else { "..." };
            map.insert(format!("{cid}.aromaPrinciple"), json!(filler));
            map.insert(format!("{cid}.banter.alive.endTurnPing"), json!(filler));
            map.insert(format!("{cid}.banter.dead.endTurnPing"), json!(filler));
            map.insert(format!("{cid}.eventDeathPrevention"), json!(filler));
            map.insert(format!("{cid}.goldMonologue"), json!(filler));
            map.insert(
                format!("{cid}.unlockText"),
                json!(if zhs {
                    "用[pink]{Prerequisite}[/pink]进行一局游戏来解锁这个角色。"
                } else {
                    "Complete a run with [pink]{Prerequisite}[/pink] to unlock this character."
                }),
            );

            // 先古对话占位（缺键可能导致游戏内文本报错，先补全，供后续手动润色）
            let amap = buckets.entry((lang.clone(), "ancients.json")).or_default();
            let next = if zhs { "继续" } else { "Continue" };
            const ANCIENTS: &[&str] =
                &["DARV", "NEOW", "NONUPEIPE", "OROBAS", "PAEL", "TANX", "TEZCATARA", "VAKUU"];
            for a in ANCIENTS {
                for (suffix, val) in [
                    ("0-0.char", filler),
                    ("0-0.next", next),
                    ("0-1.ancient", filler),
                    ("1-0r.ancient", filler),
                    ("2-0.ancient", filler),
                    ("2-0.next", next),
                    ("2-1.char", filler),
                    ("2-1.next", next),
                    ("2-2.ancient", filler),
                ] {
                    amap.insert(format!("{a}.talk.{cid}.{suffix}"), json!(val));
                }
            }
            let class_stem = ids::upper_snake(&ch.class_name);
            for n in 0..3 {
                for (suffix, val) in [
                    (format!("{n}-0r.ancient"), filler),
                    (format!("{n}-0r.next"), next),
                    (format!("{n}-1r.char"), filler),
                    (format!("{n}-1r.next"), next),
                    (format!("{n}-2r.ancient"), filler),
                ] {
                    amap.insert(format!("THE_ARCHITECT.talk.{cid}.{suffix}"), json!(val));
                }
                amap.insert(format!("THE_ARCHITECT.talk.{class_stem}.{n}-attack"), json!("Both"));
            }
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
