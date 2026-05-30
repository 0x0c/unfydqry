import 'package:flutter/services.dart';

import 'hit.dart';
import 'search_exception.dart';

/// Thin wrapper around the platform's SearchEngine.
///
/// The actual search logic runs in Rust via UniFFI; this class only
/// forwards calls over the method channel.
///
/// ```dart
/// final engine = await SearchEngine.open(dbPath);
/// await engine.index(1, 'Ｐｙｔｈｏｮ 入門');
/// final hits = await engine.search('python');
/// await engine.dispose();
/// ```
class SearchEngine {
  static const _channel = MethodChannel('unfydqry/search');

  int _handle;

  SearchEngine._(this._handle);

  bool get _disposed => _handle < 0;

  void _checkAlive() {
    if (_disposed) throw StateError('SearchEngine used after dispose()');
  }

  /// Opens (or creates) the SQLite FTS index at [dbPath].
  static Future<SearchEngine> open(String dbPath) async {
    final handle = await _channel.invokeMethod<int>('open', {'dbPath': dbPath});
    if (handle == null) throw const SearchException('open returned null handle');
    return SearchEngine._(handle);
  }

  /// Indexes or re-indexes [text] under [id].
  Future<void> index(int id, String text) {
    _checkAlive();
    return _channel.invokeMethod<void>(
      'index',
      {'handle': _handle, 'id': id, 'text': text},
    );
  }

  /// Removes the entry with [id] from the index.
  Future<void> remove(int id) {
    _checkAlive();
    return _channel.invokeMethod<void>(
      'remove',
      {'handle': _handle, 'id': id},
    );
  }

  /// Searches for [query], returning at most [limit] results ordered by relevance.
  Future<List<Hit>> search(String query, {int limit = 50}) async {
    _checkAlive();
    final raw = await _channel.invokeMethod<List<dynamic>>(
      'search',
      {'handle': _handle, 'query': query, 'limit': limit},
    );
    try {
      return (raw ?? [])
          .cast<Map<dynamic, dynamic>>()
          .map((m) => Hit(
                id: (m['id'] as num).toInt(),
                score: (m['score'] as num).toDouble(),
              ))
          .toList();
    } catch (e) {
      throw SearchException('malformed hit payload: $e');
    }
  }

  /// Releases native resources. The engine must not be used after this.
  ///
  /// Idempotent: calling it more than once is a no-op.
  Future<void> dispose() async {
    if (_disposed) return;
    final handle = _handle;
    _handle = -1;
    await _channel.invokeMethod<void>('dispose', {'handle': handle});
  }
}
