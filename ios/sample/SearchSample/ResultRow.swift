/// A search result row: the matched record plus which of its fields matched.
struct ResultRow: Identifiable, Hashable {
    let record: Record
    let matchedSlots: [UInt8]
    /// Marked normalized text per matched slot, as produced by the engine's
    /// `highlight` (matches wrapped in `Highlight.open`/`Highlight.close`).
    /// A slot is present only when the query actually matched its field.
    let highlights: [UInt8: String]
    var id: Int64 { record.id }
}
