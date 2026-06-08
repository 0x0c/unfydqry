package com.unfydqry.searchsample

/// A search result row: the record plus which of its fields matched.
///
/// [highlights] holds the marked normalized text per matched slot, as produced
/// by the engine's `highlight` (matches wrapped in `Highlight.OPEN`/`CLOSE`).
/// A slot is present only when the query actually matched its field.
data class ResultRow(
    val record: Record,
    val matchedSlots: List<UByte>,
    val highlights: Map<Int, String> = emptyMap(),
)
