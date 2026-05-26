import 'package:flutter/services.dart';

/// A single search result returned by [SearchEngine.search].
class Hit {
  final int id;
  final double score;
  const Hit({required this.id, required this.score});

  @override
  String toString() => 'Hit(id: $id, score: $score)';
}

/// Thrown when the native search engine reports an error.
class SearchException implements Exception {
  final String message;
  const SearchException(this.message);

  @override
  String toString() => 'SearchException: $message';
}

/// Thin wrapper around the platform's SearchEngine.
///
/// The actual search logic runs in Rust via UniFFI; this class only
/// forwards calls over the method channel.
///
/// ```dart
/// final engine = await SearchEngine.open(dbPath);
/// await engine.index(1, 'Ｐｙｔｈｏｎ 入門');
/// final hits = await engine.search('python');
/// await engine.dispose();
/// ```
class SearchEngine {
  static const _channel = MethodChannel('unfydqry/search');

  final int _handle;
  SearchEngine._(this._handle);

  /// Opens (or creates) the SQLite FTS index at [dbPath].
  static Future<SearchEngine> open(String dbPath) async {
    final handle = await _channel.invokeMethod<int>('open', {'dbPath': dbPath});
    if (handle == null) throw const SearchException('open returned null handle');
    return SearchEngine._(handle);
  }

  /// Indexes or re-indexes [text] under [id].
  Future<void> index(int id, String text) => _channel.invokeMethod<void>(
        'index',
        {'handle': _handle, 'id': id, 'text': text},
      );

  /// Removes the entry with [id] from the index.
  Future<void> remove(int id) => _channel.invokeMethod<void>(
        'remove',
        {'handle': _handle, 'id': id},
      );

  /// Searches for [query], returning at most [limit] results ordered by relevance.
  Future<List<Hit>> search(String query, {int limit = 50}) async {
    final raw = await _channel.invokeMethod<List<dynamic>>(
      'search',
      {'handle': _handle, 'query': query, 'limit': limit},
    );
    return (raw ?? [])
        .cast<Map<dynamic, dynamic>>()
        .map((m) => Hit(
              id: (m['id'] as num).toInt(),
              score: (m['score'] as num).toDouble(),
            ))
        .toList();
  }

  /// Releases native resources. The engine must not be used after this.
  Future<void> dispose() => _channel.invokeMethod<void>(
        'dispose',
        {'handle': _handle},
      );
}
