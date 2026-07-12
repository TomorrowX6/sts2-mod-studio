//! M6：卡牌深化——自定义关键词 / 标签 / 命名数值 / 悬浮提示。

use std::collections::BTreeMap;

use sts2mod_core::codegen;
use sts2mod_core::model::{self, CardText, KeywordDef, VarDef};

fn file<'a>(out: &'a codegen::GenOutput, path: &str) -> &'a str {
    &out
        .files
        .iter()
        .find(|f| f.rel_path.to_string_lossy() == path)
        .unwrap_or_else(|| panic!("缺少生成文件 {path}"))
        .content
}

fn zhs_card_text(title: &str, desc: &str) -> BTreeMap<String, CardText> {
    let mut m = BTreeMap::new();
    m.insert("zhs".into(), CardText { title: title.into(), description: desc.into() });
    m
}

fn m6_project() -> model::Project {
    let mut p = model::starter_project("Test", "x");
    p.keywords.push(KeywordDef {
        name: "Unique".into(),
        icon: Some("assets/keywords/unique.png".into()),
        placement: "BeforeCardDescription".into(),
        text: zhs_card_text("唯一", "卡组中只能有一张同名牌。"),
    });
    p.card_tags.push("Heavy".into());
    p.powers.push(sts2mod_core::model::PowerDef {
        class_name: "TestPower".into(),
        power_type: "Buff".into(),
        stack_type: "Counter".into(),
        icon: None,
        triggers: vec![],
        text: BTreeMap::new(),
        extra_code: None,
    });

    let card = &mut p.cards[0];
    card.keywords = vec!["Exhaust".into(), "Unique".into()];
    card.tags = vec!["Strike".into(), "Heavy".into()];
    card.hover_tip_cards = vec!["SampleStrike".into()];
    card.hover_tip_powers = vec!["TestPower".into()];
    card.vars.push(VarDef {
        kind: "Custom".into(),
        power: None,
        name: Some("Leech".into()),
        tooltip: zhs_card_text("汲取", "吸取等量生命。"),
        value: 3,
        props: vec![],
        upgrade: 1,
    });
    p
}

#[test]
fn m6_keywords_tags_generation() {
    let p = m6_project();
    let out = codegen::generate(&p).unwrap();

    // 关键词注册类（教程原文结构）
    let kw = file(&out, "Scripts/ModKeywords.cs");
    assert!(kw.contains("[RegisterOwnedCardKeyword(nameof(Unique), IconPath = \"res://Test/images/keywords/Unique.png\", CardDescriptionPlacement = ModKeywordCardDescriptionPlacement.BeforeCardDescription)]"));
    assert!(kw.contains("public class ModKeywords"));
    assert!(kw.contains("ModContentRegistry.GetQualifiedKeywordId(Entry.ModId, nameof(Unique)).GetModCardKeyword()"));

    // 标签注册类
    let tags = file(&out, "Scripts/ModCardTags.cs");
    assert!(tags.contains("[RegisterOwnedCardTag(nameof(Heavy))]"));
    assert!(tags.contains("GetQualifiedCardTagId(Entry.ModId, nameof(Heavy)).GetModCardTag()"));

    // 卡牌引用块：原版枚举 + 自定义字段
    let cs = file(&out, "Scripts/Cards/SampleStrike.cs");
    assert!(cs.contains("public override IEnumerable<CardKeyword> CanonicalKeywords => ["));
    assert!(cs.contains("        CardKeyword.Exhaust,\n        ModKeywords.Unique"));
    assert!(cs.contains("protected override HashSet<CardTag> CanonicalTags => ["));
    assert!(cs.contains("        CardTag.Strike,\n        ModCardTags.Heavy"));
    assert!(cs.contains("HoverTipFactory.FromCard<SampleStrike>()"));
    assert!(cs.contains("HoverTipFactory.FromPower<TestPower>()"));
    assert!(cs.contains("using Test.Powers;"));

    // 自定义命名数值 + 悬浮提示链
    assert!(cs.contains("ModCardVars.Int(\"Leech\", 3)"));
    assert!(cs.contains(".WithSharedTooltip(\"TEST_LEECH\")"));
    // 升级走索引器
    assert!(cs.contains("DynamicVars[\"Leech\"].UpgradeValueBy(1);"));

    // 本地化
    let kw_json = file(&out, "Test/localization/zhs/card_keywords.json");
    assert!(kw_json.contains("\"TEST_KEYWORD_UNIQUE.title\": \"唯一\""));
    assert!(kw_json.contains("\"TEST_KEYWORD_UNIQUE.description\": \"卡组中只能有一张同名牌。\""));
    let tips_json = file(&out, "Test/localization/zhs/static_hover_tips.json");
    assert!(tips_json.contains("\"TEST_LEECH.title\": \"汲取\""));

    // 关键词图标登记复制
    assert!(out.copies.iter().any(|c| {
        c.src_rel == "assets/keywords/unique.png"
            && c.dst_rel.to_string_lossy() == "Test/images/keywords/Unique.png"
    }));
}

#[test]
fn m6_no_extra_files_when_unused() {
    let p = model::starter_project("Test", "x");
    let out = codegen::generate(&p).unwrap();
    assert!(!out.files.iter().any(|f| f.rel_path.to_string_lossy().contains("ModKeywords")));
    assert!(!out.files.iter().any(|f| f.rel_path.to_string_lossy().contains("ModCardTags")));
    let cs = file(&out, "Scripts/Cards/SampleStrike.cs");
    assert!(!cs.contains("CanonicalKeywords"));
    assert!(!cs.contains("CanonicalTags"));
    assert!(!cs.contains("AdditionalHoverTips"));
}

#[test]
fn m6_validation() {
    // placement 非法
    let mut p = m6_project();
    p.keywords[0].placement = "Above".into();
    assert!(p.validate().unwrap_err().to_string().contains("placement"));

    // Custom 数值缺 name
    let mut p = m6_project();
    p.cards[0].vars.last_mut().unwrap().name = None;
    assert!(p.validate().unwrap_err().to_string().contains("name"));

    // 悬浮提示引用不存在的能力
    let mut p = m6_project();
    p.cards[0].hover_tip_powers = vec!["Nobody".into()];
    assert!(p.validate().unwrap_err().to_string().contains("Nobody"));

    // 关键词重名
    let mut p = m6_project();
    let dup = p.keywords[0].clone();
    p.keywords.push(dup);
    assert!(p.validate().unwrap_err().to_string().contains("重复"));

    // 合法项目通过
    assert!(m6_project().validate().is_ok());
}
