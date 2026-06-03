package com.unfydqry.searchsample

/// Minimal multi-field record standing in for the app's "source-of-truth DB"
/// (equivalent to a SwiftData / Room entity with several searchable columns).
data class Record(val id: Long, val name: String, val yomi: String)
