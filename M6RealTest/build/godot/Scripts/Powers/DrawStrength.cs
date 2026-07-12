// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Powers;
using MegaCrit.Sts2.Core.ValueProps;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Scaffolding.Content;

namespace M6RealTest.Powers;

[RegisterPower]
public class DrawStrength : ModPowerTemplate
{
    // 类型：Buff 或 Debuff
    public override PowerType Type => PowerType.Buff;
    // 叠加类型：Counter 可叠加，Single 不可叠加
    public override PowerStackType StackType => PowerStackType.Counter;

    // 图标资源（小图 / 大图）
    public override PowerAssetProfile AssetProfile => new(
        IconPath: "res://M6RealTest/images/powers/DrawStrength.png",
        BigIconPath: "res://M6RealTest/images/powers/DrawStrength.png"
    );
}
