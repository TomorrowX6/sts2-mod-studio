# STS2 Mod Studio

[![CI](https://github.com/TomorrowX6/sts2-mod-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/TomorrowX6/sts2-mod-studio/actions/workflows/ci.yml)

《杀戮尖塔2》mod 图形化制作工具——低代码/零代码制作 mod。
不想自己编译的话，直接从 [Releases](https://github.com/TomorrowX6/sts2-mod-studio/releases)
下载对应平台的包（Windows 用户解压后运行 `sts2mod-studio.exe` 即可）。

基于 [RitsuLib](https://github.com/BAKAOLC/STS2-RitsuLib)。你在界面/项目文件里描述内容（卡牌、数值、效果、文本），
工具生成完整的 C# + Godot 工程并一键完成 编译 dll → 导出 pck → 部署到游戏 mods 目录。

## 组成

| 目录 | 说明 |
|------|------|
| `crates/core` | 核心库：项目模型、代码生成、构建流水线 |
| `crates/cli` | `sts2mod` 命令行 |
| `apps/studio` | Tauri 桌面应用（最小 UI，调用 core） |

## 环境要求（使用者机器）

按官方教程准备（详见 SlayTheSpire2ModdingTutorials/Basics/01）：

- 《杀戮尖塔2》本体（`public-beta`）
- [.NET SDK 9+](https://dotnet.microsoft.com/download)
- [Godot 4.5.1 Mono](https://godotengine.org/download/archive/4.5.1-stable/)（.NET 版）
- 游戏 `mods/` 里装好 RitsuLib（编译不需要，游戏内加载需要）

开发本仓库还需要 Rust stable；Linux 上编译 studio 需 `webkit2gtk-4.1`。

## CLI 快速上手

```bash
cargo build --release

# 一次性配置（Windows 路径示例）
sts2mod config set sts2Dir "D:/Steam/steamapps/common/Slay the Spire 2"
sts2mod config set godotExe "D:/godot/Godot_v4.5.1-stable_mono_win64.exe"
sts2mod doctor              # 环境自检

# 建项目 → 一键部署
sts2mod new MyMod
cd MyMod
sts2mod deploy              # 生成 + dotnet build + godot 导出 pck → 游戏 mods/
```

进游戏后战斗中按 `~` 控制台输入 `card MY_MOD_CARD_SAMPLE_STRIKE` 即可拿到示例卡牌
（`deploy` 成功后会打印每张卡的确切指令；注意 RitsuLib 对 modid 也做驼峰蛇形化：`MyMod` → `MY_MOD`）。

## 项目文件

一个 mod 项目 = 一个目录：

```text
MyMod/
├── project.stsmod.json   # 唯一事实来源：清单 + 内容定义（UI 和 CLI 都读写它）
├── assets/               # 卡图等素材（项目文件里按相对路径引用）
├── src/                  # 逃生舱：自定义 .cs 会被原样带进生成工程一起编译
└── build/godot/          # 生成产物（每次 generate 整体重建，勿手改）
```

卡牌定义示例（`project.stsmod.json` 片段）：

```json
{
  "className": "FireBall",
  "pool": "Colorless",
  "cardType": "Attack",
  "rarity": "Uncommon",
  "target": "AnyEnemy",
  "energyCost": 2,
  "portrait": "assets/cards/FireBall.png",
  "vars": [
    { "kind": "Damage", "value": 14, "props": ["Move"], "upgrade": 4 },
    { "kind": "Power", "power": "WeakPower", "value": 1 }
  ],
  "onPlay": [
    { "op": "damage" },
    { "op": "applyPower", "power": "WeakPower" }
  ],
  "text": {
    "zhs": { "title": "火球", "description": "造成{Damage:diff()}点伤害，给予{WeakPower:diff()}层[gold]虚弱[/gold]。" }
  }
}
```

效果积木一览（不同宿主可用性不同，非法组合在生成时报错）：

| 积木 | 说明 | 生成的指令（均经教程/反编译验证） |
|------|------|------|
| `damage` | 攻击伤害（吃力量、有动画），仅限有敌人目标的卡牌 | `DamageCmd.Attack(...).FromCard(this)...` |
| `directDamage` | 直接伤害/失去生命（默认不可格挡不吃力量），`toSelf` 可对自己 | `CreatureCmd.Damage(...)` |
| `block` | 获得格挡（吃敏捷） | `CreatureCmd.GainBlock(...)` |
| `heal` | 治疗自己 | `CreatureCmd.Heal(...)` |
| `draw` | 抽牌（能力上下文不可用） | `CardPileCmd.Draw(...)` |
| `applyPower` | 给予能力，`toSelf` 或给目标；能力宿主默认层数 `Amount` | `PowerCmd.Apply<X>(...)` |
| `gainGold` | 获得金币（能力上下文不可用） | `PlayerCmd.GainGold(...)` |
| `playSfx` | 播放音效，如 `event:/sfx/block_gain` | `SfxCmd.Play(...)` |
| `playVfx` | 播放特效，如 `vfx/vfx_bloody_impact` | `VfxCmd.PlayOnCreature(...)` |
| `if` | 条件分支：`when`（C# 布尔表达式）+ `then`/`else` 子效果，可嵌套 | `if (...) { ... } else { ... }` |
| `repeat` | 重复 `times` 次，子效果可嵌套 | `for` 循环 |
| `custom` | 原样插入 C# 代码 | — |

数值来源统一为：`var`（引用数值名）> `amount`（字面量）> 默认（能力触发器为 `Amount`，否则同名数值）。

能力触发器新增 `AfterOwnerTurnEnd`（己方回合结束后）：生成真实钩子 `AfterSideTurnEnd` 并自动加
`side != Owner.Side` 守卫（惯用法取自 RitsuLib 源码）。

除卡牌外还支持 **遗物 / 能力 / 药水**（`relics` / `powers` / `potions` 数组）：

- 遗物：稀有度、数值、触发器（`AfterPlayerTurnStart`）、文本（含 flavor）
- 能力：Buff/Debuff、叠加方式、触发器（`AfterCardDrawn`，applyPower 默认层数为 `Amount`）、
  文本（含 `smartDescription`，可用 `{Amount}`）
- 药水：稀有度、使用方式、目标、`onUse` 效果、文本
- 所有内容都有 `extraCode` 逃生舱：原样插入类体，可重写白名单外的任意钩子
- 触发器白名单外的钩子名会在生成时报错（防止拼错静默失效）

M6 新增**卡牌深化**（均经教程与 RitsuLib 0.4.54 反编译核对）：

- **关键词**：卡牌加 `keywords`——原版名（`Exhaust` 等）直接写，自定义关键词在
  项目级 `keywords` 定义（标识/图标/描述插入位置/双语文本），生成
  `[RegisterOwnedCardKeyword]` 注册类与 `card_keywords.json`
- **标签**：卡牌加 `tags`（原版 `Strike`/`Defend` 或项目级 `cardTags` 自定义，
  生成 `[RegisterOwnedCardTag]`；打击木偶等按标签判定）
- **自定义命名数值**：数值种类选 `Custom` + 变量名（描述里 `{Leech}` 引用），
  生成 `ModCardVars.Int`；填了悬浮提示文本则自动
  `.WithSharedTooltip` + `static_hover_tips.json`
- **额外悬浮提示**：`hoverTipCards` / `hoverTipPowers` 引用本项目内容，
  生成 `AdditionalHoverTips`（悬浮时旁侧展示卡牌预览/能力说明）
- 编辑器实时预览会按位置渲染关键词行（金色）、解析自定义变量数值

M4 新增 **怪物 / 遭遇 / 事件 / 人物**（`monsters` / `encounters` / `events` / `characters` 数组）：

- **怪物**：血量区间、招式列表（意图 `attack`/`defend`/`custom` + 效果 + 每招标题/对白），
  招式按顺序循环（`ModMonsterMoveStateMachines.Cycle`）。战斗场景用内置 tscn 模板 +
  你的图片自动生成（含 Visuals/Bounds/IntentPos/CenterPos/TalkPos 唯一名节点），
  也可用 `scene` 指定自制场景。怪物没有 DynamicVars，效果数值须填固定值。
- **遭遇**：房间类型（Monster/Elite/Boss）、注册到幕（`Overgrowth`/`Hive`/`Glory`，
  不选则 `[RegisterGlobalEncounter]`）、弱怪池标记、出场怪物列表。多怪自动生成
  Marker2D 槽位场景（每排 4 个，槽位名可自定义）。控制台 `fight <遭遇ID>` 可直接进入。
- **事件**：多页面状态机——第一页固定 `INITIAL`，选项执行效果后跳到下一页，
  无选项的页面即结束页；`condition` 生成 `IsAllowed`。事件专用积木：
  `loseGold` / `rewardCards` / `rewardPotion` / `startCombat`（战斗事件自动补
  `LayoutType.Combat` 与 `CanonicalEncounter`）。控制台 `event <事件ID>` 可直接进入。
- **人物**：向导式——主题色、性别、血量金币、初始卡组/遗物（引用本项目内容）、
  七类图片。自动生成三件套卡池/遗物池/药水池（`{类名}CardPool` 等，把卡牌的"池"
  字段填成它即可入池），未提供的资源经 `CharacterAssetProfiles.Merge` 回退到所选
  原版人物（铁甲/静默/缺陷/摄政/缚灵）。战斗模型、能量表盘、头像、选人背景四个
  tscn 场景由模板生成；`characters.json` 人称代词按性别给默认值；先古对话
  `ancients.json` 生成占位文本（发布前建议润色）。

## 发布到创意工坊

对接官方 [sts2-mod-uploader](https://github.com/megacrit/sts2-mod-uploader)：

```bash
sts2mod config set modUploaderExe "D:/tools/sts2-mod-uploader/ModUploader.exe"
sts2mod publish              # deploy + 组装 workshop/ 工作区 + 调用上传器
sts2mod publish --skip-deploy  # 跳过构建，直接上传现有产物
```

- 发布信息写在项目文件的 `workshop` 段：`previewImage`（png、<1MB，Steam 硬性要求）、
  `tags`（**上传后无法在网页修改**，常用 `Cards`/`Characters`/`QoL`/`schinese`/`English`）、
  `changeNote`、`visibility`（仅首次写入，默认 private）、`dependencies`（工坊条目数字 ID）
- 工作区在 `<项目>/workshop/`（结构与官方 uploader 一致）；首次发布会生成
  `workshop/mod_id.txt`——**请随项目提交**，它标识你的工坊条目
- 更新发布只写入 tags/changeNote 等明确设置的字段，工坊网页上手工改过的标题、
  描述等不会被覆盖（官方约定：缺省字段=保持不变）
- 需要 Steam 客户端正在运行；成功后打印工坊链接

## 导入已有 mod

把一个已部署的 mod（游戏 `mods/<id>/` 或工坊订阅目录，含 `<id>.json` + `<id>.pck`）
反向导入为可编辑项目：

```bash
sts2mod import "D:/.../mods/SomeMod" ./SomeModProject
```

能恢复：清单、全部本地化文本（卡牌/遗物/能力/药水会按 RitsuLib 键规则反推成
内容条目）、pck 里的图片（Godot `.ctex` 无损纹理自动还原为 webp/png）。
**不能恢复：C# 逻辑**——效果/数值/触发器在 dll 里不可逆，导入后为默认值，
需在编辑器里重填。怪物/事件等复杂内容不做反推，原始本地化文件保留在
`assets/imported/localization/` 供参考。自研 pck 解析器支持 Godot 4.4+ 的
PCK 格式版本 3（对 Godot 4.5.1 真实导出逐字节验证）。

## 桌面应用

```bash
cargo run -p sts2mod-studio
```

打开/新建项目 → 表单编辑清单、卡牌、数值、效果、双语文本 → 底部一键部署，日志实时输出。
WinUI 风格界面（跟随系统深浅色）；卡牌编辑带**整卡实时预览**——真实游戏卡框按
卡池换色（换色着色器逐行移植自 RitsuLib `ReplaceHue`，人物专属池显示主题色）、
占位符与 BBCode 颜色/特效实时渲染、一键切换升级后数值；遗物/能力/药水显示
游戏悬浮提示样式预览。
工具链设置存全局配置（`~/.config/sts2mod/config.json` 或 `%APPDATA%/sts2mod`），
项目目录下可用 `sts2mod.local.json` 覆盖。

## 验证状态

**2026-07-12 真机测试通过**（Windows + 游戏本体，按 TESTING.md 全量清单执行，
未反馈问题）：覆盖 M2~M6 全部内容类型与效果积木、工坊发布与导入流程、
UI 与实时预览。下方各"待真机确认"推断点均未报告异常，视为通过；
后续版本更新若出现行为变化，仍以该清单为回归基准。

## 离线验证记录（2026-07-12，Linux 开发机）

M5（工坊发布 / 导入）已验证：

- workshop.json 字段与工作区结构对官方 sts2-mod-uploader **源码**核对
  （ModConfig.cs / UploadCommand.cs / 模板）：`content/`、`image.png`（<1MB）、
  可见性四值、`upload -w <目录> [-i id]`、缺省字段不覆盖等
- pck 解析器对 Godot 4.5.1 真实导出的 pck 逐字节验证（格式 v3：目录在尾部、
  路径无 res:// 前缀、偏移相对 fileBase）；`.ctex` 无损纹理还原为 webp 且
  round-trip（导入的项目可直接 generate）
- 测试 fixture `crates/core/tests/fixtures/TestMod/` 即真实导出的 pck

M5 曾待真机确认（2026-07-12 已通过）：`ModUploader.exe upload` 全流程。
中文教程描述的 `workspace.json`/`image.jpg` 为旧版说法，
以本工具生成的 `workshop.json`/`image.png` 为准（与官方源码一致）。

## 离线验证记录（2026-07-11，Linux 开发机）

M4（怪物/遭遇/事件/人物）已在无游戏环境验证：

- 模板类与辅助 API 全部对 RitsuLib 0.4.54 反编译核对：`ModMonsterTemplate` /
  `ModEncounterTemplate` / `ModEventTemplate` / `ModCharacterTemplate<,,>`、
  `MonsterAssetProfile` / `EncounterAssetProfile` / `EventAssetProfile`、
  `ModMonsterMoveStateMachines.Cycle`、`CharacterAssetProfiles.Merge/Ironclad/...`、
  `TypeList*PoolModel`、`StartingDeckEntry`、`InitialOptionKey/ModOptionKey/PageDescription`、
  五个注册属性、幕类型 `Overgrowth/Hive/Glory`，签名全部一致
- 生成工程 stub 编译（无 sts2.dll）：报错全部指向缺失的 sts2.dll，无 RitsuLib 误用
- 怪物移动/事件页面等 C# 结构逐行对照官方教程原文

M4 曾待真机确认的推断点（2026-07-12 真机测试未报告异常）：

- 遭遇本地化键推断为 `{MOD}_ENCOUNTER_{类名}.title/.loss`（教程该节疑似未更新，
  若游戏内不显示标题请反馈）
- `RoomType.Elite` / `RoomType.Boss` 枚举名、`CharacterGender.Feminine/Neutral` 枚举名
- 怪物招式中 `applyPower` 给目标用 `targets[0]`（教程未覆盖，仅验证了给自己）
- 先古对话 `THE_ARCHITECT.*-attack` 键名的前缀形式

## 早期验证状态（2026-07-10，Linux 开发机）

无游戏本体的环境下已验证：

- 代码生成快照测试 5 项全过；CLI `new → generate → doctor` 端到端
- `dotnet build`（.NET SDK 9.0.118）：NuGet 成功还原 `Godot.NET.Sdk/4.5.1` 与 `STS2.RitsuLib`
- 生成代码对 **RitsuLib 0.4.54 真实包**反编译比对：`ModCardTemplate` 五参构造、
  `RegisterCardAttribute(Type)`、`CreateLogger` / `EnsureGodotScriptsRegistered` /
  `RegisterModAssembly` 签名全部一致；stub 编译的全部报错均指向缺失的 sts2.dll，无 RitsuLib 误用
- Godot 4.5.1 Mono 无头导出 pck 成功，pck 内含 `{modid}/localization/{lang}/cards.json`
  且 ID 键名正确；产物正确落到 `<游戏目录>/mods/<ModId>/`
- 构建失败时 CLI 正确返回非零退出码；Tauri 应用 Xvfb 冒烟测试通过

待真实环境（装有游戏的 Windows 机器）验证：

- 引用真实 sts2.dll 的完整编译（`damage` 效果链出自官方教程原文；`draw` / `applyPower`
  两个积木的签名由教程推断，如报错请反馈调整模板）
- 游戏内加载与卡牌实际效果

## 路线图

- [x] M1 流水线打通：项目格式、卡牌代码生成、CLI、最小 UI（已在真机游戏内验证）
- [x] M2 遗物 / 能力 / 药水编辑器、图片一键导入、extraCode 逃生舱
- [x] M3 效果积木扩展：8 种新积木（格挡/治疗/直伤/金币/音效/特效/条件/循环，支持嵌套）、能力新触发器
- [x] M4 怪物 / 遭遇 / 事件 / 人物向导（内置 tscn 模板、事件多页面、人物三池自动生成）
- [x] M5 工坊上传（`sts2mod publish` 对接官方 sts2-mod-uploader）、导入已有 mod（pck 反向解析）
- [x] M6 卡牌深化：自定义关键词 / 卡牌标签 / 命名数值+悬浮提示 / AdditionalHoverTips
- [x] CI/发版：push 跑测试；`git tag v* && git push origin v*` 自动出三平台包（GitHub Actions）
