# 真机测试教程（M2 + M3 验证）

目标：在装有《杀戮尖塔2》的 Windows 机器上，验证工具生成的**遗物 / 能力 / 药水**以及
**全部 12 种效果积木**能正常编译并在游戏内按预期工作。

预计耗时：约 20 分钟。

## 0. 前置条件

- 游戏本体（public-beta）+ `mods/` 里已装 RitsuLib（M1 验证时已装好就不用动）
- .NET SDK 9+、Godot 4.5.1 Mono（M1 已配置过则跳过）
- Rust 工具链（编译本工具用）

## 1. 编译工具并检查环境

```powershell
git pull
cargo build --release
# 以下用 target/release/sts2mod，或把它加进 PATH

# M1 时已配置过可跳过这两行
sts2mod config set sts2Dir "D:/Steam/steamapps/common/Slay the Spire 2"
sts2mod config set godotExe "D:/godot/Godot_v4.5.1-stable_mono_win64.exe"

sts2mod doctor   # 应当全部 ✔（RitsuLib 一项也要 ✔）
```

## 2. 创建测试项目

```powershell
sts2mod new StudioTest
cd StudioTest
```

把 `project.stsmod.json` 整个替换为下面的内容（覆盖了所有内容类型和所有积木）：

```json
{
  "formatVersion": 1,
  "manifest": {
    "id": "StudioTest",
    "name": "Studio 测试包",
    "author": "tester",
    "description": "M2+M3 验证用",
    "version": "0.1.0",
    "minGameVersion": "0.107.1",
    "affectsGameplay": true,
    "dependencies": [{ "id": "STS2-RitsuLib", "minVersion": "0.2.27" }]
  },
  "cards": [
    {
      "className": "KitchenSink",
      "pool": "Colorless",
      "cardType": "Attack",
      "rarity": "Common",
      "target": "AnyEnemy",
      "energyCost": 1,
      "vars": [
        { "kind": "Damage", "value": 6, "props": ["Move"], "upgrade": 3 },
        { "kind": "Block", "value": 4 },
        { "kind": "Cards", "value": 1 },
        { "kind": "Gold", "value": 5 }
      ],
      "onPlay": [
        { "op": "damage" },
        { "op": "block" },
        { "op": "heal", "amount": 2 },
        { "op": "gainGold" },
        { "op": "playSfx", "event": "event:/sfx/block_gain" },
        { "op": "playVfx", "path": "vfx/vfx_bloody_impact" },
        {
          "op": "if",
          "when": "Owner.Creature.Block > 10",
          "then": [{ "op": "draw" }],
          "else": [{ "op": "heal", "amount": 1 }]
        },
        { "op": "repeat", "times": 2, "do": [{ "op": "playVfx", "path": "vfx/vfx_block", "onSelf": true }] }
      ],
      "text": {
        "zhs": {
          "title": "全家桶",
          "description": "造成{Damage:diff()}点伤害，获得{Block:diff()}点格挡，回复2点生命，获得{Gold}金币。若格挡大于10，抽{Cards:diff()}张牌，否则再回复1点生命。"
        }
      }
    },
    {
      "className": "BloodPact",
      "pool": "Colorless",
      "cardType": "Skill",
      "rarity": "Common",
      "target": "Self",
      "energyCost": 0,
      "vars": [{ "kind": "Block", "value": 6 }],
      "onPlay": [
        { "op": "directDamage", "amount": 3, "toSelf": true },
        { "op": "block" },
        { "op": "applyPower", "power": "StrengthPower", "amount": 2, "toSelf": true }
      ],
      "text": {
        "zhs": {
          "title": "血契",
          "description": "失去3点生命，获得{Block:diff()}点格挡和2点[gold]力量[/gold]。"
        }
      },
      "extraCode": "// extraCode 编译验证标记\nprivate const int ExtraCodeMarker = 1;"
    }
  ],
  "relics": [
    {
      "className": "TurnEngine",
      "pool": "Shared",
      "rarity": "Common",
      "vars": [
        { "kind": "Cards", "value": 1 },
        { "kind": "Gold", "value": 3 }
      ],
      "triggers": [
        {
          "trigger": "AfterPlayerTurnStart",
          "effects": [
            { "op": "draw" },
            { "op": "gainGold" },
            { "op": "playSfx", "event": "event:/sfx/block_gain" }
          ]
        }
      ],
      "text": {
        "zhs": {
          "title": "回合引擎",
          "description": "每回合开始时，抽[blue]{Cards}[/blue]张牌并获得[blue]{Gold}[/blue]金币。",
          "flavor": "咔哒，咔哒。"
        }
      }
    }
  ],
  "powers": [
    {
      "className": "DrawStrength",
      "powerType": "Buff",
      "stackType": "Counter",
      "triggers": [
        {
          "trigger": "AfterCardDrawn",
          "effects": [{ "op": "applyPower", "power": "StrengthPower", "toSelf": true }]
        }
      ],
      "text": {
        "zhs": {
          "title": "抽牌怒火",
          "description": "每次抽牌时，获得力量。",
          "smartDescription": "每次抽牌时，获得[blue]{Amount}[/blue]点[gold]力量[/gold]。"
        }
      }
    },
    {
      "className": "SlowBurn",
      "powerType": "Debuff",
      "stackType": "Counter",
      "triggers": [
        {
          "trigger": "AfterOwnerTurnEnd",
          "effects": [
            { "op": "directDamage", "amount": 2, "toSelf": true },
            { "op": "playVfx", "path": "vfx/vfx_bite", "onSelf": true }
          ]
        }
      ],
      "text": {
        "zhs": {
          "title": "缓燃",
          "description": "己方回合结束时受到伤害。",
          "smartDescription": "持有者回合结束时，受到2点不可格挡伤害。当前[blue]{Amount}[/blue]层。"
        }
      }
    }
  ],
  "potions": [
    {
      "className": "AcidFlask",
      "pool": "Shared",
      "rarity": "Common",
      "usage": "CombatOnly",
      "target": "AnyEnemy",
      "vars": [{ "kind": "Damage", "value": 8 }],
      "onUse": [
        { "op": "directDamage" },
        { "op": "applyPower", "power": "WeakPower", "amount": 2 }
      ],
      "text": {
        "zhs": {
          "title": "酸液瓶",
          "description": "对目标造成{Damage}点伤害并给予2层[gold]虚弱[/gold]。"
        }
      }
    },
    {
      "className": "TonicOfInsight",
      "pool": "Shared",
      "rarity": "Uncommon",
      "usage": "CombatOnly",
      "target": "Self",
      "vars": [{ "kind": "Cards", "value": 2 }],
      "onUse": [
        { "op": "draw" },
        { "op": "block", "amount": 5 }
      ],
      "text": {
        "zhs": {
          "title": "洞察药剂",
          "description": "抽{Cards}张牌，获得5点格挡。"
        }
      }
    }
  ]
}
```

## 3. 一键部署

```powershell
sts2mod deploy
```

预期：`dotnet build` 无错误、pck 导出成功，最后打印每个内容物的获取指令。
**如果编译报错，把错误全文发回来**——这是本轮测试最有价值的产出。

## 4. 游戏内验证清单

启动游戏（确认右下角"已加载模组"）→ 开一局 → 进入任意战斗 → 按 `~` 开控制台。

建议先输 `instant`（跳过动画延迟）。输 `dump` 可把所有已注册 ID 打进日志，
确认 7 个 `STUDIO_TEST_*` ID 都在。

| # | 指令 | 期望表现 |
|---|------|----------|
| 1 | `card STUDIO_TEST_CARD_KITCHEN_SINK` | 打出后：目标扣 6 血、+4 格挡、回 2 血、+5 金币、有音效、目标身上有特效；因格挡只有 4（≤10）走 else 分支**再回 1 血不抽牌**；自身特效播 2 次 |
| 2 | `upgrade 0`（该卡在最左）再打出 | 伤害变 9，其余不变 |
| 3 | `card STUDIO_TEST_CARD_BLOOD_PACT` | 打出后：失去 3 点生命（不可格挡）、+6 格挡、+2 力量 |
| 4 | `relic add STUDIO_TEST_RELIC_TURN_ENGINE` 然后结束回合 | 新回合开始：多抽 1 张牌、+3 金币、有音效；遗物悬浮描述与风味文本正常 |
| 5 | `power STUDIO_TEST_POWER_DRAW_STRENGTH 2 0` 然后 `draw 1` | 每抽 1 张牌获得 **2** 点力量（层数=Amount）；悬浮自己看动态描述显示 2 |
| 6 | `power STUDIO_TEST_POWER_SLOW_BURN 3 1` 然后结束回合 | **敌人**回合结束时敌人受 2 点不可格挡伤害并播特效；你回合结束时不触发 |
| 7 | `potion STUDIO_TEST_POTION_ACID_FLASK`，对敌人使用 | 敌人扣 8 血 + 2 层虚弱 |
| 8 | `potion STUDIO_TEST_POTION_TONIC_OF_INSIGHT`，使用 | 抽 2 张牌 + 5 格挡 |

文本抽查：卡牌描述里 `{Damage:diff()}` 应显示为数字且升级后变绿；图片全部是
RitsuLib 占位图（本项目故意不配图，`art card` 可列出缺图内容）。

## 5. UI（桌面端）验证

```powershell
cargo run --release -p sts2mod-studio
```

1. 「打开项目…」选择 StudioTest → 左侧应出现 2 卡牌 / 1 遗物 / 2 能力 / 2 药水
2. 点开 KitchenSink → 效果列表应正确显示嵌套的 if/repeat 块
3. 随便新建一个遗物，用「选择图片…」导入任意 png → 路径自动填入，`assets/relics/` 出现文件
4. 底部「一键部署」→ 日志实时滚动，结束后打印指令清单
5. 保存后 `project.stsmod.json` 内容无损（嵌套效果结构保持）

## 6. 出问题时收集什么

- **编译错误**：`sts2mod deploy` 的完整输出（重点是 `error CS` 行）
- **游戏内异常**：控制台红字/异常堆栈截图；`open logs` 打开日志目录，取最新日志
- **表现不符**：说明第几项、实际表现 vs 期望表现

## 7. 本轮重点观察点（推断而非验证的表达式）

这些是最可能出编译错误的地方，出错请原文发回：

1. 遗物触发器里的 `player.Creature`（draw 用 `player` 是教程原文，`.Creature` 是推断）
2. 药水 `OnUse` 里的 `Owner`（draw）与 `Owner.Creature`（block/heal/directDamage）
3. `CardPileCmd.Draw` / `PowerCmd.Apply` 在卡牌 `OnPlay` 里的用法（M1 遗留未验证）
4. `PlayerCmd.GainGold(n, Owner)` 在卡牌上下文中 `Owner` 是否是 `Player` 类型
5. 能力触发器 `AfterSideTurnEnd` + `Owner.Side` 守卫（签名来自反编译，应当稳）
6. VFX 路径（`vfx/vfx_bloody_impact` 等来自教程特效清单，路径无效顶多不显示，不会崩）
