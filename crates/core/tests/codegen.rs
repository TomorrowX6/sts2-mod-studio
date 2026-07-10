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
    assert!(out.warnings.iter().any(|w| w.contains("未设置图片")));
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
    assert!(cs.contains("await PowerCmd.Apply<WeakPower>(choiceContext, cardPlay.Target!, DynamicVars[\"WeakPower\"].IntValue, Owner.Creature, null);"));
    assert!(cs.contains("Log.Info(\"hi\");"));
    assert!(cs.contains("DynamicVars[\"WeakPower\"].UpgradeValueBy(1);"));

    // 本地化 ID 前缀：modid 驼峰也会拆分（游戏内实测确认）
    let zhs = file(&out, "MyMod/localization/zhs/cards.json");
    assert!(zhs.contains("MY_MOD_CARD_SAMPLE_STRIKE.title"));
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

#[test]
fn relic_power_potion_generation() {
    use sts2mod_core::model::{PotionDef, PowerDef, RelicDef, RelicText, PowerText, TriggerDef};
    use std::collections::BTreeMap;

    let mut project = model::starter_project("MyMod", "x");

    let mut relic_text = BTreeMap::new();
    relic_text.insert("zhs".to_string(), RelicText {
        title: "测试遗物".into(),
        description: "每回合开始时，抽[blue]{Cards}[/blue]张牌。".into(),
        flavor: "眼熟吗？".into(),
    });
    project.relics.push(RelicDef {
        class_name: "LuckyCoin".into(),
        pool: "Shared".into(),
        rarity: "Common".into(),
        icon: Some("assets/relics/LuckyCoin.png".into()),
        vars: vec![VarDef { kind: "Cards".into(), power: None, value: 1, props: vec![], upgrade: 0 }],
        triggers: vec![TriggerDef {
            trigger: "AfterPlayerTurnStart".into(),
            effects: vec![Effect::Draw { var: None }],
        }],
        text: relic_text,
        extra_code: None,
    });

    let mut power_text = BTreeMap::new();
    power_text.insert("zhs".to_string(), PowerText {
        title: "邪火".into(),
        description: "每次抽牌时，获得力量。".into(),
        smart_description: "每次抽牌时，获得[blue]{Amount}[/blue]点力量。".into(),
    });
    project.powers.push(PowerDef {
        class_name: "EvilFlame".into(),
        power_type: "Buff".into(),
        stack_type: "Counter".into(),
        icon: None,
        triggers: vec![TriggerDef {
            trigger: "AfterCardDrawn".into(),
            effects: vec![Effect::ApplyPower {
                power: "StrengthPower".into(),
                var: None,
                amount: None,
                to_self: true,
            }],
        }],
        text: power_text,
        extra_code: Some("public int Extra;".into()),
    });

    let mut potion_text = BTreeMap::new();
    potion_text.insert("zhs".to_string(), CardText {
        title: "抽牌药水".into(),
        description: "抽{Cards}张牌。".into(),
    });
    project.potions.push(PotionDef {
        class_name: "DrawPotion".into(),
        pool: "Shared".into(),
        rarity: "Common".into(),
        usage: "CombatOnly".into(),
        target: "Self".into(),
        image: None,
        vars: vec![VarDef { kind: "Cards".into(), power: None, value: 3, props: vec![], upgrade: 0 }],
        on_use: vec![Effect::Draw { var: None }],
        text: potion_text,
        extra_code: None,
    });

    let out = codegen::generate(&project).unwrap();

    // 遗物
    let relic = file(&out, "Scripts/Relics/LuckyCoin.cs");
    assert!(relic.contains("[RegisterRelic(typeof(SharedRelicPool))]"));
    assert!(relic.contains("public class LuckyCoin : ModRelicTemplate"));
    assert!(relic.contains("RelicRarity.Common"));
    assert!(relic.contains("new CardsVar(1)"));
    assert!(relic.contains("public override async Task AfterPlayerTurnStart(PlayerChoiceContext choiceContext, Player player)"));
    assert!(relic.contains("await CardPileCmd.Draw(choiceContext, DynamicVars.Cards.IntValue, player);"));
    assert!(relic.contains("IconOutlinePath: \"res://MyMod/images/relics/LuckyCoin.png\""));

    // 能力
    let power = file(&out, "Scripts/Powers/EvilFlame.cs");
    assert!(power.contains("[RegisterPower]"));
    assert!(power.contains("public class EvilFlame : ModPowerTemplate"));
    assert!(power.contains("PowerType.Buff"));
    assert!(power.contains("PowerStackType.Counter"));
    assert!(power.contains("public override async Task AfterCardDrawn(PlayerChoiceContext choiceContext, CardModel card, bool fromHandDraw)"));
    // 未指定数值时能力默认用 Amount（与教程一致）
    assert!(power.contains("await PowerCmd.Apply<StrengthPower>(choiceContext, Owner, Amount, Owner, null);"));
    assert!(power.contains("public int Extra;"));

    // 药水
    let potion = file(&out, "Scripts/Potions/DrawPotion.cs");
    assert!(potion.contains("[RegisterPotion(typeof(SharedPotionPool))]"));
    assert!(potion.contains("public class DrawPotion : ModPotionTemplate"));
    assert!(potion.contains("PotionUsage.CombatOnly"));
    assert!(potion.contains("TargetType.Self"));
    assert!(potion.contains("protected override async Task OnUse(PlayerChoiceContext choiceContext, Creature? target)"));
    assert!(potion.contains("await CardPileCmd.Draw(choiceContext, DynamicVars.Cards.IntValue, Owner);"));

    // 本地化
    let relics_zhs = file(&out, "MyMod/localization/zhs/relics.json");
    assert!(relics_zhs.contains("MY_MOD_RELIC_LUCKY_COIN.title"));
    assert!(relics_zhs.contains("MY_MOD_RELIC_LUCKY_COIN.flavor"));
    let powers_zhs = file(&out, "MyMod/localization/zhs/powers.json");
    assert!(powers_zhs.contains("MY_MOD_POWER_EVIL_FLAME.smartDescription"));
    let potions_zhs = file(&out, "MyMod/localization/zhs/potions.json");
    assert!(potions_zhs.contains("MY_MOD_POTION_DRAW_POTION.title"));
}

#[test]
fn invalid_trigger_rejected() {
    use sts2mod_core::model::{RelicDef, TriggerDef};

    let mut project = model::starter_project("Test", "x");
    project.relics.push(RelicDef {
        class_name: "BadRelic".into(),
        pool: "Shared".into(),
        rarity: "Common".into(),
        icon: None,
        vars: vec![],
        triggers: vec![TriggerDef { trigger: "NoSuchHook".into(), effects: vec![Effect::Draw { var: None }] }],
        text: Default::default(),
        extra_code: None,
    });
    let err = match codegen::generate(&project) { Err(e) => e.to_string(), Ok(_) => panic!("应当失败") };
    assert!(err.contains("不支持的触发器"), "实际错误: {err}");

    // damage 在遗物触发器里不可用
    let mut project = model::starter_project("Test", "x");
    project.relics.push(RelicDef {
        class_name: "BadRelic2".into(),
        pool: "Shared".into(),
        rarity: "Common".into(),
        icon: None,
        vars: vec![],
        triggers: vec![TriggerDef {
            trigger: "AfterPlayerTurnStart".into(),
            effects: vec![Effect::Damage { var: None }],
        }],
        text: Default::default(),
        extra_code: None,
    });
    let err = match codegen::generate(&project) { Err(e) => e.to_string(), Ok(_) => panic!("应当失败") };
    assert!(err.contains("damage（攻击）积木需要敌人目标"), "实际错误: {err}");
}

#[test]
fn m3_effect_blocks() {
    let mut project = model::starter_project("Test", "x");
    let card = &mut project.cards[0];
    card.vars.push(VarDef { kind: "Block".into(), power: None, value: 5, props: vec![], upgrade: 0 });
    card.on_play = vec![
        Effect::Block { var: None, amount: None },
        Effect::Heal { var: None, amount: Some(3) },
        Effect::DirectDamage { var: None, amount: Some(2), props: vec![], to_self: true },
        Effect::GainGold { var: None, amount: Some(10) },
        Effect::PlaySfx { event: "event:/sfx/block_gain".into() },
        Effect::PlayVfx { path: "vfx/vfx_block".into(), on_self: true },
        Effect::If {
            when: "Owner.Creature.Block > 0".into(),
            then: vec![Effect::Draw { var: None }],
            otherwise: vec![Effect::Heal { var: None, amount: Some(1) }],
        },
        Effect::Repeat { times: 3, body: vec![Effect::Damage { var: None }] },
    ];

    let out = codegen::generate(&project).unwrap();
    let cs = file(&out, "Scripts/Cards/SampleStrike.cs");
    assert!(cs.contains("await CreatureCmd.GainBlock(Owner.Creature, DynamicVars.Block.BaseValue, ValueProp.Move, null);"));
    assert!(cs.contains("await CreatureCmd.Heal(Owner.Creature, 3);"));
    assert!(cs.contains("await CreatureCmd.Damage(choiceContext, [Owner.Creature], 2, ValueProp.Unblockable | ValueProp.Unpowered, Owner.Creature);"));
    assert!(cs.contains("await PlayerCmd.GainGold(10, Owner);"));
    assert!(cs.contains("SfxCmd.Play(\"event:/sfx/block_gain\");"));
    assert!(cs.contains("VfxCmd.PlayOnCreature(Owner.Creature, \"vfx/vfx_block\");"));
    assert!(cs.contains("if (Owner.Creature.Block > 0)"));
    assert!(cs.contains("else"));
    assert!(cs.contains("for (var i = 0; i < 3; i++)"));
    // 嵌套的 damage 在 for 循环内
    assert!(cs.contains("    await DamageCmd.Attack(DynamicVars.Damage.BaseValue)"));
}

#[test]
fn m3_power_owner_turn_end_trigger() {
    use sts2mod_core::model::{PowerDef, TriggerDef};

    let mut project = model::starter_project("Test", "x");
    project.powers.push(PowerDef {
        class_name: "BurnPower".into(),
        power_type: "Debuff".into(),
        stack_type: "Counter".into(),
        icon: None,
        triggers: vec![TriggerDef {
            trigger: "AfterOwnerTurnEnd".into(),
            effects: vec![Effect::DirectDamage { var: None, amount: None, props: vec![], to_self: true }],
        }],
        text: Default::default(),
        extra_code: None,
    });
    let out = codegen::generate(&project).unwrap();
    let cs = file(&out, "Scripts/Powers/BurnPower.cs");
    // 真实钩子 + 己方过滤守卫
    assert!(cs.contains("public override async Task AfterSideTurnEnd(PlayerChoiceContext choiceContext, CombatSide side, IEnumerable<Creature> participants)"));
    assert!(cs.contains("if (side != Owner.Side)"));
    // 能力上下文默认数值 Amount
    assert!(cs.contains("await CreatureCmd.Damage(choiceContext, [Owner], Amount, ValueProp.Unblockable | ValueProp.Unpowered, Owner);"));
}

#[test]
fn m3_context_restrictions() {
    use sts2mod_core::model::PotionDef;

    // 无目标药水不能用攻击 damage
    let mut project = model::starter_project("Test", "x");
    project.potions.push(PotionDef {
        class_name: "BadPotion".into(),
        pool: "Shared".into(),
        rarity: "Common".into(),
        usage: "CombatOnly".into(),
        target: "AnyEnemy".into(),
        image: None,
        vars: vec![],
        on_use: vec![Effect::Damage { var: None }],
        text: Default::default(),
        extra_code: None,
    });
    let err = match codegen::generate(&project) { Err(e) => e.to_string(), Ok(_) => panic!("应当失败") };
    assert!(err.contains("damage（攻击）积木需要敌人目标"), "实际错误: {err}");

    // 有目标药水可以用 directDamage 打目标
    let mut project = model::starter_project("Test", "x");
    project.potions.push(PotionDef {
        class_name: "AcidPotion".into(),
        pool: "Shared".into(),
        rarity: "Common".into(),
        usage: "CombatOnly".into(),
        target: "AnyEnemy".into(),
        image: None,
        vars: vec![VarDef { kind: "Damage".into(), power: None, value: 8, props: vec![], upgrade: 0 }],
        on_use: vec![Effect::DirectDamage { var: None, amount: None, props: vec![], to_self: false }],
        text: Default::default(),
        extra_code: None,
    });
    let out = codegen::generate(&project).unwrap();
    let cs = file(&out, "Scripts/Potions/AcidPotion.cs");
    assert!(cs.contains("await CreatureCmd.Damage(choiceContext, [target!], DynamicVars.Damage.BaseValue, ValueProp.Unblockable | ValueProp.Unpowered, Owner.Creature);"));
}
