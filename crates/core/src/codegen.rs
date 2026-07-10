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

    for card in &project.cards {
        files.push(GeneratedFile {
            rel_path: format!("Scripts/Cards/{}.cs", card.class_name).into(),
            content: card_cs(project, card, &mut warnings)?,
        });
        if let Some(src) = &card.portrait {
            let ext = std::path::Path::new(src)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            copies.push(AssetCopy {
                src_rel: src.clone(),
                dst_rel: format!("{id}/images/cards/{}.{ext}", card.class_name).into(),
            });
        } else {
            warnings.push(format!(
                "卡牌 {} 未设置卡图，游戏内将显示 RitsuLib 占位图（预期路径 assets/cards/{}.png）",
                card.class_name, card.class_name
            ));
        }
    }

    for (lang, content) in localization_files(project)? {
        files.push(GeneratedFile {
            rel_path: format!("{id}/localization/{lang}/cards.json").into(),
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

fn effect_code(card: &CardDef, effect: &Effect, warnings: &mut Vec<String>) -> String {
    match effect {
        Effect::Damage { var } => {
            let name = var.clone().unwrap_or_else(|| "Damage".into());
            if matches!(card.target.as_str(), "None" | "Self") {
                warnings.push(format!(
                    "卡牌 {}: damage 效果需要敌人目标，但 target 是 {}",
                    card.class_name, card.target
                ));
            }
            format!(
                "await DamageCmd.Attack({}.BaseValue)\n    .FromCard(this)\n    .Targeting(cardPlay.Target!)\n    .Execute(choiceContext);",
                var_accessor(&name)
            )
        }
        Effect::Draw { var } => {
            let name = var.clone().unwrap_or_else(|| "Cards".into());
            format!(
                "await CardPileCmd.Draw(choiceContext, {}.IntValue, Owner);",
                var_accessor(&name)
            )
        }
        Effect::ApplyPower { power, var, amount, to_self } => {
            let amount_expr = match (var, amount) {
                (Some(v), _) => format!("{}.IntValue", var_accessor(v)),
                (None, Some(n)) => n.to_string(),
                // 未指定时默认引用与能力同名的 PowerVar
                (None, None) => format!("{}.IntValue", var_accessor(power)),
            };
            let target = if *to_self { "Owner".to_string() } else { "cardPlay.Target!".to_string() };
            format!("await PowerCmd.Apply<{power}>(choiceContext, {target}, {amount_expr}, Owner, null);")
        }
        Effect::Custom { code } => code.trim_end().to_string(),
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

    let on_play = if card.on_play.is_empty() {
        if card.card_type == "Attack" {
            warnings.push(format!("卡牌 {class}: 攻击牌没有任何打出效果", ));
        }
        String::new()
    } else {
        let body = card
            .on_play
            .iter()
            .map(|e| {
                let code = effect_code(card, e, warnings);
                code.lines()
                    .map(|l| if l.is_empty() { l.to_string() } else { format!("        {l}") })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n");
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
{on_play}{on_upgrade}}}
"#,
        cost = card.energy_cost,
        card_type = card.card_type,
        rarity = card.rarity,
        target = card.target,
        show = card.show_in_library,
    ))
}

/// 每种语言一个 cards.json：{ID}.title / {ID}.description。
fn localization_files(project: &Project) -> Result<Vec<(String, String)>> {
    let mut langs: BTreeMap<String, serde_json::Map<String, serde_json::Value>> = BTreeMap::new();
    for card in &project.cards {
        let content_id = ids::content_id(&project.manifest.id, "CARD", &card.class_name);
        for (lang, text) in &card.text {
            let map = langs.entry(lang.clone()).or_default();
            map.insert(format!("{content_id}.title"), json!(text.title));
            map.insert(format!("{content_id}.description"), json!(text.description));
        }
    }
    langs
        .into_iter()
        .map(|(lang, map)| {
            Ok((lang, serde_json::to_string_pretty(&serde_json::Value::Object(map))? + "\n"))
        })
        .collect()
}
