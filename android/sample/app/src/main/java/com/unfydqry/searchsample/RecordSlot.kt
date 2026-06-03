package com.unfydqry.searchsample

/// Field slots for the record-layer API. Raw values are the slot numbers baked
/// into the FFI record layer and the on-disk index, so they are stable and must
/// never be renumbered — only appended.
enum class RecordSlot(val slot: Int) {
    NAME(0),
    YOMI(1),
    ;

    /// Human-readable label for this slot.
    val label: String
        get() = when (this) {
            NAME -> "名前"
            YOMI -> "よみ"
        }

    companion object {
        /// Number of fields per record, derived from the defined slots.
        val fieldCount: UInt get() = entries.size.toUInt()

        /// Label for a raw slot value as returned by the engine (`matchedSlots`).
        fun labelFor(slot: UByte): String =
            entries.firstOrNull { it.slot.toUByte() == slot }?.label ?: "slot $slot"
    }
}
