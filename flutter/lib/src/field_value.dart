/// A single field of a host record, for the record-layer indexing API
/// ([SearchEngine.indexRecord]).
///
/// [slot] is a small, stable per-field number (0-based) chosen by the host. The
/// native engine packs `(recordId, slot)` into the id it stores under, so a
/// slot, once used, must not be renumbered.
class FieldValue {
  /// Stable per-field slot. Must be `< 2^fieldBits` (default 8 → 0..255).
  final int slot;

  /// Raw field text; the engine normalizes it the same way as [SearchEngine.index].
  final String text;

  const FieldValue({required this.slot, required this.text});

  Map<String, dynamic> toMap() => {'slot': slot, 'text': text};

  @override
  String toString() => 'FieldValue(slot: $slot, text: $text)';
}
