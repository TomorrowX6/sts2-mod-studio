//! 内容物 ID 规则（RitsuLib 约定）。
//!
//! RitsuLib 的 `ModContentRegistry.GetFixedPublicEntry` 生成
//! `{stem(modId)}_{stem(类别)}_{stem(类名)}` 三段 ID，三段都经过
//! `NormalizePublicStem`：非字母数字→`_`、驼峰/缩写边界插`_`、折叠去重、大写。
//! 例如 modId=MyMod、类名 SampleStrike → `MY_MOD_CARD_SAMPLE_STRIKE`
//! （已由 0.4.54 反编译与游戏内实测确认）。

/// 对齐 RitsuLib `NormalizePublicStem` 的实现。
pub fn normalize_public_stem(value: &str) -> String {
    let chars: Vec<char> = value.trim().chars().collect();
    let mut out = String::with_capacity(chars.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_ascii_alphanumeric() {
            out.push('_');
            continue;
        }
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
    // 折叠连续下划线并去除首尾
    let mut result = String::with_capacity(out.len());
    let mut prev_underscore = false;
    for c in out.chars() {
        if c == '_' {
            if !prev_underscore {
                result.push('_');
            }
            prev_underscore = true;
        } else {
            result.push(c);
            prev_underscore = false;
        }
    }
    result.trim_matches('_').to_string()
}

/// PascalCase / camelCase → 大写蛇形。`TestCard` → `TEST_CARD`，`HPUpCard` → `HP_UP_CARD`。
pub fn upper_snake(name: &str) -> String {
    normalize_public_stem(name)
}

/// mod id → 内容 ID 前缀。注意驼峰也会拆分：`MyMod` → `MY_MOD`。
pub fn mod_prefix(mod_id: &str) -> String {
    normalize_public_stem(mod_id)
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
    fn mod_prefix_camel_splits() {
        // 游戏内实测：MyMod 的卡 ID 是 MY_MOD_CARD_SAMPLE_STRIKE
        assert_eq!(mod_prefix("MyMod"), "MY_MOD");
        assert_eq!(mod_prefix("Test"), "TEST");
        assert_eq!(mod_prefix("test"), "TEST");
        assert_eq!(mod_prefix("My-Mod"), "MY_MOD");
        assert_eq!(mod_prefix("STS2-RitsuLib"), "STS2_RITSU_LIB");
        assert_eq!(mod_prefix("DemoMod"), "DEMO_MOD");
    }

    #[test]
    fn content_ids() {
        assert_eq!(content_id("Test", "CARD", "TestCard"), "TEST_CARD_TEST_CARD");
        assert_eq!(content_id("MyMod", "CARD", "SampleStrike"), "MY_MOD_CARD_SAMPLE_STRIKE");
    }
}
