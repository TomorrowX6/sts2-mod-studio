//! M7 实时预览测试：
//! - live 模式生成代码里的 Live.Int 路径与 live_numbers 严格一致（防两边漂移）
//! - 非 live 输出不含任何实时运行时痕迹（发布安全）
//! - 结构指纹：数值/文本改动不变，结构改动必变
//! - live.json 形状与语言码归一

use std::collections::BTreeSet;

use sts2mod_core::codegen::{self, GenOptions};
use sts2mod_core::live;
use sts2mod_core::model::{self, Effect, Project};

/// 覆盖全部宿主与积木字面量的"全家桶"项目。
fn kitchen_sink() -> Project {
    let raw = r#"{
      "formatVersion": 1,
      "manifest": {
        "id": "LiveTest", "name": "实时测试", "author": "t", "description": "",
        "version": "0.1.0", "minGameVersion": "0.107.1", "affectsGameplay": true,
        "dependencies": [{ "id": "STS2-RitsuLib", "minVersion": "0.4.54" }]
      },
      "cards": [{
        "className": "KitchenSink", "pool": "Colorless", "cardType": "Attack",
        "rarity": "Common", "target": "AnyEnemy", "energyCost": 1,
        "vars": [
          { "kind": "Damage", "value": 6, "props": ["Move"], "upgrade": 3 },
          { "kind": "Power", "power": "WeakPower", "value": 2 },
          { "kind": "Custom", "name": "Leech", "value": 4 }
        ],
        "onPlay": [
          { "op": "damage" },
          { "op": "block", "amount": 4 },
          { "op": "heal", "amount": 2 },
          { "op": "gainGold", "amount": 5 },
          { "op": "applyPower", "power": "WeakPower", "amount": 2 },
          { "op": "directDamage", "amount": 3, "toSelf": true },
          { "op": "if", "when": "Owner.Creature.Block > 10",
            "then": [{ "op": "draw" }, { "op": "heal", "amount": 1 }],
            "else": [{ "op": "block", "amount": 2 }] },
          { "op": "repeat", "times": 2, "do": [{ "op": "playVfx", "path": "vfx/vfx_block", "onSelf": true }] }
        ],
        "text": {
          "zhs": { "title": "全家桶", "description": "造成{Damage:diff()}点伤害。" },
          "en": { "title": "Kitchen Sink", "description": "Deal {Damage:diff()} damage." }
        }
      }],
      "relics": [{
        "className": "TurnEngine", "pool": "Shared", "rarity": "Common",
        "vars": [{ "kind": "Cards", "value": 1 }],
        "triggers": [{ "trigger": "AfterPlayerTurnStart",
          "effects": [{ "op": "draw" }, { "op": "gainGold", "amount": 3 }] }],
        "text": { "zhs": { "title": "回合引擎", "description": "抽{Cards}张牌。", "flavor": "咔哒。" } }
      }],
      "powers": [{
        "className": "SlowBurn", "powerType": "Debuff", "stackType": "Counter",
        "triggers": [{ "trigger": "AfterOwnerTurnEnd",
          "effects": [{ "op": "directDamage", "amount": 2, "toSelf": true }] }],
        "text": { "zhs": { "title": "缓燃", "description": "受到伤害。", "smartDescription": "{Amount}层。" } }
      }],
      "potions": [{
        "className": "AcidFlask", "pool": "Shared", "rarity": "Common",
        "usage": "CombatOnly", "target": "AnyEnemy",
        "vars": [{ "kind": "Damage", "value": 8 }],
        "onUse": [{ "op": "directDamage" }, { "op": "applyPower", "power": "WeakPower", "amount": 2 }],
        "text": { "zhs": { "title": "酸液瓶", "description": "造成{Damage}点伤害。" } }
      }],
      "monsters": [{
        "className": "TrainingDummy", "minHp": 15, "maxHp": 20,
        "moves": [{
          "name": "BASIC_ATTACK",
          "intents": [{ "kind": "attack", "amount": 3 }, { "kind": "defend" }],
          "effects": [{ "op": "damage", "amount": 3 }, { "op": "block", "amount": 8 }],
          "title": { "zhs": "基础攻击" },
          "banter": { "zhs": "接招！" }
        }],
        "text": { "zhs": { "name": "训练假人" } }
      }],
      "encounters": [{
        "className": "DummyEncounter", "acts": ["Glory"], "roomType": "Monster",
        "monsters": [{ "monster": "TrainingDummy" }],
        "text": { "zhs": { "title": "一只假人", "loss": "败了。" } }
      }],
      "events": [{
        "className": "DummyMeeting", "acts": ["Glory"],
        "vars": [{ "kind": "Gold", "value": 60 }],
        "pages": [
          { "key": "INITIAL",
            "description": { "zhs": "岔路口。" },
            "options": [
              { "key": "PAY", "title": { "zhs": "交钱" },
                "effects": [{ "op": "loseGold" }, { "op": "rewardCards", "count": 3 }], "goto": "DONE" },
              { "key": "FIGHT", "title": { "zhs": "开打" },
                "effects": [{ "op": "startCombat", "encounter": "DummyEncounter" }] }
            ] },
          { "key": "DONE", "description": { "zhs": "结束。" } }
        ],
        "title": { "zhs": "路边假人" }
      }],
      "characters": [{
        "className": "Trainee", "color": "#8080FF", "gender": "Neutral",
        "startingHp": 80, "startingGold": 99, "base": "Ironclad",
        "startingDeck": [{ "card": "KitchenSink", "count": 5 }],
        "startingRelics": ["TurnEngine"],
        "text": { "zhs": { "title": "见习者", "description": "介绍。" } }
      }]
    }"#;
    serde_json::from_str(raw).unwrap()
}

/// 从生成的所有文件里扫出 `Live.Int("路径"` 中的路径。
fn extract_live_paths(out: &codegen::GenOutput) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for f in &out.files {
        let mut rest = f.content.as_str();
        while let Some(pos) = rest.find("Live.Int(\"") {
            rest = &rest[pos + "Live.Int(\"".len()..];
            let end = rest.find('"').expect("Live.Int 路径未闭合");
            paths.insert(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    paths
}

#[test]
fn live_paths_match_live_numbers_exactly() {
    let project = kitchen_sink();
    let out = codegen::generate_with(&project, &GenOptions { live: true }).unwrap();
    let in_code = extract_live_paths(&out);
    let in_data: BTreeSet<String> = live::live_numbers(&project).into_keys().collect();
    assert_eq!(
        in_code, in_data,
        "生成代码里的 Live.Int 路径必须与 live_numbers 完全一致（谁多谁少都会导致推送不生效）"
    );
    // 抽查关键路径形状
    for expected in [
        "card.KitchenSink.cost",
        "card.KitchenSink.var.Damage",
        "card.KitchenSink.var.Damage.up",
        "card.KitchenSink.var.WeakPower",
        "card.KitchenSink.var.Leech",
        "card.KitchenSink.onPlay.1.amount",
        "card.KitchenSink.onPlay.6.then.1.amount",
        "card.KitchenSink.onPlay.6.else.0.amount",
        "card.KitchenSink.onPlay.7.times",
        "relic.TurnEngine.var.Cards",
        "relic.TurnEngine.trigger.AfterPlayerTurnStart.1.amount",
        "power.SlowBurn.trigger.AfterOwnerTurnEnd.0.amount",
        "potion.AcidFlask.var.Damage",
        "potion.AcidFlask.onUse.1.amount",
        "monster.TrainingDummy.minHp",
        "monster.TrainingDummy.moves.BASIC_ATTACK.intent.0",
        "monster.TrainingDummy.moves.BASIC_ATTACK.effects.0.amount",
        "event.DummyMeeting.var.Gold",
        "event.DummyMeeting.pages.INITIAL.options.PAY.effects.1.count",
        "character.Trainee.startingHp",
        "character.Trainee.deck.0",
    ] {
        assert!(in_data.contains(expected), "缺少路径 {expected}，实际: {in_data:?}");
    }
}

#[test]
fn non_live_output_contains_no_live_runtime() {
    let project = kitchen_sink();
    let out = codegen::generate(&project).unwrap();
    for f in &out.files {
        assert!(
            !f.content.contains("Live.Int(") && !f.content.contains("Live.Init()"),
            "非 live 构建不应包含实时运行时调用: {}",
            f.rel_path.display()
        );
        assert!(
            f.rel_path.to_string_lossy() != "Scripts/Live/Live.cs",
            "非 live 构建不应生成 Live.cs"
        );
    }
    // 数值仍是编译期字面量 / const
    let card = out
        .files
        .iter()
        .find(|f| f.rel_path.to_string_lossy() == "Scripts/Cards/KitchenSink.cs")
        .unwrap();
    assert!(card.content.contains("private const int energyCost = 1;"));
    assert!(card.content.contains("new DamageVar(6, ValueProp.Move)"));
}

#[test]
fn live_output_wires_runtime() {
    let project = kitchen_sink();
    let out = codegen::generate_with(&project, &GenOptions { live: true }).unwrap();
    let file = |p: &str| {
        &out.files
            .iter()
            .find(|f| f.rel_path.to_string_lossy() == p)
            .unwrap_or_else(|| panic!("缺少 {p}"))
            .content
    };
    assert!(file("Scripts/Entry.cs").contains("Live.Init();"));
    let live_cs = file("Scripts/Live/Live.cs");
    assert!(live_cs.contains("MergeWith"));
    assert!(live_cs.contains("RuntimeAssetRefreshCoordinator.Request"));
    assert!(live_cs.contains("SubscribeToLocaleChange"));
    assert!(live_cs.contains("FileSystemWatcher"));
    let card = file("Scripts/Cards/KitchenSink.cs");
    assert!(card.contains("private static int energyCost => Live.Int(\"card.KitchenSink.cost\", 1);"));
    assert!(card.contains("new DamageVar(Live.Int(\"card.KitchenSink.var.Damage\", 6), ValueProp.Move)"));
    assert!(card.contains("DynamicVars.Damage.UpgradeValueBy(Live.Int(\"card.KitchenSink.var.Damage.up\", 3));"));
    let monster = file("Scripts/Monsters/TrainingDummy.cs");
    assert!(monster.contains("MinInitialHp => Live.Int(\"monster.TrainingDummy.minHp\", 15)"));
    assert!(monster.contains("new SingleAttackIntent(Live.Int(\"monster.TrainingDummy.moves.BASIC_ATTACK.intent.0\", 3))"));
}

#[test]
fn live_mode_rejects_class_named_live() {
    let mut project = kitchen_sink();
    project.cards[0].class_name = "Live".into();
    // 非 live 模式无所谓
    assert!(codegen::generate(&project).is_ok());
    let err = codegen::generate_with(&project, &GenOptions { live: true }).unwrap_err();
    assert!(format!("{err:#}").contains("Live"));
}

#[test]
fn fingerprint_ignores_tunables_but_tracks_structure() {
    let base = kitchen_sink();
    let fp = live::live_fingerprint(&base);

    // 数值/文本改动：指纹不变（推送即生效）
    let mut p = kitchen_sink();
    p.cards[0].energy_cost = 3;
    p.cards[0].vars[0].value = 99;
    p.cards[0].vars[0].upgrade = 7;
    p.monsters[0].min_hp = 1;
    p.characters[0].starting_gold = 1;
    if let Effect::Block { amount, .. } = &mut p.cards[0].on_play[1] {
        *amount = Some(40);
    } else {
        panic!("onPlay[1] 应是 block");
    }
    p.cards[0].text.get_mut("zhs").unwrap().title = "改名".into();
    assert_eq!(fp, live::live_fingerprint(&p), "数值/文本改动不应改变结构指纹");

    // 增删内容：指纹变化
    let mut p = kitchen_sink();
    p.cards.push(model::starter_project("LiveTest", "x").cards[0].clone());
    assert_ne!(fp, live::live_fingerprint(&p));

    // 效果积木增删：指纹变化
    let mut p = kitchen_sink();
    p.cards[0].on_play.push(Effect::Draw { var: None });
    assert_ne!(fp, live::live_fingerprint(&p));

    // 字面量 ↔ 引用 var 的切换（amount 字段增删）：指纹变化
    let mut p = kitchen_sink();
    if let Effect::Block { amount, .. } = &mut p.cards[0].on_play[1] {
        *amount = None;
    }
    assert_ne!(fp, live::live_fingerprint(&p));

    // if 条件表达式：指纹变化
    let mut p = kitchen_sink();
    if let Effect::If { when, .. } = &mut p.cards[0].on_play[6] {
        *when = "true".into();
    }
    assert_ne!(fp, live::live_fingerprint(&p));

    // 工坊信息与指纹无关
    let mut p = kitchen_sink();
    p.workshop = Some(Default::default());
    assert_eq!(fp, live::live_fingerprint(&p));
}

#[test]
fn live_data_shape_and_lang_normalization() {
    let project = kitchen_sink();
    let data = live::live_data(&project, "abcd1234").unwrap();
    let v: serde_json::Value = serde_json::from_str(&data.json).unwrap();
    assert_eq!(v["fingerprint"], "abcd1234");
    assert_eq!(v["modId"], "LiveTest");
    // "en" 归一成 "eng"
    assert!(v["loc"]["eng"]["cards"]["LIVE_TEST_CARD_KITCHEN_SINK.title"].is_string());
    assert!(v["loc"]["zhs"]["cards"]["LIVE_TEST_CARD_KITCHEN_SINK.description"].is_string());
    assert!(v["loc"].get("en").is_none());
    // 数值表
    assert_eq!(v["num"]["card.KitchenSink.var.Damage"], 6);
    assert_eq!(v["num"]["monster.TrainingDummy.maxHp"], 20);
    assert!(data.nums > 0 && data.texts > 0);

    // 生成的本地化文件也用归一后的语言目录
    let out = codegen::generate(&project).unwrap();
    let paths: Vec<String> =
        out.files.iter().map(|f| f.rel_path.to_string_lossy().into_owned()).collect();
    assert!(paths.contains(&"LiveTest/localization/eng/cards.json".to_string()), "实际: {paths:?}");
    assert!(!paths.contains(&"LiveTest/localization/en/cards.json".to_string()));
}
