//! Individual, composable normalization steps.
//!
//! Each step is a pure `&str -> String` transform with no dictionary or
//! tokenizer, so it stays deterministic and identical across platforms. The
//! composable [`Normalizer`](super::Composable) runs the enabled steps in a
//! fixed canonical order after NFKC (see `normalize/mod.rs`).

use unicode_normalization::UnicodeNormalization;

use super::katakana_to_hiragana;

/// Latin combining diacritics (Unicode `Combining Diacritical Marks`).
/// Japanese voiced marks live at U+3099/U+309A, *outside* this range, so a
/// decompose → drop → recompose round trip preserves dakuten/handakuten.
const COMBINING_MARKS: std::ops::RangeInclusive<char> = '\u{0300}'..='\u{036F}';

/// `char::to_lowercase` over the whole string.
pub fn lowercase(input: &str) -> String {
    input.chars().flat_map(char::to_lowercase).collect()
}

/// Katakana → Hiragana (dakuten-marked forms map correctly too; see
/// [`katakana_to_hiragana`]).
pub fn kana_fold(input: &str) -> String {
    input.chars().map(katakana_to_hiragana).collect()
}

/// Strips Latin/Western combining diacritics (`café` → `cafe`).
///
/// Decomposes (NFD), drops only the `Combining Diacritical Marks` block, then
/// recomposes (NFC). Because the Japanese voiced-sound marks are at U+3099/
/// U+309A, dakuten and handakuten survive the round trip unchanged.
pub fn fold_diacritics(input: &str) -> String {
    input
        .nfd()
        .filter(|c| !COMBINING_MARKS.contains(c))
        .nfc()
        .collect()
}

/// Folds the prolonged-sound mark `ー` (U+30FC) when it follows kana, so
/// `サーバー` and `サーバ` collapse to the same key. A `ー` not preceded by kana
/// (e.g. used as a dash) is left untouched.
pub fn fold_choonpu(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_is_kana = false;
    for c in input.chars() {
        if c == '\u{30FC}' && prev_is_kana {
            // Drop the mark; a run of marks collapses (prev_is_kana stays true).
            continue;
        }
        out.push(c);
        prev_is_kana = is_kana(c);
    }
    out
}

/// Expands iteration marks by repeating the preceding character:
/// `々` repeats a kanji (`時々` → `時時`); `ゝゞ` / `ヽヾ` repeat the preceding
/// kana, with the voiced variants (`ゞ`/`ヾ`) adding dakuten (`こゞ` → `こご`).
/// A mark with no eligible preceding character is left as-is.
pub fn expand_iteration_marks(input: &str) -> String {
    let mut out: Vec<char> = Vec::with_capacity(input.chars().count());
    for c in input.chars() {
        let replacement = match c {
            '々' => out.last().copied().filter(|p| is_kanji(*p)),
            '\u{309D}' | '\u{30FD}' => out.last().copied().filter(|p| is_kana(*p)),
            '\u{309E}' | '\u{30FE}' => out.last().copied().filter(|p| is_kana(*p)).map(add_dakuten),
            _ => None,
        };
        out.push(replacement.unwrap_or(c));
    }
    out.into_iter().collect()
}

/// Unifies the dash/hyphen family to ASCII `-`. The prolonged-sound mark `ー`
/// (U+30FC) is deliberately excluded — it is handled by [`fold_choonpu`].
pub fn normalize_hyphens(input: &str) -> String {
    input
        .chars()
        .map(|c| if is_dash(c) { '-' } else { c })
        .collect()
}

/// Removes a comma that groups digits (`1,000` → `1000`); a comma not flanked
/// by ASCII digits on both sides is kept.
pub fn strip_digit_grouping(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ',' {
            let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
            let next_digit = chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
            if prev_digit && next_digit {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Collapses runs of Unicode whitespace to a single ASCII space and trims the
/// ends.
pub fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;
    for c in input.chars() {
        if c.is_whitespace() {
            pending_space = true;
        } else {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        }
    }
    out
}

const fn is_kana(c: char) -> bool {
    // Hiragana and Katakana blocks (covers small kana, ー, iteration marks).
    matches!(c as u32, 0x3041..=0x3096 | 0x309D..=0x309F | 0x30A1..=0x30FF)
}

const fn is_kanji(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF      // CJK Ext A
        | 0x4E00..=0x9FFF    // CJK Unified
        | 0xF900..=0xFAFF    // CJK Compatibility Ideographs
        | 0x20000..=0x2FFFF) // CJK Ext B+ (surrogate-pair ideographs)
}

const fn is_dash(c: char) -> bool {
    matches!(
        c,
        '\u{002D}'           // HYPHEN-MINUS
        | '\u{2010}'
            ..='\u{2015}' // hyphen, NB hyphen, figure/en/em dash, horizontal bar
        | '\u{2212}'         // MINUS SIGN
        | '\u{FF0D}'
    ) // FULLWIDTH HYPHEN-MINUS
}

/// Adds dakuten to a voiceable kana by the +1 code-point offset shared by the
/// hiragana and katakana layouts (`こ`→`ご`, `カ`→`ガ`). Non-voiceable kana are
/// returned unchanged.
fn add_dakuten(c: char) -> char {
    if is_voiceable_kana(c) {
        char::from_u32(c as u32 + 1).unwrap_or(c)
    } else {
        c
    }
}

/// Whether the base kana has a `+1` voiced form (か/さ/た/は rows and their
/// katakana counterparts). `う`→`ゔ` is non-adjacent and intentionally excluded.
const fn is_voiceable_kana(c: char) -> bool {
    matches!(
        c,
        // Hiragana: か き く け こ さ し す せ そ た ち つ て と は ひ ふ へ ほ
        'か' | 'き' | 'く' | 'け' | 'こ'
        | 'さ' | 'し' | 'す' | 'せ' | 'そ'
        | 'た' | 'ち' | 'つ' | 'て' | 'と'
        | 'は' | 'ひ' | 'ふ' | 'へ' | 'ほ'
        // Katakana: カ キ ク ケ コ サ シ ス セ ソ タ チ ツ テ ト ハ ヒ フ ヘ ホ
        | 'カ' | 'キ' | 'ク' | 'ケ' | 'コ'
        | 'サ' | 'シ' | 'ス' | 'セ' | 'ソ'
        | 'タ' | 'チ' | 'ツ' | 'テ' | 'ト'
        | 'ハ' | 'ヒ' | 'フ' | 'ヘ' | 'ホ'
    )
}
