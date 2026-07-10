//! 代码生成快照测试：确保生成的关键文件符合教程约定的结构。

use sts2mod_core::codegen;
use sts2mod_core::model::{self, CardText, Effect, VarDef};

fn file<'a>(out: &'a codegen::GenOutput, path: &str) -> &'a str {
    &out
        .files
        .iter()
        .find(|f| f.rel_path.to_string_lossy() == path)
        .unwrap_or_else(|| panic!("缺少生成文件 {path}"))
        .content
}

#[test]
fn starter_project_generates_expected_tree() {
    let project = model::starter_project("Test", "测试Mod");
    let out = codegen::generate(&project).unwrap();

    let paths: Vec<String> = out.files.iter().map(|f| f.rel_path.to_string_lossy().into_owned()).collect();
    for expected in [
        "Test.json",
        "Test.csproj",
        "project.godot",
        "export_presets.cfg",
        "Scripts/Entry.cs",
        "Scripts/Cards/SampleStrike.cs",
        "Test/localization/zhs/cards.json",
        "Test/localization/en/cards.json",
    ] {
        assert!(paths.contains(&expected.to_string()), "缺少 {expected}，实际: {paths:?}");
    }

    // 游戏清单：snake_case 字段 + 依赖
    let manifest = file(&out, "Test.json");
    assert!(manifest.contains("\"min_game_version\": \"0.107.1\""));
    assert!(manifest.contains("\"has_pck\": true"));
    assert!(manifest.contains("\"STS2-RitsuLib\""));

    // 卡牌类：注册、基类、数值、效果、升级
    let card = file(&out, "Scripts/Cards/SampleStrike.cs");
    assert!(card.contains("[RegisterCard(typeof(ColorlessCardPool))]"));
    assert!(card.contains("public class SampleStrike : ModCardTemplate"));
    assert!(card.contains("new DamageVar(9, ValueProp.Move)"));
    assert!(card.contains("await DamageCmd.Attack(DynamicVars.Damage.BaseValue)"));
    assert!(card.contains(".Targeting(cardPlay.Target!)"));
    assert!(card.contains("DynamicVars.Damage.UpgradeValueBy(3);"));
    assert!(card.contains("PortraitPath: \"res://Test/images/cards/SampleStrike.png\""));
    assert!(card.contains("namespace Test.Cards;"));

    // 本地化：RitsuLib 的 ID 规则 {MODID}_CARD_{SNAKE}
    let zhs = file(&out, "Test/localization/zhs/cards.json");
    assert!(zhs.contains("\"TEST_CARD_SAMPLE_STRIKE.title\": \"示例打击\""));
    assert!(zhs.contains("{Damage:diff()}"));

    // Entry：RitsuLib 初始化三件套
    let entry = file(&out, "Scripts/Entry.cs");
    assert!(entry.contains("[ModInitializer(nameof(Init))]"));
    assert!(entry.contains("RitsuLibFramework.EnsureGodotScriptsRegistered(assembly, Logger);"));
    assert!(entry.contains("ModTypeDiscoveryHub.RegisterModAssembly(ModId, assembly);"));
    assert!(entry.contains("public const string ModId = \"Test\";"));

    // csproj：Godot SDK + 游戏引用 + RitsuLib nuget
    let csproj = file(&out, "Test.csproj");
    assert!(csproj.contains("Godot.NET.Sdk/4.5.1"));
    assert!(csproj.contains("sts2.dll"));
    assert!(csproj.contains("STS2.RitsuLib"));

    // 无卡图 → 有警告
    assert!(out.warnings.iter().any(|w| w.contains("未设置卡图")));
}

#[test]
fn effects_and_power_vars() {
    let mut project = model::starter_project("MyMod", "x");
    let card = &mut project.cards[0];
    card.vars.push(VarDef {
        kind: "Power".into(),
        power: Some("WeakPower".into()),
        value: 2,
        props: vec![],
        upgrade: 1,
    });
    card.vars.push(VarDef { kind: "Cards".into(), power: None, value: 1, props: vec![], upgrade: 0 });
    card.on_play.push(Effect::Draw { var: None });
    card.on_play.push(Effect::ApplyPower {
        power: "WeakPower".into(),
        var: None,
        amount: None,
        to_self: false,
    });
    card.on_play.push(Effect::Custom { code: "Log.Info(\"hi\");".into() });

    let out = codegen::generate(&project).unwrap();
    let cs = file(&out, "Scripts/Cards/SampleStrike.cs");
    assert!(cs.contains("new PowerVar<WeakPower>(2)"));
    assert!(cs.contains("new CardsVar(1)"));
    assert!(cs.contains("await CardPileCmd.Draw(choiceContext, DynamicVars.Cards.IntValue, Owner);"));
    assert!(cs.contains("await PowerCmd.Apply<WeakPower>(choiceContext, cardPlay.Target!, DynamicVars[\"WeakPower\"].IntValue, Owner, null);"));
    assert!(cs.contains("Log.Info(\"hi\");"));
    assert!(cs.contains("DynamicVars[\"WeakPower\"].UpgradeValueBy(1);"));

    // 本地化 ID 前缀
    let zhs = file(&out, "MyMod/localization/zhs/cards.json");
    assert!(zhs.contains("MYMOD_CARD_SAMPLE_STRIKE.title"));
}

#[test]
fn validation_rejects_bad_input() {
    let mut p = model::starter_project("Test", "x");
    p.manifest.version = "1.0".into();
    assert!(p.validate().is_err());

    let mut p = model::starter_project("Test", "x");
    p.cards[0].class_name = "badName".into();
    assert!(p.validate().is_err());

    let mut p = model::starter_project("Test", "x");
    p.cards.push(p.cards[0].clone());
    assert!(p.validate().is_err());

    let mut p = model::starter_project("Test", "x");
    p.cards[0].text.insert("zhs".into(), CardText { title: "t".into(), description: "d".into() });
    assert!(p.validate().is_ok());
}
