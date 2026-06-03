package com.unfydqry.searchsample

import uniffi.unfydqry.SearchStrategy

/// UI-facing label for a search strategy (the FFI enum).
val SearchStrategy.label: String
    get() = when (this) {
        SearchStrategy.TRIGRAM_BM25 -> "trigram + bm25"
        SearchStrategy.SUBSTRING -> "substring"
        SearchStrategy.PREFIX -> "prefix"
        SearchStrategy.SUFFIX -> "suffix"
        SearchStrategy.ALL_TERMS -> "all terms"
        SearchStrategy.FUZZY_TRIGRAM -> "fuzzy trigram"
        SearchStrategy.LEVENSHTEIN -> "levenshtein"
        SearchStrategy.DAMERAU_LEVENSHTEIN -> "damerau-levenshtein"
    }
