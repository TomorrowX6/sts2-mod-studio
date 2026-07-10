# STS2 Mod Studio

《杀戮尖塔2》mod 图形化制作工具——低代码/零代码制作 mod。

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

进游戏后战斗中按 `~` 控制台输入 `card MYMOD_CARD_SAMPLE_STRIKE` 即可拿到示例卡牌。

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

效果积木目前支持 `damage` / `draw` / `applyPower` / `custom`（原样插入 C#）。

## 桌面应用

```bash
cargo run -p sts2mod-studio
```

打开/新建项目 → 表单编辑清单、卡牌、数值、效果、双语文本 → 底部一键部署，日志实时输出。
工具链设置存全局配置（`~/.config/sts2mod/config.json` 或 `%APPDATA%/sts2mod`），
项目目录下可用 `sts2mod.local.json` 覆盖。

## 验证状态（2026-07-10，Linux 开发机）

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

- [x] M1 流水线打通：项目格式、卡牌代码生成、CLI、最小 UI
- [ ] M2 遗物 / 能力 / 药水编辑器、图片导入、本地化表格
- [ ] M3 积木式效果编辑器扩展（触发时机 × 指令 × 条件）
- [ ] M4 怪物 / 遭遇 / 事件 / 人物向导（内置 tscn 模板）
- [ ] M5 工坊上传（对接官方 sts2-mod-uploader）、导入已有 mod
