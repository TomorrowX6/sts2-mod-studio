//! M5：工坊发布工作区组装 + pck 解析 / mod 导入（fixture 为 Godot 4.5.1 真实导出）。

use std::path::{Path, PathBuf};

use sts2mod_core::model::{self, WorkshopDef};
use sts2mod_core::{import, pck, pipeline};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// 每个测试独立的临时目录（进程退出后由系统清理 /tmp）。
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sts2mod-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn pck_reader_parses_real_godot_export() {
    let mut p = pck::Pck::open(&fixture("TestMod/TestMod.pck")).unwrap();
    let paths: Vec<&str> = p.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"TestMod/localization/zhs/cards.json"), "实际: {paths:?}");
    assert!(paths.contains(&"TestMod/localization/en/cards.json"));
    assert!(paths.contains(&"TestMod/images/cards/FireBall.png.import"));

    // 内容可读且是合法 JSON
    let entry = p
        .entries
        .iter()
        .find(|e| e.path == "TestMod/localization/zhs/cards.json")
        .cloned()
        .unwrap();
    let bytes = p.read(&entry).unwrap();
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(map["TEST_MOD_CARD_FIRE_BALL.title"], "火球");

    // ctex 内嵌图片可抠出
    let ctex_entry = p
        .entries
        .iter()
        .find(|e| e.path.ends_with(".ctex"))
        .cloned()
        .expect("应有导入纹理");
    let ctex = p.read(&ctex_entry).unwrap();
    let (ext, img) = pck::extract_ctex_image(&ctex).expect("无损纹理应可还原");
    assert_eq!(ext, "webp");
    assert_eq!(&img[..4], b"RIFF");
}

#[test]
fn import_mod_scaffolds_project() {
    let out = temp_dir("import");
    let mut log = |_: &str| {};
    let summary = import::import_mod(&fixture("TestMod"), &out, &mut log).unwrap();
    assert_eq!(summary.cards, 1);
    assert_eq!(summary.relics, 1);
    assert_eq!(summary.images, 1);
    assert_eq!(summary.localization_files, 3);

    let project = model::Project::load(&out).unwrap();
    assert_eq!(project.manifest.id, "TestMod");
    assert_eq!(project.manifest.version, "1.2.0"); // 两段版本号补全
    let card = &project.cards[0];
    assert_eq!(card.class_name, "FireBall");
    assert_eq!(card.text["zhs"].title, "火球");
    assert_eq!(card.text["en"].title, "Fire Ball");
    assert_eq!(card.portrait.as_deref(), Some("assets/imported/images/cards/FireBall.webp"));
    assert!(card.on_play.is_empty(), "逻辑不可导入，应留空");
    assert_eq!(project.relics[0].class_name, "LuckyCoin");
    assert_eq!(project.relics[0].text["zhs"].flavor, "叮当。");

    // 还原的图片是合法 webp
    let img = std::fs::read(out.join("assets/imported/images/cards/FireBall.webp")).unwrap();
    assert_eq!(&img[..4], b"RIFF");

    // 导入的骨架可以直接再生成
    let gen = sts2mod_core::codegen::generate(&project).unwrap();
    assert!(gen.files.iter().any(|f| f.rel_path.to_string_lossy() == "Scripts/Cards/FireBall.cs"));

    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn workshop_workspace_assembly() {
    let project_dir = temp_dir("ws-project");
    let artifacts = temp_dir("ws-artifacts");

    let mut project = model::starter_project("Test", "测试");
    project.workshop = Some(WorkshopDef {
        preview_image: Some("assets/preview.png".into()),
        tags: vec!["Cards".into(), "schinese".into()],
        change_note: "first".into(),
        ..Default::default()
    });
    project.save(&project_dir).unwrap();
    std::fs::create_dir_all(project_dir.join("assets")).unwrap();
    std::fs::write(project_dir.join("assets/preview.png"), b"\x89PNG fake").unwrap();
    for f in ["Test.json", "Test.dll", "Test.pck"] {
        std::fs::write(artifacts.join(f), b"artifact").unwrap();
    }

    let mut log = |_: &str| {};
    let ws = pipeline::assemble_workshop_workspace(&project, &project_dir, &artifacts, &mut log)
        .unwrap();

    for f in ["content/Test.json", "content/Test.dll", "content/Test.pck", "image.png"] {
        assert!(ws.join(f).exists(), "缺少 {f}");
    }
    // 首次发布：补全标题/描述/可见性（默认 private）
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ws.join("workshop.json")).unwrap()).unwrap();
    assert_eq!(json["title"], "测试");
    assert_eq!(json["visibility"], "private");
    assert_eq!(json["changeNote"], "first");
    assert_eq!(json["tags"], serde_json::json!(["Cards", "schinese"]));
    assert!(json.get("dependencies").is_none(), "空依赖不应写入");

    // 已有 mod_id.txt = 更新发布：不写标题/可见性，避免覆盖工坊现值
    std::fs::write(ws.join("mod_id.txt"), "123456").unwrap();
    pipeline::assemble_workshop_workspace(&project, &project_dir, &artifacts, &mut log).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ws.join("workshop.json")).unwrap()).unwrap();
    assert!(json.get("title").is_none());
    assert!(json.get("visibility").is_none());
    assert_eq!(json["changeNote"], "first");

    // 预览图超 1MB 拒绝
    std::fs::write(project_dir.join("assets/preview.png"), vec![0u8; 1024 * 1024 + 1]).unwrap();
    let err = pipeline::assemble_workshop_workspace(&project, &project_dir, &artifacts, &mut log)
        .err()
        .unwrap();
    assert!(err.to_string().contains("1MB"), "实际: {err}");

    // 缺 dll 拒绝
    std::fs::write(project_dir.join("assets/preview.png"), b"\x89PNG fake").unwrap();
    std::fs::remove_file(artifacts.join("Test.dll")).unwrap();
    let err = pipeline::assemble_workshop_workspace(&project, &project_dir, &artifacts, &mut log)
        .err()
        .unwrap();
    assert!(err.to_string().contains("Test.dll"), "实际: {err}");

    std::fs::remove_dir_all(&project_dir).ok();
    std::fs::remove_dir_all(&artifacts).ok();
}

#[test]
fn workshop_validation() {
    let mut p = model::starter_project("Test", "x");
    p.workshop = Some(WorkshopDef { visibility: Some("hidden".into()), ..Default::default() });
    assert!(p.validate().unwrap_err().to_string().contains("visibility"));

    let mut p = model::starter_project("Test", "x");
    p.workshop = Some(WorkshopDef { preview_image: Some("a.jpg".into()), ..Default::default() });
    assert!(p.validate().unwrap_err().to_string().contains("png"));

    let mut p = model::starter_project("Test", "x");
    p.workshop = Some(WorkshopDef {
        preview_image: Some("a.png".into()),
        visibility: Some("public".into()),
        content_descriptors: vec!["general_mature".into()],
        ..Default::default()
    });
    assert!(p.validate().is_ok());
}
