package com.unfydqry.searchsample

/// A search result row: the record plus which of its fields matched.
data class ResultRow(val record: Record, val matchedSlots: List<UByte>)
