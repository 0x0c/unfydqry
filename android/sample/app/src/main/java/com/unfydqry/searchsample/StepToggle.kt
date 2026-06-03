package com.unfydqry.searchsample

import uniffi.unfydqry.NormalizeOptions

/// One normalization step toggle, bound to a field of [NormalizeOptions].
data class StepToggle(
    val label: String,
    val get: (NormalizeOptions) -> Boolean,
    val set: (NormalizeOptions, Boolean) -> NormalizeOptions,
) {
    companion object {
        /// All normalization step toggles, in display order.
        val all = listOf(
            StepToggle("小文字化", { it.lowercase }, { o, v -> o.copy(lowercase = v) }),
            StepToggle("カナ→かな", { it.kanaFold }, { o, v -> o.copy(kanaFold = v) }),
            StepToggle("アクセント除去 (café→cafe)", { it.foldDiacritics }, { o, v -> o.copy(foldDiacritics = v) }),
            StepToggle("長音畳み込み (サーバー→サーバ)", { it.foldChoonpu }, { o, v -> o.copy(foldChoonpu = v) }),
            StepToggle("繰り返し記号展開 (時々→時時)", { it.expandIterationMarks }, { o, v -> o.copy(expandIterationMarks = v) }),
            StepToggle("ハイフン統一", { it.normalizeHyphens }, { o, v -> o.copy(normalizeHyphens = v) }),
            StepToggle("桁区切り除去 (1,000→1000)", { it.stripDigitGrouping }, { o, v -> o.copy(stripDigitGrouping = v) }),
            StepToggle("空白圧縮", { it.collapseWhitespace }, { o, v -> o.copy(collapseWhitespace = v) }),
        )
    }
}
