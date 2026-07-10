//! 内容物 ID 规则（RitsuLib 约定）。
//!
//! RitsuLib 注册的内容 ID 形如 `{MODID}_{类别}_{类名的大写蛇形}`，
//! 例如 modid=Test、类别=CARD、类名 TestCard → `TEST_CARD_TEST_CARD`。

/// PascalCase / camelCase → 大写蛇形。`TestCard` → `TEST_CARD`，`HPUpCard` → `HP_UP_CARD`。
pub fn upper_snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).map(|n| n.is_ascii_lowercase()).unwrap_or(false);
            // 小写/数字后遇大写，或大写串结束进入新单词（如 HPUp 的 U）
            if prev.is_ascii_lowercase() || prev.is_ascii_digit() || (prev.is_ascii_uppercase() && next_lower) {
                out.push('_');
            }
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// mod id → 内容 ID 前缀：大写，非字母数字折叠为 `_`。
pub fn mod_prefix(mod_id: &str) -> String {
    mod_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .collect()
}

/// 完整内容 ID，category 传 "CARD"、"RELIC"、"POWER" 等。
pub fn content_id(mod_id: &str, category: &str, class_name: &str) -> String {
    format!("{}_{}_{}", mod_prefix(mod_id), category, upper_snake(class_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_rules() {
        assert_eq!(upper_snake("TestCard"), "TEST_CARD");
        assert_eq!(upper_snake("SampleStrike"), "SAMPLE_STRIKE");
        assert_eq!(upper_snake("HPUpCard"), "HP_UP_CARD");
        assert_eq!(upper_snake("Card2Test"), "CARD2_TEST");
    }

    #[test]
    fn content_ids() {
        assert_eq!(content_id("Test", "CARD", "TestCard"), "TEST_CARD_TEST_CARD");
        assert_eq!(content_id("My-Mod", "CARD", "FooBar"), "MY_MOD_CARD_FOO_BAR");
    }
}
