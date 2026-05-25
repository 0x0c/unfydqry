use unicode_normalization::UnicodeNormalization;

fn katakana_to_hiragana(c: char) -> char {
    match c as u32 {
        // 濁点付き(ガ=U+30AC, ヴ=U+30F4 等)も -0x60 で正しく写る
        0x30A1..=0x30F6 => char::from_u32(c as u32 - 0x60).unwrap_or(c),
        _ => c,
    }
}

/// 大小・全半角・かな種別を畳み込む。濁点/半濁点は保持する。
pub fn normalize_loose(input: &str) -> String {
    input
        .nfkc()
        .map(katakana_to_hiragana)
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 設計書 §2.2 のトレース表をそのまま検証する。
    #[test]
    fn dakuten_kept_kana_unified() {
        // 濁点ありは「が」に揃う
        for s in ["ガ", "が", "ｶﾞ"] {
            assert_eq!(normalize_loose(s), "が", "input={s}");
        }
        // 濁点なしは「か」に揃う(「が」とは別キー)
        for s in ["カ", "か", "ｶ"] {
            assert_eq!(normalize_loose(s), "か", "input={s}");
        }
        assert_ne!(normalize_loose("が"), normalize_loose("か"));
    }

    #[test]
    fn handakuten_kept_kana_unified() {
        for s in ["パ", "ぱ", "ﾊﾟ"] {
            assert_eq!(normalize_loose(s), "ぱ", "input={s}");
        }
        assert_ne!(normalize_loose("ぱ"), normalize_loose("は"));
    }

    #[test]
    fn vu_kana_unified() {
        for s in ["ヴ", "ｳﾞ"] {
            assert_eq!(normalize_loose(s), "ゔ", "input={s}");
        }
    }

    #[test]
    fn fullwidth_and_case_folded() {
        for s in ["Ｐ", "P", "ｐ", "p"] {
            assert_eq!(normalize_loose(s), "p", "input={s}");
        }
    }

    #[test]
    fn mixed_string() {
        // 「東京 ﾄｳｷｮｳ Tokyo」 →（漢字はそのまま）+ ひらがな化 + 小文字
        let s = "東京 ﾄｳｷｮｳ Tokyo";
        let n = normalize_loose(s);
        assert_eq!(n, "東京 とうきょう tokyo");
    }

    #[test]
    fn empty_is_empty() {
        assert_eq!(normalize_loose(""), "");
    }
}
