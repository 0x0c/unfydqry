/// A record-level search result returned by [SearchEngine.searchRecords].
///
/// As with [Hit], only the record id and score cross the boundary; the host
/// re-fetches the full record from its own store.
class RecordHit {
  /// The host record id the matching fields belong to.
  final int recordId;

  /// Best (smallest) score among the record's matching fields.
  final double score;

  /// Slots of the fields that matched, ordered best (smallest score) first.
  final List<int> matchedSlots;

  const RecordHit({
    required this.recordId,
    required this.score,
    required this.matchedSlots,
  });

  @override
  String toString() =>
      'RecordHit(recordId: $recordId, score: $score, matchedSlots: $matchedSlots)';
}
