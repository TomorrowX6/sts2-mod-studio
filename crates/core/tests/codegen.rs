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
        "Test.sln",
        "project.godot",
        "export_presets.cfg",
        "Scripts/Entry.cs",
        "Scripts/Cards/SampleStrike.cs",
        "Test/localization/zhs/cards.json",
        // 项目里写 "en"，生成时归一成游戏语言码 "eng"（否则游戏不加载英文文本）
        "Test/localization/eng/cards.json",
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

    // Godot Mono 导出要求项目根目录存在同名 solution。
    let solution = file(&out, "Test.sln");
    assert!(solution.contains("\"Test\", \"Test.csproj\""));
    assert!(solution.contains("Debug|Any CPU"));

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
        name: None,
        tooltip: Default::default(),
        value: 2,
        props: vec![],
        upgrade: 1,
    });
    card.vars.push(VarDef { kind: "Cards".into(), power: None, name: None, tooltip: Default::default(), value: 1, props: vec![], upgrade: 0 });
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
        vars: vec![VarDef { kind: "Cards".into(), power: None, name: None, tooltip: Default::default(), value: 1, props: vec![], upgrade: 0 }],
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
        vars: vec![VarDef { kind: "Cards".into(), power: None, name: None, tooltip: Default::default(), value: 3, props: vec![], upgrade: 0 }],
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
            effects: vec![Effect::Damage { var: None, amount: None }],
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
    card.vars.push(VarDef { kind: "Block".into(), power: None, name: None, tooltip: Default::default(), value: 5, props: vec![], upgrade: 0 });
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
        Effect::Repeat { times: 3, body: vec![Effect::Damage { var: None, amount: None }] },
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
        on_use: vec![Effect::Damage { var: None, amount: None }],
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
        vars: vec![VarDef { kind: "Damage".into(), power: None, name: None, tooltip: Default::default(), value: 8, props: vec![], upgrade: 0 }],
        on_use: vec![Effect::DirectDamage { var: None, amount: None, props: vec![], to_self: false }],
        text: Default::default(),
        extra_code: None,
    });
    let out = codegen::generate(&project).unwrap();
    let cs = file(&out, "Scripts/Potions/AcidPotion.cs");
    assert!(cs.contains("await CreatureCmd.Damage(choiceContext, [target!], DynamicVars.Damage.BaseValue, ValueProp.Unblockable | ValueProp.Unpowered, Owner.Creature);"));
}

// ---------- M4: 怪物 / 遭遇 / 事件 / 人物 ----------

fn m4_monster(name: &str) -> sts2mod_core::model::MonsterDef {
    use sts2mod_core::model::{IntentDef, MonsterDef, MonsterMoveDef, MonsterText};
    use std::collections::BTreeMap;

    let mut text = BTreeMap::new();
    text.insert("zhs".to_string(), MonsterText { name: "戈多".into() });
    let mut title = BTreeMap::new();
    title.insert("zhs".to_string(), "基础攻击".to_string());
    let mut banter = BTreeMap::new();
    banter.insert("zhs".to_string(), "接招！".to_string());
    MonsterDef {
        class_name: name.into(),
        min_hp: 15,
        max_hp: 20,
        image: Some("assets/monsters/m.png".into()),
        scene: None,
        moves: vec![
            MonsterMoveDef {
                name: "BASIC_ATTACK".into(),
                intents: vec![IntentDef::Attack { amount: 3 }, IntentDef::Defend],
                effects: vec![
                    Effect::Damage { var: None, amount: Some(3) },
                    Effect::Block { var: None, amount: Some(8) },
                ],
                title,
                banter,
            },
            MonsterMoveDef {
                name: "HEAVY_ATTACK".into(),
                intents: vec![IntentDef::Attack { amount: 6 }],
                effects: vec![Effect::Damage { var: None, amount: Some(6) }],
                title: BTreeMap::new(),
                banter: BTreeMap::new(),
            },
        ],
        text,
        extra_code: None,
    }
}

#[test]
fn m4_monster_and_encounter_generation() {
    use sts2mod_core::model::{EncounterDef, EncounterMonster, EncounterText};
    use std::collections::BTreeMap;

    let mut project = model::starter_project("Test", "x");
    project.monsters.push(m4_monster("TestMonster"));

    let mut enc_text = BTreeMap::new();
    enc_text.insert(
        "zhs".to_string(),
        EncounterText { title: "一只戈多".into(), loss: "{character}倒下了。".into() },
    );
    project.encounters.push(EncounterDef {
        class_name: "TestEncounter".into(),
        acts: vec!["Glory".into()],
        room_type: "Monster".into(),
        is_weak: false,
        monsters: vec![EncounterMonster { monster: "TestMonster".into(), slot: None }],
        camera_scaling: None,
        text: enc_text,
        extra_code: None,
    });
    project.encounters.push(EncounterDef {
        class_name: "TestMultiEncounter".into(),
        acts: vec![],
        room_type: "Monster".into(),
        is_weak: true,
        monsters: vec![
            EncounterMonster { monster: "TestMonster".into(), slot: Some("left".into()) },
            EncounterMonster { monster: "TestMonster".into(), slot: None },
        ],
        camera_scaling: Some(0.8),
        text: BTreeMap::new(),
        extra_code: None,
    });

    let out = codegen::generate(&project).unwrap();

    // 怪物类：注册、血量、状态机循环、招式方法、对白
    let cs = file(&out, "Scripts/Monsters/TestMonster.cs");
    assert!(cs.contains("[RegisterMonster]"));
    assert!(cs.contains("public class TestMonster : ModMonsterTemplate"));
    assert!(cs.contains("public override int MinInitialHp => 15;"));
    assert!(cs.contains("public override int MaxInitialHp => 20;"));
    assert!(cs.contains("new MoveState(\n            \"BASIC_ATTACK\",\n            BasicAttackMove"));
    assert!(cs.contains("new SingleAttackIntent(3)"));
    assert!(cs.contains("new DefendIntent()"));
    assert!(cs.contains("ModMonsterMoveStateMachines.Cycle(basicAttack, heavyAttack)"));
    assert!(cs.contains(".FromMonster(this)"));
    assert!(cs.contains("TalkCmd.Play(L10NMonsterLookup(\"TEST_MONSTER_TEST_MONSTER.moves.BASIC_ATTACK.banter\"), Creature, VfxColor.Blue);"));
    assert!(cs.contains("await CreatureCmd.GainBlock(Creature, 8, ValueProp.Move, null);"));
    assert!(cs.contains("RitsuGodotNodeFactories.CreateFromScenePath<NCreatureVisuals>"));

    // 内置怪物场景：唯一名节点齐全
    let tscn = file(&out, "Test/scenes/test_monster.tscn");
    for node in ["Visuals", "Bounds", "IntentPos", "CenterPos", "TalkPos"] {
        assert!(tscn.contains(&format!("[node name=\"{node}\"")), "场景缺少 {node}");
    }
    assert!(tscn.contains("res://Test/images/monsters/TestMonster.png"));

    // 单怪遭遇：无场景、slot 为 null
    let enc = file(&out, "Scripts/Encounters/TestEncounter.cs");
    assert!(enc.contains("[RegisterActEncounter(typeof(Glory))]"));
    assert!(enc.contains("public override RoomType RoomType => RoomType.Monster;"));
    assert!(enc.contains("(ModelDb.Monster<TestMonster>().ToMutable(), null)"));
    assert!(!enc.contains("EncounterScenePath"));

    // 多怪遭遇：全局注册 + 场景 + 槽位（自定义名和自动名混合）
    let multi = file(&out, "Scripts/Encounters/TestMultiEncounter.cs");
    assert!(multi.contains("[RegisterGlobalEncounter]"));
    assert!(multi.contains("public override IReadOnlyList<string> Slots => [\"left\", \"m2\"];"));
    assert!(multi.contains("(ModelDb.Monster<TestMonster>().ToMutable(), \"left\")"));
    assert!(multi.contains("public override float GetCameraScaling() => 0.8f;"));
    let enc_tscn = file(&out, "Test/scenes/test_multi_encounter.tscn");
    assert!(enc_tscn.contains("[node name=\"left\" type=\"Marker2D\""));
    assert!(enc_tscn.contains("[node name=\"m2\" type=\"Marker2D\""));

    // 本地化：怪物名 / 招式标题 / 对白 / 遭遇标题
    let monsters_json = file(&out, "Test/localization/zhs/monsters.json");
    assert!(monsters_json.contains("\"TEST_MONSTER_TEST_MONSTER.name\": \"戈多\""));
    assert!(monsters_json.contains("\"TEST_MONSTER_TEST_MONSTER.moves.BASIC_ATTACK.title\": \"基础攻击\""));
    assert!(monsters_json.contains("\"TEST_MONSTER_TEST_MONSTER.moves.BASIC_ATTACK.banter\": \"接招！\""));
    let encounters_json = file(&out, "Test/localization/zhs/encounters.json");
    assert!(encounters_json.contains("\"TEST_ENCOUNTER_TEST_ENCOUNTER.title\": \"一只戈多\""));
    assert!(encounters_json.contains("\"TEST_ENCOUNTER_TEST_ENCOUNTER.loss\""));
}

fn m4_event() -> sts2mod_core::model::EventDef {
    use sts2mod_core::model::{EventDef, EventOptionDef, EventPage};
    use std::collections::BTreeMap;

    let zhs = |s: &str| {
        let mut m = BTreeMap::new();
        m.insert("zhs".to_string(), s.to_string());
        m
    };
    EventDef {
        class_name: "TestEvent".into(),
        acts: vec!["Glory".into()],
        image: Some("assets/events/e.png".into()),
        vars: vec![
            VarDef {
                kind: "Damage".into(),
                power: None,
                name: None,
                tooltip: Default::default(),
                value: 10,
                props: vec!["Unblockable".into(), "Unpowered".into()],
                upgrade: 0,
            },
            VarDef { kind: "Gold".into(), power: None, name: None, tooltip: Default::default(), value: 60, props: vec![], upgrade: 0 },
        ],
        condition: Some("runState.Players.All(p => p.Gold >= DynamicVars.Gold.BaseValue)".into()),
        pages: vec![
            EventPage {
                key: "INITIAL".into(),
                description: zhs("你遇到了戈多。"),
                options: vec![
                    EventOptionDef {
                        key: "TAKE_DAMAGE".into(),
                        title: zhs("挨打"),
                        description: zhs("受到[red]{Damage}[/red]点伤害。"),
                        effects: vec![Effect::DirectDamage {
                            var: Some("Damage".into()),
                            amount: None,
                            props: vec![],
                            to_self: true,
                        }],
                        goto: Some("REWARD".into()),
                    },
                    EventOptionDef {
                        key: "LOSE_GOLD".into(),
                        title: zhs("给钱"),
                        description: zhs("失去[gold]{Gold}[/gold]金币。"),
                        effects: vec![Effect::LoseGold { var: Some("Gold".into()), amount: None }],
                        goto: Some("REWARD".into()),
                    },
                    EventOptionDef {
                        key: "FIGHT".into(),
                        title: zhs("战斗"),
                        description: BTreeMap::new(),
                        effects: vec![Effect::StartCombat { encounter: "TestEncounter".into() }],
                        goto: None,
                    },
                ],
            },
            EventPage {
                key: "REWARD".into(),
                description: zhs("选择奖励。"),
                options: vec![
                    EventOptionDef {
                        key: "CHOOSE_CARDS".into(),
                        title: zhs("拿牌"),
                        description: BTreeMap::new(),
                        effects: vec![Effect::RewardCards { count: 3 }],
                        goto: Some("DONE".into()),
                    },
                    EventOptionDef {
                        key: "CHOOSE_POTION".into(),
                        title: zhs("拿药"),
                        description: BTreeMap::new(),
                        effects: vec![Effect::RewardPotion],
                        goto: Some("DONE".into()),
                    },
                ],
            },
            EventPage { key: "DONE".into(), description: zhs("戈多消失了。"), options: vec![] },
        ],
        title: zhs("与戈多相遇"),
        extra_code: None,
    }
}

#[test]
fn m4_event_generation() {
    use sts2mod_core::model::{EncounterDef, EncounterMonster};
    use std::collections::BTreeMap;

    let mut project = model::starter_project("Test", "x");
    project.monsters.push(m4_monster("TestMonster"));
    project.encounters.push(EncounterDef {
        class_name: "TestEncounter".into(),
        acts: vec!["Glory".into()],
        room_type: "Monster".into(),
        is_weak: false,
        monsters: vec![EncounterMonster { monster: "TestMonster".into(), slot: None }],
        camera_scaling: None,
        text: BTreeMap::new(),
        extra_code: None,
    });
    project.events.push(m4_event());

    let out = codegen::generate(&project).unwrap();
    let cs = file(&out, "Scripts/Events/TestEvent.cs");

    assert!(cs.contains("[RegisterActEvent(typeof(Glory))]"));
    assert!(cs.contains("public sealed class TestEvent : ModEventTemplate"));
    assert!(cs.contains("InitialPortraitPath: \"res://Test/images/events/TestEvent.png\""));
    assert!(cs.contains("new DamageVar(10, ValueProp.Unblockable | ValueProp.Unpowered)"));
    assert!(cs.contains("new GoldVar(60)"));
    assert!(cs.contains("public override bool IsAllowed(IRunState runState) => runState.Players.All"));

    // 初始选项 + 页面跳转 + 结束页
    assert!(cs.contains("protected override IReadOnlyList<EventOption> GenerateInitialOptions()"));
    assert!(cs.contains("new EventOption(this, TakeDamage, InitialOptionKey(\"TAKE_DAMAGE\"))"));
    assert!(cs.contains("new EventOption(this, RewardChooseCards, ModOptionKey(\"REWARD\", \"CHOOSE_CARDS\"))"));
    assert!(cs.contains("SetEventState(PageDescription(\"REWARD\")"));
    assert!(cs.contains("SetEventFinished(PageDescription(\"DONE\"));"));

    // 效果：事件单目标伤害重载 / 失去金币 / 奖励
    assert!(cs.contains("await CreatureCmd.Damage(new ThrowingPlayerChoiceContext(), Owner!.Creature, DynamicVars.Damage, null, null);"));
    assert!(cs.contains("await PlayerCmd.LoseGold(DynamicVars.Gold.BaseValue, Owner!, GoldLossType.Stolen);"));
    assert!(cs.contains("new CardReward(CardCreationOptions.ForNonCombatWithDefaultOdds([Owner!.Character.CardPool]), 3, Owner)"));
    assert!(cs.contains("new PotionReward(Owner!)"));

    // 战斗事件：LayoutType + CanonicalEncounter + 进战斗
    assert!(cs.contains("public override EventLayoutType LayoutType => EventLayoutType.Combat;"));
    assert!(cs.contains("public override EncounterModel CanonicalEncounter => ModelDb.Encounter<TestEncounter>();"));
    assert!(cs.contains("EnterCombatWithoutExitingEvent<TestEncounter>([], shouldResumeAfterCombat: false);"));
    assert!(cs.contains("using Test.Encounters;"));

    // 本地化键
    let json = file(&out, "Test/localization/zhs/events.json");
    assert!(json.contains("\"TEST_EVENT_TEST_EVENT.title\": \"与戈多相遇\""));
    assert!(json.contains("\"TEST_EVENT_TEST_EVENT.pages.INITIAL.description\""));
    assert!(json.contains("\"TEST_EVENT_TEST_EVENT.pages.INITIAL.options.TAKE_DAMAGE.title\": \"挨打\""));
    assert!(json.contains("\"TEST_EVENT_TEST_EVENT.pages.REWARD.options.CHOOSE_POTION.title\""));
    assert!(json.contains("\"TEST_EVENT_TEST_EVENT.pages.DONE.description\""));
}

#[test]
fn m4_character_generation() {
    use sts2mod_core::model::{CharacterDef, CharacterText, StartingCard};
    use std::collections::BTreeMap;

    let mut project = model::starter_project("Test", "x");
    let mut text = BTreeMap::new();
    text.insert(
        "zhs".to_string(),
        CharacterText { title: "戈多".into(), description: "等待的存在。".into() },
    );
    project.characters.push(CharacterDef {
        class_name: "Godot".into(),
        color: "#8080FF".into(),
        gender: "Masculine".into(),
        starting_hp: 80,
        starting_gold: 99,
        base: "Ironclad".into(),
        combat_image: Some("assets/characters/godot.png".into()),
        portrait: Some("assets/characters/portrait.png".into()),
        select_icon: None,
        select_icon_locked: None,
        map_marker: None,
        energy_icon: Some("assets/characters/e24.png".into()),
        energy_icon_big: Some("assets/characters/e74.png".into()),
        starting_deck: vec![StartingCard { card: "SampleStrike".into(), count: 5 }],
        starting_relics: vec![],
        text,
        extra_code: None,
    });

    let out = codegen::generate(&project).unwrap();

    // 三池：类型列表池 + 主题色 + 能量图标
    let pools = file(&out, "Scripts/Characters/GodotPools.cs");
    assert!(pools.contains("public class GodotCardPool : TypeListCardPoolModel"));
    assert!(pools.contains("public class GodotRelicPool : TypeListRelicPoolModel"));
    assert!(pools.contains("public class GodotPotionPool : TypeListPotionPoolModel"));
    assert!(pools.contains("public override string EnergyColorName => \"test_godot\";"));
    assert!(pools.contains("MaterialUtils.CreateReplaceHueShaderMaterial"));
    assert!(pools.contains("TextEnergyIconPath => \"res://Test/images/characters/GodotEnergy.png\""));

    // 人物类：注册、模板泛型、资源 Merge 兜底、初始卡组
    let cs = file(&out, "Scripts/Characters/Godot.cs");
    assert!(cs.contains("[RegisterCharacter]"));
    assert!(cs.contains("public class Godot : ModCharacterTemplate<GodotCardPool, GodotRelicPool, GodotPotionPool>"));
    assert!(cs.contains("public override CharacterGender Gender => CharacterGender.Masculine;"));
    assert!(cs.contains("public override int StartingHp => 80;"));
    assert!(cs.contains("CharacterAssetProfiles.Merge(\n        CharacterAssetProfiles.Ironclad(),"));
    assert!(cs.contains("VisualsPath: \"res://Test/scenes/godot.tscn\""));
    assert!(cs.contains("EnergyCounterPath: \"res://Test/scenes/godot_energy.tscn\""));
    assert!(cs.contains("CharacterSelectBgPath: \"res://Test/scenes/godot_bg.tscn\""));
    assert!(cs.contains("new(typeof(SampleStrike), 5)"));
    assert!(cs.contains("public override bool RequiresEpochAndTimeline => false;"));

    // 生成场景：战斗模型 / 能量表盘 / 头像 / 选人背景
    for scene in ["godot.tscn", "godot_energy.tscn", "godot_icon.tscn", "godot_bg.tscn"] {
        assert!(
            out.files.iter().any(|f| f.rel_path.to_string_lossy() == format!("Test/scenes/{scene}")),
            "缺少场景 {scene}"
        );
    }

    // 本地化：标题 / 代词默认值 / 先古占位
    let chars_json = file(&out, "Test/localization/zhs/characters.json");
    assert!(chars_json.contains("\"TEST_CHARACTER_GODOT.title\": \"戈多\""));
    assert!(chars_json.contains("\"TEST_CHARACTER_GODOT.pronounSubject\": \"他\""));
    assert!(chars_json.contains("\"TEST_CHARACTER_GODOT.unlockText\""));
    let ancients_json = file(&out, "Test/localization/zhs/ancients.json");
    assert!(ancients_json.contains("\"DARV.talk.TEST_CHARACTER_GODOT.0-0.char\""));
    assert!(ancients_json.contains("\"THE_ARCHITECT.talk.GODOT.0-attack\": \"Both\""));

    // 先古占位提醒
    assert!(out.warnings.iter().any(|w| w.contains("先古对话")));
}

#[test]
fn m4_validation_rules() {
    use sts2mod_core::model::{
        EncounterDef, EncounterMonster, EventOptionDef, EventPage,
    };
    use std::collections::BTreeMap;

    // 遭遇引用不存在的怪物
    let mut p = model::starter_project("Test", "x");
    p.encounters.push(EncounterDef {
        class_name: "BadEncounter".into(),
        acts: vec![],
        room_type: "Monster".into(),
        is_weak: false,
        monsters: vec![EncounterMonster { monster: "Nobody".into(), slot: None }],
        camera_scaling: None,
        text: BTreeMap::new(),
        extra_code: None,
    });
    assert!(p.validate().unwrap_err().to_string().contains("不存在的怪物"));

    // 招式名必须大写蛇形
    let mut p = model::starter_project("Test", "x");
    let mut m = m4_monster("TestMonster");
    m.moves[0].name = "basicAttack".into();
    m.moves[0].intents = vec![];
    p.monsters.push(m);
    assert!(p.validate().unwrap_err().to_string().contains("大写蛇形"));

    // 事件第一页必须 INITIAL；跳转页必须存在
    let mut p = model::starter_project("Test", "x");
    let mut ev = m4_event();
    ev.acts = vec![];
    ev.pages[0].key = "FIRST".into();
    p.events.push(ev);
    assert!(p.validate().unwrap_err().to_string().contains("INITIAL"));

    let mut p = model::starter_project("Test", "x");
    p.monsters.push(m4_monster("TestMonster"));
    p.encounters.push(EncounterDef {
        class_name: "TestEncounter".into(),
        acts: vec![],
        room_type: "Monster".into(),
        is_weak: false,
        monsters: vec![EncounterMonster { monster: "TestMonster".into(), slot: None }],
        camera_scaling: None,
        text: BTreeMap::new(),
        extra_code: None,
    });
    let mut ev = m4_event();
    ev.pages[0].options[0].goto = Some("NOWHERE".into());
    p.events.push(ev);
    assert!(p.validate().unwrap_err().to_string().contains("NOWHERE"));

    // 无 goto 且无 startCombat 的选项被拒绝
    let mut p = model::starter_project("Test", "x");
    p.events.push(sts2mod_core::model::EventDef {
        class_name: "E2".into(),
        acts: vec![],
        image: None,
        vars: vec![],
        condition: None,
        pages: vec![EventPage {
            key: "INITIAL".into(),
            description: BTreeMap::new(),
            options: vec![EventOptionDef {
                key: "GO".into(),
                title: BTreeMap::new(),
                description: BTreeMap::new(),
                effects: vec![],
                goto: None,
            }],
        }],
        title: BTreeMap::new(),
        extra_code: None,
    });
    assert!(p.validate().is_err());

    // 人物：坏颜色 / 初始卡组引用不存在的卡
    let mut p = model::starter_project("Test", "x");
    p.characters.push(sts2mod_core::model::CharacterDef {
        class_name: "C".into(),
        color: "red".into(),
        gender: "Neutral".into(),
        starting_hp: 80,
        starting_gold: 99,
        base: "Ironclad".into(),
        combat_image: None,
        portrait: None,
        select_icon: None,
        select_icon_locked: None,
        map_marker: None,
        energy_icon: None,
        energy_icon_big: None,
        starting_deck: vec![],
        starting_relics: vec![],
        text: BTreeMap::new(),
        extra_code: None,
    });
    assert!(p.validate().unwrap_err().to_string().contains("RRGGBB"));

    // 怪物没有 DynamicVars：招式里用 var 报错
    let mut p = model::starter_project("Test", "x");
    let mut m = m4_monster("TestMonster");
    m.moves[0].effects = vec![Effect::Block { var: Some("Block".into()), amount: None }];
    m.moves[0].intents = vec![];
    p.monsters.push(m);
    assert!(codegen::generate(&p).err().unwrap().to_string().contains("DynamicVars"));

    // 怪物 damage 必须给固定值
    let mut p = model::starter_project("Test", "x");
    let mut m = m4_monster("TestMonster");
    m.moves[0].effects = vec![Effect::Damage { var: None, amount: None }];
    p.monsters.push(m);
    assert!(codegen::generate(&p).err().unwrap().to_string().contains("固定值"));
}

#[test]
fn m4_monster_without_moves_needs_extra_code() {
    let mut p = model::starter_project("Test", "x");
    let mut m = m4_monster("TestMonster");
    m.moves.clear();
    p.monsters.push(m);
    assert!(codegen::generate(&p).err().unwrap().to_string().contains("招式"));

    let mut p = model::starter_project("Test", "x");
    let mut m = m4_monster("TestMonster");
    m.moves.clear();
    m.extra_code = Some("// 自定义状态机".into());
    p.monsters.push(m);
    assert!(codegen::generate(&p).is_ok());
}
