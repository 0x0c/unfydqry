/// A single search result returned by [SearchEngine.search].
class Hit {
  final int id;
  final double score;

  const Hit({required this.id, required this.score});

  @override
  String toString() => 'Hit(id: $id, score: $score)';
}
