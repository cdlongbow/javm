//! 别名文本处理：归一化、语言判断、书写体系排序。纯函数，无 DB 依赖。

/// 归一化匹配键：全角→半角、小写、去除所有空白。
/// 例：「三上 悠亜」「三上悠亜」「Yua Mikami」「yuamikami」分别归一为可匹配键。
pub fn normalize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        let mapped = match ch {
            '\u{3000}' => ' ',
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
            _ => ch,
        };
        if mapped.is_whitespace() {
            continue;
        }
        out.extend(mapped.to_lowercase());
    }
    out
}

/// 粗略语言判断（仅作 `lang` 字段提示，排序/canonical 用 [`script_rank`]）。
pub fn detect_lang(name: &str) -> &'static str {
    let mut has_kana = false;
    let mut has_cjk = false;
    let mut has_ascii_alpha = false;
    for ch in name.chars() {
        let u = ch as u32;
        if (0x3040..=0x30FF).contains(&u) {
            has_kana = true;
        } else if (0x4E00..=0x9FFF).contains(&u) {
            has_cjk = true;
        } else if ch.is_ascii_alphabetic() {
            has_ascii_alpha = true;
        }
    }
    if has_kana {
        "ja"
    } else if has_cjk {
        "zh"
    } else if has_ascii_alpha {
        "en"
    } else {
        "unknown"
    }
}

/// 查询/canonical 偏好按**书写体系**排序（不靠不可靠的 ja/zh 判别）：
/// 含假名→0；含汉字→1；纯 ASCII(罗马音)→2。源偏好的日文/汉字名自然排在前。
pub(super) fn script_rank(name: &str) -> u8 {
    let mut has_kana = false;
    let mut has_cjk = false;
    for ch in name.chars() {
        let u = ch as u32;
        if (0x3040..=0x30FF).contains(&u) {
            has_kana = true;
        } else if (0x4E00..=0x9FFF).contains(&u) {
            has_cjk = true;
        }
    }
    if has_kana {
        0
    } else if has_cjk {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_handles_width_case_space() {
        assert_eq!(normalize_name("Yua Mikami"), "yuamikami");
        assert_eq!(normalize_name("三上 悠亜"), "三上悠亜");
        assert_eq!(normalize_name("ＳＯＤ"), "sod"); // 全角字母
    }

    #[test]
    fn detect_lang_basic() {
        assert_eq!(detect_lang("深田えいみ"), "ja"); // 含假名
        assert_eq!(detect_lang("三上悠亚"), "zh"); // 纯表意
        assert_eq!(detect_lang("Yua Mikami"), "en");
    }

    #[test]
    fn script_rank_orders_kana_then_cjk_then_ascii() {
        assert!(script_rank("えいみ") < script_rank("三上悠亜"));
        assert!(script_rank("三上悠亜") < script_rank("Yua Mikami"));
    }
}
