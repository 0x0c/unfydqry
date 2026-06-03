package com.unfydqry.searchsample

import uniffi.unfydqry.NormalizeOptions

/// The `loose` preset as composable options (lowercase + kana fold).
fun NormalizeOptions.Companion.loose() = NormalizeOptions(lowercase = true, kanaFold = true)
