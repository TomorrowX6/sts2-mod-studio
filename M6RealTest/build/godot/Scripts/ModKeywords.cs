// 由 sts2mod 生成，勿手改（每次生成会覆盖）。自定义代码请放在项目的 src/ 目录。
using MegaCrit.Sts2.Core.Entities.Cards;
using STS2RitsuLib.Content;
using STS2RitsuLib.Interop.AutoRegistration;
using STS2RitsuLib.Keywords;

namespace M6RealTest;

// 自定义卡牌关键词（卡牌的"关键词"字段按名称引用；教程注明：不能是 static 类）
[RegisterOwnedCardKeyword(nameof(Unique), CardDescriptionPlacement = ModKeywordCardDescriptionPlacement.BeforeCardDescription)]
public class ModKeywords
{
    public static readonly CardKeyword Unique =
        ModContentRegistry.GetQualifiedKeywordId(Entry.ModId, nameof(Unique)).GetModCardKeyword();
}
