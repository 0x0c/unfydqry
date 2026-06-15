/// An `(id, text)` pair for the batch indexing API ([SearchEngine.indexBatch]).
///
/// Mirrors the native `IndexItem` UniFFI record: each item is the same `(id,
/// text)` a single [SearchEngine.index] call would take, but a whole list is
/// indexed in one transaction for much better throughput on large batches.
class IndexItem {
  /// The document id, same semantics as [SearchEngine.index].
  final int id;

  /// Raw document text; normalized by the engine the same way as
  /// [SearchEngine.index].
  final String text;

  const IndexItem({required this.id, required this.text});

  Map<String, dynamic> toMap() => {'id': id, 'text': text};

  @override
  String toString() => 'IndexItem(id: $id, text: $text)';
}
